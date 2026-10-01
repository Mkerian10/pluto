//! Transition linearity and must-release obligations for typestates
//! (docs/design/rfc-typestates.md, phases 2–3).
//!
//! # Phase 2: transition consumption
//!
//! A *transition method* is a method of a typestate class whose return type
//! is the same class with a **state parameter** changed — `fn acquire(self)
//! Partition<Owned>` on `Partition<S>`. A state parameter is one named on the
//! left of any `where` clause in the class; classes with no `where` clauses
//! have no state params and never participate, and methods that only change
//! *data* params (`Box<T>.map() Box<U>`) never consume. Calling a transition
//! through a local binding **consumes** that binding: the value has moved to
//! its new state, and the old-state alias must not be used again.
//!
//! ```text
//! let u = Partition<Unowned> { id: 7 }
//! let o = u.acquire()
//! u.describe()        // error: 'u' was consumed by the transition
//! ```
//!
//! # Phase 3: degradation errors and must-release states
//!
//! **State-carrying (degradation) errors.** An error declaration with a field
//! whose type is a typestate class with a state parameter changed relative to
//! a method's receiver (`error Degraded { lease: Lease<Revoked> }` raised by
//! a method of `Lease<S> where S == Held`) is a *degradation error* for that
//! method — the mirror of the transition rule. Raising one moves the receiver
//! into the error payload, so catching it **consumes the receiver on the
//! error path**: inside the catch body (and after a fall-through catch join)
//! the receiver binding is consumed; if the catch path terminates, the
//! success path keeps the binding with no ceremony. Recovery is extracting
//! the payload (`let stale = e.lease`).
//!
//! **Must-release states** (`must_release Held`). A binding whose type is the
//! class at a must-release state is fully linear:
//! - moves consume (`let b = a`, passing as an argument, returning) — the
//!   single obligation travels with the value;
//! - closure/spawn capture is a compile error (the obligation would be
//!   duplicated);
//! - storing into a field (or a container literal) is a compile error (the
//!   obligation would escape the analysis);
//! - scope exit — including `return`/`raise` — with a live obligation is a
//!   compile error naming the obligation and suggesting the transitions out;
//! - discharge = a consuming transition out of the state, moving the value
//!   onward, or returning it.
//!
//! A typed catch of an error carrying a must-release payload takes on the
//! payload's obligation (`e.lease` must be extracted and discharged);
//! wildcard/shorthand handlers cannot discharge a payload and are rejected
//! when such an error can reach them. Errors carrying only droppable states
//! keep today's relaxed rules — degradation to a droppable state is the
//! expected common case.
//!
//! Flow rules (shared):
//! - Reassignment (`u = ...`) or a fresh `let u = ...` revives the binding
//!   (but dropping a live obligation by rebinding is an error).
//! - Joins are conservative: consumed on ANY branch means consumed after the
//!   join; a live obligation on ANY branch stays live. Loop bodies are
//!   analyzed twice so a transition in iteration one is caught as a
//!   use-after-consume (or an obligation drop) in iteration two.
//! - Closure and spawn bodies are checked against a snapshot of the current
//!   consumed set (capturing a consumed value is a use); their own effects
//!   don't escape the closure (captures are by-value snapshots).
//! - Error propagation (`!`) does not check obligations: the error path exits
//!   the function and any state the error carries rides in its payload.
//!   A literal `raise` is a definite, author-visible exit and IS checked.
//!
//! Runs after error inference (degradation discrimination needs `fn_errors`)
//! and before `sweep_skolems`: generic templates record their resolutions
//! under skolem-instance names, which this pass reconstructs.

use std::collections::{HashMap, HashSet};

use crate::parser::ast::*;
use crate::span::Spanned;
use crate::typeck::env::{mangle_method, MethodResolution, TypeEnv};
use crate::typeck::types::PlutoType;
use crate::diagnostics::CompileError;
use crate::visit::{walk_expr, Visitor};

pub(crate) fn check_transition_linearity(
    program: &Program,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    let transitions = collect_transitions(env);
    let must_release = collect_must_release(env);
    let state_errors = collect_state_errors(env);
    if transitions.is_empty() && must_release.is_empty() {
        return Ok(()); // no typestate classes in this program
    }
    let tables = Tables {
        transitions: &transitions,
        must_release: &must_release,
        state_errors: &state_errors,
    };
    for func in &program.functions {
        // Concrete functions and generic templates both record resolutions
        // under the function's own name.
        check_body(&func.node.name.node, &func.node, env, &tables)?;
    }
    for class in &program.classes {
        if class.node.type_params.is_empty() {
            for m in &class.node.methods {
                let key = mangle_method(&class.node.name.node, &m.node.name.node);
                check_body(&key, &m.node, env, &tables)?;
            }
        } else {
            // Generic class template bodies were checked against skolem (or,
            // for `where`-constrained methods, state-bound) instantiations —
            // reconstruct the same instance name to find their resolutions.
            for m in &class.node.methods {
                let key = template_method_key(&class.node, &m.node.name.node, env);
                check_body(&key, &m.node, env, &tables)?;
            }
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            let key = mangle_method(&app.node.name.node, &m.node.name.node);
            check_body(&key, &m.node, env, &tables)?;
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            let key = mangle_method(&stage.node.name.node, &m.node.name.node);
            check_body(&key, &m.node, env, &tables)?;
        }
    }
    Ok(())
}

/// The `current_fn` key under which a generic-class template method's
/// resolutions were recorded: the class instantiated at skolem args, except
/// `where`-constrained params which were bound to their state types
/// (mirrors templates.rs::check_class_template).
fn template_method_key(class: &ClassDecl, method_name: &str, env: &TypeEnv) -> String {
    let type_params: Vec<String> = class.type_params.iter().map(|tp| tp.node.clone()).collect();
    let mut args: Vec<PlutoType> = type_params
        .iter()
        .map(|tp| PlutoType::Class(format!("%{tp}")))
        .collect();
    if let Some(gen_info) = env.generic_classes.get(&class.name.node)
        && let Some(cs) = gen_info.method_state_constraints.get(method_name)
    {
        for (param, state) in cs {
            if let Some(idx) = type_params.iter().position(|p| p == param) {
                args[idx] = if env.enums.contains_key(state) {
                    PlutoType::Enum(state.clone())
                } else {
                    PlutoType::Class(state.clone())
                };
            }
        }
    }
    let mangled_class = crate::typeck::env::mangle_name(&class.name.node, &args);
    mangle_method(&mangled_class, method_name)
}

/// base class name -> method names that change a state parameter.
type TransitionTable = HashMap<String, HashSet<String>>;

/// From each typestate class's TEMPLATE signatures: a method is a transition
/// iff its return type is the same class with some state-param position bound
/// to something other than that parameter itself.
fn collect_transitions(env: &TypeEnv) -> TransitionTable {
    let mut table: TransitionTable = HashMap::new();
    for (base, info) in &env.generic_classes {
        let state_params: HashSet<&String> = info
            .method_state_constraints
            .values()
            .flatten()
            .map(|(p, _)| p)
            .collect();
        if state_params.is_empty() {
            continue;
        }
        let state_positions: Vec<usize> = info
            .type_params
            .iter()
            .enumerate()
            .filter(|(_, p)| state_params.contains(p))
            .map(|(i, _)| i)
            .collect();
        for (mname, sig) in &info.method_sigs {
            let PlutoType::GenericInstance(_, ret_base, ret_args) = &sig.return_type else {
                continue;
            };
            if ret_base != base || ret_args.len() != info.type_params.len() {
                continue;
            }
            let changes_state = state_positions.iter().any(|&i| {
                ret_args[i] != PlutoType::TypeParam(info.type_params[i].clone())
            });
            if changes_state {
                table.entry(base.clone()).or_default().insert(mname.clone());
            }
        }
    }
    table
}

/// A must-release marking: the class's state param at `position` bound to
/// `state` makes a binding fully linear. `exits` are the transition methods
/// constrained to that state (suggested discharges in diagnostics).
struct MustReleaseEntry {
    position: usize,
    state: String,
    exits: Vec<String>,
}

/// base class name -> its must-release state markings.
type MustReleaseTable = HashMap<String, Vec<MustReleaseEntry>>;

fn collect_must_release(env: &TypeEnv) -> MustReleaseTable {
    let mut table: MustReleaseTable = HashMap::new();
    for (base, info) in &env.generic_classes {
        if info.must_release.is_empty() {
            continue;
        }
        let transitions: HashSet<&String> = {
            // Recompute transition methods for this class (collect_transitions
            // output is keyed by base too, but keep this self-contained).
            let state_params: HashSet<&String> = info
                .method_state_constraints
                .values()
                .flatten()
                .map(|(p, _)| p)
                .collect();
            let state_positions: Vec<usize> = info
                .type_params
                .iter()
                .enumerate()
                .filter(|(_, p)| state_params.contains(p))
                .map(|(i, _)| i)
                .collect();
            info.method_sigs
                .iter()
                .filter(|(_, sig)| {
                    if let PlutoType::GenericInstance(_, ret_base, ret_args) = &sig.return_type {
                        ret_base == base
                            && ret_args.len() == info.type_params.len()
                            && state_positions.iter().any(|&i| {
                                ret_args[i] != PlutoType::TypeParam(info.type_params[i].clone())
                            })
                    } else {
                        false
                    }
                })
                .map(|(m, _)| m)
                .collect()
        };
        for (param, state) in &info.must_release {
            let Some(position) = info.type_params.iter().position(|p| p == param) else {
                continue;
            };
            // Transition methods available in this state: candidates for the
            // "how do I discharge this" suggestion.
            let mut exits: Vec<String> = info
                .method_state_constraints
                .iter()
                .filter(|(m, cs)| {
                    transitions.contains(m)
                        && cs.iter().any(|(p, s)| p == param && s == state)
                })
                .map(|(m, _)| format!(".{m}()"))
                .collect();
            exits.sort();
            table.entry(base.clone()).or_default().push(MustReleaseEntry {
                position,
                state: state.clone(),
                exits,
            });
        }
    }
    table
}

/// A state-carrying payload field of an error declaration: its type is a
/// typestate class instantiation.
struct StatePayload {
    field: String,
    base: String,
    /// Mangled-name segments after the base ("Lease$$Revoked" -> ["Revoked"]).
    args: Vec<String>,
    /// Display form, e.g. "Lease<Revoked>".
    display: String,
}

/// error name -> its state-carrying payload fields.
type StateErrorTable = HashMap<String, Vec<StatePayload>>;

fn collect_state_errors(env: &TypeEnv) -> StateErrorTable {
    let mut table: StateErrorTable = HashMap::new();
    for (ename, info) in &env.errors {
        for (fname, ty) in &info.fields {
            let PlutoType::Class(inst) = ty else { continue };
            let Some((base, args)) = instance_parts(inst) else { continue };
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if args.is_empty() {
                continue;
            }
            let Some(gen_info) = env.generic_classes.get(base) else { continue };
            let has_state_params = gen_info
                .method_state_constraints
                .values()
                .any(|cs| !cs.is_empty());
            if !has_state_params {
                continue;
            }
            table.entry(ename.clone()).or_default().push(StatePayload {
                field: fname.clone(),
                base: base.to_string(),
                args,
                display: display_instance(inst),
            });
        }
    }
    table
}

struct Tables<'a> {
    transitions: &'a TransitionTable,
    must_release: &'a MustReleaseTable,
    state_errors: &'a StateErrorTable,
}

fn check_body(
    current_fn: &str,
    func: &Function,
    env: &TypeEnv,
    tables: &Tables<'_>,
) -> Result<(), CompileError> {
    let mut lin = Linearity {
        current_fn,
        env,
        tables,
        consumed: HashMap::new(),
        obligations: HashMap::new(),
        var_instances: HashMap::new(),
        error_vars: HashMap::new(),
        forbidden_captures: HashMap::new(),
        scope_stack: Vec::new(),
        error: None,
    };
    // Parameters: a non-self param in a must-release state carries the
    // obligation into the callee ("move transfers obligation — caller clean,
    // callee must discharge"). `self` is exempt: the class's own methods
    // define the protocol and are themselves the discharge points.
    if let Some(sig) = env.functions.get(current_fn) {
        for (p, ty) in func.params.iter().zip(&sig.params) {
            if p.name.node == "self" {
                continue;
            }
            if let PlutoType::Class(inst) = ty {
                lin.var_instances.insert(p.name.node.clone(), inst.clone());
                if let Some(ob) = lin.obligation_for(inst) {
                    lin.obligations.insert(p.name.node.clone(), ob);
                }
            }
        }
    }
    let terminated = lin.visit_block_scope(&func.body.node, false);
    if !terminated && lin.error.is_none() {
        // Fall-through function exit: every live obligation is a leak
        // (parameters included).
        let mut live: Vec<(&String, &Obligation)> = lin.obligations.iter().collect();
        live.sort_by_key(|(n, _)| n.as_str());
        if let Some((name, ob)) = live.first() {
            lin.error = Some(leak_error(name, ob, func.body.span));
        }
    }
    match lin.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// What a binding was consumed by, for the error message.
#[derive(Clone)]
enum Cause {
    /// A state-transition method call through the binding.
    Transition { method: String, new_state: String },
    /// A move of a must-release value (let, argument, return, raise payload).
    Moved { to: String },
    /// Consumed on the error path of a call that raised a state-carrying
    /// error: the value rides in the error payload.
    ErrorPath { method: String, error: String, new_state: String },
}

/// A live must-release obligation attached to a binding (or to a caught
/// error's payload, keyed "var.field").
#[derive(Clone)]
struct Obligation {
    /// Display type, e.g. "Lease<Held>".
    display: String,
    state: String,
    /// Transition methods that discharge the state (".release()").
    exits: Vec<String>,
    acquired: crate::span::Span,
}

struct ScopeFrame {
    declared: Vec<String>,
    is_loop_body: bool,
}

struct Linearity<'a> {
    current_fn: &'a str,
    env: &'a TypeEnv,
    tables: &'a Tables<'a>,
    consumed: HashMap<String, Cause>,
    obligations: HashMap<String, Obligation>,
    /// binding -> mangled class instance ("Lease$$Held"), tracked for
    /// typestate-class-typed bindings only.
    var_instances: HashMap<String, String>,
    /// typed-catch binding -> error type name.
    error_vars: HashMap<String, String>,
    /// Obligated bindings of enclosing scopes while inside a closure/spawn
    /// body: any use is a capture that would duplicate the obligation.
    forbidden_captures: HashMap<String, Obligation>,
    scope_stack: Vec<ScopeFrame>,
    error: Option<CompileError>,
}

fn suggest_exits(ob: &Obligation) -> String {
    if ob.exits.is_empty() {
        format!("transition it out of '{}'", ob.state)
    } else {
        format!(
            "transition it out of '{}' (e.g. {})",
            ob.state,
            ob.exits.join(" or ")
        )
    }
}

fn leak_error(name: &str, ob: &Obligation, span: crate::span::Span) -> CompileError {
    CompileError::type_err(
        format!(
            "'{name}' still holds {display}, a must_release state, when it goes out of \
             scope; {suggest}, move it onward, or return it",
            display = ob.display,
            suggest = suggest_exits(ob),
        ),
        span,
    )
}

/// Split a mangled instance name into its base and positional argument
/// segments: `mangle_name` writes `Base$$arg1$arg2$...` (the "$$" separates
/// base from args; single '$' separates args from each other). Nested
/// generic args contain '$'/"$$" themselves and corrupt positions AFTER
/// them — states are plain named types and conventionally lead, so state
/// positions parse correctly in practice.
fn instance_parts(mangled: &str) -> Option<(&str, Vec<&str>)> {
    let (base, rest) = mangled.split_once("$$")?;
    Some((base, rest.split('$').collect()))
}

/// `Partition$$Owned` → `Partition<Owned>` for error messages.
fn display_instance(mangled: &str) -> String {
    match instance_parts(mangled) {
        Some((base, args)) => format!("{base}<{}>", args.join(", ")),
        None => mangled.to_string(),
    }
}

impl Linearity<'_> {
    /// If `inst` (a mangled class-instance name) is in a must-release state,
    /// the obligation to attach to a binding holding it.
    fn obligation_for(&self, inst: &str) -> Option<Obligation> {
        let (base, args) = instance_parts(inst)?;
        let entries = self.tables.must_release.get(base)?;
        for e in entries {
            if args.get(e.position).copied() == Some(e.state.as_str()) {
                return Some(Obligation {
                    display: display_instance(inst),
                    state: e.state.clone(),
                    exits: e.exits.clone(),
                    acquired: crate::span::Span::new(0, 0),
                });
            }
        }
        None
    }

    /// The mangled class-instance type of an expression, for the cases this
    /// analysis understands (bindings, struct literals, resolved calls,
    /// catch/propagate wrappers, and typed-catch payload fields).
    fn expr_instance(&self, expr: &Spanned<Expr>) -> Option<String> {
        match &expr.node {
            Expr::Ident(name) => self.var_instances.get(name).cloned(),
            Expr::StructLit { name, type_args, .. } => {
                if type_args.is_empty() {
                    return None; // non-generic classes are never typestates
                }
                self.env
                    .generic_rewrites
                    .get(&(expr.span.start, expr.span.end))
                    .cloned()
            }
            Expr::MethodCall { method, .. } => {
                let key = (self.current_fn.to_string(), method.span.start);
                let MethodResolution::Class { mangled_name } =
                    self.env.method_resolutions.get(&key)?
                else {
                    return None;
                };
                match self.env.functions.get(mangled_name).map(|s| &s.return_type) {
                    Some(PlutoType::Class(inst)) => Some(inst.clone()),
                    _ => None,
                }
            }
            Expr::Call { name, .. } => {
                let fn_name = self
                    .env
                    .generic_rewrites
                    .get(&(expr.span.start, expr.span.end))
                    .cloned()
                    .unwrap_or_else(|| name.node.clone());
                match self.env.functions.get(&fn_name).map(|s| &s.return_type) {
                    Some(PlutoType::Class(inst)) => Some(inst.clone()),
                    _ => None,
                }
            }
            Expr::FieldAccess { object, field } => {
                let Expr::Ident(obj) = &object.node else { return None };
                let ename = self.error_vars.get(obj)?;
                let info = self.env.errors.get(ename)?;
                match info.fields.iter().find(|(n, _)| *n == field.node) {
                    Some((_, PlutoType::Class(inst))) => Some(inst.clone()),
                    _ => None,
                }
            }
            Expr::Propagate { expr: inner } => self.expr_instance(inner),
            Expr::Catch { expr: inner, .. } => self.expr_instance(inner),
            _ => None,
        }
    }

    /// Visit a block as a lexical scope: must-release bindings declared in it
    /// must be discharged before it ends. Returns true when the block
    /// terminates (return/raise/break/continue) — exits check obligations
    /// themselves and the block end is unreachable.
    fn visit_block_scope(&mut self, block: &Block, is_loop_body: bool) -> bool {
        self.scope_stack.push(ScopeFrame { declared: Vec::new(), is_loop_body });
        let mut terminated = false;
        for stmt in &block.stmts {
            if self.error.is_some() {
                break;
            }
            self.visit_stmt(stmt);
            if matches!(
                stmt.node,
                Stmt::Return(_) | Stmt::Raise { .. } | Stmt::Break | Stmt::Continue
            ) {
                terminated = true;
                break;
            }
        }
        let frame = self.scope_stack.pop().expect("scope frame pushed above");
        for name in &frame.declared {
            if self.error.is_none()
                && !terminated
                && let Some(ob) = self.obligations.get(name)
            {
                self.error = Some(leak_error(name, ob, ob.acquired));
            }
            // Out of scope either way.
            self.obligations.remove(name);
        }
        terminated
    }

    /// If this resolved call is a state-changing transition method, the
    /// display name of the state the value moved to.
    fn transition_target(&self, method_span_start: usize) -> Option<String> {
        let mangled_name = self.resolved_method(method_span_start)?;
        // mangled = "<class-instance>$<method>"; method names contain no '$'.
        let (class_inst, method) = mangled_name.rsplit_once('$')?;
        let class_inst = class_inst.trim_end_matches('$');
        let base = class_inst.split("$$").next()?;
        if !self.tables.transitions.get(base).is_some_and(|ms| ms.contains(method)) {
            return None;
        }
        let new_state = match self.env.functions.get(&mangled_name).map(|s| &s.return_type) {
            Some(PlutoType::Class(ret)) => display_instance(ret),
            _ => "its new state".to_string(),
        };
        Some(new_state)
    }

    fn resolved_method(&self, method_span_start: usize) -> Option<String> {
        let key = (self.current_fn.to_string(), method_span_start);
        let MethodResolution::Class { mangled_name } = self.env.method_resolutions.get(&key)?
        else {
            return None;
        };
        Some(mangled_name.clone())
    }

    /// Degradation errors a resolved method call can raise, relative to its
    /// receiver: errors in the method's inferred set that carry the same
    /// class with a state position changed from the receiver's instantiation.
    /// Returns (error name, payload display) pairs.
    fn degradation_errors(&self, method_span_start: usize) -> Vec<(String, String)> {
        let Some(mangled) = self.resolved_method(method_span_start) else {
            return Vec::new();
        };
        let Some((class_inst, _)) = mangled.rsplit_once('$') else {
            return Vec::new();
        };
        let class_inst = class_inst.trim_end_matches('$');
        let Some((base, recv_args)) = instance_parts(class_inst) else {
            return Vec::new();
        };
        let Some(gen_info) = self.env.generic_classes.get(base) else {
            return Vec::new();
        };
        let state_params: HashSet<&String> = gen_info
            .method_state_constraints
            .values()
            .flatten()
            .map(|(p, _)| p)
            .collect();
        let state_positions: Vec<usize> = gen_info
            .type_params
            .iter()
            .enumerate()
            .filter(|(_, p)| state_params.contains(p))
            .map(|(i, _)| i)
            .collect();
        let Some(errs) = self.env.fn_errors.get(&mangled) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for ename in errs {
            let Some(payloads) = self.tables.state_errors.get(ename) else { continue };
            for p in payloads {
                if p.base != base {
                    continue;
                }
                let changed = state_positions.iter().any(|&i| {
                    match (p.args.get(i), recv_args.get(i)) {
                        (Some(pa), Some(ra)) => pa != ra,
                        _ => false,
                    }
                });
                if changed {
                    out.push((ename.clone(), p.display.clone()));
                    break;
                }
            }
        }
        out.sort();
        out
    }

    fn use_var(&mut self, name: &str, span: crate::span::Span) {
        if self.error.is_some() {
            return;
        }
        if let Some(ob) = self.forbidden_captures.get(name) {
            self.error = Some(CompileError::type_err(
                format!(
                    "'{name}' holds {}, a must_release state, and cannot be captured by a \
                     closure or spawned task — the capture would duplicate the release \
                     obligation; {} first, or restructure so the closure does not use it",
                    ob.display,
                    suggest_exits(ob),
                ),
                span,
            ));
            return;
        }
        if let Some(cause) = self.consumed.get(name) {
            let msg = match cause {
                Cause::Transition { method, new_state } => format!(
                    "'{name}' was consumed by the transition '{method}' (it is now {new_state}); \
                     use the transition's result, or rebind '{name}'"
                ),
                Cause::Moved { to } => format!(
                    "'{name}' was moved ({to}); a must_release value has a single owner — \
                     use the new owner, or rebind '{name}'"
                ),
                Cause::ErrorPath { method, error, new_state } => format!(
                    "'{name}' was consumed on the error path: '{method}' raised '{error}' \
                     carrying the value as {new_state}; recover it from the error payload, \
                     or rebind '{name}'"
                ),
            };
            self.error = Some(CompileError::type_err(msg, span));
        }
    }

    /// Record a fresh binding `name` produced by `value` (let/assign/loop
    /// var): revive it, track its instance type, and attach a must-release
    /// obligation when the type demands one. Dropping a still-live obligation
    /// by rebinding is an error.
    fn bind(&mut self, name: &str, value: Option<&Spanned<Expr>>, span: crate::span::Span) {
        if self.error.is_some() {
            return;
        }
        if let Some(ob) = self.obligations.get(name) {
            self.error = Some(CompileError::type_err(
                format!(
                    "rebinding '{name}' drops {display}, a must_release state, without \
                     releasing it; {suggest}, move it onward, or return it first",
                    display = ob.display,
                    suggest = suggest_exits(ob),
                ),
                span,
            ));
            return;
        }
        self.consumed.remove(name);
        self.error_vars.remove(name);
        // Shadowing inside a closure makes the name a fresh local, not a
        // capture of the forbidden outer binding.
        self.forbidden_captures.remove(name);
        let inst = value.and_then(|v| self.expr_instance(v));
        match inst {
            Some(inst) => {
                if let Some(mut ob) = self.obligation_for(&inst) {
                    ob.acquired = span;
                    self.obligations.insert(name.to_string(), ob);
                    if let Some(frame) = self.scope_stack.last_mut()
                        && !frame.declared.iter().any(|n| n == name)
                    {
                        frame.declared.push(name.to_string());
                    }
                }
                self.var_instances.insert(name.to_string(), inst);
            }
            None => {
                self.var_instances.remove(name);
            }
        }
    }

    /// Treat `expr` as a move target (argument, raise payload, return value):
    /// a bare obligated binding — or a typed-catch payload field — is moved,
    /// which discharges its obligation here and consumes the binding. Other
    /// expressions are visited normally.
    fn move_expr(&mut self, expr: &Spanned<Expr>, to: &str) {
        if self.error.is_some() {
            return;
        }
        match &expr.node {
            Expr::Ident(name) if self.obligations.contains_key(name) => {
                // A use first (catches capture bans / already-consumed).
                self.use_var(name, expr.span);
                if self.error.is_some() {
                    return;
                }
                self.obligations.remove(name);
                self.consumed
                    .insert(name.clone(), Cause::Moved { to: to.to_string() });
            }
            Expr::FieldAccess { object, field }
                if matches!(&object.node, Expr::Ident(obj)
                    if self.error_vars.contains_key(obj)) =>
            {
                let Expr::Ident(obj) = &object.node else { unreachable!() };
                let key = format!("{obj}.{}", field.node);
                if self.consumed.contains_key(&key) {
                    self.use_var(&key, expr.span);
                    return;
                }
                if self.obligations.remove(&key).is_some() {
                    self.consumed.insert(key, Cause::Moved { to: to.to_string() });
                } else {
                    self.visit_expr(expr);
                }
            }
            _ => self.visit_expr(expr),
        }
    }

    /// At a definite function exit (return/raise), every obligation still
    /// live on this path is a leak.
    fn check_exit(&mut self, what: &str, span: crate::span::Span) {
        if self.error.is_some() {
            return;
        }
        let mut live: Vec<(&String, &Obligation)> = self.obligations.iter().collect();
        live.sort_by_key(|(n, _)| n.as_str());
        if let Some((name, ob)) = live.first() {
            self.error = Some(CompileError::type_err(
                format!(
                    "{what} while '{name}' still holds {display}, a must_release state; \
                     {suggest}, move it onward, or return it first",
                    display = ob.display,
                    suggest = suggest_exits(ob),
                ),
                span,
            ));
        }
    }

    /// Reject a must-release value flowing into a place the analysis cannot
    /// track (fields, container literals).
    fn check_untracked_store(&mut self, value: &Spanned<Expr>, place: &str) {
        if self.error.is_some() {
            return;
        }
        if let Some(inst) = self.expr_instance(value)
            && let Some(ob) = self.obligation_for(&inst)
        {
            self.error = Some(CompileError::type_err(
                format!(
                    "a value in the must_release state {display} may not be stored in \
                     {place} — the release obligation would escape the analysis; keep it \
                     in a local binding and {suggest} instead",
                    display = ob.display,
                    suggest = suggest_exits(&ob),
                ),
                value.span,
            ));
        }
    }

    fn visit_block_stmts(&mut self, block: &Block) {
        for stmt in &block.stmts {
            if self.error.is_some() {
                return;
            }
            self.visit_stmt(stmt);
        }
    }

    /// Visit a branch against a snapshot; returns the branch's flow state.
    fn branch(
        &mut self,
        entry: &FlowState,
        block: &Block,
        is_loop_body: bool,
    ) -> BranchResult {
        let saved_consumed = std::mem::replace(&mut self.consumed, entry.consumed.clone());
        let saved_obligations =
            std::mem::replace(&mut self.obligations, entry.obligations.clone());
        let terminated = self.visit_block_scope(block, is_loop_body);
        BranchResult {
            state: FlowState {
                consumed: std::mem::replace(&mut self.consumed, saved_consumed),
                obligations: std::mem::replace(&mut self.obligations, saved_obligations),
            },
            terminated,
        }
    }

    fn snapshot(&self) -> FlowState {
        FlowState {
            consumed: self.consumed.clone(),
            obligations: self.obligations.clone(),
        }
    }

    /// Conservative join: consumed on any path stays consumed; an obligation
    /// live on any path stays live.
    fn union_into(&mut self, other: FlowState) {
        for (k, v) in other.consumed {
            self.consumed.entry(k).or_insert(v);
        }
        for (k, v) in other.obligations {
            self.obligations.entry(k).or_insert(v);
        }
    }
}

#[derive(Clone)]
struct FlowState {
    consumed: HashMap<String, Cause>,
    obligations: HashMap<String, Obligation>,
}

impl FlowState {
    fn seed_from(&mut self, other: &FlowState) {
        for (k, v) in &other.consumed {
            self.consumed.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for (k, v) in &other.obligations {
            self.obligations.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
}

struct BranchResult {
    state: FlowState,
    terminated: bool,
}

impl Visitor for Linearity<'_> {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        if self.error.is_some() {
            return;
        }
        match &stmt.node {
            Stmt::Let { name, value, .. } => {
                self.move_expr(value, &format!("into '{}'", name.node));
                self.bind(&name.node, Some(value), name.span);
            }
            Stmt::Assign { target, value } => {
                self.move_expr(value, &format!("into '{}'", target.node));
                self.bind(&target.node, Some(value), target.span);
            }
            Stmt::FieldAssign { object, field: _, value } => {
                self.visit_expr(object);
                self.check_untracked_store(value, "a field");
                self.visit_expr(value);
            }
            Stmt::Return(value) => {
                if let Some(v) = value {
                    self.move_expr(v, "returned to the caller");
                }
                self.check_exit("cannot return", stmt.span);
            }
            Stmt::Raise { error_name, fields, .. } => {
                for (fname, fval) in fields {
                    self.move_expr(
                        fval,
                        &format!("into '{}.{}'", error_name.node, fname.node),
                    );
                }
                self.check_exit(&format!("cannot raise '{}'", error_name.node), stmt.span);
            }
            Stmt::Break | Stmt::Continue => {
                // Bindings declared since the loop body began die on this
                // edge; their obligations must be discharged first. Outer
                // obligations survive (the loop exit rejoins their scope).
                let mut names: Vec<String> = Vec::new();
                for frame in self.scope_stack.iter().rev() {
                    names.extend(frame.declared.iter().cloned());
                    if frame.is_loop_body {
                        break;
                    }
                }
                names.sort();
                for name in names {
                    if self.error.is_some() {
                        break;
                    }
                    if let Some(ob) = self.obligations.get(&name) {
                        let what = if matches!(stmt.node, Stmt::Break) {
                            "cannot break"
                        } else {
                            "cannot continue"
                        };
                        self.error = Some(CompileError::type_err(
                            format!(
                                "{what} while '{name}' still holds {display}, a \
                                 must_release state; {suggest}, move it onward, or \
                                 return it first",
                                display = ob.display,
                                suggest = suggest_exits(ob),
                            ),
                            stmt.span,
                        ));
                    }
                }
            }
            Stmt::If { condition, then_block, else_block } => {
                self.visit_expr(condition);
                let entry = self.snapshot();
                let after_then = self.branch(&entry, &then_block.node, false);
                let after_else = match else_block {
                    Some(eb) => self.branch(&entry, &eb.node, false),
                    None => BranchResult { state: entry, terminated: false },
                };
                // Conservative join over the paths that fall through.
                if !after_then.terminated {
                    self.union_into(after_then.state);
                }
                if !after_else.terminated {
                    self.union_into(after_else.state);
                }
            }
            Stmt::While { condition, body } => {
                self.visit_expr(condition);
                let entry = self.snapshot();
                let after_one = self.branch(&entry, &body.node, true);
                // Second pass from the post-body set catches transitions that
                // consume a loop-external binding on iteration one and use it
                // on iteration two.
                let mut seed = entry;
                seed.seed_from(&after_one.state);
                let after_two = self.branch(&seed, &body.node, true);
                self.union_into(after_one.state);
                self.union_into(after_two.state);
            }
            Stmt::For { var, iterable, body } => {
                self.visit_expr(iterable);
                self.bind(&var.node, None, var.span);
                let mut entry = self.snapshot();
                entry.consumed.remove(&var.node);
                let after_one = self.branch(&entry, &body.node, true);
                let mut seed = entry;
                seed.seed_from(&after_one.state);
                let after_two = self.branch(&seed, &body.node, true);
                self.union_into(after_one.state);
                self.union_into(after_two.state);
            }
            Stmt::Match { expr, arms } => {
                self.visit_expr(expr);
                let entry = self.snapshot();
                let mut joined: Vec<FlowState> = Vec::new();
                for arm in arms {
                    let r = self.branch(&entry, &arm.body.node, false);
                    if !r.terminated {
                        joined.push(r.state);
                    }
                }
                for j in joined {
                    self.union_into(j);
                }
            }
            Stmt::Expr(e) => {
                self.visit_expr(e);
                // A must-release value produced and immediately dropped at
                // statement position is a leak.
                if self.error.is_none()
                    && !matches!(e.node, Expr::Ident(_))
                    && let Some(inst) = self.expr_instance(e)
                    && let Some(ob) = self.obligation_for(&inst)
                {
                    self.error = Some(CompileError::type_err(
                        format!(
                            "this expression produces {display}, a must_release state, \
                             and immediately drops it; bind the result (`let held = ...`) \
                             and {suggest}",
                            display = ob.display,
                            suggest = suggest_exits(&ob),
                        ),
                        e.span,
                    ));
                }
            }
            _ => crate::visit::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        match &expr.node {
            Expr::Ident(name) => {
                self.use_var(name, expr.span);
            }
            Expr::MethodCall { object, method, args, .. } => {
                self.visit_expr(object);
                for a in args {
                    // Passing a must-release binding as an argument moves it.
                    self.move_expr(a, &format!("passed to '.{}()'", method.node));
                }
                if let Expr::Ident(recv) = &object.node
                    && let Some(new_state) = self.transition_target(method.span.start)
                {
                    // The transition consumes the receiver and discharges its
                    // obligation — the value (and any obligation of the new
                    // state) lives on in the result.
                    self.obligations.remove(recv);
                    self.consumed.insert(
                        recv.clone(),
                        Cause::Transition {
                            method: format!(".{}()", method.node),
                            new_state,
                        },
                    );
                }
            }
            Expr::Call { name, args, .. } => {
                for a in args {
                    self.move_expr(a, &format!("passed to '{}()'", name.node));
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, fval) in fields {
                    self.check_untracked_store(fval, "a field");
                    self.visit_expr(fval);
                }
            }
            Expr::ArrayLit { elements } => {
                for e in elements {
                    self.check_untracked_store(e, "a container literal");
                    self.visit_expr(e);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.visit_expr(k);
                    self.check_untracked_store(v, "a container literal");
                    self.visit_expr(v);
                }
            }
            Expr::SetLit { elements, .. } => {
                for e in elements {
                    self.check_untracked_store(e, "a container literal");
                    self.visit_expr(e);
                }
            }
            Expr::Closure { body, params, .. } => {
                // Captures are by-value snapshots: uses of consumed outer
                // bindings inside the closure are errors (the capture reads a
                // moved value), and capturing a live must-release binding
                // would duplicate its obligation. The closure body is its own
                // scope for obligations it creates (it runs later, possibly
                // never, possibly on another task).
                let mut entry_consumed = self.consumed.clone();
                for p in params {
                    entry_consumed.remove(&p.name.node);
                }
                let saved_consumed = std::mem::replace(&mut self.consumed, entry_consumed);
                let saved_obligations = std::mem::take(&mut self.obligations);
                let saved_forbidden = self.forbidden_captures.clone();
                for (name, ob) in &saved_obligations {
                    if !params.iter().any(|p| p.name.node == *name) && !name.contains('.') {
                        self.forbidden_captures.insert(name.clone(), ob.clone());
                    }
                }
                let saved_scopes = std::mem::take(&mut self.scope_stack);
                let terminated = self.visit_block_scope(&body.node, false);
                if !terminated && self.error.is_none() {
                    let mut live: Vec<(&String, &Obligation)> =
                        self.obligations.iter().collect();
                    live.sort_by_key(|(n, _)| n.as_str());
                    if let Some((name, ob)) = live.first() {
                        self.error = Some(leak_error(name, ob, body.span));
                    }
                }
                self.scope_stack = saved_scopes;
                self.forbidden_captures = saved_forbidden;
                self.obligations = saved_obligations;
                self.consumed = saved_consumed;
            }
            Expr::Spawn { call } => {
                self.visit_expr(call);
            }
            Expr::Catch { expr: inner, handlers } => {
                self.visit_expr(inner);
                if self.error.is_some() {
                    return;
                }
                let inner_errors =
                    super::errors::inner_error_set(&inner.node, self.current_fn, self.env);
                // The receiver this call's degradation errors would consume
                // on the error path, with the payload display for messages.
                let degradation: Option<(String, String, Vec<(String, String)>)> =
                    if let Expr::MethodCall { object, method, .. } = &inner.node
                        && let Expr::Ident(recv) = &object.node
                    {
                        let degr = self.degradation_errors(method.span.start);
                        if degr.is_empty() {
                            None
                        } else {
                            Some((recv.clone(), format!(".{}()", method.node), degr))
                        }
                    } else {
                        None
                    };
                // Item 5: an error carrying a must-release payload cannot be
                // handled by a wildcard/shorthand arm — the payload (and its
                // obligation) would be unreachable.
                let typed_for: HashSet<&str> = handlers
                    .iter()
                    .filter_map(|h| match h {
                        CatchHandler::Typed { error_type, .. } => {
                            Some(error_type.node.rsplit('.').next().unwrap_or(&error_type.node))
                        }
                        _ => None,
                    })
                    .collect();
                let has_catch_all = handlers.iter().any(|h| {
                    matches!(h, CatchHandler::Wildcard { .. } | CatchHandler::Shorthand(_))
                });
                if has_catch_all {
                    let mut sorted: Vec<&String> = inner_errors.iter().collect();
                    sorted.sort();
                    for ename in sorted {
                        let un = ename.rsplit('.').next().unwrap_or(ename);
                        if typed_for.contains(un) {
                            continue;
                        }
                        let Some(payloads) = self.tables.state_errors.get(ename) else {
                            continue;
                        };
                        if let Some(p) = payloads
                            .iter()
                            .find(|p| self.obligation_for_payload(p).is_some())
                        {
                            self.error = Some(CompileError::type_err(
                                format!(
                                    "the call can raise '{ename}', which carries {} in \
                                     field '{}' — a must_release state; a wildcard or \
                                     shorthand catch cannot discharge the payload. Catch \
                                     it with a typed handler `catch err: {un} {{ ... }}`, \
                                     extract the payload, and release it",
                                    p.display, p.field
                                ),
                                expr.span,
                            ));
                            return;
                        }
                    }
                }
                let entry = {
                    let mut e = self.snapshot();
                    // On the error path the receiver moved into the payload:
                    // consumed as a binding, its obligation (if any) riding
                    // in the error.
                    if let Some((recv, method, degr)) = &degradation {
                        let (ename, display) = &degr[0];
                        e.obligations.remove(recv);
                        e.consumed.insert(
                            recv.clone(),
                            Cause::ErrorPath {
                                method: method.clone(),
                                error: ename.clone(),
                                new_state: display.clone(),
                            },
                        );
                    }
                    e
                };
                let mut joined: Vec<FlowState> = Vec::new();
                for handler in handlers {
                    if self.error.is_some() {
                        return;
                    }
                    match handler {
                        CatchHandler::Wildcard { var, body } => {
                            let mut e = entry.clone();
                            e.consumed.remove(&var.node);
                            let r = self.branch(&e, &body.node, false);
                            if !r.terminated {
                                joined.push(r.state);
                            }
                        }
                        CatchHandler::Typed { var, error_type, body } => {
                            let mut e = entry.clone();
                            e.consumed.remove(&var.node);
                            // The handler takes on the obligations of the
                            // error's must-release payload fields, reachable
                            // as `var.field`.
                            let mut payload_keys: Vec<String> = Vec::new();
                            if let Some(payloads) =
                                self.tables.state_errors.get(&error_type.node)
                            {
                                for p in payloads {
                                    if let Some(mut ob) = self.obligation_for_payload(p) {
                                        let key = format!("{}.{}", var.node, p.field);
                                        ob.acquired = var.span;
                                        e.obligations.insert(key.clone(), ob);
                                        payload_keys.push(key);
                                    }
                                }
                            }
                            let prev_error_var =
                                self.error_vars.insert(var.node.clone(), error_type.node.clone());
                            let mut r = self.branch(&e, &body.node, false);
                            match prev_error_var {
                                Some(prev) => {
                                    self.error_vars.insert(var.node.clone(), prev);
                                }
                                None => {
                                    self.error_vars.remove(&var.node);
                                }
                            }
                            if self.error.is_some() {
                                return;
                            }
                            if !r.terminated {
                                // Falling out of the handler with the payload
                                // obligation still live leaks it.
                                for key in &payload_keys {
                                    if let Some(ob) = r.state.obligations.get(key) {
                                        self.error = Some(CompileError::type_err(
                                            format!(
                                                "caught '{}' but its payload '{key}' still \
                                                 holds {display}, a must_release state, when \
                                                 the catch block ends; extract it \
                                                 (`let held = {key}`) and {suggest}",
                                                error_type.node,
                                                display = ob.display,
                                                suggest = suggest_exits(ob),
                                            ),
                                            var.span,
                                        ));
                                        return;
                                    }
                                    r.state.obligations.remove(key);
                                }
                                joined.push(r.state);
                            }
                        }
                        CatchHandler::Shorthand(fb) => {
                            // The fallback evaluates on the error path.
                            let saved = self.snapshot();
                            self.consumed = entry.consumed.clone();
                            self.obligations = entry.obligations.clone();
                            self.visit_expr(fb);
                            let fb_state = self.snapshot();
                            self.consumed = saved.consumed;
                            self.obligations = saved.obligations;
                            joined.push(fb_state);
                        }
                    }
                }
                for j in joined {
                    self.union_into(j);
                }
            }
            _ => walk_expr(self, expr),
        }
    }
}

impl Linearity<'_> {
    /// The obligation a state-carrying payload field imposes, if its state is
    /// must-release.
    fn obligation_for_payload(&self, p: &StatePayload) -> Option<Obligation> {
        let entries = self.tables.must_release.get(&p.base)?;
        for e in entries {
            if p.args.get(e.position).map(|s| s.as_str()) == Some(e.state.as_str()) {
                return Some(Obligation {
                    display: p.display.clone(),
                    state: e.state.clone(),
                    exits: e.exits.clone(),
                    acquired: crate::span::Span::new(0, 0),
                });
            }
        }
        None
    }
}

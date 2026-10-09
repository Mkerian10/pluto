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
//! A method whose *body* transitions `self` must itself be a transition by
//! signature: callers discriminate transitions by the return type, so a
//! hidden `self`-transition would leave the caller's binding live (and its
//! obligation undischarged) for a value that was already consumed — the
//! checker would then *demand* a double release (issue #420).
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
//! class at a must-release state is fully linear, and enforcement is
//! **default-deny**: a move discharges the mover's obligation ONLY when the
//! destination provably carries the obligation onward. The carrying
//! destinations are:
//! - a parameter whose declared type is the same concrete typestated class
//!   at the must-release state (the obligation is re-imposed in the callee);
//! - the function's declared return type, when it is that same concrete
//!   typestated class at the must-release state (the obligation passes to
//!   the caller);
//! - an error payload field of the must-release type (the obligation rides
//!   in the error; a typed catch takes it on);
//! - a plain `let`/assignment (the obligation is re-imposed on the new
//!   binding).
//!
//! Every other destination REJECTS, naming the laundering boundary: generic
//! type parameters (#404), trait upcasts (#405), nullable positions (#406),
//! container/builtin methods and channel sends (#407), if/match expression
//! aliasing (#408), enum variant payloads (#409), aliasing a caught error
//! binding whose payload is obligated (#410), closure parameters and
//! function-value calls (#411), casts, spawn/`at` boundaries, generator
//! yields and scope seeds. The expression walker is exhaustive over `Expr`
//! so a future variant cannot silently become a laundering door.
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
//! - If/match EXPRESSIONS yield values: an arm that yields a must-release
//!   binding moves it into the expression's result, and all fall-through
//!   arms must agree on which obligations they discharged (yielding the same
//!   binding from every arm is a consume-once of that binding).
//! - Closure and spawn bodies are checked against a snapshot of the current
//!   consumed set (capturing a consumed value is a use); their own effects
//!   don't escape the closure (captures are by-value snapshots). Closure
//!   parameters typed at a must-release state carry the obligation into the
//!   closure body exactly like top-level function parameters.
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
use crate::typeck::env::{mangle_method, mangle_type, MethodResolution, TypeEnv};
use crate::typeck::types::PlutoType;
use crate::diagnostics::CompileError;
use crate::visit::Visitor;

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
        check_body(&func.node.name.node, &func.node, env, &tables, true)?;
    }
    for class in &program.classes {
        let base = &class.node.name.node;
        let is_transition =
            |m: &str| transitions.get(base).is_some_and(|ms| ms.contains(m));
        if class.node.type_params.is_empty() {
            for m in &class.node.methods {
                let key = mangle_method(base, &m.node.name.node);
                check_body(&key, &m.node, env, &tables, is_transition(&m.node.name.node))?;
            }
        } else {
            // Generic class template bodies were checked against skolem (or,
            // for `where`-constrained methods, state-bound) instantiations —
            // reconstruct the same instance name to find their resolutions.
            for m in &class.node.methods {
                let key = template_method_key(&class.node, &m.node.name.node, env);
                check_body(&key, &m.node, env, &tables, is_transition(&m.node.name.node))?;
            }
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            let key = mangle_method(&app.node.name.node, &m.node.name.node);
            check_body(&key, &m.node, env, &tables, true)?;
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            let key = mangle_method(&stage.node.name.node, &m.node.name.node);
            check_body(&key, &m.node, env, &tables, true)?;
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

/// A must-release marking: the class's state param `param` (at `position`)
/// bound to `state` makes a binding fully linear.
struct MustReleaseEntry {
    position: usize,
    param: String,
    state: String,
}

/// base class name -> its must-release state markings.
type MustReleaseTable = HashMap<String, Vec<MustReleaseEntry>>;

fn collect_must_release(env: &TypeEnv) -> MustReleaseTable {
    let mut table: MustReleaseTable = HashMap::new();
    for (base, info) in &env.generic_classes {
        if info.must_release.is_empty() {
            continue;
        }
        for (param, state) in &info.must_release {
            let Some(position) = info.type_params.iter().position(|p| p == param) else {
                continue;
            };
            table.entry(base.clone()).or_default().push(MustReleaseEntry {
                position,
                param: param.clone(),
                state: state.clone(),
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
    /// Positional type-argument keys ("Lease$$Revoked" -> ["Revoked"]),
    /// recovered structurally (see `instance_info`).
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
            let Some((base, args)) = instance_info(env, inst) else { continue };
            if args.is_empty() {
                continue;
            }
            let Some(gen_info) = env.generic_classes.get(&base) else { continue };
            let has_state_params = gen_info
                .method_state_constraints
                .values()
                .any(|cs| !cs.is_empty());
            if !has_state_params {
                continue;
            }
            table.entry(ename.clone()).or_default().push(StatePayload {
                field: fname.clone(),
                base,
                args,
                display: display_instance(env, inst),
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

/// Where a `return` lands: a named function's declared return type (the
/// obligation can pass to the caller when the type carries it), or a closure
/// result (a function-value boundary the analysis cannot follow).
#[derive(Clone)]
enum RetCtx {
    Fn(Option<PlutoType>),
    Closure,
}

fn check_body(
    current_fn: &str,
    func: &Function,
    env: &TypeEnv,
    tables: &Tables<'_>,
    self_transition_ok: bool,
) -> Result<(), CompileError> {
    // Signature in the form the BODY was checked against: concrete functions
    // from env.functions; generic templates keep TypeParam types (their
    // bodies never re-impose an obligation on a T-typed parameter).
    let (sig_params, sig_ret): (Option<Vec<PlutoType>>, Option<PlutoType>) =
        match env.functions.get(current_fn) {
            Some(s) => (Some(s.params.clone()), Some(s.return_type.clone())),
            None => match env.generic_functions.get(current_fn) {
                Some(g) => (Some(g.params.clone()), Some(g.return_type.clone())),
                None => (None, None),
            },
        };
    let mut lin = Linearity {
        current_fn,
        current_method: func.name.node.clone(),
        self_transition_ok,
        env,
        tables,
        consumed: HashMap::new(),
        obligations: HashMap::new(),
        var_instances: HashMap::new(),
        error_vars: HashMap::new(),
        forbidden_captures: HashMap::new(),
        scope_stack: Vec::new(),
        return_ctxs: vec![RetCtx::Fn(sig_ret)],
        error: None,
    };
    // Parameters: a non-self param in a must-release state carries the
    // obligation into the callee ("move transfers obligation — caller clean,
    // callee must discharge"). `self` is exempt: the class's own methods
    // define the protocol and are themselves the discharge points.
    if let Some(params) = &sig_params {
        for (p, ty) in func.params.iter().zip(params) {
            if p.name.node == "self" {
                continue;
            }
            if let Some(inst) = type_instance_key(ty) {
                lin.var_instances.insert(p.name.node.clone(), inst.clone());
                if let Some(ob) = lin.obligation_for(&inst) {
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
    /// Transition methods that discharge the state (".release()") and exist
    /// on the binding's actual instantiation.
    exits: Vec<String>,
    acquired: crate::span::Span,
}

/// Whether a move destination carries the obligation onward (discharging the
/// mover) or launders it (rejected, naming the boundary).
enum DestVerdict {
    Carries,
    Deny(String),
}

struct ScopeFrame {
    declared: Vec<String>,
    is_loop_body: bool,
}

struct Linearity<'a> {
    current_fn: &'a str,
    /// The method/function name being checked (unmangled), for diagnostics.
    current_method: String,
    /// Whether this body may consume `self` via a transition: true only when
    /// the method is itself a transition by signature (issue #420).
    self_transition_ok: bool,
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
    /// Innermost return destination (function signature or closure).
    return_ctxs: Vec<RetCtx>,
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

/// The base name and positional type-argument keys of a mangled class
/// instance. Structural first (env.class_instance_args, recorded at every
/// instantiation — exact even for nested generic args, issue #412); the
/// naive `Base$$a$b` split remains only as a fallback for instance names
/// that never went through `ensure_generic_class_instantiated`, where args
/// are un-nested by construction.
fn instance_info(env: &TypeEnv, mangled: &str) -> Option<(String, Vec<String>)> {
    if let Some((base, args)) = env.class_instance_args.get(mangled) {
        return Some((base.clone(), args.iter().map(mangle_type).collect()));
    }
    let (base, rest) = mangled.split_once("$$")?;
    Some((base.to_string(), rest.split('$').map(|s| s.to_string()).collect()))
}

/// `Partition$$Owned` → `Partition<Owned>` for error messages, demangling
/// nested generic args structurally where possible.
fn display_instance(env: &TypeEnv, mangled: &str) -> String {
    if let Some((base, args)) = env.class_instance_args.get(mangled) {
        let shown: Vec<String> = args
            .iter()
            .map(|a| match a {
                PlutoType::Class(n) | PlutoType::Enum(n) => display_instance(env, n),
                other => format!("{other}"),
            })
            .collect();
        return format!("{base}<{}>", shown.join(", "));
    }
    match mangled.split_once("$$") {
        Some((base, rest)) => {
            format!("{base}<{}>", rest.split('$').collect::<Vec<_>>().join(", "))
        }
        None => mangled.to_string(),
    }
}

/// The mangled-instance key a signature type corresponds to, when it names a
/// class instance (concrete or template form).
fn type_instance_key(ty: &PlutoType) -> Option<String> {
    match ty {
        PlutoType::Class(inst) => Some(inst.clone()),
        PlutoType::GenericInstance(_, base, args) => {
            Some(crate::typeck::env::mangle_name(base, args))
        }
        _ => None,
    }
}

impl Linearity<'_> {
    /// If `inst` (a mangled class-instance name) is in a must-release state,
    /// the obligation to attach to a binding holding it.
    fn obligation_for(&self, inst: &str) -> Option<Obligation> {
        let (base, args) = instance_info(self.env, inst)?;
        let entries = self.tables.must_release.get(&base)?;
        for e in entries {
            if args.get(e.position).map(|s| s.as_str()) == Some(e.state.as_str()) {
                return Some(Obligation {
                    display: display_instance(self.env, inst),
                    state: e.state.clone(),
                    exits: self.exits_for(&base, &args, &e.param, &e.state),
                    acquired: crate::span::Span::new(0, 0),
                });
            }
        }
        None
    }

    /// Transition methods that discharge `param == state` AND exist on the
    /// binding's actual instantiation (every `where` constraint of the
    /// suggested method must hold for these args — issue #414).
    fn exits_for(&self, base: &str, arg_keys: &[String], param: &str, state: &str) -> Vec<String> {
        let Some(gi) = self.env.generic_classes.get(base) else { return Vec::new() };
        let Some(ts) = self.tables.transitions.get(base) else { return Vec::new() };
        let pos_of = |p: &str| gi.type_params.iter().position(|x| x == p);
        let mut exits: Vec<String> = gi
            .method_state_constraints
            .iter()
            .filter(|(m, cs)| {
                ts.contains(*m)
                    && cs.iter().any(|(p, s)| p == param && s == state)
                    && cs.iter().all(|(p, s)| {
                        pos_of(p)
                            .and_then(|i| arg_keys.get(i))
                            .is_some_and(|k| k == s)
                    })
            })
            .map(|(m, _)| format!(".{m}()"))
            .collect();
        exits.sort();
        exits
    }

    /// The mangled class-instance type of an expression, for the cases this
    /// analysis understands (bindings, struct literals, resolved calls,
    /// catch/propagate wrappers, typed-catch payload fields, and if/match
    /// expression results).
    fn expr_instance(&self, expr: &Spanned<Expr>) -> Option<String> {
        match &expr.node {
            Expr::Ident(name) => self.var_instances.get(name).cloned(),
            Expr::StructLit { type_args, .. } => {
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
            Expr::If { then_block, .. } => {
                let tail = block_tail(&then_block.node)?;
                self.expr_instance(tail)
            }
            Expr::Match { arms, .. } => {
                let first = arms.first()?;
                self.expr_instance(&first.value)
            }
            _ => None,
        }
    }

    /// The live obligation `expr` would move, if any: a bare obligated
    /// binding, a typed-catch payload field, or an expression producing a
    /// value in a must-release state.
    fn value_obligation(&self, expr: &Spanned<Expr>) -> Option<Obligation> {
        match &expr.node {
            Expr::Ident(name) => self.obligations.get(name).cloned(),
            Expr::FieldAccess { object, field } => {
                if let Expr::Ident(obj) = &object.node
                    && self.error_vars.contains_key(obj)
                {
                    return self.obligations.get(&format!("{obj}.{}", field.node)).cloned();
                }
                None
            }
            _ => self
                .expr_instance(expr)
                .and_then(|inst| self.obligation_for(&inst)),
        }
    }

    /// Whether moving an obligated value into a destination of declared type
    /// `ty` carries the obligation onward.
    fn dest_for_type(&self, ty: &PlutoType) -> DestVerdict {
        match ty {
            PlutoType::Class(inst) => {
                if self.obligation_for(inst).is_some() {
                    DestVerdict::Carries
                } else if inst.contains('%') {
                    DestVerdict::Deny(
                        "a generic position — the callee is checked with its type \
                         parameters unbound, so the release obligation would not be \
                         re-imposed there"
                            .to_string(),
                    )
                } else {
                    DestVerdict::Deny(format!(
                        "a position of type {} — no release obligation is re-imposed there",
                        display_instance(self.env, inst)
                    ))
                }
            }
            PlutoType::GenericInstance(_, base, args) => {
                let keys: Vec<String> = args.iter().map(mangle_type).collect();
                match self.tables.must_release.get(base.as_str()) {
                    Some(entries)
                        if entries.iter().any(|e| {
                            keys.get(e.position).map(|k| k.as_str()) == Some(e.state.as_str())
                        }) =>
                    {
                        DestVerdict::Carries
                    }
                    Some(entries)
                        if entries.iter().any(|e| {
                            matches!(args.get(e.position), Some(PlutoType::TypeParam(_)))
                        }) =>
                    {
                        DestVerdict::Deny(format!(
                            "a position whose state parameter is generic ('{ty}') — the \
                             release obligation would not be re-imposed there"
                        ))
                    }
                    _ => DestVerdict::Deny(format!(
                        "a position of type {ty} — no release obligation is re-imposed there"
                    )),
                }
            }
            PlutoType::TypeParam(p) => DestVerdict::Deny(format!(
                "the generic type parameter '{p}' — a type parameter cannot carry a \
                 release obligation"
            )),
            PlutoType::Trait(t) => DestVerdict::Deny(format!(
                "the trait type '{t}' — upcasting erases the typestate and its release \
                 obligation"
            )),
            PlutoType::Nullable(_) => DestVerdict::Deny(
                "a nullable type — a release obligation does not survive nullable wrapping"
                    .to_string(),
            ),
            other => DestVerdict::Deny(format!(
                "a position of type {other} — it cannot carry a release obligation"
            )),
        }
    }

    /// Emit the default-deny diagnostic for an obligated value flowing into a
    /// non-carrying destination.
    fn deny_move(&mut self, ob: &Obligation, boundary: &str, span: crate::span::Span) {
        if self.error.is_some() {
            return;
        }
        self.error = Some(CompileError::type_err(
            format!(
                "a value in the must_release state {display} may not be moved into \
                 {boundary}; keep it in a local binding and {suggest}, pass it to a \
                 parameter of type {display}, or return it at type {display}",
                display = ob.display,
                suggest = suggest_exits(ob),
            ),
            span,
        ));
    }

    /// Aliasing a caught error binding whose type carries a must-release
    /// payload would duplicate access to the payload (issue #410). Rejected
    /// whether or not the payload was already extracted: the alias's reads
    /// are untracked. Returns true when an error was emitted.
    fn check_error_alias(&mut self, expr: &Spanned<Expr>) -> bool {
        if self.error.is_some() {
            return true;
        }
        let Expr::Ident(name) = &expr.node else { return false };
        let Some(ename) = self.error_vars.get(name) else { return false };
        let Some(payloads) = self.tables.state_errors.get(ename) else { return false };
        let Some(p) = payloads
            .iter()
            .find(|p| self.obligation_for_payload(p).is_some())
        else {
            return false;
        };
        self.error = Some(CompileError::type_err(
            format!(
                "cannot copy the caught error binding '{name}': '{ename}' carries {} in \
                 field '{}', a must_release state, and the copy's reads of the payload \
                 would not be tracked; extract the payload once \
                 (`let held = {name}.{}`) and use that binding",
                p.display, p.field, p.field
            ),
            expr.span,
        ));
        true
    }

    /// Visit a block as a lexical scope: must-release bindings declared in it
    /// must be discharged before it ends. Returns true when the block
    /// terminates (return/raise/break/continue) — exits check obligations
    /// themselves and the block end is unreachable.
    fn visit_block_scope(&mut self, block: &Block, is_loop_body: bool) -> bool {
        self.visit_block_scope_inner(block, is_loop_body, None)
    }

    /// Like `visit_block_scope`, but when `yield_to` is set the block's tail
    /// expression statement is a VALUE YIELD (if-expression arm): the value
    /// moves into the expression's result instead of being dropped.
    fn visit_block_scope_inner(
        &mut self,
        block: &Block,
        is_loop_body: bool,
        yield_to: Option<&str>,
    ) -> bool {
        self.scope_stack.push(ScopeFrame { declared: Vec::new(), is_loop_body });
        let mut terminated = false;
        let last_idx = block.stmts.len().checked_sub(1);
        for (i, stmt) in block.stmts.iter().enumerate() {
            if self.error.is_some() {
                break;
            }
            if let Some(to) = yield_to
                && Some(i) == last_idx
                && let Stmt::Expr(e) = &stmt.node
            {
                // The arm's value: a move into the expression's result (the
                // result carries the obligation onward; see Expr::If/Match).
                self.move_expr(e, to);
            } else {
                self.visit_stmt(stmt);
            }
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
            Some(PlutoType::Class(ret)) => display_instance(self.env, ret),
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

    /// The parameter types a resolved class-method call declares, one per
    /// argument (the `self` slot stripped), in the form the callee's BODY was
    /// checked against: generic-class template sigs keep TypeParam, hoisted
    /// generic methods come from their template, concrete methods from
    /// env.functions.
    fn method_param_types(&self, mangled: &str, nargs: usize) -> Option<Vec<PlutoType>> {
        let full: Option<Vec<PlutoType>> = (|| {
            // Hoisted generic method instance: "C$foo$$int" — template "C$foo".
            if let Some((prefix, _)) = mangled.split_once("$$")
                && let Some(g) = self.env.generic_functions.get(prefix)
            {
                return Some(g.params.clone());
            }
            // Generic-class method: template sig (TypeParam visible).
            if let Some((class_inst, m)) = mangled.rsplit_once('$') {
                let class_inst = class_inst.trim_end_matches('$');
                if let Some((base, _)) = instance_info(self.env, class_inst)
                    && let Some(gi) = self.env.generic_classes.get(&base)
                    && let Some(sig) = gi.method_sigs.get(m)
                {
                    return Some(sig.params.clone());
                }
            }
            self.env.functions.get(mangled).map(|s| s.params.clone())
        })();
        let ps = full?;
        if ps.len() == nargs + 1 {
            Some(ps[1..].to_vec())
        } else if ps.len() == nargs {
            // Static method (no self) — params align 1:1.
            Some(ps)
        } else {
            None
        }
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
        let Some((base, recv_args)) = instance_info(self.env, class_inst) else {
            return Vec::new();
        };
        let Some(gen_info) = self.env.generic_classes.get(&base) else {
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

    /// Treat `expr` as a move into a CARRYING destination (binding, carrying
    /// parameter, carrying return/raise payload, if/match yield): a bare
    /// obligated binding — or a typed-catch payload field — is moved, which
    /// discharges its obligation here and consumes the binding. Other
    /// expressions are visited normally.
    fn move_expr(&mut self, expr: &Spanned<Expr>, to: &str) {
        if self.error.is_some() {
            return;
        }
        if self.check_error_alias(expr) {
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

    /// Move `expr` into a destination whose carrying status was classified by
    /// the caller: a carrying destination discharges as usual; any other
    /// destination rejects an obligated value, naming the boundary.
    fn move_arg(&mut self, expr: &Spanned<Expr>, to: &str, verdict: DestVerdict) {
        if self.error.is_some() {
            return;
        }
        if self.check_error_alias(expr) {
            return;
        }
        if let Some(ob) = self.value_obligation(expr) {
            match verdict {
                DestVerdict::Carries => self.move_expr(expr, to),
                DestVerdict::Deny(boundary) => self.deny_move(&ob, &boundary, expr.span),
            }
        } else {
            self.move_expr(expr, to)
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
    /// track (fields, container literals, enum variant payloads, container
    /// elements).
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

    /// Reject an obligated TEMPORARY (non-binding expression) in a position
    /// that discards its value (operator operands, interpolations, indexes):
    /// nothing would hold the obligation afterwards.
    fn check_discarded_temp(&mut self, e: &Spanned<Expr>) {
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

    /// Visit a branch against a snapshot; returns the branch's flow state.
    fn branch(
        &mut self,
        entry: &FlowState,
        block: &Block,
        is_loop_body: bool,
    ) -> BranchResult {
        self.branch_inner(entry, block, is_loop_body, None)
    }

    fn branch_inner(
        &mut self,
        entry: &FlowState,
        block: &Block,
        is_loop_body: bool,
        yield_to: Option<&str>,
    ) -> BranchResult {
        let saved_consumed = std::mem::replace(&mut self.consumed, entry.consumed.clone());
        let saved_obligations =
            std::mem::replace(&mut self.obligations, entry.obligations.clone());
        let terminated = self.visit_block_scope_inner(block, is_loop_body, yield_to);
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

    /// Join for exhaustive branch sets (if/else, match): control continues
    /// only through the fall-through branches, so obligations after the join
    /// are exactly those live on SOME fall-through path — an obligation
    /// discharged on every path is discharged. Consumption stays
    /// conservative (consumed on any path, before or inside, stays
    /// consumed).
    fn join_exclusive(&mut self, fallthroughs: Vec<FlowState>) {
        self.obligations.clear();
        for ft in fallthroughs {
            for (k, v) in ft.consumed {
                self.consumed.entry(k).or_insert(v);
            }
            for (k, v) in ft.obligations {
                self.obligations.entry(k).or_insert(v);
            }
        }
    }

    /// Join the arms of an if/match EXPRESSION: all fall-through arms must
    /// agree on which pre-existing obligations they discharged (yielding the
    /// same binding from every arm is a consume-once; discharging in one arm
    /// but not another is rejected — the aliasing door of issue #408).
    fn join_value_arms(
        &mut self,
        entry: &FlowState,
        results: Vec<BranchResult>,
        form: &str,
        span: crate::span::Span,
    ) {
        if self.error.is_some() {
            return;
        }
        let fallthroughs: Vec<FlowState> = results
            .into_iter()
            .filter(|r| !r.terminated)
            .map(|r| r.state)
            .collect();
        let discharged = |st: &FlowState| -> Vec<String> {
            let mut v: Vec<String> = entry
                .obligations
                .keys()
                .filter(|k| !st.obligations.contains_key(*k))
                .cloned()
                .collect();
            v.sort();
            v
        };
        if let Some(first) = fallthroughs.first() {
            let base = discharged(first);
            for st in &fallthroughs[1..] {
                let d = discharged(st);
                if d != base {
                    let name = base
                        .iter()
                        .chain(d.iter())
                        .find(|n| base.contains(n) != d.contains(n))
                        .cloned()
                        .unwrap_or_default();
                    self.error = Some(CompileError::type_err(
                        format!(
                            "the arms of this {form} disagree about '{name}', which holds \
                             a must_release state: one arm moves it (consuming the \
                             binding) and another does not; make every arm consume the \
                             same must_release bindings"
                        ),
                        span,
                    ));
                    return;
                }
            }
        }
        self.join_exclusive(fallthroughs);
    }

    /// Names bound by a match pattern (payload bindings).
    fn pattern_names(pattern: &MatchPattern) -> Vec<Spanned<String>> {
        match pattern {
            MatchPattern::Variant { bindings, .. } => bindings
                .iter()
                .map(|(field, alias)| alias.clone().unwrap_or_else(|| field.clone()))
                .collect(),
            MatchPattern::Wildcard { .. } => Vec::new(),
        }
    }

    /// Enter a match arm's pattern scope: a binding may not shadow a name
    /// holding a live obligation (uses inside the arm would be mis-attributed
    /// to the shadowed binding), and bound names are fresh locals for the
    /// arm. Returns the saved var_instances entries to restore, or None when
    /// an error was emitted.
    fn enter_pattern(&mut self, pattern: &MatchPattern) -> Option<Vec<(String, Option<String>)>> {
        let names = Self::pattern_names(pattern);
        for n in &names {
            if let Some(ob) = self.obligations.get(&n.node) {
                self.error = Some(CompileError::type_err(
                    format!(
                        "pattern binding '{}' shadows a binding that still holds {}, a \
                         must_release state; rename the pattern binding, or {} first",
                        n.node,
                        ob.display,
                        suggest_exits(ob),
                    ),
                    n.span,
                ));
                return None;
            }
        }
        let saved: Vec<(String, Option<String>)> = names
            .iter()
            .map(|n| (n.node.clone(), self.var_instances.get(&n.node).cloned()))
            .collect();
        for n in &names {
            self.var_instances.remove(&n.node);
            self.consumed.remove(&n.node);
            self.error_vars.remove(&n.node);
        }
        Some(saved)
    }

    fn exit_pattern(&mut self, saved: Vec<(String, Option<String>)>) {
        for (name, inst) in saved {
            match inst {
                Some(i) => {
                    self.var_instances.insert(name, i);
                }
                None => {
                    self.var_instances.remove(&name);
                }
            }
        }
    }

    /// After a match arm, pattern-bound names die with the arm: reset their
    /// consumed status in the arm's result to the entry state so the arm's
    /// local shadowing never leaks into the join.
    fn reset_pattern_consumed(entry: &FlowState, state: &mut FlowState, pattern: &MatchPattern) {
        for n in Self::pattern_names(pattern) {
            match entry.consumed.get(&n.node) {
                Some(c) => {
                    state.consumed.insert(n.node.clone(), c.clone());
                }
                None => {
                    state.consumed.remove(&n.node);
                }
            }
        }
    }

    /// The hidden self-transition rule (issue #420): a body may consume
    /// `self` via a transition only when the method's own signature declares
    /// the transition — callers discriminate by signature, and a hidden
    /// consumption would force them into a double release.
    fn check_self_transition(&mut self, method: &Spanned<String>) {
        if self.error.is_some() || self.self_transition_ok {
            return;
        }
        self.error = Some(CompileError::type_err(
            format!(
                "method '{}' calls the transition '.{}()' on 'self' but its signature \
                 does not declare a transition; a method that consumes 'self' must \
                 return the transitioned class so callers see the state change — \
                 change the return type to the new state and return the transition's \
                 result, or move the transition to the caller",
                self.current_method, method.node
            ),
            method.span,
        ));
    }
}

/// The trailing expression statement of a block (an if-expression arm's
/// value), if any.
fn block_tail(block: &Block) -> Option<&Spanned<Expr>> {
    match &block.stmts.last()?.node {
        Stmt::Expr(e) => Some(e),
        _ => None,
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
            Stmt::Let { name, ty, value, .. } => {
                // `_` is a non-binding discard (#507). It must not launder a
                // release obligation: dropping a must_release / typestate
                // result (or an obligated binding) via `let _ = ...` would
                // otherwise skip the scope-exit leak check. Reject it like an
                // immediate drop; an ordinary value is just visited (so the
                // receiver of `let _ = x.transition()` is still consumed) and
                // not bound.
                if name.node == "_" {
                    if let Some(ob) = self.value_obligation(value) {
                        self.error = Some(CompileError::type_err(
                            format!(
                                "`let _ = ...` discards {display}, a must_release state; \
                                 bind the result (`let held = ...`) and {suggest}",
                                display = ob.display,
                                suggest = suggest_exits(&ob),
                            ),
                            value.span,
                        ));
                        return;
                    }
                    self.visit_expr(value);
                    return;
                }
                // A binding at a laundering type annotation (nullable, trait)
                // erases the obligation even though the value itself is
                // tracked — reject the annotation form outright.
                if let Some(ob) = self.value_obligation(value)
                    && let Some(ann) = ty
                {
                    let boundary: Option<String> = match &ann.node {
                        TypeExpr::Nullable(_) => Some(
                            "a nullable binding — a release obligation does not survive \
                             nullable wrapping"
                                .to_string(),
                        ),
                        TypeExpr::Named(n) if self.env.traits.contains_key(n) => Some(format!(
                            "a binding at trait type '{n}' — upcasting erases the \
                             typestate and its release obligation"
                        )),
                        _ => None,
                    };
                    if let Some(b) = boundary {
                        self.deny_move(&ob, &b, value.span);
                        return;
                    }
                }
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
            Stmt::IndexAssign { object, index, value } => {
                self.visit_expr(object);
                self.visit_expr(index);
                self.check_untracked_store(value, "a container element");
                self.visit_expr(value);
            }
            Stmt::Return(value) => {
                if let Some(v) = value {
                    let verdict = match self.return_ctxs.last() {
                        Some(RetCtx::Closure) => DestVerdict::Deny(
                            "the result of a closure — a release obligation cannot flow \
                             through function values"
                                .to_string(),
                        ),
                        Some(RetCtx::Fn(Some(ty))) => {
                            let ty = ty.clone();
                            self.dest_for_type(&ty)
                        }
                        Some(RetCtx::Fn(None)) | None => DestVerdict::Deny(
                            "a return position whose declared type this analysis cannot \
                             see"
                                .to_string(),
                        ),
                    };
                    self.move_arg(v, "returned to the caller", verdict);
                }
                self.check_exit("cannot return", stmt.span);
            }
            Stmt::Raise { error_name, fields, .. } => {
                let einfo = self.env.errors.get(&error_name.node).cloned().or_else(|| {
                    // Flattened modules may register the error under a
                    // prefixed name; match on the unqualified suffix.
                    let suffix =
                        error_name.node.rsplit('.').next().unwrap_or(&error_name.node);
                    let mut found = None;
                    for (k, v) in &self.env.errors {
                        if k.rsplit('.').next() == Some(suffix) {
                            if found.is_some() {
                                return None; // ambiguous — don't guess
                            }
                            found = Some(v.clone());
                        }
                    }
                    found
                });
                for (fname, fval) in fields {
                    let verdict = match einfo
                        .as_ref()
                        .and_then(|ei| ei.fields.iter().find(|(n, _)| n == &fname.node))
                    {
                        Some((_, ty)) => {
                            let ty = ty.clone();
                            self.dest_for_type(&ty)
                        }
                        None => DestVerdict::Deny(format!(
                            "the error field '{}.{}' whose declared type this analysis \
                             cannot see",
                            error_name.node, fname.node
                        )),
                    };
                    self.move_arg(
                        fval,
                        &format!("into '{}.{}'", error_name.node, fname.node),
                        verdict,
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
                let mut fallthroughs = Vec::new();
                if !after_then.terminated {
                    fallthroughs.push(after_then.state);
                }
                if !after_else.terminated {
                    fallthroughs.push(after_else.state);
                }
                self.join_exclusive(fallthroughs);
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
                let mut fallthroughs: Vec<FlowState> = Vec::new();
                for arm in arms {
                    if self.error.is_some() {
                        return;
                    }
                    let Some(saved) = self.enter_pattern(&arm.pattern) else { return };
                    let mut r = self.branch(&entry, &arm.body.node, false);
                    self.exit_pattern(saved);
                    Self::reset_pattern_consumed(&entry, &mut r.state, &arm.pattern);
                    if !r.terminated {
                        fallthroughs.push(r.state);
                    }
                }
                // Match is exhaustive: control continues only through the
                // fall-through arms.
                self.join_exclusive(fallthroughs);
            }
            Stmt::LetChan { sender, receiver, elem_type: _, capacity } => {
                if let Some(c) = capacity {
                    self.visit_expr(c);
                }
                self.bind(&sender.node, None, sender.span);
                self.bind(&receiver.node, None, receiver.span);
            }
            Stmt::Select { arms, default, after } => {
                // Channel operations first: sending a must-release value into
                // a channel is a laundering boundary like `.send()`.
                for arm in arms {
                    match &arm.op {
                        SelectOp::Recv { binding: _, channel } => {
                            self.visit_expr(channel);
                        }
                        SelectOp::Send { channel, value } => {
                            self.visit_expr(channel);
                            let verdict = DestVerdict::Deny(
                                "a channel — a release obligation cannot be sent through \
                                 a channel"
                                    .to_string(),
                            );
                            self.move_arg(value, "sent into a channel", verdict);
                        }
                    }
                }
                let entry = self.snapshot();
                let mut states: Vec<FlowState> = Vec::new();
                for arm in arms {
                    if self.error.is_some() {
                        return;
                    }
                    if let SelectOp::Recv { binding, .. } = &arm.op {
                        // The received value's type is untracked (channels
                        // cannot hold obligated values); the binding is a
                        // fresh local inside the arm.
                        let saved_inst = self.var_instances.remove(&binding.node);
                        let r = self.branch(&entry, &arm.body.node, false);
                        if let Some(i) = saved_inst {
                            self.var_instances.insert(binding.node.clone(), i);
                        }
                        states.push(r.state);
                    } else {
                        let r = self.branch(&entry, &arm.body.node, false);
                        states.push(r.state);
                    }
                }
                if let Some(d) = default {
                    let r = self.branch(&entry, &d.node, false);
                    states.push(r.state);
                }
                if let Some(a) = after {
                    self.visit_expr(&a.duration);
                    let r = self.branch(&entry, &a.body.node, false);
                    states.push(r.state);
                }
                for st in states {
                    self.union_into(st);
                }
            }
            Stmt::Scope { seeds, bindings: _, body } => {
                for seed in seeds {
                    let verdict = DestVerdict::Deny(
                        "a scope seed — the release obligation would escape into the \
                         scope's injected fields"
                            .to_string(),
                    );
                    self.move_arg(seed, "seeded into a scope", verdict);
                }
                self.visit_block_scope(&body.node, false);
            }
            Stmt::Yield { value } => {
                let verdict = DestVerdict::Deny(
                    "a generator yield — the release obligation cannot be tracked \
                     through a stream"
                        .to_string(),
                );
                self.move_arg(value, "yielded from a generator", verdict);
            }
            Stmt::Assert { expr } => {
                self.visit_expr(expr);
            }
            Stmt::ExpectRaises { body, .. } => {
                // The block may stop at any raising statement: any prefix of
                // it may have run. Consumption grows monotonically along the
                // block, so unioning the entry state with the full-execution
                // state covers every prefix.
                let entry = self.snapshot();
                let r = self.branch(&entry, &body.node, false);
                self.union_into(r.state);
            }
            Stmt::Serve { service, port } => {
                self.visit_expr(service);
                self.visit_expr(port);
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
        }
    }

    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if self.error.is_some() {
            return;
        }
        match &expr.node {
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::NoneLit
            | Expr::EnumUnit { .. }
            | Expr::QualifiedAccess { .. }
            | Expr::ClosureCreate { .. } => {}
            Expr::Ident(name) => {
                self.use_var(name, expr.span);
            }
            Expr::FieldAccess { object, field: _ } => {
                // Reading a field is a plain use; payload-field moves and
                // method calls on payload fields are handled by the move
                // machinery and the MethodCall receiver rules.
                self.visit_expr(object);
            }
            Expr::BinOp { op: _, lhs, rhs } => {
                self.visit_expr(lhs);
                self.check_discarded_temp(lhs);
                self.visit_expr(rhs);
                self.check_discarded_temp(rhs);
            }
            Expr::CompareChain { operands, .. } => {
                for operand in operands {
                    self.visit_expr(operand);
                    self.check_discarded_temp(operand);
                }
            }
            Expr::UnaryOp { op: _, operand } => {
                self.visit_expr(operand);
                self.check_discarded_temp(operand);
            }
            Expr::MethodCall { object, method, args, .. } => {
                self.visit_expr(object);
                // Argument destinations: carrying only when the callee's
                // declared parameter re-imposes the obligation.
                let resolution = self
                    .env
                    .method_resolutions
                    .get(&(self.current_fn.to_string(), method.span.start));
                enum ArgPolicy {
                    Params(Vec<PlutoType>),
                    Deny(String),
                }
                let policy = match resolution {
                    Some(MethodResolution::Class { mangled_name }) => {
                        match self.method_param_types(mangled_name, args.len()) {
                            Some(ps) => ArgPolicy::Params(ps),
                            None => ArgPolicy::Deny(format!(
                                "'.{}()' — a callee whose parameter types this analysis \
                                 cannot see",
                                method.node
                            )),
                        }
                    }
                    Some(MethodResolution::RemoteClass { .. }) => ArgPolicy::Deny(format!(
                        "the remote call '.{}()' — a release obligation cannot cross a \
                         service boundary",
                        method.node
                    )),
                    Some(MethodResolution::TraitDynamic { trait_name, method_name }) => {
                        let sig = self
                            .env
                            .traits
                            .get(trait_name)
                            .and_then(|ti| ti.methods.iter().find(|(n, _)| n == method_name))
                            .map(|(_, s)| s.params.clone());
                        match sig {
                            Some(ps) if ps.len() == args.len() + 1 => {
                                ArgPolicy::Params(ps[1..].to_vec())
                            }
                            Some(ps) if ps.len() == args.len() => ArgPolicy::Params(ps),
                            _ => ArgPolicy::Deny(format!(
                                "the trait-dispatched call '.{}()' — a callee whose \
                                 parameter types this analysis cannot see",
                                method.node
                            )),
                        }
                    }
                    Some(MethodResolution::Builtin) => ArgPolicy::Deny(format!(
                        "the builtin method '.{}()' — a container or builtin cannot carry \
                         the release obligation onward",
                        method.node
                    )),
                    Some(MethodResolution::ChannelSend | MethodResolution::ChannelTrySend) => {
                        ArgPolicy::Deny(format!(
                            "a channel ('.{}()') — a release obligation cannot be sent \
                             through a channel",
                            method.node
                        ))
                    }
                    Some(
                        MethodResolution::ChannelRecv
                        | MethodResolution::ChannelTryRecv
                        | MethodResolution::ChannelRecvTimeout
                        | MethodResolution::TaskGet { .. }
                        | MethodResolution::TaskDetach
                        | MethodResolution::TaskCancel,
                    )
                    | None => ArgPolicy::Deny(format!(
                        "'.{}()' — a callee whose parameter types this analysis cannot see",
                        method.node
                    )),
                };
                for (i, a) in args.iter().enumerate() {
                    let verdict = match &policy {
                        ArgPolicy::Params(ps) => match ps.get(i) {
                            Some(t) => self.dest_for_type(t),
                            None => DestVerdict::Deny(format!(
                                "'.{}()' — a parameter position this analysis cannot see",
                                method.node
                            )),
                        },
                        ArgPolicy::Deny(b) => DestVerdict::Deny(b.clone()),
                    };
                    self.move_arg(a, &format!("passed to '.{}()'", method.node), verdict);
                }
                if self.error.is_some() {
                    return;
                }
                // Receiver effects.
                match &object.node {
                    Expr::Ident(recv) => {
                        if let Some(new_state) = self.transition_target(method.span.start) {
                            if recv == "self" {
                                self.check_self_transition(method);
                                if self.error.is_some() {
                                    return;
                                }
                            }
                            // The transition consumes the receiver and
                            // discharges its obligation — the value (and any
                            // obligation of the new state) lives on in the
                            // result.
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
                    Expr::FieldAccess { object: inner, field }
                        if matches!(&inner.node, Expr::Ident(o)
                            if self.error_vars.contains_key(o)) =>
                    {
                        let Expr::Ident(obj) = &inner.node else { unreachable!() };
                        let key = format!("{obj}.{}", field.node);
                        if self.obligations.contains_key(&key)
                            || self.consumed.contains_key(&key)
                        {
                            self.error = Some(CompileError::type_err(
                                format!(
                                    "cannot call '.{}()' directly on the error payload \
                                     '{key}' — its must_release obligation is tracked \
                                     through extraction; extract it first \
                                     (`let held = {key}`) and call the method on the \
                                     binding",
                                    method.node
                                ),
                                expr.span,
                            ));
                        }
                    }
                    _ => {
                        // An obligated TEMPORARY receiver: legal only when
                        // this very call is the transition that consumes it
                        // (the result carries the follow-on state).
                        if let Some(inst) = self.expr_instance(object)
                            && let Some(ob) = self.obligation_for(&inst)
                            && self.transition_target(method.span.start).is_none()
                        {
                            self.error = Some(CompileError::type_err(
                                format!(
                                    "the receiver of '.{}()' is an unbound value in the \
                                     must_release state {display}, and the call does not \
                                     transition it out — the value would be dropped; bind \
                                     it first (`let held = ...`) and {suggest}",
                                    method.node,
                                    display = ob.display,
                                    suggest = suggest_exits(&ob),
                                ),
                                object.span,
                            ));
                        }
                    }
                }
            }
            Expr::Call { name, args, .. } => {
                // Param types in the form the callee's BODY was checked
                // against: generic templates keep TypeParam; concrete
                // functions carry concrete types; unknown callees (closure
                // variables, builtins) deny.
                let params: Option<Vec<PlutoType>> =
                    if let Some(g) = self.env.generic_functions.get(&name.node) {
                        Some(g.params.clone())
                    } else {
                        self.env.functions.get(&name.node).map(|s| s.params.clone())
                    };
                for (i, a) in args.iter().enumerate() {
                    let verdict = match &params {
                        Some(ps) => match ps.get(i) {
                            Some(t) => self.dest_for_type(t),
                            None => DestVerdict::Deny(format!(
                                "'{}()' — a parameter position this analysis cannot see",
                                name.node
                            )),
                        },
                        None => DestVerdict::Deny(format!(
                            "'{}()' — not a named function, so a release obligation \
                             cannot flow through it (function values and builtins cannot \
                             carry obligations)",
                            name.node
                        )),
                    };
                    self.move_arg(a, &format!("passed to '{}()'", name.node), verdict);
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, fval) in fields {
                    self.check_untracked_store(fval, "a field");
                    self.visit_expr(fval);
                }
            }
            Expr::EnumData { fields, .. } => {
                for (_, fval) in fields {
                    self.check_untracked_store(fval, "an enum variant payload");
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
            Expr::Index { object, index } => {
                self.visit_expr(object);
                self.check_discarded_temp(object);
                self.visit_expr(index);
                self.check_discarded_temp(index);
            }
            Expr::StringInterp { parts } => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr(e);
                        self.check_discarded_temp(e);
                    }
                }
            }
            Expr::Closure { body, params, .. } => {
                // Captures are by-value snapshots: uses of consumed outer
                // bindings inside the closure are errors (the capture reads a
                // moved value), and capturing a live must-release binding
                // would duplicate its obligation. The closure body is its own
                // scope for obligations it creates (it runs later, possibly
                // never, possibly on another task). A parameter typed at a
                // must-release state carries the obligation into the body
                // exactly like a top-level function parameter (issue #411).
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
                // Seed parameter obligations from the resolved closure
                // signature; params shadow outer instance tracking.
                let ptypes = self
                    .env
                    .closure_param_types
                    .get(&(expr.span.start, expr.span.end));
                let saved_insts: Vec<(String, Option<String>)> = params
                    .iter()
                    .map(|p| {
                        (p.name.node.clone(), self.var_instances.get(&p.name.node).cloned())
                    })
                    .collect();
                for (i, p) in params.iter().enumerate() {
                    let inst = ptypes.and_then(|ps| ps.get(i)).and_then(type_instance_key);
                    match inst {
                        Some(inst) => {
                            if let Some(mut ob) = self.obligation_for(&inst) {
                                ob.acquired = p.name.span;
                                self.obligations.insert(p.name.node.clone(), ob);
                            }
                            self.var_instances.insert(p.name.node.clone(), inst);
                        }
                        None => {
                            self.var_instances.remove(&p.name.node);
                        }
                    }
                }
                let saved_scopes = std::mem::take(&mut self.scope_stack);
                self.return_ctxs.push(RetCtx::Closure);
                let terminated = self.visit_block_scope(&body.node, false);
                self.return_ctxs.pop();
                if !terminated && self.error.is_none() {
                    let mut live: Vec<(&String, &Obligation)> =
                        self.obligations.iter().collect();
                    live.sort_by_key(|(n, _)| n.as_str());
                    if let Some((name, ob)) = live.first() {
                        self.error = Some(leak_error(name, ob, body.span));
                    }
                }
                self.scope_stack = saved_scopes;
                for (name, inst) in saved_insts {
                    match inst {
                        Some(i) => {
                            self.var_instances.insert(name, i);
                        }
                        None => {
                            self.var_instances.remove(&name);
                        }
                    }
                }
                self.forbidden_captures = saved_forbidden;
                self.obligations = saved_obligations;
                self.consumed = saved_consumed;
            }
            Expr::Spawn { call, .. } => {
                self.visit_expr(call);
            }
            Expr::Propagate { expr: inner } => {
                self.visit_expr(inner);
            }
            Expr::Range { start, end, inclusive: _ } => {
                self.visit_expr(start);
                self.check_discarded_temp(start);
                self.visit_expr(end);
                self.check_discarded_temp(end);
            }
            Expr::NullCoalesce { lhs, rhs } => {
                if let Some(ob) = self.value_obligation(lhs) {
                    self.deny_move(
                        &ob,
                        "the '??' operator — null-coalescing aliases the value without \
                         consuming it",
                        lhs.span,
                    );
                    return;
                }
                if let Some(ob) = self.value_obligation(rhs) {
                    self.deny_move(
                        &ob,
                        "the '??' operator — the fallback value's release obligation \
                         would be untracked",
                        rhs.span,
                    );
                    return;
                }
                self.visit_expr(lhs);
                self.visit_expr(rhs);
            }
            Expr::NullPropagate { expr: inner } => {
                if let Some(ob) = self.value_obligation(inner) {
                    self.deny_move(
                        &ob,
                        "the '?' operator — it aliases the must_release value without \
                         consuming it; the binding is already narrowed, use it directly",
                        inner.span,
                    );
                    return;
                }
                self.visit_expr(inner);
            }
            Expr::StaticTraitCall { args, method_name, .. } => {
                for a in args {
                    let verdict = DestVerdict::Deny(format!(
                        "the static trait call '{}' — a callee whose parameter types \
                         this analysis cannot see",
                        method_name.node
                    ));
                    self.move_arg(a, &format!("passed to '{}'", method_name.node), verdict);
                }
            }
            Expr::At { domain, method, args } => {
                self.visit_expr(domain);
                for a in args {
                    let verdict = DestVerdict::Deny(
                        "an `at` placement boundary — a release obligation cannot cross \
                         an execution domain"
                            .to_string(),
                    );
                    self.move_arg(
                        a,
                        &format!("passed to '.{}()' at a domain", method.node),
                        verdict,
                    );
                }
            }
            Expr::If { condition, then_block, else_block } => {
                // If-EXPRESSION: each arm's value is yielded into the
                // expression's result — yielding a must-release binding moves
                // it (consume-once when every arm yields the same binding),
                // and the arms must agree on what they discharged (#408).
                self.visit_expr(condition);
                let entry = self.snapshot();
                let r1 = self.branch_inner(
                    &entry,
                    &then_block.node,
                    false,
                    Some("yielded from an if-expression"),
                );
                let r2 = self.branch_inner(
                    &entry,
                    &else_block.node,
                    false,
                    Some("yielded from an if-expression"),
                );
                self.join_value_arms(&entry, vec![r1, r2], "if-expression", expr.span);
            }
            Expr::Match { expr: scrutinee, arms } => {
                self.visit_expr(scrutinee);
                let entry = self.snapshot();
                let mut results: Vec<BranchResult> = Vec::new();
                for arm in arms {
                    if self.error.is_some() {
                        return;
                    }
                    let Some(saved) = self.enter_pattern(&arm.pattern) else { return };
                    let saved_consumed =
                        std::mem::replace(&mut self.consumed, entry.consumed.clone());
                    let saved_obligations =
                        std::mem::replace(&mut self.obligations, entry.obligations.clone());
                    for n in Self::pattern_names(&arm.pattern) {
                        self.consumed.remove(&n.node);
                    }
                    self.move_expr(&arm.value, "yielded from a match expression");
                    let mut state = FlowState {
                        consumed: std::mem::replace(&mut self.consumed, saved_consumed),
                        obligations: std::mem::replace(
                            &mut self.obligations,
                            saved_obligations,
                        ),
                    };
                    self.exit_pattern(saved);
                    Self::reset_pattern_consumed(&entry, &mut state, &arm.pattern);
                    results.push(BranchResult { state, terminated: false });
                }
                self.join_value_arms(&entry, results, "match expression", expr.span);
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
                    exits: self.exits_for(&p.base, &p.args, &e.param, &e.state),
                    acquired: crate::span::Span::new(0, 0),
                });
            }
        }
        None
    }
}

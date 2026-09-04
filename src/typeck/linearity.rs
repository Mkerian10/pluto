//! Transition linearity for typestates (docs/design/rfc-typestates.md, phase 2).
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
//! Scope and rules (phase 2):
//! - Only simple local receivers (`u.acquire()`) consume; transitions through
//!   fields or temporaries are outside the analysis.
//! - Reassignment (`u = ...`) or a fresh `let u = ...` revives the binding.
//! - Joins are conservative: consumed on ANY branch means consumed after the
//!   join. Loop bodies are analyzed twice so a transition in iteration one is
//!   caught as a use-after-consume in iteration two.
//! - Closure and spawn bodies are checked against a snapshot of the current
//!   consumed set (capturing a consumed value is a use); their own effects
//!   don't escape the closure (captures are by-value snapshots).
//!
//! Runs after body checking (method resolutions populated) and before
//! `sweep_skolems`: generic templates record their resolutions under
//! skolem-instance names, which this pass reconstructs.

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
    if transitions.is_empty() {
        return Ok(()); // no typestate classes in this program
    }
    for func in &program.functions {
        // Concrete functions and generic templates both record resolutions
        // under the function's own name.
        check_body(&func.node.name.node, &func.node.body.node, env, &transitions)?;
    }
    for class in &program.classes {
        if class.node.type_params.is_empty() {
            for m in &class.node.methods {
                let key = mangle_method(&class.node.name.node, &m.node.name.node);
                check_body(&key, &m.node.body.node, env, &transitions)?;
            }
        } else {
            // Generic class template bodies were checked against skolem (or,
            // for `where`-constrained methods, state-bound) instantiations —
            // reconstruct the same instance name to find their resolutions.
            for m in &class.node.methods {
                let key = template_method_key(&class.node, &m.node.name.node, env);
                check_body(&key, &m.node.body.node, env, &transitions)?;
            }
        }
    }
    if let Some(app) = &program.app {
        for m in &app.node.methods {
            let key = mangle_method(&app.node.name.node, &m.node.name.node);
            check_body(&key, &m.node.body.node, env, &transitions)?;
        }
    }
    for stage in &program.stages {
        for m in &stage.node.methods {
            let key = mangle_method(&stage.node.name.node, &m.node.name.node);
            check_body(&key, &m.node.body.node, env, &transitions)?;
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

fn check_body(
    current_fn: &str,
    block: &Block,
    env: &TypeEnv,
    transitions: &TransitionTable,
) -> Result<(), CompileError> {
    let mut lin = Linearity {
        current_fn,
        env,
        transitions,
        consumed: HashMap::new(),
        error: None,
    };
    lin.visit_block_stmts(block);
    match lin.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// What a binding was consumed by, for the error message.
#[derive(Clone)]
struct ConsumeInfo {
    method: String,
    new_state: String,
}

struct Linearity<'a> {
    current_fn: &'a str,
    env: &'a TypeEnv,
    transitions: &'a TransitionTable,
    consumed: HashMap<String, ConsumeInfo>,
    error: Option<CompileError>,
}

impl Linearity<'_> {
    fn visit_block_stmts(&mut self, block: &Block) {
        for stmt in &block.stmts {
            if self.error.is_some() {
                return;
            }
            self.visit_stmt(stmt);
        }
    }

    /// If this resolved call is a state-changing transition method, the
    /// display name of the state the value moved to.
    fn transition_target(&self, method_span_start: usize) -> Option<String> {
        let key = (self.current_fn.to_string(), method_span_start);
        let MethodResolution::Class { mangled_name } = self.env.method_resolutions.get(&key)?
        else {
            return None;
        };
        // mangled = "<class-instance>$<method>"; method names contain no '$'.
        let (class_inst, method) = mangled_name.rsplit_once('$')?;
        let class_inst = class_inst.trim_end_matches('$');
        let base = class_inst.split("$$").next()?;
        if !self.transitions.get(base).is_some_and(|ms| ms.contains(method)) {
            return None;
        }
        let new_state = match self.env.functions.get(mangled_name).map(|s| &s.return_type) {
            Some(PlutoType::Class(ret)) => display_instance(ret),
            _ => "its new state".to_string(),
        };
        Some(new_state)
    }

    fn use_var(&mut self, name: &str, span: crate::span::Span) {
        if self.error.is_some() {
            return;
        }
        if let Some(info) = self.consumed.get(name) {
            self.error = Some(CompileError::type_err(
                format!(
                    "'{name}' was consumed by the transition '{}' (it is now {}); \
                     use the transition's result, or rebind '{name}'",
                    info.method, info.new_state
                ),
                span,
            ));
        }
    }

    /// Visit a branch against a snapshot; returns the branch's consumed set.
    fn branch(&mut self, entry: &HashMap<String, ConsumeInfo>, block: &Block) -> HashMap<String, ConsumeInfo> {
        let saved = std::mem::replace(&mut self.consumed, entry.clone());
        self.visit_block_stmts(block);
        std::mem::replace(&mut self.consumed, saved)
    }

    fn union_into(&mut self, other: HashMap<String, ConsumeInfo>) {
        for (k, v) in other {
            self.consumed.entry(k).or_insert(v);
        }
    }
}

/// `Partition$$Owned` → `Partition<Owned>` for error messages.
fn display_instance(mangled: &str) -> String {
    match mangled.split_once("$$") {
        Some((base, args)) => format!("{base}<{}>", args.replace("$$", ", ")),
        None => mangled.to_string(),
    }
}

impl Visitor for Linearity<'_> {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        if self.error.is_some() {
            return;
        }
        match &stmt.node {
            Stmt::Let { name, value, .. } => {
                self.visit_expr(value);
                // A fresh binding of this name is usable again.
                self.consumed.remove(&name.node);
            }
            Stmt::Assign { target, value } => {
                self.visit_expr(value);
                self.consumed.remove(&target.node);
            }
            Stmt::If { condition, then_block, else_block } => {
                self.visit_expr(condition);
                let entry = self.consumed.clone();
                let after_then = self.branch(&entry, &then_block.node);
                let after_else = match else_block {
                    Some(eb) => self.branch(&entry, &eb.node),
                    None => entry,
                };
                // Conservative join: consumed on any path stays consumed.
                self.union_into(after_then);
                self.union_into(after_else);
            }
            Stmt::While { condition, body } => {
                self.visit_expr(condition);
                let entry = self.consumed.clone();
                let after_one = self.branch(&entry, &body.node);
                // Second pass from the post-body set catches transitions that
                // consume a loop-external binding on iteration one and use it
                // on iteration two.
                let mut seed = entry;
                for (k, v) in &after_one {
                    seed.entry(k.clone()).or_insert_with(|| v.clone());
                }
                let after_two = self.branch(&seed, &body.node);
                self.union_into(after_one);
                self.union_into(after_two);
            }
            Stmt::For { var, iterable, body } => {
                self.visit_expr(iterable);
                let mut entry = self.consumed.clone();
                entry.remove(&var.node);
                let after_one = self.branch(&entry, &body.node);
                let mut seed = entry;
                for (k, v) in &after_one {
                    seed.entry(k.clone()).or_insert_with(|| v.clone());
                }
                let after_two = self.branch(&seed, &body.node);
                self.union_into(after_one);
                self.union_into(after_two);
            }
            Stmt::Match { expr, arms } => {
                self.visit_expr(expr);
                let entry = self.consumed.clone();
                let mut joined: Vec<HashMap<String, ConsumeInfo>> = Vec::new();
                for arm in arms {
                    joined.push(self.branch(&entry, &arm.body.node));
                }
                for j in joined {
                    self.union_into(j);
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
                    self.visit_expr(a);
                }
                if let Expr::Ident(recv) = &object.node
                    && let Some(new_state) = self.transition_target(method.span.start)
                {
                    self.consumed.insert(
                        recv.clone(),
                        ConsumeInfo {
                            method: format!(".{}()", method.node),
                            new_state,
                        },
                    );
                }
            }
            Expr::Closure { body, params, .. } => {
                // Captures are by-value snapshots: uses of consumed outer
                // bindings inside the closure are errors (the capture reads a
                // moved value), but the closure's own transitions don't
                // escape to the enclosing scope.
                let mut entry = self.consumed.clone();
                for p in params {
                    entry.remove(&p.name.node);
                }
                let _ = self.branch(&entry, &body.node);
            }
            Expr::Spawn { call } => {
                self.visit_expr(call);
            }
            Expr::Catch { expr: inner, handlers } => {
                self.visit_expr(inner);
                let entry = self.consumed.clone();
                let mut joined: Vec<HashMap<String, ConsumeInfo>> = Vec::new();
                for handler in handlers {
                    match handler {
                        CatchHandler::Wildcard { var, body } => {
                            let mut e = entry.clone();
                            e.remove(&var.node);
                            joined.push(self.branch(&e, &body.node));
                        }
                        CatchHandler::Typed { var, body, .. } => {
                            let mut e = entry.clone();
                            e.remove(&var.node);
                            joined.push(self.branch(&e, &body.node));
                        }
                        CatchHandler::Shorthand(fb) => {
                            self.visit_expr(fb);
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

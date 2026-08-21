use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use crate::visit::{walk_expr, Visitor};
use super::env::{mangle_method, mangle_name, InstKind, MethodResolution, TypeEnv};

pub(crate) fn infer_error_sets(program: &Program, env: &mut TypeEnv) {
    let mut direct_errors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut propagation_edges: HashMap<String, HashSet<String>> = HashMap::new();
    let mut closure_call_sites: HashMap<(String, usize), String> = HashMap::new();

    // Collect effects from top-level functions. Generic templates are included
    // under their unmangled name: their bodies aren't type-checked yet, so
    // method-call edges inside them are missed (no resolutions recorded), but
    // direct raises and named-function edges are type-independent and let call
    // sites of generics be enforced before monomorphization.
    for func in &program.functions {
        let name = func.node.name.node.clone();
        collect_decl_effects(&func.node.body.node, &name, env,
            &mut direct_errors, &mut propagation_edges, &mut closure_call_sites);
    }

    // Collect effects from class methods (generic templates included, keyed
    // under the template class name; instances get a copy after the fixed point)
    for class in &program.classes {
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let mangled = mangle_method(class_name, &method.node.name.node);
            collect_decl_effects(&method.node.body.node, &mangled, env,
                &mut direct_errors, &mut propagation_edges, &mut closure_call_sites);
        }
    }

    // Collect effects from inherited default trait methods
    for class in &program.classes {
        let class_name = &class.node.name.node;
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if let Some(body) = &tm.body && !class_method_names.contains(&tm.name.node) {
                            let mangled = mangle_method(class_name, &tm.name.node);
                            collect_decl_effects(&body.node, &mangled, env,
                                &mut direct_errors, &mut propagation_edges, &mut closure_call_sites);
                        }
                    }
                }
            }
        }
    }

    // Collect effects from app methods
    if let Some(app_spanned) = &program.app {
        let app_name = &app_spanned.node.name.node;
        for method in &app_spanned.node.methods {
            let mangled = mangle_method(app_name, &method.node.name.node);
            collect_decl_effects(&method.node.body.node, &mangled, env,
                &mut direct_errors, &mut propagation_edges, &mut closure_call_sites);
        }
    }

    // Collect effects from stage methods
    for stage_spanned in &program.stages {
        let stage_name = &stage_spanned.node.name.node;
        for method in &stage_spanned.node.methods {
            let mangled = mangle_method(stage_name, &method.node.name.node);
            collect_decl_effects(&method.node.body.node, &mangled, env,
                &mut direct_errors, &mut propagation_edges, &mut closure_call_sites);
        }
    }

    env.closure_call_sites = closure_call_sites;

    // Fixed-point iteration: propagate error sets through call edges.
    // Start from pre-existing fn_errors (e.g. seeded FFI fallible functions).
    let mut fn_errors: HashMap<String, HashSet<String>> = env.fn_errors.clone();
    for (name, directs) in &direct_errors {
        fn_errors.entry(name.clone()).or_default().extend(directs.iter().cloned());
    }

    loop {
        let mut changed = false;
        for (fn_name, edges) in &propagation_edges {
            let mut new_errors = fn_errors.get(fn_name).cloned().unwrap_or_default();
            for callee in edges {
                if let Some(callee_errors) = fn_errors.get(callee) {
                    for e in callee_errors {
                        if new_errors.insert(e.clone()) {
                            changed = true;
                        }
                    }
                }
            }
            fn_errors.insert(fn_name.clone(), new_errors);
        }
        if !changed {
            break;
        }
    }

    env.fn_errors = fn_errors;

    // Copy template error sets onto the instantiations recorded so far, so
    // method calls on eagerly-instantiated generic classes (whose resolutions
    // use instance-mangled names like `Box$$int$get`) are enforceable.
    // Instantiations discovered later during monomorphize get the same copy in
    // `monomorphize::instantiate_function`/`instantiate_class`.
    let instantiations: Vec<_> = env.instantiations.iter().cloned().collect();
    for inst in instantiations {
        match &inst.kind {
            InstKind::Function(name) => {
                let mangled = mangle_name(name, &inst.type_args);
                copy_error_set(env, name, &mangled);
            }
            InstKind::Class(name) => {
                let mangled = mangle_name(name, &inst.type_args);
                copy_class_method_error_sets(program, env, name, &mangled);
            }
            InstKind::Enum(_) => {}
        }
    }
}

/// Copy the inferred error set of `from` (a generic template key) onto `to`
/// (an instance-mangled key). Union semantics: safe to call repeatedly.
pub(crate) fn copy_error_set(env: &mut TypeEnv, from: &str, to: &str) {
    if let Some(errs) = env.fn_errors.get(from).cloned()
        && !errs.is_empty()
    {
        env.fn_errors.entry(to.to_string()).or_default().extend(errs);
    }
}

/// Copy the error sets of every method of generic class template `class_name`
/// (including inherited default trait methods) onto the instance `mangled`.
pub(crate) fn copy_class_method_error_sets(
    program: &Program,
    env: &mut TypeEnv,
    class_name: &str,
    mangled: &str,
) {
    let Some(class) = program
        .classes
        .iter()
        .find(|c| c.node.name.node == class_name)
    else {
        return;
    };
    for method in &class.node.methods {
        let m = &method.node.name.node;
        let from = mangle_method(class_name, m);
        let to = mangle_method(mangled, m);
        copy_error_set(env, &from, &to);
    }
    let class_method_names: Vec<&str> =
        class.node.methods.iter().map(|m| m.node.name.node.as_str()).collect();
    let mut inherited: Vec<String> = Vec::new();
    for trait_name in &class.node.impl_traits {
        if let Some(trait_info) = env.traits.get(&trait_name.node) {
            inherited.extend(
                trait_info
                    .default_methods
                    .iter()
                    .filter(|m| !class_method_names.contains(&m.as_str()))
                    .cloned(),
            );
        }
    }
    for m in inherited {
        let from = mangle_method(class_name, &m);
        let to = mangle_method(mangled, &m);
        copy_error_set(env, &from, &to);
    }
}

/// The set of error types a catch's inner call can raise (used to validate
/// typed catch). Resolves the callee and reads its inferred error set; a remote
/// call also adds NetworkError. A call through a closure variable reads the
/// closure node recorded during effect collection.
fn inner_error_set(inner: &Expr, current_fn: &str, env: &TypeEnv) -> HashSet<String> {
    match inner {
        Expr::Call { name, .. } => {
            if let Some(node) = env.closure_call_sites.get(&(current_fn.to_string(), name.span.start)) {
                return env.fn_errors.get(node).cloned().unwrap_or_default();
            }
            env.fn_errors.get(&name.node).cloned().unwrap_or_default()
        }
        Expr::MethodCall { method, .. } => {
            let key = (current_fn.to_string(), method.span.start);
            match env.method_resolutions.get(&key) {
                Some(MethodResolution::Class { mangled_name }) =>
                    env.fn_errors.get(mangled_name).cloned().unwrap_or_default(),
                Some(MethodResolution::RemoteClass { mangled_name }) => {
                    let mut s = env.fn_errors.get(mangled_name).cloned().unwrap_or_default();
                    s.insert("NetworkError".to_string());
                    s
                }
                _ => HashSet::new(),
            }
        }
        _ => HashSet::new(),
    }
}

/// Key for a closure literal's node in the error-inference graph, derived from
/// its body span (unique within a program; cannot collide with function names).
fn closure_node_key(span: crate::span::Span) -> String {
    format!("<closure@{}>", span.start)
}

/// Mutable state threaded through effect collection. Effects accrue to
/// `current_node`: the enclosing named function, or — inside a closure literal
/// bound to a variable — that closure's own node, so defining a fallible
/// closure does not by itself make the definer fallible; calling it (with `!`)
/// or letting it escape does.
struct EffectCtx<'a> {
    env: &'a TypeEnv,
    /// Enclosing named function — method resolutions and closure call sites
    /// are keyed under it.
    current_fn: String,
    /// Node currently accruing effects (named function or closure node).
    current_node: String,
    direct: &'a mut HashMap<String, HashSet<String>>,
    edges: &'a mut HashMap<String, HashSet<String>>,
    /// Lexical scopes mapping closure-bound variable names to closure nodes.
    closure_scopes: Vec<HashMap<String, String>>,
    /// (enclosing fn, callee name span start) -> closure node, consumed by
    /// enforcement and typed-catch coverage.
    closure_call_sites: &'a mut HashMap<(String, usize), String>,
}

impl EffectCtx<'_> {
    fn raise(&mut self, err: String) {
        self.direct.entry(self.current_node.clone()).or_default().insert(err);
    }

    fn edge(&mut self, callee: String) {
        self.edges.entry(self.current_node.clone()).or_default().insert(callee);
    }

    fn bind_closure(&mut self, var: &str, node: String) {
        if let Some(scope) = self.closure_scopes.last_mut() {
            scope.insert(var.to_string(), node);
        }
    }

    fn lookup_closure(&self, var: &str) -> Option<&String> {
        self.closure_scopes.iter().rev().find_map(|s| s.get(var))
    }

    fn record_closure_call(&mut self, span_start: usize, node: String) {
        self.closure_call_sites.insert((self.current_fn.clone(), span_start), node);
    }

    /// Collect a closure literal's body under its own fresh node.
    fn collect_closure(&mut self, body: &Spanned<Block>) -> String {
        let node = closure_node_key(body.span);
        self.direct.entry(node.clone()).or_default();
        self.edges.entry(node.clone()).or_default();
        self.collect_closure_into(body, node.clone());
        node
    }

    /// Collect a closure literal's body into an existing node (used to union a
    /// reassigned variable's closures, keeping resolution flow-insensitive).
    fn collect_closure_into(&mut self, body: &Spanned<Block>, node: String) {
        let prev = std::mem::replace(&mut self.current_node, node);
        self.closure_scopes.push(HashMap::new());
        for stmt in &body.node.stmts {
            collect_stmt_effects(&stmt.node, self);
        }
        self.closure_scopes.pop();
        self.current_node = prev;
    }
}

/// Collect direct error raises and propagation edges from a declaration body
/// into the shared per-node maps.
fn collect_decl_effects(
    block: &Block,
    name: &str,
    env: &TypeEnv,
    direct: &mut HashMap<String, HashSet<String>>,
    edges: &mut HashMap<String, HashSet<String>>,
    closure_call_sites: &mut HashMap<(String, usize), String>,
) {
    direct.entry(name.to_string()).or_default();
    edges.entry(name.to_string()).or_default();
    let mut ctx = EffectCtx {
        env,
        current_fn: name.to_string(),
        current_node: name.to_string(),
        direct,
        edges,
        closure_scopes: vec![HashMap::new()],
        closure_call_sites,
    };
    for stmt in &block.stmts {
        collect_stmt_effects(&stmt.node, &mut ctx);
    }
}

/// Collect a nested block's statements in a fresh closure-binding scope.
fn collect_block_stmts(stmts: &[Spanned<Stmt>], ctx: &mut EffectCtx) {
    ctx.closure_scopes.push(HashMap::new());
    for s in stmts {
        collect_stmt_effects(&s.node, ctx);
    }
    ctx.closure_scopes.pop();
}

fn collect_stmt_effects(stmt: &Stmt, ctx: &mut EffectCtx) {
    match stmt {
        Stmt::Raise { error_name, fields, .. } => {
            ctx.raise(error_name.node.clone());
            for (_, val) in fields {
                collect_expr_effects(&val.node, ctx);
            }
        }
        Stmt::Let { name, value, .. } => {
            // A closure literal bound to a variable gets its own node; calls
            // through the variable resolve to it.
            if let Expr::Closure { body, .. } = &value.node {
                let node = ctx.collect_closure(body);
                ctx.bind_closure(&name.node, node);
            } else {
                collect_expr_effects(&value.node, ctx);
            }
        }
        Stmt::Expr(expr) => {
            collect_expr_effects(&expr.node, ctx);
        }
        Stmt::Return(Some(expr)) => {
            collect_expr_effects(&expr.node, ctx);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { target, value } => {
            if let Expr::Closure { body, .. } = &value.node {
                // Reassignment unions the new closure's effects into the
                // variable's existing node (flow-insensitive but sound for
                // calls that precede the assignment in a loop); a closure
                // assigned to an untracked variable starts tracking it.
                if let Some(node) = ctx.lookup_closure(&target.node).cloned() {
                    ctx.collect_closure_into(body, node);
                } else {
                    let node = ctx.collect_closure(body);
                    ctx.bind_closure(&target.node, node);
                }
            } else {
                collect_expr_effects(&value.node, ctx);
            }
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_expr_effects(&object.node, ctx);
            collect_expr_effects(&value.node, ctx);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_expr_effects(&object.node, ctx);
            collect_expr_effects(&index.node, ctx);
            collect_expr_effects(&value.node, ctx);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_expr_effects(&condition.node, ctx);
            collect_block_stmts(&then_block.node.stmts, ctx);
            if let Some(eb) = else_block {
                collect_block_stmts(&eb.node.stmts, ctx);
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_effects(&condition.node, ctx);
            collect_block_stmts(&body.node.stmts, ctx);
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_effects(&iterable.node, ctx);
            collect_block_stmts(&body.node.stmts, ctx);
        }
        Stmt::Match { expr, arms } => {
            collect_expr_effects(&expr.node, ctx);
            for arm in arms {
                collect_block_stmts(&arm.body.node.stmts, ctx);
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                collect_expr_effects(&cap.node, ctx);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        collect_expr_effects(&channel.node, ctx);
                    }
                    SelectOp::Send { channel, value } => {
                        collect_expr_effects(&channel.node, ctx);
                        collect_expr_effects(&value.node, ctx);
                    }
                }
                collect_block_stmts(&arm.body.node.stmts, ctx);
            }
            if let Some(def) = default {
                collect_block_stmts(&def.node.stmts, ctx);
            }
            // Select without default is implicitly fallible — raises ChannelClosed
            // when all channels are closed
            if default.is_none() {
                ctx.raise("ChannelClosed".to_string());
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                collect_expr_effects(&seed.node, ctx);
            }
            collect_block_stmts(&body.node.stmts, ctx);
        }
        Stmt::Assert { expr } => {
            collect_expr_effects(&expr.node, ctx);
        }
        // The generated serve loop handles dispatched methods' errors internally
        // (replying with an error response), so it adds none to the enclosing fn.
        Stmt::Serve { service, port } => {
            collect_expr_effects(&service.node, ctx);
            collect_expr_effects(&port.node, ctx);
        }
        Stmt::Yield { value, .. } => {
            collect_expr_effects(&value.node, ctx);
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn collect_expr_effects(expr: &Expr, ctx: &mut EffectCtx) {
    match expr {
        Expr::Propagate { expr: inner } => {
            match &inner.node {
                Expr::Call { name, args, .. } => {
                    if let Some(node) = ctx.lookup_closure(&name.node).cloned() {
                        ctx.record_closure_call(name.span.start, node.clone());
                        ctx.edge(node);
                    } else if name.node == "pow"
                        && ctx
                            .env
                            .fallible_builtin_calls
                            .contains(&(ctx.current_fn.clone(), name.span.start))
                    {
                        ctx.raise("MathError".to_string());
                    } else {
                        ctx.edge(name.node.clone());
                    }
                    for arg in args {
                        collect_expr_effects(&arg.node, ctx);
                    }
                }
                Expr::MethodCall { object, method, args } => {
                    collect_expr_effects(&object.node, ctx);
                    for arg in args {
                        collect_expr_effects(&arg.node, ctx);
                    }
                    let key = (ctx.current_fn.clone(), method.span.start);
                    let resolution = ctx.env.method_resolutions.get(&key).cloned();
                    match resolution {
                        Some(MethodResolution::Class { mangled_name }) => {
                            ctx.edge(mangled_name);
                        }
                        // Remote boundary call: always adds NetworkError (the
                        // transport can fail), and also inherits the interface
                        // method's declared error set so those typed errors —
                        // which the server propagates over the wire — can be
                        // handled with `catch` on the caller side.
                        Some(MethodResolution::RemoteClass { mangled_name }) => {
                            ctx.raise("NetworkError".to_string());
                            ctx.edge(mangled_name);
                        }
                        Some(MethodResolution::TraitDynamic { trait_name, method_name }) => {
                            let impls: Vec<String> = ctx
                                .env
                                .classes
                                .iter()
                                .filter(|(_, info)| info.impl_traits.iter().any(|t| *t == trait_name))
                                .map(|(class_name, _)| mangle_method(class_name, &method_name))
                                .collect();
                            for m in impls {
                                ctx.edge(m);
                            }
                        }
                        Some(MethodResolution::TaskGet { spawned_fn }) => {
                            match spawned_fn {
                                Some(fn_name) => {
                                    ctx.edge(fn_name);
                                }
                                None => {
                                    // Unknown origin — conservatively add all declared error types
                                    let all: Vec<String> = ctx.env.errors.keys().cloned().collect();
                                    for err_name in all {
                                        ctx.raise(err_name);
                                    }
                                }
                            }
                        }
                        Some(MethodResolution::ChannelSend) => {
                            ctx.raise("ChannelClosed".to_string());
                        }
                        Some(MethodResolution::ChannelRecv) => {
                            ctx.raise("ChannelClosed".to_string());
                        }
                        Some(MethodResolution::ChannelTrySend) => {
                            ctx.raise("ChannelClosed".to_string());
                            ctx.raise("ChannelFull".to_string());
                        }
                        Some(MethodResolution::ChannelTryRecv) => {
                            ctx.raise("ChannelClosed".to_string());
                            ctx.raise("ChannelEmpty".to_string());
                        }
                        Some(MethodResolution::TaskDetach) => {}
                        Some(MethodResolution::TaskCancel) => {}
                        Some(MethodResolution::Builtin) => {}
                        None => {}
                    }
                }
                _ => collect_expr_effects(&inner.node, ctx),
            }
        }
        Expr::Catch { expr: inner, handlers } => {
            match &inner.node {
                Expr::Call { name, args, .. } => {
                    if let Some(node) = ctx.lookup_closure(&name.node).cloned() {
                        ctx.record_closure_call(name.span.start, node);
                    }
                    for arg in args {
                        collect_expr_effects(&arg.node, ctx);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    collect_expr_effects(&object.node, ctx);
                    for arg in args {
                        collect_expr_effects(&arg.node, ctx);
                    }
                }
                _ => collect_expr_effects(&inner.node, ctx),
            }
            for handler in handlers {
                match handler {
                    CatchHandler::Wildcard { body, .. } | CatchHandler::Typed { body, .. } => {
                        collect_block_stmts(&body.node.stmts, ctx);
                    }
                    CatchHandler::Shorthand(fb) => {
                        collect_expr_effects(&fb.node, ctx);
                    }
                }
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr_effects(&lhs.node, ctx);
            collect_expr_effects(&rhs.node, ctx);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_expr_effects(&operand.node, ctx);
        }
        Expr::Cast { expr: inner, .. } => {
            collect_expr_effects(&inner.node, ctx);
        }
        Expr::Call { name, args, .. } => {
            // A plain (unhandled) call adds no edge — enforcement requires
            // fallible calls to be handled — but a call through a closure
            // variable records its resolution for enforcement to consult.
            if let Some(node) = ctx.lookup_closure(&name.node).cloned() {
                ctx.record_closure_call(name.span.start, node);
            }
            for arg in args {
                collect_expr_effects(&arg.node, ctx);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_expr_effects(&object.node, ctx);
            for arg in args {
                collect_expr_effects(&arg.node, ctx);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, ctx);
            }
        }
        Expr::FieldAccess { object, .. } => {
            collect_expr_effects(&object.node, ctx);
        }
        Expr::ArrayLit { elements } => {
            for e in elements {
                collect_expr_effects(&e.node, ctx);
            }
        }
        Expr::Index { object, index } => {
            collect_expr_effects(&object.node, ctx);
            collect_expr_effects(&index.node, ctx);
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, ctx);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_expr_effects(&e.node, ctx);
                }
            }
        }
        Expr::Closure { body, .. } => {
            // A closure literal in a non-binding position (argument, return
            // value, field...) escapes by construction: collect its body under
            // its own node and conservatively absorb that node's errors into
            // the enclosing node — someone may call it where we can't see.
            let node = ctx.collect_closure(body);
            ctx.edge(node);
        }
        Expr::Ident(name) => {
            // A closure variable referenced outside call position escapes the
            // local analysis (passed as argument, returned, stored). Absorb
            // its error set into the enclosing node, conservatively.
            if let Some(node) = ctx.lookup_closure(name).cloned() {
                ctx.edge(node);
            }
        }
        Expr::Spawn { call } => {
            // Spawn is opaque to the error system — do NOT recurse into the closure body.
            // Only collect effects from spawn arg expressions (inside the closure's inner Call/MethodCall).
            if let Expr::Closure { body, .. } = &call.node {
                for stmt in &body.node.stmts {
                    if let Stmt::Return(Some(ret_expr)) = &stmt.node {
                        let args = match &ret_expr.node {
                            Expr::Call { args, .. } => Some(args),
                            Expr::MethodCall { args, .. } => Some(args),
                            _ => None,
                        };
                        if let Some(args) = args {
                            for arg in args {
                                collect_expr_effects(&arg.node, ctx);
                            }
                        }
                    }
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_expr_effects(&k.node, ctx);
                collect_expr_effects(&v.node, ctx);
            }
        }
        Expr::SetLit { elements, .. } => {
            for e in elements {
                collect_expr_effects(&e.node, ctx);
            }
        }
        Expr::Range { start, end, .. } => {
            collect_expr_effects(&start.node, ctx);
            collect_expr_effects(&end.node, ctx);
        }
        Expr::NullPropagate { expr: inner } => {
            collect_expr_effects(&inner.node, ctx);
        }
        Expr::StaticTraitCall { args, .. } => {
            for arg in args {
                collect_expr_effects(&arg.node, ctx);
            }
        }
        Expr::If { condition, then_block, else_block } => {
            collect_expr_effects(&condition.node, ctx);
            collect_block_stmts(&then_block.node.stmts, ctx);
            collect_block_stmts(&else_block.node.stmts, ctx);
        }
        Expr::Match { expr, arms } => {
            collect_expr_effects(&expr.node, ctx);
            for arm in arms {
                collect_expr_effects(&arm.value.node, ctx);
            }
        }
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before error analysis. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } | Expr::NoneLit => {}
    }
}

// ── Phase 2c: Error handling enforcement ──────────────────────────────────────

/// Enforce error handling at call sites. Generic template bodies are enforced
/// in `lenient` mode: their bodies were never type-checked (no method
/// resolutions exist), so unresolved method calls are skipped and `catch`/`!`
/// are never rejected as applied-to-infallible — but unhandled calls to known
/// fallible named functions are still errors, closing the soundness hole where
/// a generic body could silently leak an error past the checker.
pub(crate) fn enforce_error_handling(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    for func in &program.functions {
        let lenient = !func.node.type_params.is_empty();
        let current_fn = func.node.name.node.clone();
        enforce_block(&func.node.body.node, &current_fn, env, lenient)?;
    }
    for class in &program.classes {
        let lenient = !class.node.type_params.is_empty();
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let current_fn = mangle_method(class_name, &method.node.name.node);
            enforce_block(&method.node.body.node, &current_fn, env, lenient)?;
        }
    }
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if let Some(body) = &tm.body && !class_method_names.contains(&tm.name.node) {
                            let current_fn = mangle_method(class_name, &tm.name.node);
                            enforce_block(&body.node, &current_fn, env, false)?;
                        }
                    }
                }
            }
        }
    }
    if let Some(app_spanned) = &program.app {
        let app_name = &app_spanned.node.name.node;
        for method in &app_spanned.node.methods {
            let current_fn = mangle_method(app_name, &method.node.name.node);
            enforce_block(&method.node.body.node, &current_fn, env, false)?;
        }
    }
    // Enforce error handling in stage methods
    for stage_spanned in &program.stages {
        let stage_name = &stage_spanned.node.name.node;
        for method in &stage_spanned.node.methods {
            let current_fn = mangle_method(stage_name, &method.node.name.node);
            enforce_block(&method.node.body.node, &current_fn, env, false)?;
        }
    }
    Ok(())
}

fn enforce_block(
    block: &Block,
    current_fn: &str,
    env: &TypeEnv,
    lenient: bool,
) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        enforce_stmt(&stmt.node, stmt.span, current_fn, env, lenient)?;
    }
    Ok(())
}

fn enforce_stmt(
    stmt: &Stmt,
    _span: crate::span::Span,
    current_fn: &str,
    env: &TypeEnv,
    lenient: bool,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { value, .. } => enforce_expr(&value.node, value.span, current_fn, env, lenient),
        Stmt::Expr(expr) => enforce_expr(&expr.node, expr.span, current_fn, env, lenient),
        Stmt::Return(Some(expr)) => enforce_expr(&expr.node, expr.span, current_fn, env, lenient),
        Stmt::Return(None) => Ok(()),
        Stmt::Assign { value, .. } => enforce_expr(&value.node, value.span, current_fn, env, lenient),
        Stmt::FieldAssign { object, value, .. } => {
            enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
            enforce_expr(&value.node, value.span, current_fn, env, lenient)
        }
        Stmt::IndexAssign { object, index, value } => {
            enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
            enforce_expr(&index.node, index.span, current_fn, env, lenient)?;
            enforce_expr(&value.node, value.span, current_fn, env, lenient)
        }
        Stmt::If { condition, then_block, else_block } => {
            enforce_expr(&condition.node, condition.span, current_fn, env, lenient)?;
            enforce_block(&then_block.node, current_fn, env, lenient)?;
            if let Some(eb) = else_block {
                enforce_block(&eb.node, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            enforce_expr(&condition.node, condition.span, current_fn, env, lenient)?;
            enforce_block(&body.node, current_fn, env, lenient)
        }
        Stmt::For { iterable, body, .. } => {
            enforce_expr(&iterable.node, iterable.span, current_fn, env, lenient)?;
            enforce_block(&body.node, current_fn, env, lenient)
        }
        Stmt::Match { expr, arms } => {
            enforce_expr(&expr.node, expr.span, current_fn, env, lenient)?;
            for arm in arms {
                enforce_block(&arm.body.node, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                enforce_expr(&cap.node, cap.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        enforce_expr(&channel.node, channel.span, current_fn, env, lenient)?;
                    }
                    SelectOp::Send { channel, value } => {
                        enforce_expr(&channel.node, channel.span, current_fn, env, lenient)?;
                        enforce_expr(&value.node, value.span, current_fn, env, lenient)?;
                    }
                }
                enforce_block(&arm.body.node, current_fn, env, lenient)?;
            }
            if let Some(def) = default {
                enforce_block(&def.node, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                enforce_expr(&seed.node, seed.span, current_fn, env, lenient)?;
            }
            enforce_block(&body.node, current_fn, env, lenient)?;
            Ok(())
        }
        Stmt::Assert { expr } => {
            enforce_expr(&expr.node, expr.span, current_fn, env, lenient)?;
            Ok(())
        }
        Stmt::Serve { service, port } => {
            enforce_expr(&service.node, service.span, current_fn, env, lenient)?;
            enforce_expr(&port.node, port.span, current_fn, env, lenient)?;
            Ok(())
        }
        Stmt::Yield { value, .. } => {
            enforce_expr(&value.node, value.span, current_fn, env, lenient)?;
            Ok(())
        }
        Stmt::Break | Stmt::Continue => Ok(()),
    }
}

fn enforce_expr(
    expr: &Expr,
    span: crate::span::Span,
    current_fn: &str,
    env: &TypeEnv,
    lenient: bool,
) -> Result<(), CompileError> {
    match expr {
        Expr::Call { name, args, .. } => {
            for arg in args {
                enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
            }
            if let Some(node) = env.closure_call_sites.get(&(current_fn.to_string(), name.span.start)) {
                if env.is_fn_fallible(node) {
                    return Err(CompileError::type_err(
                        format!(
                            "call to fallible closure '{}' must be handled with ! or catch",
                            name.node
                        ),
                        span,
                    ));
                }
                return Ok(());
            }
            let is_fallible_pow = name.node == "pow"
                && env
                    .fallible_builtin_calls
                    .contains(&(current_fn.to_string(), name.span.start));
            if is_fallible_pow || env.is_fn_fallible(&name.node) {
                return Err(CompileError::type_err(
                    format!(
                        "call to fallible function '{}' must be handled with ! or catch",
                        name.node
                    ),
                    span,
                ));
            }
            Ok(())
        }
        Expr::MethodCall { object, method, args } => {
            enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
            for arg in args {
                enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
            }
            let is_fallible = match env.resolve_method_fallibility(current_fn, method.span.start) {
                Ok(f) => f,
                // Lenient (generic template) bodies were never type-checked,
                // so their method calls have no recorded resolution — skip.
                Err(_) if lenient => false,
                Err(msg) => return Err(CompileError::type_err(msg, method.span)),
            };
            if is_fallible {
                return Err(CompileError::type_err(
                    format!("call to fallible method '{}' must be handled with ! or catch", method.node),
                    span,
                ));
            }
            Ok(())
        }
        Expr::Propagate { expr: inner } => match &inner.node {
            Expr::Call { name, args, .. } => {
                for arg in args {
                    enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
                }
                let is_fallible = match env.closure_call_sites.get(&(current_fn.to_string(), name.span.start)) {
                    Some(node) => env.is_fn_fallible(node),
                    None => {
                        (name.node == "pow"
                            && env
                                .fallible_builtin_calls
                                .contains(&(current_fn.to_string(), name.span.start)))
                            || env.is_fn_fallible(&name.node)
                    }
                };
                if !lenient && !is_fallible {
                    return Err(CompileError::type_err(
                        format!("'!' applied to infallible function '{}'", name.node),
                        span,
                    ));
                }
                Ok(())
            }
            Expr::MethodCall { object, method, args } => {
                enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
                for arg in args {
                    enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
                }
                if !lenient {
                    let is_fallible = env.resolve_method_fallibility(current_fn, method.span.start)
                        .map_err(|msg| CompileError::type_err(msg, method.span))?;
                    if !is_fallible {
                        return Err(CompileError::type_err(
                            format!("'!' applied to infallible method '{}'", method.node),
                            span,
                        ));
                    }
                }
                Ok(())
            }
            _ => Err(CompileError::type_err(
                "! can only be applied to function calls",
                inner.span,
            )),
        },
        Expr::Catch { expr: inner, handlers } => {
            match &inner.node {
                Expr::Call { name, args, .. } => {
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
                    }
                    let is_fallible = match env.closure_call_sites.get(&(current_fn.to_string(), name.span.start)) {
                        Some(node) => env.is_fn_fallible(node),
                        None => {
                            (name.node == "pow"
                                && env
                                    .fallible_builtin_calls
                                    .contains(&(current_fn.to_string(), name.span.start)))
                                || env.is_fn_fallible(&name.node)
                        }
                    };
                    if !lenient && !is_fallible {
                        return Err(CompileError::type_err(
                            format!("catch applied to infallible function '{}'", name.node),
                            span,
                        ));
                    }
                }
                Expr::MethodCall { object, method, args } => {
                    enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
                    }
                    if !lenient {
                        let is_fallible = env.resolve_method_fallibility(current_fn, method.span.start)
                            .map_err(|msg| CompileError::type_err(msg, method.span))?;
                        if !is_fallible {
                            return Err(CompileError::type_err(
                                format!("catch applied to infallible method '{}'", method.node),
                                span,
                            ));
                        }
                    }
                }
                _ => {
                    return Err(CompileError::type_err(
                        "catch can only be applied to function calls",
                        inner.span,
                    ));
                }
            }
            // Coverage: the handlers must collectively handle every error the
            // call can raise — otherwise an un-caught error would escape
            // inference. A wildcard/shorthand handler is a catch-all; otherwise
            // the union of typed error types must cover the call's error set.
            let has_catch_all = handlers.iter().any(|h|
                matches!(h, CatchHandler::Wildcard { .. } | CatchHandler::Shorthand(_)));
            if !has_catch_all {
                let inner_errors = inner_error_set(&inner.node, current_fn, env);
                let handled: HashSet<&str> = handlers.iter().filter_map(|h| match h {
                    CatchHandler::Typed { error_type, .. } =>
                        Some(error_type.node.rsplit('.').next().unwrap_or(&error_type.node)),
                    _ => None,
                }).collect();
                for e in &inner_errors {
                    let un = e.rsplit('.').next().unwrap_or(e);
                    if !handled.contains(un) {
                        return Err(CompileError::type_err(
                            format!("the call can raise '{e}', which no catch handler covers; \
                                     add `catch err: {un} {{ ... }}` or a wildcard `catch err {{ ... }}`"),
                            span,
                        ));
                    }
                }
            }
            for handler in handlers {
                match handler {
                    CatchHandler::Wildcard { body, .. } | CatchHandler::Typed { body, .. } =>
                        enforce_block(&body.node, current_fn, env, lenient)?,
                    CatchHandler::Shorthand(fb) =>
                        enforce_expr(&fb.node, fb.span, current_fn, env, lenient)?,
                }
            }
            Ok(())
        }
        Expr::BinOp { lhs, rhs, .. } => {
            enforce_expr(&lhs.node, lhs.span, current_fn, env, lenient)?;
            enforce_expr(&rhs.node, rhs.span, current_fn, env, lenient)
        }
        Expr::UnaryOp { operand, .. } => enforce_expr(&operand.node, operand.span, current_fn, env, lenient),
        Expr::Cast { expr: inner, .. } => enforce_expr(&inner.node, inner.span, current_fn, env, lenient),
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::FieldAccess { object, .. } => enforce_expr(&object.node, object.span, current_fn, env, lenient),
        Expr::ArrayLit { elements } => {
            for e in elements {
                enforce_expr(&e.node, e.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            enforce_expr(&object.node, object.span, current_fn, env, lenient)?;
            enforce_expr(&index.node, index.span, current_fn, env, lenient)
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    enforce_expr(&e.node, e.span, current_fn, env, lenient)?;
                }
            }
            Ok(())
        }
        Expr::Closure { body, .. } => {
            enforce_block(&body.node, current_fn, env, lenient)
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                enforce_expr(&k.node, k.span, current_fn, env, lenient)?;
                enforce_expr(&v.node, v.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::SetLit { elements, .. } => {
            for e in elements {
                enforce_expr(&e.node, e.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::Range { start, end, .. } => {
            enforce_expr(&start.node, start.span, current_fn, env, lenient)?;
            enforce_expr(&end.node, end.span, current_fn, env, lenient)
        }
        Expr::Spawn { call } => {
            // Enforce spawn arg expressions + reject Propagate in args.
            // Do NOT enforce the inner call itself or the closure body as a whole.
            if let Expr::Closure { body, .. } = &call.node {
                for stmt in &body.node.stmts {
                    if let Stmt::Return(Some(ret_expr)) = &stmt.node {
                        let args = match &ret_expr.node {
                            Expr::Call { args, .. } => Some(args),
                            Expr::MethodCall { args, .. } => Some(args),
                            _ => None,
                        };
                        if let Some(args) = args {
                            for arg in args {
                                enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
                                if contains_propagate(arg) {
                                    return Err(CompileError::type_err(
                                        "error propagation (!) is not allowed in spawn arguments; evaluate before spawn",
                                        arg.span,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        Expr::NullPropagate { expr: inner } => {
            enforce_expr(&inner.node, inner.span, current_fn, env, lenient)
        }
        Expr::StaticTraitCall { args, .. } => {
            for arg in args {
                enforce_expr(&arg.node, arg.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::If { condition, then_block, else_block } => {
            enforce_expr(&condition.node, condition.span, current_fn, env, lenient)?;
            for stmt in &then_block.node.stmts {
                enforce_stmt(&stmt.node, stmt.span, current_fn, env, lenient)?;
            }
            for stmt in &else_block.node.stmts {
                enforce_stmt(&stmt.node, stmt.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::Match { expr, arms } => {
            enforce_expr(&expr.node, expr.span, current_fn, env, lenient)?;
            for arm in arms {
                enforce_expr(&arm.value.node, arm.value.span, current_fn, env, lenient)?;
            }
            Ok(())
        }
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before error analysis. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } | Expr::NoneLit => Ok(()),
    }
}

/// Visitor that detects Expr::Propagate nodes in an expression tree.
struct PropagateDetector {
    found: bool,
}

impl Visitor for PropagateDetector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if matches!(expr.node, Expr::Propagate { .. }) {
            self.found = true;
            // No need to recurse once found (optimization)
            return;
        }
        walk_expr(self, expr);
    }
}

/// Check if an expression tree contains any Expr::Propagate node.
fn contains_propagate(expr: &Spanned<Expr>) -> bool {
    let mut detector = PropagateDetector { found: false };
    detector.visit_expr(expr);
    detector.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    fn sp<T>(node: T) -> Spanned<T> {
        Spanned::new(node, Span::dummy())
    }

    // ===== contains_propagate tests =====

    #[test]
    fn test_contains_propagate_simple_propagate() {
        let expr = sp(Expr::Propagate {
            expr: Box::new(sp(Expr::IntLit(42))),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_no_propagate() {
        let expr = sp(Expr::IntLit(42));
        assert!(!contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_binop_lhs() {
        let expr = sp(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("foo".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            rhs: Box::new(sp(Expr::IntLit(1))),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_binop_rhs() {
        let expr = sp(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(sp(Expr::IntLit(1))),
            rhs: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("bar".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_nested_in_array() {
        let expr = sp(Expr::ArrayLit {
            elements: vec![
                sp(Expr::IntLit(1)),
                sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_value".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_call_args() {
        let expr = sp(Expr::Call {
            name: sp("foo".to_string()),
            args: vec![
                sp(Expr::IntLit(1)),
                sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("bar".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
            ],
            type_args: vec![],
            target_id: None,
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_unary_op() {
        let expr = sp(Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_num".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_field_access_object() {
        let expr = sp(Expr::FieldAccess {
            object: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_obj".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            field: sp("value".to_string()),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_index_object() {
        let expr = sp(Expr::Index {
            object: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_array".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            index: Box::new(sp(Expr::IntLit(0))),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_index_index() {
        let expr = sp(Expr::Index {
            object: Box::new(sp(Expr::Ident("arr".to_string()))),
            index: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_index".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_struct_lit_field() {
        let expr = sp(Expr::StructLit {
            name: sp("Point".to_string()),
            type_args: vec![],
            fields: vec![
                (sp("x".to_string()), sp(Expr::IntLit(1))),
                (sp("y".to_string()), sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_y".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                })),
            ],
            target_id: None,
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_map_key() {
        let expr = sp(Expr::MapLit {
            key_type: sp(TypeExpr::Named("int".to_string())),
            value_type: sp(TypeExpr::Named("int".to_string())),
            entries: vec![
                (sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_key".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }), sp(Expr::IntLit(42))),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_map_value() {
        let expr = sp(Expr::MapLit {
            key_type: sp(TypeExpr::Named("int".to_string())),
            value_type: sp(TypeExpr::Named("int".to_string())),
            entries: vec![
                (sp(Expr::IntLit(1)), sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_value".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                })),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_set_element() {
        let expr = sp(Expr::SetLit {
            elem_type: sp(TypeExpr::Named("int".to_string())),
            elements: vec![
                sp(Expr::IntLit(1)),
                sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_elem".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_range_start() {
        let expr = sp(Expr::Range {
            start: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_start".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            end: Box::new(sp(Expr::IntLit(10))),
            inclusive: false,
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_range_end() {
        let expr = sp(Expr::Range {
            start: Box::new(sp(Expr::IntLit(0))),
            end: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_end".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            inclusive: false,
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_cast() {
        let expr = sp(Expr::Cast {
            expr: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_num".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            target_type: sp(TypeExpr::Named("float".to_string())),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_in_null_propagate() {
        let expr = sp(Expr::NullPropagate {
            expr: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_value".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_literals_false() {
        // Test that all literal types return false
        assert!(!contains_propagate(&sp(Expr::IntLit(42))));
        assert!(!contains_propagate(&sp(Expr::FloatLit(3.14))));
        assert!(!contains_propagate(&sp(Expr::BoolLit(true))));
        assert!(!contains_propagate(&sp(Expr::StringLit("hello".to_string()))));
        assert!(!contains_propagate(&sp(Expr::NoneLit)));
        assert!(!contains_propagate(&sp(Expr::Ident("x".to_string()))));
    }

    #[test]
    fn test_contains_propagate_complex_nested() {
        // Deeply nested: array containing binop with propagate in rhs
        let expr = sp(Expr::ArrayLit {
            elements: vec![
                sp(Expr::BinOp {
                    op: BinOp::Mul,
                    lhs: Box::new(sp(Expr::IntLit(2))),
                    rhs: Box::new(sp(Expr::Propagate {
                        expr: Box::new(sp(Expr::Call {
                            name: sp("compute".to_string()),
                            args: vec![],
                            type_args: vec![],
                            target_id: None,
                        })),
                    })),
                }),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_method_call_object() {
        let expr = sp(Expr::MethodCall {
            object: Box::new(sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_obj".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })),
            method: sp("compute".to_string()),
            args: vec![],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_method_call_args() {
        let expr = sp(Expr::MethodCall {
            object: Box::new(sp(Expr::Ident("obj".to_string()))),
            method: sp("compute".to_string()),
            args: vec![sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_arg".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_enum_data_field() {
        let expr = sp(Expr::EnumData {
            enum_name: sp("Result".to_string()),
            variant: sp("Ok".to_string()),
            type_args: vec![],
            fields: vec![
                (sp("value".to_string()), sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_value".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                })),
            ],
            enum_id: None,
            variant_id: None,
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_string_interp() {
        let expr = sp(Expr::StringInterp {
            parts: vec![
                StringInterpPart::Lit("Value: ".to_string()),
                StringInterpPart::Expr(sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("get_value".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                })),
            ],
        });
        assert!(contains_propagate(&expr));
    }

    #[test]
    fn test_contains_propagate_static_trait_call_args() {
        let expr = sp(Expr::StaticTraitCall {
            trait_name: sp("TypeInfo".to_string()),
            method_name: sp("type_name".to_string()),
            type_args: vec![],
            args: vec![sp(Expr::Propagate {
                expr: Box::new(sp(Expr::Call {
                    name: sp("get_arg".to_string()),
                    args: vec![],
                    type_args: vec![],
                    target_id: None,
                })),
            })],
        });
        assert!(contains_propagate(&expr));
    }

    // ===== PropagateDetector tests =====

    #[test]
    fn test_propagate_detector_stops_after_first_match() {
        // Create an expression with multiple propagate nodes
        // Detector should stop after finding the first one
        let expr = sp(Expr::ArrayLit {
            elements: vec![
                sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("first".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
                sp(Expr::Propagate {
                    expr: Box::new(sp(Expr::Call {
                        name: sp("second".to_string()),
                        args: vec![],
                        type_args: vec![],
                        target_id: None,
                    })),
                }),
            ],
        });

        let mut detector = PropagateDetector { found: false };
        detector.visit_expr(&expr);
        assert!(detector.found);
    }
}

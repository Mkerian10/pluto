use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::visit::{walk_expr, walk_stmt, Visitor};
use super::env::{mangle_method, TypeEnv};
use super::types::PlutoType;
use super::resolve::resolve_type;
use super::infer::infer_expr;
use super::types_compatible;
use crate::parser::ast::Expr;

pub(crate) fn check_function(func: &Function, env: &mut TypeEnv, class_name: Option<&str>) -> Result<(), CompileError> {
    let prev_fn = env.current_fn.take();
    env.current_fn = Some(if let Some(cn) = class_name {
        mangle_method(cn, &func.name.node)
    } else {
        func.name.node.clone()
    });
    let result = check_function_body(func, env, class_name);
    env.current_fn = prev_fn;
    result
}

/// Checks if a block has ANY potential return path.
/// This is a very conservative check - it returns true if there's any statement that COULD
/// provide a return (return, raise, if, match, while, for). The goal is to avoid Cranelift
/// panics on straight-line code with no return. Actual control flow validation happens at codegen.
pub(crate) fn has_potential_return_path(block: &Block) -> bool {
    for stmt in &block.stmts {
        if stmt_has_potential_return(&stmt.node) {
            return true;
        }
    }
    false
}

/// Checks if a statement could potentially provide a return path.
fn stmt_has_potential_return(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Return(_) | Stmt::Raise { .. } | Stmt::If { .. } | Stmt::Match { .. } | Stmt::While { .. } | Stmt::For { .. }
    )
}

fn check_function_body(func: &Function, env: &mut TypeEnv, class_name: Option<&str>) -> Result<(), CompileError> {
    env.invalidated_task_vars.clear();
    env.push_scope();

    // Add parameters to scope
    for p in &func.params {
        let ty = if p.name.node == "self" {
            if let Some(cn) = class_name {
                PlutoType::Class(cn.to_string())
            } else {
                return Err(CompileError::type_err(
                    "'self' used outside of class method",
                    p.name.span,
                ));
            }
        } else {
            resolve_type(&p.ty, env)?
        };
        env.define(p.name.node.clone(), ty);
    }

    let lookup_name = if let Some(cn) = class_name {
        mangle_method(cn, &func.name.node)
    } else {
        func.name.node.clone()
    };
    let expected_return = env.functions.get(&lookup_name).ok_or_else(|| {
        CompileError::type_err(
            format!("unknown function '{}'", lookup_name),
            func.name.span,
        )
    })?.return_type.clone();

    // For generators, set the element type context and pass Void as expected return
    // (generators don't return a value — they yield)
    let prev_gen_elem = env.current_generator_elem.take();
    let effective_return = if let PlutoType::Stream(ref elem) = expected_return {
        env.current_generator_elem = Some(*elem.clone());
        PlutoType::Void
    } else {
        expected_return.clone()
    };

    // Check body
    check_block(&func.body.node, env, &effective_return)?;

    // Verify non-void functions have a return statement.
    // This catches the simple cases that cause Cranelift to panic (no control flow at all).
    // Complex control flow (incomplete if/else, match, loops) is still validated at codegen.
    if !matches!(effective_return, PlutoType::Void) && !has_potential_return_path(&func.body.node) {
        return Err(CompileError::type_err(
            format!("missing return statement in function with return type {}", effective_return),
            func.body.span,
        ));
    }

    env.current_generator_elem = prev_gen_elem;
    env.pop_scope();
    Ok(())
}

pub(crate) fn check_block(block: &Block, env: &mut TypeEnv, return_type: &PlutoType) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        check_stmt(&stmt.node, stmt.span, env, return_type)?;
    }
    Ok(())
}

pub(crate) fn check_block_stmt(
    stmt: &Stmt,
    span: crate::span::Span,
    env: &mut TypeEnv,
    return_type: &PlutoType,
) -> Result<(), CompileError> {
    check_stmt(stmt, span, env, return_type)
}

fn check_stmt(
    stmt: &Stmt,
    span: crate::span::Span,
    env: &mut TypeEnv,
    return_type: &PlutoType,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { name, ty, value, is_mut } => {
            // Handle empty array literals with type annotations: `let x: [int] = []`
            let is_empty_array = matches!(&value.node, Expr::ArrayLit { elements } if elements.is_empty());
            let val_type = if is_empty_array {
                if let Some(declared_ty) = ty {
                    let expected = resolve_type(declared_ty, env)?;
                    if !matches!(&expected, PlutoType::Array(_)) {
                        return Err(CompileError::type_err(
                            format!("type mismatch: expected {expected}, found empty array"),
                            value.span,
                        ));
                    }
                    expected.clone()
                } else {
                    return Err(CompileError::type_err(
                        "cannot infer type of empty array literal; add a type annotation".to_string(),
                        value.span,
                    ));
                }
            } else {
                infer_expr(&value.node, value.span, env)?
            };
            // Check for same-scope redeclaration
            let current_depth = env.scope_depth() - 1;
            if let Some((_, existing_depth)) = env.lookup_with_depth(&name.node) {
                if existing_depth == current_depth {
                    return Err(CompileError::type_err(
                        format!("variable '{}' is already declared in this scope", name.node),
                        name.span,
                    ));
                }
            }
            if let Some(declared_ty) = ty {
                let expected = resolve_type(declared_ty, env)?;
                if !types_compatible(&val_type, &expected, env) {
                    return Err(CompileError::type_err(
                        format!("type mismatch: expected {expected}, found {val_type}"),
                        value.span,
                    ));
                }
                env.define(name.node.clone(), expected.clone());
            } else {
                env.define(name.node.clone(), val_type.clone());
            }
            // Track immutable bindings (let without mut)
            if !is_mut {
                env.mark_immutable(&name.node);
            }
            // Track variable declaration for unused-variable warnings
            let depth = env.scope_depth() - 1;
            env.variable_decls.insert((name.node.clone(), depth), name.span);
            // Track task origin for spawn expressions
            if let Expr::Spawn { .. } = &value.node && let Some(fn_name) = env.spawn_target_fns.get(&(value.span.start, value.span.end)) {
                env.define_task_origin(name.node.clone(), fn_name.clone());
            }
            // Track taint propagation: if inside a scope block and value is tainted, mark variable
            if !env.scope_tainted.is_empty() && is_scope_tainted_expr(&value.node, value.span, env) {
                env.scope_tainted.insert(name.node.clone(), ());
            }
        }
        Stmt::Return(value) => {
            // Generators: bare return is allowed (means "done"), return with value is not
            if env.current_generator_elem.is_some() {
                if let Some(expr) = value {
                    return Err(CompileError::type_err(
                        "return with a value is not allowed in generator functions; use yield instead".to_string(),
                        expr.span,
                    ));
                }
                // bare return in generator — fine, means "done"
                return Ok(());
            }
            let actual = match value {
                Some(expr) => infer_expr(&expr.node, expr.span, env)?,
                None => PlutoType::Void,
            };
            if !types_compatible(&actual, return_type, env) {
                let err_span = value.as_ref().map_or(span, |v| v.span);
                return Err(CompileError::type_err(
                    format!("return type mismatch: expected {return_type}, found {actual}"),
                    err_span,
                ));
            }
            // Reject scope-tainted closures escaping via return
            if let Some(expr) = value {
                if !env.scope_tainted.is_empty() && is_scope_tainted_expr(&expr.node, expr.span, env) {
                    return Err(CompileError::type_err(
                        "closure capturing scope binding cannot escape scope block via return",
                        expr.span,
                    ));
                }
            }
        }
        Stmt::Assign { target, value } => {
            let var_type = env.lookup(&target.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("undefined variable '{}'", target.node),
                    target.span,
                )
            })?.clone();
            if matches!(&var_type, PlutoType::Sender(_) | PlutoType::Receiver(_)) {
                return Err(CompileError::type_err(
                    "cannot reassign channel sender/receiver variable".to_string(),
                    target.span,
                ));
            }
            // Check if variable is immutable (declared without mut)
            if env.is_immutable(&target.node) {
                return Err(CompileError::type_err(
                    format!("cannot assign to immutable variable '{}'", target.node),
                    target.span,
                ));
            }
            let val_type = infer_expr(&value.node, value.span, env)?;
            if !types_compatible(&val_type, &var_type, env) {
                return Err(CompileError::type_err(
                    format!("type mismatch in assignment: expected {var_type}, found {val_type}"),
                    value.span,
                ));
            }
            // Permanently invalidate task origin on reassignment
            if matches!(&var_type, PlutoType::Task(_)) {
                env.invalidated_task_vars.insert(target.node.clone());
            }
            // Reject scope-tainted closures escaping via assignment to outer variable
            if !env.scope_tainted.is_empty() && is_scope_tainted_expr(&value.node, value.span, env) {
                if let Some(scope_depth) = env.scope_body_depths.last() {
                    if let Some((_, var_depth)) = env.lookup_with_depth(&target.node) {
                        if var_depth < *scope_depth {
                            return Err(CompileError::type_err(
                                "closure capturing scope binding cannot escape scope block via assignment to outer variable",
                                value.span,
                            ));
                        }
                    }
                }
            }
        }
        Stmt::FieldAssign { object, field, value } => {
            check_field_assign(object, field, value, env)?;
        }
        Stmt::If { condition, then_block, else_block } => {
            let cond_type = infer_expr(&condition.node, condition.span, env)?;
            if cond_type != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("condition must be bool, found {cond_type}"),
                    condition.span,
                ));
            }
            env.push_scope();
            check_block(&then_block.node, env, return_type)?;
            env.pop_scope();
            if let Some(else_blk) = else_block {
                env.push_scope();
                check_block(&else_blk.node, env, return_type)?;
                env.pop_scope();
            }
        }
        Stmt::While { condition, body } => {
            let cond_type = infer_expr(&condition.node, condition.span, env)?;
            if cond_type != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("while condition must be bool, found {cond_type}"),
                    condition.span,
                ));
            }
            env.push_scope();
            env.loop_depth += 1;
            check_block(&body.node, env, return_type)?;
            env.loop_depth -= 1;
            env.pop_scope();
        }
        Stmt::For { var, iterable, body } => {
            let iter_type = infer_expr(&iterable.node, iterable.span, env)?;
            let elem_type = match iter_type {
                PlutoType::Array(elem) => *elem,
                PlutoType::Range => PlutoType::Int,
                PlutoType::String => PlutoType::String,
                PlutoType::Bytes => PlutoType::Byte,
                PlutoType::Receiver(elem) => *elem,
                PlutoType::Stream(elem) => *elem,
                _ => {
                    return Err(CompileError::type_err(
                        format!("for loop requires array, range, string, bytes, receiver, or stream, found {iter_type}"),
                        iterable.span,
                    ));
                }
            };
            env.push_scope();
            env.define(var.node.clone(), elem_type);
            env.loop_depth += 1;
            check_block(&body.node, env, return_type)?;
            env.loop_depth -= 1;
            env.pop_scope();
        }
        Stmt::IndexAssign { object, index, value } => {
            check_index_assign(object, index, value, env)?;
        }
        Stmt::Match { expr, arms } => {
            check_match_stmt(expr, arms, span, env, return_type)?;
        }
        Stmt::Raise { error_name, fields, .. } => {
            check_raise(error_name, fields, span, env)?;
        }
        Stmt::Assert { expr } => {
            let ty = infer_expr(&expr.node, expr.span, env)?;
            if ty != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("assert expression must be bool, found {ty}"),
                    expr.span,
                ));
            }
        }
        Stmt::Break => {
            if env.loop_depth == 0 {
                return Err(CompileError::type_err(
                    "'break' can only be used inside a loop",
                    span,
                ));
            }
        }
        Stmt::Continue => {
            if env.loop_depth == 0 {
                return Err(CompileError::type_err(
                    "'continue' can only be used inside a loop",
                    span,
                ));
            }
        }
        Stmt::Expr(expr) => {
            let expr_type = infer_expr(&expr.node, expr.span, env)?;
            // Bare expect() as statement is likely a bug (forgot .to_equal() etc.)
            if let Expr::Call { name, .. } = &expr.node && name.node == "expect" {
                return Err(CompileError::type_err(
                    "expect() must be followed by an assertion method like .to_equal(), .to_be_true(), or .to_be_false()",
                    expr.span,
                ));
            }
            // Must-use: bare spawn as statement is a compile error
            if matches!(&expr_type, PlutoType::Task(_)) {
                return Err(CompileError::type_err(
                    "Task handle must be used -- call .get(), .detach(), or assign to a variable",
                    expr.span,
                ));
            }
        }
        Stmt::LetChan { sender, receiver, elem_type, capacity } => {
            let elem = resolve_type(elem_type, env)?;
            if let Some(cap) = capacity {
                let cap_type = infer_expr(&cap.node, cap.span, env)?;
                if cap_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("channel capacity must be int, found {cap_type}"),
                        cap.span,
                    ));
                }
            }
            env.define(sender.node.clone(), PlutoType::Sender(Box::new(elem.clone())));
            env.define(receiver.node.clone(), PlutoType::Receiver(Box::new(elem)));
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { binding, channel } => {
                        let chan_type = infer_expr(&channel.node, channel.span, env)?;
                        match &chan_type {
                            PlutoType::Receiver(elem_type) => {
                                // Bind the received value in the arm body's scope
                                env.push_scope();
                                env.define(binding.node.clone(), *elem_type.clone());
                                check_block(&arm.body.node, env, return_type)?;
                                env.pop_scope();
                            }
                            _ => {
                                return Err(CompileError::type_err(
                                    format!("select recv arm requires a Receiver, found {chan_type}"),
                                    channel.span,
                                ));
                            }
                        }
                    }
                    SelectOp::Send { channel, value } => {
                        let chan_type = infer_expr(&channel.node, channel.span, env)?;
                        match &chan_type {
                            PlutoType::Sender(elem_type) => {
                                let val_type = infer_expr(&value.node, value.span, env)?;
                                if val_type != **elem_type {
                                    return Err(CompileError::type_err(
                                        format!("select send expects {}, found {val_type}", elem_type),
                                        value.span,
                                    ));
                                }
                                check_block(&arm.body.node, env, return_type)?;
                            }
                            _ => {
                                return Err(CompileError::type_err(
                                    format!("select send arm requires a Sender, found {chan_type}"),
                                    channel.span,
                                ));
                            }
                        }
                    }
                }
            }
            if let Some(def) = default {
                check_block(&def.node, env, return_type)?;
            }
        }
        Stmt::Scope { seeds, bindings, body } => {
            check_scope_stmt(seeds, bindings, body, span, env, return_type)?;
        }
        Stmt::Yield { value } => {
            let elem_type = match &env.current_generator_elem {
                Some(t) => t.clone(),
                None => {
                    return Err(CompileError::type_err(
                        "yield can only be used inside a generator function (one that returns stream T)".to_string(),
                        span,
                    ));
                }
            };
            let val_type = infer_expr(&value.node, value.span, env)?;
            if !super::types_compatible(&val_type, &elem_type, env) {
                return Err(CompileError::type_err(
                    format!("yield type mismatch: expected {elem_type}, found {val_type}"),
                    value.span,
                ));
            }
        }
    }
    Ok(())
}

fn check_scope_stmt(
    seeds: &[Spanned<Expr>],
    bindings: &[ScopeBinding],
    body: &Spanned<Block>,
    span: crate::span::Span,
    env: &mut TypeEnv,
    return_type: &PlutoType,
) -> Result<(), CompileError> {
    use std::collections::{HashMap as DMap, HashSet as DSet, VecDeque};
    use crate::parser::ast::Lifecycle;
    use super::env::{FieldWiring, ScopeResolution};

    // 1. Check seed expressions and verify each is a scoped class
    let mut seed_types: Vec<(String, usize)> = Vec::new(); // (class_name, seed_index)
    let mut seed_class_names: DSet<String> = DSet::new();

    for (i, seed) in seeds.iter().enumerate() {
        let ty = infer_expr(&seed.node, seed.span, env)?;
        match &ty {
            PlutoType::Class(name) => {
                let info = env.classes.get(name).ok_or_else(|| {
                    CompileError::type_err(
                        format!("unknown class '{name}' in scope seed"),
                        seed.span,
                    )
                })?;
                if info.lifecycle != Lifecycle::Scoped {
                    return Err(CompileError::type_err(
                        format!(
                            "scope seed must be a scoped class, but '{name}' has lifecycle '{}'; \
                             add 'scoped' keyword: scoped class {name} {{ ... }}",
                            info.lifecycle
                        ),
                        seed.span,
                    ));
                }
                seed_types.push((name.clone(), i));
                seed_class_names.insert(name.clone());
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("scope seed must be a class instance, found {ty}"),
                    seed.span,
                ));
            }
        }
    }

    // 2. Resolve binding types
    let mut binding_types: Vec<(String, PlutoType)> = Vec::new(); // (class_name, type)
    for binding in bindings {
        let ty = resolve_type(&binding.ty, env)?;
        match &ty {
            PlutoType::Class(name) => {
                if !env.classes.contains_key(name) {
                    return Err(CompileError::type_err(
                        format!("unknown class '{name}' in scope binding"),
                        binding.ty.span,
                    ));
                }
                binding_types.push((name.clone(), ty));
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("scope binding must be a class type, found {ty}"),
                    binding.ty.span,
                ));
            }
        }
    }

    // 3. Build scope DI graph — BFS from bindings to discover all needed scoped classes
    let mut needed: DSet<String> = DSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Start from all binding types
    for (name, _) in &binding_types {
        if !needed.contains(name) {
            needed.insert(name.clone());
            queue.push_back(name.clone());
        }
    }
    // Also include seed types (they're provided, but may have deps)
    for (name, _) in &seed_types {
        if !needed.contains(name) {
            needed.insert(name.clone());
            queue.push_back(name.clone());
        }
    }

    // BFS: for each needed class, examine injected fields → add scoped deps
    while let Some(class_name) = queue.pop_front() {
        let info = match env.classes.get(&class_name) {
            Some(i) => i.clone(),
            None => continue,
        };
        for (_, field_ty, is_injected) in &info.fields {
            if !is_injected { continue; }
            if let PlutoType::Class(dep_name) = field_ty {
                let dep_info = env.classes.get(dep_name);
                if let Some(dep_info) = dep_info {
                    // Only add scoped deps to the needed set; singletons are accessed via globals
                    if dep_info.lifecycle == Lifecycle::Scoped && !needed.contains(dep_name) {
                        needed.insert(dep_name.clone());
                        queue.push_back(dep_name.clone());
                    }
                }
            }
        }
    }

    // 4. Validate: scoped classes that aren't seeds must have only injected fields (auto-creatable)
    for class_name in &needed {
        if seed_class_names.contains(class_name) { continue; }
        let info = env.classes.get(class_name).ok_or_else(|| {
            CompileError::type_err(
                format!("scope: unknown class '{class_name}'"),
                span,
            )
        })?;
        if info.lifecycle != Lifecycle::Scoped && info.lifecycle != Lifecycle::Singleton {
            // Transient classes are not wired through scope blocks
            continue;
        }
        if info.lifecycle == Lifecycle::Scoped {
            // Check if all fields are injected (auto-creatable)
            let has_non_injected = info.fields.iter().any(|(_, _, inj)| !*inj);
            if has_non_injected {
                return Err(CompileError::type_err(
                    format!(
                        "scoped class '{class_name}' has non-injected fields and must be provided as a seed; \
                         provide it as a seed expression: scope({class_name} {{ field: val }}) |...| {{ ... }}"
                    ),
                    span,
                ));
            }
        }
    }

    // 5. Topological sort of scoped classes to create (excluding seeds — they're already provided)
    let classes_to_create: Vec<String> = needed.iter()
        .filter(|n| !seed_class_names.contains(*n))
        .filter(|n| {
            env.classes.get(*n).map_or(false, |i| i.lifecycle == Lifecycle::Scoped)
        })
        .cloned()
        .collect();

    // Build dependency graph: A depends on B means A has an injected field of type B (scoped)
    let mut graph: DMap<String, Vec<String>> = DMap::new();
    let mut all_nodes: DSet<String> = DSet::new();

    for name in &classes_to_create {
        all_nodes.insert(name.clone());
        let info = env.classes.get(name).unwrap();
        let deps: Vec<String> = info.fields.iter()
            .filter(|(_, _, inj)| *inj)
            .filter_map(|(_, ty, _)| {
                if let PlutoType::Class(dep_name) = ty {
                    if classes_to_create.contains(dep_name) {
                        return Some(dep_name.clone());
                    }
                }
                None
            })
            .collect();
        graph.insert(name.clone(), deps);
    }
    // Add seeds as nodes too (they don't need creation but are deps)
    for (name, _) in &seed_types {
        all_nodes.insert(name.clone());
        graph.entry(name.clone()).or_default();
    }

    // Kahn's algorithm — edge A → B means A depends on B, B must be created first
    let mut in_degree: DMap<String, usize> = DMap::new();
    for c in &all_nodes {
        in_degree.insert(c.clone(), graph.get(c).map_or(0, |v| v.len()));
    }
    let mut topo_queue: VecDeque<String> = VecDeque::new();
    for (c, deg) in &in_degree {
        if *deg == 0 {
            topo_queue.push_back(c.clone());
        }
    }
    let mut creation_order: Vec<String> = Vec::new();
    while let Some(node) = topo_queue.pop_front() {
        if classes_to_create.contains(&node) {
            creation_order.push(node.clone());
        }
        // Decrement in-degree for dependents
        for (class, deps) in &graph {
            if deps.contains(&node) {
                if let Some(deg) = in_degree.get_mut(class) {
                    *deg -= 1;
                    if *deg == 0 {
                        topo_queue.push_back(class.clone());
                    }
                }
            }
        }
    }

    if creation_order.len() != classes_to_create.len() {
        let in_cycle: Vec<String> = classes_to_create.iter()
            .filter(|c| !creation_order.contains(c))
            .cloned()
            .collect();
        let cycle_str = in_cycle.join(" -> ");
        return Err(CompileError::type_err(
            format!(
                "scope block: circular dependency detected among scoped classes: {cycle_str}"
            ),
            span,
        ));
    }

    // 6. Compute field wirings for each created class
    let mut field_wirings: DMap<String, Vec<(String, FieldWiring)>> = DMap::new();
    for class_name in &creation_order {
        let info = env.classes.get(class_name).unwrap();
        let mut wirings = Vec::new();
        for (field_name, field_ty, is_injected) in &info.fields {
            if !is_injected { continue; }
            if let PlutoType::Class(dep_name) = field_ty {
                let dep_info = env.classes.get(dep_name);
                let wiring = if let Some((_, idx)) = seed_types.iter().find(|(n, _)| n == dep_name) {
                    FieldWiring::Seed(*idx)
                } else if dep_info.map_or(false, |d| d.lifecycle == Lifecycle::Singleton) {
                    FieldWiring::Singleton(dep_name.clone())
                } else if creation_order.contains(dep_name) || seed_class_names.contains(dep_name) {
                    FieldWiring::ScopedInstance(dep_name.clone())
                } else {
                    return Err(CompileError::type_err(
                        format!(
                            "scope block: cannot wire field '{field_name}' of class '{class_name}': \
                             dependency '{dep_name}' is not available as a seed, singleton, or scoped instance; \
                             make '{dep_name}' a seed, or ensure it is a singleton or scoped class in the DI graph"
                        ),
                        span,
                    ));
                };
                wirings.push((field_name.clone(), wiring));
            }
        }
        field_wirings.insert(class_name.clone(), wirings);
    }

    // 7. Compute binding sources — how each binding gets its value
    let mut binding_sources: Vec<FieldWiring> = Vec::new();
    for (binding_class, _) in &binding_types {
        if let Some((_, idx)) = seed_types.iter().find(|(n, _)| n == binding_class) {
            binding_sources.push(FieldWiring::Seed(*idx));
        } else if creation_order.contains(binding_class) {
            binding_sources.push(FieldWiring::ScopedInstance(binding_class.clone()));
        } else {
            return Err(CompileError::type_err(
                format!(
                    "scope block: binding type '{binding_class}' is not reachable from seeds; \
                     add a seed for '{binding_class}' or one of its transitive scoped dependencies"
                ),
                span,
            ));
        }
    }

    // 8. Store ScopeResolution
    env.scope_resolutions.insert(
        (span.start, span.end),
        ScopeResolution {
            creation_order,
            field_wirings,
            binding_sources,
        },
    );

    // 9. Type-check body with bindings in scope
    env.scope_body_depths.push(env.scope_depth());
    env.scope_tainted.push_scope();
    env.push_scope();
    env.scope_bindings.push_scope();
    for binding in bindings {
        env.scope_bindings.insert(binding.name.node.clone(), ());
    }
    for (i, binding) in bindings.iter().enumerate() {
        let (_, ty) = &binding_types[i];
        env.define(binding.name.node.clone(), ty.clone());
    }
    check_block(&body.node, env, return_type)?;
    env.scope_bindings.pop_scope();
    env.pop_scope();
    env.scope_body_depths.pop();
    env.scope_tainted.pop_scope();

    Ok(())
}

/// Check if an expression is a scope-tainted closure (directly or via tainted variable).
fn is_scope_tainted_expr(expr: &Expr, span: crate::span::Span, env: &TypeEnv) -> bool {
    // Direct closure whose span is tainted
    if matches!(expr, Expr::Closure { .. }) && env.scope_tainted_closures.contains(&(span.start, span.end)) {
        return true;
    }
    // Variable that holds a tainted closure
    if let Expr::Ident(name) = expr {
        if env.scope_tainted.contains(name) {
            return true;
        }
    }
    false
}

/// Extracts the root variable name from nested field access chains.
/// e.g. `x.inner.val` → Some("x"), `get_thing().field` → None
pub(super) fn root_variable(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name),
        Expr::FieldAccess { object, .. } => root_variable(&object.node),
        _ => None,
    }
}

/// Collect all `Expr::Ident` names referenced in a block.
struct IdentCollector<'a> {
    idents: &'a mut std::collections::HashSet<String>,
}

impl Visitor for IdentCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Collect identifier if this is an Ident expression
        if let Expr::Ident(name) = &expr.node {
            self.idents.insert(name.clone());
        }
        // Recurse into sub-expressions
        walk_expr(self, expr);
    }
}

pub(super) fn collect_idents_in_block(block: &Block, idents: &mut std::collections::HashSet<String>) {
    let spanned_block = Spanned::new(block.clone(), Span::dummy());
    let mut collector = IdentCollector { idents };
    collector.visit_block(&spanned_block);
}


fn check_field_assign(
    object: &Spanned<Expr>,
    field: &Spanned<String>,
    value: &Spanned<Expr>,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    // Check caller-side mutability
    if let Some(root) = root_variable(&object.node) && root != "self" && env.is_immutable(root) {
        return Err(CompileError::type_err(
            format!(
                "cannot assign to field of immutable variable '{}'; declare with 'let mut' to allow mutation",
                root
            ),
            object.span,
        ));
    }
    let obj_type = infer_expr(&object.node, object.span, env)?;
    let class_name = match &obj_type {
        PlutoType::Class(name) => name.clone(),
        _ => {
            return Err(CompileError::type_err(
                format!("field assignment on non-class type {obj_type}"),
                object.span,
            ));
        }
    };
    let class_info = env.classes.get(&class_name).ok_or_else(|| {
        CompileError::type_err(
            format!("unknown class '{class_name}'"),
            object.span,
        )
    })?;
    let field_type = class_info.fields.iter()
        .find(|(n, _, _)| *n == field.node)
        .map(|(_, t, _)| t.clone())
        .ok_or_else(|| {
            CompileError::type_err(
                format!("class '{class_name}' has no field '{}'", field.node),
                field.span,
            )
        })?;
    let val_type = infer_expr(&value.node, value.span, env)?;
    if val_type != field_type {
        return Err(CompileError::type_err(
            format!("field '{}': expected {field_type}, found {val_type}", field.node),
            value.span,
        ));
    }
    Ok(())
}

fn check_index_assign(
    object: &Spanned<Expr>,
    index: &Spanned<Expr>,
    value: &Spanned<Expr>,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    let obj_type = infer_expr(&object.node, object.span, env)?;
    match &obj_type {
        PlutoType::Array(elem) => {
            let idx_type = infer_expr(&index.node, index.span, env)?;
            if idx_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("array index must be int, found {idx_type}"),
                    index.span,
                ));
            }
            let val_type = infer_expr(&value.node, value.span, env)?;
            if val_type != **elem {
                return Err(CompileError::type_err(
                    format!("index assignment: expected {elem}, found {val_type}"),
                    value.span,
                ));
            }
        }
        PlutoType::Map(key_ty, val_ty) => {
            let idx_type = infer_expr(&index.node, index.span, env)?;
            if idx_type != **key_ty {
                return Err(CompileError::type_err(
                    format!("map key type mismatch: expected {key_ty}, found {idx_type}"),
                    index.span,
                ));
            }
            let val_type = infer_expr(&value.node, value.span, env)?;
            if val_type != **val_ty {
                return Err(CompileError::type_err(
                    format!("map value type mismatch: expected {val_ty}, found {val_type}"),
                    value.span,
                ));
            }
        }
        PlutoType::Bytes => {
            let idx_type = infer_expr(&index.node, index.span, env)?;
            if idx_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("bytes index must be int, found {idx_type}"),
                    index.span,
                ));
            }
            let val_type = infer_expr(&value.node, value.span, env)?;
            if val_type != PlutoType::Byte {
                return Err(CompileError::type_err(
                    format!("bytes index assignment: expected byte, found {val_type}"),
                    value.span,
                ));
            }
        }
        _ => {
            return Err(CompileError::type_err(
                format!("index assignment on non-indexable type {obj_type}"),
                object.span,
            ));
        }
    }
    Ok(())
}

fn check_match_stmt(
    expr: &Spanned<Expr>,
    arms: &[MatchArm],
    span: crate::span::Span,
    env: &mut TypeEnv,
    return_type: &PlutoType,
) -> Result<(), CompileError> {
    let scrutinee_type = infer_expr(&expr.node, expr.span, env)?;
    let enum_name = match &scrutinee_type {
        PlutoType::Enum(name) => name.clone(),
        _ => {
            return Err(CompileError::type_err(
                format!("match requires enum type, found {scrutinee_type}"),
                expr.span,
            ));
        }
    };
    let enum_info = env.enums.get(&enum_name).ok_or_else(|| {
        CompileError::type_err(
            format!("unknown enum '{enum_name}'"),
            expr.span,
        )
    })?.clone();

    let mut covered = std::collections::HashSet::new();
    for arm in arms {
        // Accept exact match, or base generic name match (e.g., "Option" matches "Option$$int")
        let arm_matches = arm.enum_name.node == enum_name
            || (env.generic_enums.contains_key(&arm.enum_name.node)
                && enum_name.starts_with(&format!("{}$$", arm.enum_name.node)));
        if !arm_matches {
            return Err(CompileError::type_err(
                format!("match arm enum '{}' does not match scrutinee enum '{}'", arm.enum_name.node, enum_name),
                arm.enum_name.span,
            ));
        }
        let variant_info = enum_info.variants.iter().find(|(n, _)| *n == arm.variant_name.node);
        let variant_fields = match variant_info {
            None => {
                return Err(CompileError::type_err(
                    format!("enum '{}' has no variant '{}'", enum_name, arm.variant_name.node),
                    arm.variant_name.span,
                ));
            }
            Some((_, fields)) => fields,
        };
        if !covered.insert(arm.variant_name.node.clone()) {
            return Err(CompileError::type_err(
                format!("duplicate match arm for variant '{}'", arm.variant_name.node),
                arm.variant_name.span,
            ));
        }
        if arm.bindings.len() != variant_fields.len() {
            return Err(CompileError::type_err(
                format!(
                    "variant '{}' has {} fields, but {} bindings provided",
                    arm.variant_name.node, variant_fields.len(), arm.bindings.len()
                ),
                arm.variant_name.span,
            ));
        }
        env.push_scope();
        for (binding_field, opt_rename) in &arm.bindings {
            let field_type = variant_fields.iter()
                .find(|(n, _)| *n == binding_field.node)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| {
                    CompileError::type_err(
                        format!("variant '{}' has no field '{}'", arm.variant_name.node, binding_field.node),
                        binding_field.span,
                    )
                })?;
            let var_name = opt_rename.as_ref().map_or(&binding_field.node, |r| &r.node);
            env.define(var_name.clone(), field_type);
        }
        check_block(&arm.body.node, env, return_type)?;
        env.pop_scope();
    }
    // Exhaustiveness check
    for (variant_name, _) in &enum_info.variants {
        if !covered.contains(variant_name) {
            return Err(CompileError::type_err(
                format!("non-exhaustive match: missing variant '{}'", variant_name),
                span,
            ));
        }
    }
    Ok(())
}

fn check_raise(
    error_name: &Spanned<String>,
    fields: &[(Spanned<String>, Spanned<Expr>)],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    let error_info = env.errors.get(&error_name.node).ok_or_else(|| {
        CompileError::type_err(
            format!("unknown error type '{}'", error_name.node),
            error_name.span,
        )
    })?.clone();
    if fields.len() != error_info.fields.len() {
        return Err(CompileError::type_err(
            format!(
                "error '{}' has {} fields, but {} were provided",
                error_name.node, error_info.fields.len(), fields.len()
            ),
            span,
        ));
    }
    for (lit_name, lit_val) in fields {
        let field_type = error_info.fields.iter()
            .find(|(n, _)| *n == lit_name.node)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| {
                CompileError::type_err(
                    format!("error '{}' has no field '{}'", error_name.node, lit_name.node),
                    lit_name.span,
                )
            })?;
        let val_type = infer_expr(&lit_val.node, lit_val.span, env)?;
        if val_type != field_type {
            return Err(CompileError::type_err(
                format!("field '{}': expected {field_type}, found {val_type}", lit_name.node),
                lit_val.span,
            ));
        }
    }
    Ok(())
}

/// Enforce that methods which mutate `self` (field assigns or transitive mut method calls)
/// declare `mut self`. Called after check_all_bodies.
pub(crate) fn enforce_mut_self(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    // Check class methods
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() { continue; } // Skip generic templates
        let class_name = &c.name.node;
        for method in &c.methods {
            let m = &method.node;
            if m.params.is_empty() || m.params[0].name.node != "self" {
                continue;
            }
            if m.params[0].is_mut {
                continue; // Already mut self — no restriction
            }
            check_body_for_self_mutation(&m.body.node, class_name, env)?;
        }
    }

    // Check app methods
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;
        for method in &app.methods {
            let m = &method.node;
            if m.params.is_empty() || m.params[0].name.node != "self" {
                continue;
            }
            if m.params[0].is_mut {
                continue;
            }
            check_body_for_self_mutation(&m.body.node, app_name, env)?;
        }
    }

    // Check stage methods
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = &stage.name.node;
        for method in &stage.methods {
            let m = &method.node;
            if m.params.is_empty() || m.params[0].name.node != "self" {
                continue;
            }
            if m.params[0].is_mut {
                continue;
            }
            check_body_for_self_mutation(&m.body.node, stage_name, env)?;
        }
    }

    Ok(())
}

/// Visitor that checks for mutations to self in non-mut methods.
struct SelfMutationChecker<'a> {
    class_name: &'a str,
    env: &'a TypeEnv,
    error: Option<CompileError>,
}

impl Visitor for SelfMutationChecker<'_> {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        // Short-circuit if we already found an error
        if self.error.is_some() {
            return;
        }

        match &stmt.node {
            Stmt::FieldAssign { object, field, .. } => {
                if matches!(&object.node, Expr::Ident(name) if name == "self") {
                    self.error = Some(CompileError::type_err(
                        format!(
                            "cannot assign to 'self.{}' in a non-mut method; declare 'mut self' to modify fields",
                            field.node
                        ),
                        stmt.span,
                    ));
                    return;
                }
            }
            Stmt::IndexAssign { object, .. } => {
                // NEW: Handle self.array[i] = x and self.field.array[i] = x
                if is_mutation_on_self(&object.node) {
                    self.error = Some(CompileError::type_err(
                        "cannot mutate self's data in a non-mut method; declare 'mut self'".to_string(),
                        stmt.span,
                    ));
                    return;
                }
            }
            Stmt::Expr(expr) => {
                // Check for method calls on self with mut self methods
                if let Err(e) = check_expr_for_mut_method_call(&expr.node, expr.span, self.class_name, self.env) {
                    self.error = Some(e);
                    return;
                }
            }
            Stmt::Let { value, .. } => {
                if let Err(e) = check_expr_for_mut_method_call(&value.node, value.span, self.class_name, self.env) {
                    self.error = Some(e);
                    return;
                }
            }
            Stmt::Return(Some(expr)) => {
                if let Err(e) = check_expr_for_mut_method_call(&expr.node, expr.span, self.class_name, self.env) {
                    self.error = Some(e);
                    return;
                }
            }
            _ => {}
        }

        // Recurse into nested blocks
        walk_stmt(self, stmt);
    }
}

/// Helper: detect mutations rooted at self (self.field[i], self[i], etc.)
fn is_mutation_on_self(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) if name == "self" => true,
        Expr::FieldAccess { object, .. } => is_mutation_on_self(&object.node),
        Expr::Index { object, .. } => is_mutation_on_self(&object.node),
        _ => false,
    }
}

/// Check if an expression contains a mut self method call on self.
fn check_expr_for_mut_method_call(
    expr: &Expr,
    span: crate::span::Span,
    class_name: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match expr {
        Expr::MethodCall { object, method, args, .. } => {
            if matches!(&object.node, Expr::Ident(name) if name == "self") {
                let mangled = mangle_method(class_name, &method.node);
                if env.mut_self_methods.contains(&mangled) {
                    return Err(CompileError::type_err(
                        format!(
                            "cannot call 'mut self' method '{}' on self in a non-mut method; declare 'mut self'",
                            method.node
                        ),
                        span,
                    ));
                }
            }
            // Recurse into args
            for arg in args {
                check_expr_for_mut_method_call(&arg.node, arg.span, class_name, env)?;
            }
            // Recurse into object
            check_expr_for_mut_method_call(&object.node, object.span, class_name, env)?;
        }
        Expr::Propagate { expr: inner } | Expr::Cast { expr: inner, .. } | Expr::Spawn { call: inner } => {
            check_expr_for_mut_method_call(&inner.node, inner.span, class_name, env)?;
        }
        Expr::Catch { expr: inner, handler } => {
            check_expr_for_mut_method_call(&inner.node, inner.span, class_name, env)?;
            if let CatchHandler::Shorthand(expr) = handler {
                check_expr_for_mut_method_call(&expr.node, expr.span, class_name, env)?;
            }
            // Note: Wildcard bodies are checked via the visitor
        }
        Expr::Call { args, .. } => {
            for arg in args {
                check_expr_for_mut_method_call(&arg.node, arg.span, class_name, env)?;
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            check_expr_for_mut_method_call(&lhs.node, lhs.span, class_name, env)?;
            check_expr_for_mut_method_call(&rhs.node, rhs.span, class_name, env)?;
        }
        Expr::UnaryOp { operand, .. } => {
            check_expr_for_mut_method_call(&operand.node, operand.span, class_name, env)?;
        }
        Expr::Index { object, index } => {
            check_expr_for_mut_method_call(&object.node, object.span, class_name, env)?;
            check_expr_for_mut_method_call(&index.node, index.span, class_name, env)?;
        }
        Expr::FieldAccess { object, .. } => {
            check_expr_for_mut_method_call(&object.node, object.span, class_name, env)?;
        }
        // Do NOT recurse into Closure bodies — they capture self by value
        Expr::Closure { .. } | Expr::ClosureCreate { .. } => {}
        _ => {}
    }
    Ok(())
}

fn check_body_for_self_mutation(
    block: &Block,
    class_name: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    let mut checker = SelfMutationChecker {
        class_name,
        env,
        error: None,
    };

    for stmt in &block.stmts {
        checker.visit_stmt(stmt);
        if let Some(err) = checker.error {
            return Err(err);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{Span, Spanned};

    fn mk_span() -> Span {
        Span { start: 0, end: 0, file_id: 0 }
    }

    fn spanned<T>(node: T) -> Spanned<T> {
        Spanned { node, span: mk_span() }
    }

    // ---- root_variable tests ----

    #[test]
    fn root_variable_simple_ident() {
        let expr = Expr::Ident("x".to_string());
        assert_eq!(root_variable(&expr), Some("x"));
    }

    #[test]
    fn root_variable_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Ident("obj".to_string()))),
            field: spanned("field".to_string()),
        };
        assert_eq!(root_variable(&expr), Some("obj"));
    }

    #[test]
    fn root_variable_nested_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::FieldAccess {
                object: Box::new(spanned(Expr::Ident("root".to_string()))),
                field: spanned("middle".to_string()),
            })),
            field: spanned("leaf".to_string()),
        };
        assert_eq!(root_variable(&expr), Some("root"));
    }

    #[test]
    fn root_variable_three_level_nesting() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::FieldAccess {
                object: Box::new(spanned(Expr::FieldAccess {
                    object: Box::new(spanned(Expr::Ident("a".to_string()))),
                    field: spanned("b".to_string()),
                })),
                field: spanned("c".to_string()),
            })),
            field: spanned("d".to_string()),
        };
        assert_eq!(root_variable(&expr), Some("a"));
    }

    #[test]
    fn root_variable_non_ident() {
        let expr = Expr::IntLit(42);
        assert_eq!(root_variable(&expr), None);
    }

    #[test]
    fn root_variable_call_result() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Call {
                name: spanned("get_thing".to_string()),
                args: vec![],
                type_args: vec![],
                target_id: None,
            })),
            field: spanned("value".to_string()),
        };
        assert_eq!(root_variable(&expr), None);
    }

    #[test]
    fn root_variable_index_expression() {
        let expr = Expr::Index {
            object: Box::new(spanned(Expr::Ident("arr".to_string()))),
            index: Box::new(spanned(Expr::IntLit(0))),
        };
        assert_eq!(root_variable(&expr), None);
    }

    // ---- is_mutation_on_self tests ----

    #[test]
    fn is_mutation_on_self_simple() {
        let expr = Expr::Ident("self".to_string());
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Ident("self".to_string()))),
            field: spanned("count".to_string()),
        };
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_nested_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::FieldAccess {
                object: Box::new(spanned(Expr::Ident("self".to_string()))),
                field: spanned("inner".to_string()),
            })),
            field: spanned("value".to_string()),
        };
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_index_access() {
        let expr = Expr::Index {
            object: Box::new(spanned(Expr::Ident("self".to_string()))),
            index: Box::new(spanned(Expr::IntLit(0))),
        };
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_nested_index_access() {
        let expr = Expr::Index {
            object: Box::new(spanned(Expr::Index {
                object: Box::new(spanned(Expr::Ident("self".to_string()))),
                index: Box::new(spanned(Expr::IntLit(0))),
            })),
            index: Box::new(spanned(Expr::IntLit(1))),
        };
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_mixed_field_and_index() {
        let expr = Expr::Index {
            object: Box::new(spanned(Expr::FieldAccess {
                object: Box::new(spanned(Expr::Ident("self".to_string()))),
                field: spanned("items".to_string()),
            })),
            index: Box::new(spanned(Expr::IntLit(0))),
        };
        assert!(is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_other_ident() {
        let expr = Expr::Ident("other".to_string());
        assert!(!is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_other_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Ident("obj".to_string()))),
            field: spanned("field".to_string()),
        };
        assert!(!is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_literal() {
        let expr = Expr::IntLit(42);
        assert!(!is_mutation_on_self(&expr));
    }

    #[test]
    fn is_mutation_on_self_call() {
        let expr = Expr::Call {
            name: spanned("foo".to_string()),
            args: vec![],
            type_args: vec![],
            target_id: None,
        };
        assert!(!is_mutation_on_self(&expr));
    }
}

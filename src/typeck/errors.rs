use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use super::env::{MethodResolution, TypeEnv};

pub(crate) fn infer_error_sets(program: &Program, env: &mut TypeEnv) {
    let mut direct_errors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut propagation_edges: HashMap<String, HashSet<String>> = HashMap::new();

    // Collect effects from top-level functions
    for func in &program.functions {
        if !func.node.type_params.is_empty() { continue; }
        let name = func.node.name.node.clone();
        let (directs, edges) = collect_block_effects(&func.node.body.node, &name, env);
        direct_errors.insert(name.clone(), directs);
        propagation_edges.insert(name, edges);
    }

    // Collect effects from class methods
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let mangled = format!("{}_{}", class_name, method.node.name.node);
            let (directs, edges) = collect_block_effects(&method.node.body.node, &mangled, env);
            direct_errors.insert(mangled.clone(), directs);
            propagation_edges.insert(mangled, edges);
        }
    }

    // Collect effects from inherited default trait methods
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            let mangled = format!("{}_{}", class_name, tm.name.node);
                            let (directs, edges) =
                                collect_block_effects(&tm.body.as_ref().unwrap().node, &mangled, env);
                            direct_errors.insert(mangled.clone(), directs);
                            propagation_edges.insert(mangled, edges);
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
            let mangled = format!("{}_{}", app_name, method.node.name.node);
            let (directs, edges) = collect_block_effects(&method.node.body.node, &mangled, env);
            direct_errors.insert(mangled.clone(), directs);
            propagation_edges.insert(mangled, edges);
        }
    }

    // Fixed-point iteration: propagate error sets through call edges
    let mut fn_errors: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, directs) in &direct_errors {
        fn_errors.insert(name.clone(), directs.clone());
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
}

/// Collect direct error raises and propagation edges from a block.
fn collect_block_effects(block: &Block, current_fn: &str, env: &TypeEnv) -> (HashSet<String>, HashSet<String>) {
    let mut direct_errors = HashSet::new();
    let mut edges = HashSet::new();
    for stmt in &block.stmts {
        collect_stmt_effects(&stmt.node, &mut direct_errors, &mut edges, current_fn, env);
    }
    (direct_errors, edges)
}

fn collect_stmt_effects(
    stmt: &Stmt,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
    current_fn: &str,
    env: &TypeEnv,
) {
    match stmt {
        Stmt::Raise { error_name, fields } => {
            direct_errors.insert(error_name.node.clone());
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges, current_fn, env);
            }
        }
        Stmt::Let { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges, current_fn, env);
        }
        Stmt::Expr(expr) => {
            collect_expr_effects(&expr.node, direct_errors, edges, current_fn, env);
        }
        Stmt::Return(Some(expr)) => {
            collect_expr_effects(&expr.node, direct_errors, edges, current_fn, env);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges, current_fn, env);
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&value.node, direct_errors, edges, current_fn, env);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&index.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&value.node, direct_errors, edges, current_fn, env);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_expr_effects(&condition.node, direct_errors, edges, current_fn, env);
            for s in &then_block.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges, current_fn, env);
            }
            if let Some(eb) = else_block {
                for s in &eb.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges, current_fn, env);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_effects(&condition.node, direct_errors, edges, current_fn, env);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges, current_fn, env);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_effects(&iterable.node, direct_errors, edges, current_fn, env);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges, current_fn, env);
            }
        }
        Stmt::Match { expr, arms } => {
            collect_expr_effects(&expr.node, direct_errors, edges, current_fn, env);
            for arm in arms {
                for s in &arm.body.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges, current_fn, env);
                }
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn collect_expr_effects(
    expr: &Expr,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
    current_fn: &str,
    env: &TypeEnv,
) {
    match expr {
        Expr::Propagate { expr: inner } => {
            match &inner.node {
                Expr::Call { name, args } => {
                    if name.node == "pow"
                        && env
                            .fallible_builtin_calls
                            .contains(&(current_fn.to_string(), name.span.start))
                    {
                        direct_errors.insert("MathError".to_string());
                    } else {
                        edges.insert(name.node.clone());
                    }
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
                    }
                }
                Expr::MethodCall { object, method, args } => {
                    collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
                    }
                    let key = (current_fn.to_string(), method.span.start);
                    match env.method_resolutions.get(&key) {
                        Some(MethodResolution::Class { mangled_name }) => {
                            edges.insert(mangled_name.clone());
                        }
                        Some(MethodResolution::TraitDynamic { trait_name, method_name }) => {
                            for (class_name, info) in &env.classes {
                                if info.impl_traits.iter().any(|t| t == trait_name) {
                                    edges.insert(format!("{}_{}", class_name, method_name));
                                }
                            }
                        }
                        Some(MethodResolution::Builtin) => {}
                        None => {}
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges, current_fn, env),
            }
        }
        Expr::Catch { expr: inner, handler } => {
            match &inner.node {
                Expr::Call { args, .. } => {
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges, current_fn, env),
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    collect_expr_effects(&body.node, direct_errors, edges, current_fn, env);
                }
                CatchHandler::Shorthand(fb) => {
                    collect_expr_effects(&fb.node, direct_errors, edges, current_fn, env);
                }
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr_effects(&lhs.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&rhs.node, direct_errors, edges, current_fn, env);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_expr_effects(&operand.node, direct_errors, edges, current_fn, env);
        }
        Expr::Cast { expr: inner, .. } => {
            collect_expr_effects(&inner.node, direct_errors, edges, current_fn, env);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::FieldAccess { object, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
        }
        Expr::ArrayLit { elements } => {
            for e in elements {
                collect_expr_effects(&e.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::Index { object, index } => {
            collect_expr_effects(&object.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&index.node, direct_errors, edges, current_fn, env);
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_expr_effects(&e.node, direct_errors, edges, current_fn, env);
                }
            }
        }
        Expr::Closure { body, .. } => {
            for stmt in &body.node.stmts {
                collect_stmt_effects(&stmt.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_expr_effects(&k.node, direct_errors, edges, current_fn, env);
                collect_expr_effects(&v.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::SetLit { elements, .. } => {
            for e in elements {
                collect_expr_effects(&e.node, direct_errors, edges, current_fn, env);
            }
        }
        Expr::Range { start, end, .. } => {
            collect_expr_effects(&start.node, direct_errors, edges, current_fn, env);
            collect_expr_effects(&end.node, direct_errors, edges, current_fn, env);
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } => {}
    }
}

// ── Phase 2c: Error handling enforcement ──────────────────────────────────────

pub(crate) fn enforce_error_handling(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    for func in &program.functions {
        if !func.node.type_params.is_empty() { continue; }
        let current_fn = func.node.name.node.clone();
        enforce_block(&func.node.body.node, &current_fn, env)?;
    }
    for class in &program.classes {
        if !class.node.type_params.is_empty() { continue; }
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let current_fn = format!("{}_{}", class_name, method.node.name.node);
            enforce_block(&method.node.body.node, &current_fn, env)?;
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
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            let current_fn = format!("{}_{}", class_name, tm.name.node);
                            enforce_block(&tm.body.as_ref().unwrap().node, &current_fn, env)?;
                        }
                    }
                }
            }
        }
    }
    if let Some(app_spanned) = &program.app {
        let app_name = &app_spanned.node.name.node;
        for method in &app_spanned.node.methods {
            let current_fn = format!("{}_{}", app_name, method.node.name.node);
            enforce_block(&method.node.body.node, &current_fn, env)?;
        }
    }
    Ok(())
}

fn enforce_block(block: &Block, current_fn: &str, env: &TypeEnv) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        enforce_stmt(&stmt.node, stmt.span, current_fn, env)?;
    }
    Ok(())
}

fn enforce_stmt(
    stmt: &Stmt,
    _span: crate::span::Span,
    current_fn: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { value, .. } => enforce_expr(&value.node, value.span, current_fn, env),
        Stmt::Expr(expr) => enforce_expr(&expr.node, expr.span, current_fn, env),
        Stmt::Return(Some(expr)) => enforce_expr(&expr.node, expr.span, current_fn, env),
        Stmt::Return(None) => Ok(()),
        Stmt::Assign { value, .. } => enforce_expr(&value.node, value.span, current_fn, env),
        Stmt::FieldAssign { object, value, .. } => {
            enforce_expr(&object.node, object.span, current_fn, env)?;
            enforce_expr(&value.node, value.span, current_fn, env)
        }
        Stmt::IndexAssign { object, index, value } => {
            enforce_expr(&object.node, object.span, current_fn, env)?;
            enforce_expr(&index.node, index.span, current_fn, env)?;
            enforce_expr(&value.node, value.span, current_fn, env)
        }
        Stmt::If { condition, then_block, else_block } => {
            enforce_expr(&condition.node, condition.span, current_fn, env)?;
            enforce_block(&then_block.node, current_fn, env)?;
            if let Some(eb) = else_block {
                enforce_block(&eb.node, current_fn, env)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            enforce_expr(&condition.node, condition.span, current_fn, env)?;
            enforce_block(&body.node, current_fn, env)
        }
        Stmt::For { iterable, body, .. } => {
            enforce_expr(&iterable.node, iterable.span, current_fn, env)?;
            enforce_block(&body.node, current_fn, env)
        }
        Stmt::Match { expr, arms } => {
            enforce_expr(&expr.node, expr.span, current_fn, env)?;
            for arm in arms {
                enforce_block(&arm.body.node, current_fn, env)?;
            }
            Ok(())
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env)?;
            }
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
) -> Result<(), CompileError> {
    match expr {
        Expr::Call { name, args } => {
            for arg in args {
                enforce_expr(&arg.node, arg.span, current_fn, env)?;
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
            enforce_expr(&object.node, object.span, current_fn, env)?;
            for arg in args {
                enforce_expr(&arg.node, arg.span, current_fn, env)?;
            }
            let is_fallible = env.resolve_method_fallibility(current_fn, method.span.start)
                .map_err(|msg| CompileError::type_err(msg, method.span))?;
            if is_fallible {
                return Err(CompileError::type_err(
                    format!("call to fallible method '{}' must be handled with ! or catch", method.node),
                    span,
                ));
            }
            Ok(())
        }
        Expr::Propagate { expr: inner } => match &inner.node {
            Expr::Call { name, args } => {
                for arg in args {
                    enforce_expr(&arg.node, arg.span, current_fn, env)?;
                }
                let is_fallible_pow = name.node == "pow"
                    && env
                        .fallible_builtin_calls
                        .contains(&(current_fn.to_string(), name.span.start));
                if !is_fallible_pow && !env.is_fn_fallible(&name.node) {
                    return Err(CompileError::type_err(
                        format!("'!' applied to infallible function '{}'", name.node),
                        span,
                    ));
                }
                Ok(())
            }
            Expr::MethodCall { object, method, args } => {
                enforce_expr(&object.node, object.span, current_fn, env)?;
                for arg in args {
                    enforce_expr(&arg.node, arg.span, current_fn, env)?;
                }
                let is_fallible = env.resolve_method_fallibility(current_fn, method.span.start)
                    .map_err(|msg| CompileError::type_err(msg, method.span))?;
                if !is_fallible {
                    return Err(CompileError::type_err(
                        format!("'!' applied to infallible method '{}'", method.node),
                        span,
                    ));
                }
                Ok(())
            }
            _ => Err(CompileError::type_err(
                "! can only be applied to function calls",
                inner.span,
            )),
        },
        Expr::Catch { expr: inner, handler } => {
            match &inner.node {
                Expr::Call { name, args } => {
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, current_fn, env)?;
                    }
                    let is_fallible_pow = name.node == "pow"
                        && env
                            .fallible_builtin_calls
                            .contains(&(current_fn.to_string(), name.span.start));
                    if !is_fallible_pow && !env.is_fn_fallible(&name.node) {
                        return Err(CompileError::type_err(
                            format!("catch applied to infallible function '{}'", name.node),
                            span,
                        ));
                    }
                }
                Expr::MethodCall { object, method, args } => {
                    enforce_expr(&object.node, object.span, current_fn, env)?;
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, current_fn, env)?;
                    }
                    let is_fallible = env.resolve_method_fallibility(current_fn, method.span.start)
                        .map_err(|msg| CompileError::type_err(msg, method.span))?;
                    if !is_fallible {
                        return Err(CompileError::type_err(
                            format!("catch applied to infallible method '{}'", method.node),
                            span,
                        ));
                    }
                }
                _ => {
                    return Err(CompileError::type_err(
                        "catch can only be applied to function calls",
                        inner.span,
                    ));
                }
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => enforce_expr(&body.node, body.span, current_fn, env),
                CatchHandler::Shorthand(fb) => enforce_expr(&fb.node, fb.span, current_fn, env),
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            enforce_expr(&lhs.node, lhs.span, current_fn, env)?;
            enforce_expr(&rhs.node, rhs.span, current_fn, env)
        }
        Expr::UnaryOp { operand, .. } => enforce_expr(&operand.node, operand.span, current_fn, env),
        Expr::Cast { expr: inner, .. } => enforce_expr(&inner.node, inner.span, current_fn, env),
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env)?;
            }
            Ok(())
        }
        Expr::FieldAccess { object, .. } => enforce_expr(&object.node, object.span, current_fn, env),
        Expr::ArrayLit { elements } => {
            for e in elements {
                enforce_expr(&e.node, e.span, current_fn, env)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            enforce_expr(&object.node, object.span, current_fn, env)?;
            enforce_expr(&index.node, index.span, current_fn, env)
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, current_fn, env)?;
            }
            Ok(())
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    enforce_expr(&e.node, e.span, current_fn, env)?;
                }
            }
            Ok(())
        }
        Expr::Closure { body, .. } => {
            enforce_block(&body.node, current_fn, env)
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                enforce_expr(&k.node, k.span, current_fn, env)?;
                enforce_expr(&v.node, v.span, current_fn, env)?;
            }
            Ok(())
        }
        Expr::SetLit { elements, .. } => {
            for e in elements {
                enforce_expr(&e.node, e.span, current_fn, env)?;
            }
            Ok(())
        }
        Expr::Range { start, end, .. } => {
            enforce_expr(&start.node, start.span, current_fn, env)?;
            enforce_expr(&end.node, end.span, current_fn, env)
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } => Ok(()),
    }
}

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use super::env::TypeEnv;

pub(crate) fn infer_error_sets(program: &Program, env: &mut TypeEnv) {
    let mut direct_errors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut propagation_edges: HashMap<String, HashSet<String>> = HashMap::new();

    // Collect effects from top-level functions
    for func in &program.functions {
        let name = func.node.name.node.clone();
        let (directs, edges) = collect_block_effects(&func.node.body.node);
        direct_errors.insert(name.clone(), directs);
        propagation_edges.insert(name, edges);
    }

    // Collect effects from class methods
    for class in &program.classes {
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let mangled = format!("{}_{}", class_name, method.node.name.node);
            let (directs, edges) = collect_block_effects(&method.node.body.node);
            direct_errors.insert(mangled.clone(), directs);
            propagation_edges.insert(mangled, edges);
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
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            let mangled = format!("{}_{}", class_name, tm.name.node);
                            let (directs, edges) =
                                collect_block_effects(&tm.body.as_ref().unwrap().node);
                            direct_errors.insert(mangled.clone(), directs);
                            propagation_edges.insert(mangled, edges);
                        }
                    }
                }
            }
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
fn collect_block_effects(block: &Block) -> (HashSet<String>, HashSet<String>) {
    let mut direct_errors = HashSet::new();
    let mut edges = HashSet::new();
    for stmt in &block.stmts {
        collect_stmt_effects(&stmt.node, &mut direct_errors, &mut edges);
    }
    (direct_errors, edges)
}

fn collect_stmt_effects(
    stmt: &Stmt,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Raise { error_name, fields } => {
            direct_errors.insert(error_name.node.clone());
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Stmt::Let { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::Expr(expr) => {
            collect_expr_effects(&expr.node, direct_errors, edges);
        }
        Stmt::Return(Some(expr)) => {
            collect_expr_effects(&expr.node, direct_errors, edges);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&index.node, direct_errors, edges);
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_expr_effects(&condition.node, direct_errors, edges);
            for s in &then_block.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
            if let Some(eb) = else_block {
                for s in &eb.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_effects(&condition.node, direct_errors, edges);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_effects(&iterable.node, direct_errors, edges);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
        }
        Stmt::Match { expr, arms } => {
            collect_expr_effects(&expr.node, direct_errors, edges);
            for arm in arms {
                for s in &arm.body.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges);
                }
            }
        }
    }
}

fn collect_expr_effects(
    expr: &Expr,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
) {
    match expr {
        Expr::Propagate { expr: inner } => {
            // ! propagates errors from the inner call
            match &inner.node {
                Expr::Call { name, args } => {
                    edges.insert(name.node.clone());
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    // MVP: can't resolve method mangled name without type info
                    collect_expr_effects(&object.node, direct_errors, edges);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges),
            }
        }
        Expr::Catch { expr: inner, handler } => {
            // catch handles errors — don't add propagation edge, but recurse into args
            match &inner.node {
                Expr::Call { args, .. } => {
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    collect_expr_effects(&object.node, direct_errors, edges);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges),
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    collect_expr_effects(&body.node, direct_errors, edges);
                }
                CatchHandler::Shorthand(fb) => {
                    collect_expr_effects(&fb.node, direct_errors, edges);
                }
            }
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr_effects(&lhs.node, direct_errors, edges);
            collect_expr_effects(&rhs.node, direct_errors, edges);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_expr_effects(&operand.node, direct_errors, edges);
        }
        Expr::Call { args, .. } => {
            // Bare call — no propagation edge (errors not propagated)
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Expr::FieldAccess { object, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
        }
        Expr::ArrayLit { elements } => {
            for e in elements {
                collect_expr_effects(&e.node, direct_errors, edges);
            }
        }
        Expr::Index { object, index } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&index.node, direct_errors, edges);
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_expr_effects(&e.node, direct_errors, edges);
                }
            }
        }
        Expr::Closure { body, .. } => {
            for stmt in &body.node.stmts {
                collect_stmt_effects(&stmt.node, direct_errors, edges);
            }
        }
        // Leaf nodes
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } => {}
    }
}

// ── Phase 2c: Error handling enforcement ──────────────────────────────────────

pub(crate) fn enforce_error_handling(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    for func in &program.functions {
        enforce_block(&func.node.body.node, env)?;
    }
    for class in &program.classes {
        for method in &class.node.methods {
            enforce_block(&method.node.body.node, env)?;
        }
    }
    // Also enforce in inherited default trait method bodies
    for class in &program.classes {
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            enforce_block(&tm.body.as_ref().unwrap().node, env)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn enforce_block(block: &Block, env: &TypeEnv) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        enforce_stmt(&stmt.node, stmt.span, env)?;
    }
    Ok(())
}

fn enforce_stmt(
    stmt: &Stmt,
    _span: crate::span::Span,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { value, .. } => enforce_expr(&value.node, value.span, env),
        Stmt::Expr(expr) => enforce_expr(&expr.node, expr.span, env),
        Stmt::Return(Some(expr)) => enforce_expr(&expr.node, expr.span, env),
        Stmt::Return(None) => Ok(()),
        Stmt::Assign { value, .. } => enforce_expr(&value.node, value.span, env),
        Stmt::FieldAssign { object, value, .. } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&value.node, value.span, env)
        }
        Stmt::IndexAssign { object, index, value } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&index.node, index.span, env)?;
            enforce_expr(&value.node, value.span, env)
        }
        Stmt::If { condition, then_block, else_block } => {
            enforce_expr(&condition.node, condition.span, env)?;
            enforce_block(&then_block.node, env)?;
            if let Some(eb) = else_block {
                enforce_block(&eb.node, env)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            enforce_expr(&condition.node, condition.span, env)?;
            enforce_block(&body.node, env)
        }
        Stmt::For { iterable, body, .. } => {
            enforce_expr(&iterable.node, iterable.span, env)?;
            enforce_block(&body.node, env)
        }
        Stmt::Match { expr, arms } => {
            enforce_expr(&expr.node, expr.span, env)?;
            for arm in arms {
                enforce_block(&arm.body.node, env)?;
            }
            Ok(())
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
    }
}

fn enforce_expr(
    expr: &Expr,
    span: crate::span::Span,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match expr {
        Expr::Call { name, args } => {
            for arg in args {
                enforce_expr(&arg.node, arg.span, env)?;
            }
            if env.is_fn_fallible(&name.node) {
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
        Expr::MethodCall { object, args, .. } => {
            enforce_expr(&object.node, object.span, env)?;
            for arg in args {
                enforce_expr(&arg.node, arg.span, env)?;
            }
            // MVP: method fallibility enforcement deferred (needs type resolution)
            Ok(())
        }
        Expr::Propagate { expr: inner } => match &inner.node {
            Expr::Call { name, args } => {
                for arg in args {
                    enforce_expr(&arg.node, arg.span, env)?;
                }
                if !env.is_fn_fallible(&name.node) {
                    return Err(CompileError::type_err(
                        format!("'!' applied to infallible function '{}'", name.node),
                        span,
                    ));
                }
                Ok(())
            }
            Expr::MethodCall { object, args, .. } => {
                enforce_expr(&object.node, object.span, env)?;
                for arg in args {
                    enforce_expr(&arg.node, arg.span, env)?;
                }
                // MVP: allow ! on method calls without fallibility check
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
                        enforce_expr(&arg.node, arg.span, env)?;
                    }
                    if !env.is_fn_fallible(&name.node) {
                        return Err(CompileError::type_err(
                            format!("catch applied to infallible function '{}'", name.node),
                            span,
                        ));
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    enforce_expr(&object.node, object.span, env)?;
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, env)?;
                    }
                    // MVP: allow catch on method calls without fallibility check
                }
                _ => {
                    return Err(CompileError::type_err(
                        "catch can only be applied to function calls",
                        inner.span,
                    ));
                }
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => enforce_expr(&body.node, body.span, env),
                CatchHandler::Shorthand(fb) => enforce_expr(&fb.node, fb.span, env),
            }
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            enforce_expr(&lhs.node, lhs.span, env)?;
            enforce_expr(&rhs.node, rhs.span, env)
        }
        Expr::UnaryOp { operand, .. } => enforce_expr(&operand.node, operand.span, env),
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
        Expr::FieldAccess { object, .. } => enforce_expr(&object.node, object.span, env),
        Expr::ArrayLit { elements } => {
            for e in elements {
                enforce_expr(&e.node, e.span, env)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&index.node, index.span, env)
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    enforce_expr(&e.node, e.span, env)?;
                }
            }
            Ok(())
        }
        Expr::Closure { body, .. } => {
            enforce_block(&body.node, env)
        }
        // Leaf nodes
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } => Ok(()),
    }
}

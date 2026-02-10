use std::collections::HashSet;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use super::env::TypeEnv;
use super::types::PlutoType;
use super::resolve::resolve_type;
use super::infer::infer_expr;
use super::check::check_block;

pub(crate) fn infer_closure(
    params: &[Param],
    return_type: &Option<Spanned<TypeExpr>>,
    body: &Spanned<Block>,
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let outer_depth = env.scope_depth();

    // Push a scope for the closure parameters
    env.push_scope();

    // Resolve and define each param
    let mut param_types = Vec::new();
    for p in params {
        let ty = resolve_type(&p.ty, env)?;
        env.define(p.name.node.clone(), ty.clone());
        param_types.push(ty);
    }

    // Determine the return type: annotated or inferred from body
    let final_ret = if let Some(rt) = return_type {
        resolve_type(rt, env)?
    } else {
        // Infer from first return-with-value in the body
        infer_closure_return_type(&body.node, env)?
    };

    // Check the body against the determined return type
    // Reset loop_depth so break/continue inside closures can't escape to enclosing loop
    let saved_loop_depth = env.loop_depth;
    env.loop_depth = 0;
    check_block(&body.node, env, &final_ret)?;
    env.loop_depth = saved_loop_depth;

    // Collect captures: find free variables that come from outer scopes
    let param_names: HashSet<&str> = params.iter().map(|p| p.name.node.as_str()).collect();
    let mut captures = Vec::new();
    let mut seen = HashSet::new();
    collect_free_vars_block(&body.node, &param_names, outer_depth, env, &mut captures, &mut seen);

    // Store captures keyed by span
    env.closure_captures.insert((span.start, span.end), captures);

    // Store return type for closure lifting (fixes Finding 5)
    env.closure_return_types.insert((span.start, span.end), final_ret.clone());

    env.pop_scope();

    Ok(PlutoType::Fn(param_types, Box::new(final_ret)))
}

/// Infer the return type of a closure body by looking for return statements.
/// If the body has a single return with an expression, we infer from that.
/// Otherwise default to Void.
fn infer_closure_return_type(block: &Block, env: &mut TypeEnv) -> Result<PlutoType, CompileError> {
    // Walk statements sequentially, processing let bindings so that
    // variables are in scope when we encounter a return statement.
    for stmt in &block.stmts {
        match &stmt.node {
            Stmt::Let { name, ty, value, .. } => {
                let val_type = infer_expr(&value.node, value.span, env)?;
                if let Some(declared_ty) = ty {
                    let expected = resolve_type(declared_ty, env)?;
                    env.define(name.node.clone(), expected);
                } else {
                    env.define(name.node.clone(), val_type);
                }
            }
            Stmt::LetChan { sender, receiver, elem_type, .. } => {
                let resolved = resolve_type(elem_type, env)?;
                env.define(sender.node.clone(), PlutoType::Sender(Box::new(resolved.clone())));
                env.define(receiver.node.clone(), PlutoType::Receiver(Box::new(resolved)));
            }
            Stmt::Return(Some(expr)) => {
                return infer_expr(&expr.node, expr.span, env);
            }
            _ => {}
        }
    }
    Ok(PlutoType::Void)
}

/// Collect free variables in a block that resolve from scopes at depth < outer_depth.
/// These are the variables captured by a closure.
fn collect_free_vars_block(
    block: &Block,
    param_names: &HashSet<&str>,
    outer_depth: usize,
    env: &TypeEnv,
    captures: &mut Vec<(String, PlutoType)>,
    seen: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        collect_free_vars_stmt(&stmt.node, param_names, outer_depth, env, captures, seen);
    }
}

fn collect_free_vars_stmt(
    stmt: &Stmt,
    param_names: &HashSet<&str>,
    outer_depth: usize,
    env: &TypeEnv,
    captures: &mut Vec<(String, PlutoType)>,
    seen: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Let { value, .. } => {
            collect_free_vars_expr(&value.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::Return(Some(expr)) => {
            collect_free_vars_expr(&expr.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            collect_free_vars_expr(&value.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_free_vars_expr(&object.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&value.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_free_vars_expr(&condition.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_block(&then_block.node, param_names, outer_depth, env, captures, seen);
            if let Some(eb) = else_block {
                collect_free_vars_block(&eb.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Stmt::While { condition, body } => {
            collect_free_vars_expr(&condition.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_block(&body.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::For { iterable, body, .. } => {
            collect_free_vars_expr(&iterable.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_block(&body.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_free_vars_expr(&object.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&index.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&value.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::Match { expr, arms } => {
            collect_free_vars_expr(&expr.node, param_names, outer_depth, env, captures, seen);
            for arm in arms {
                collect_free_vars_block(&arm.body.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Stmt::Expr(expr) => {
            collect_free_vars_expr(&expr.node, param_names, outer_depth, env, captures, seen);
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                collect_free_vars_expr(&val.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                collect_free_vars_expr(&cap.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => {
                        collect_free_vars_expr(&channel.node, param_names, outer_depth, env, captures, seen);
                    }
                    SelectOp::Send { channel, value } => {
                        collect_free_vars_expr(&channel.node, param_names, outer_depth, env, captures, seen);
                        collect_free_vars_expr(&value.node, param_names, outer_depth, env, captures, seen);
                    }
                }
                collect_free_vars_block(&arm.body.node, param_names, outer_depth, env, captures, seen);
            }
            if let Some(def) = default {
                collect_free_vars_block(&def.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn collect_free_vars_expr(
    expr: &Expr,
    param_names: &HashSet<&str>,
    outer_depth: usize,
    env: &TypeEnv,
    captures: &mut Vec<(String, PlutoType)>,
    seen: &mut HashSet<String>,
) {
    match expr {
        Expr::Ident(name) => {
            // Skip if it's a closure param, a function name, or a builtin
            if param_names.contains(name.as_str()) { return; }
            if env.functions.contains_key(name) { return; }
            if env.builtins.contains(name) { return; }
            if seen.contains(name) { return; }
            // Check if this variable resolves from an outer scope (depth < outer_depth)
            if let Some((ty, depth)) = env.lookup_with_depth(name) {
                if depth < outer_depth {
                    seen.insert(name.clone());
                    captures.push((name.clone(), ty.clone()));
                }
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_free_vars_expr(&lhs.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&rhs.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_free_vars_expr(&operand.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::Cast { expr: inner, .. } => {
            collect_free_vars_expr(&inner.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_free_vars_expr(&arg.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::FieldAccess { object, .. } => {
            collect_free_vars_expr(&object.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::MethodCall { object, args, .. } => {
            collect_free_vars_expr(&object.node, param_names, outer_depth, env, captures, seen);
            for arg in args {
                collect_free_vars_expr(&arg.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                collect_free_vars_expr(&val.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                collect_free_vars_expr(&elem.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::Index { object, index } => {
            collect_free_vars_expr(&object.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&index.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_free_vars_expr(&e.node, param_names, outer_depth, env, captures, seen);
                }
            }
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                collect_free_vars_expr(&val.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::Closure { body, .. } => {
            // Nested closures: scan their body too (captures propagate up)
            collect_free_vars_block(&body.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::Propagate { expr: inner } => {
            collect_free_vars_expr(&inner.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::Catch { expr: inner, handler } => {
            collect_free_vars_expr(&inner.node, param_names, outer_depth, env, captures, seen);
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    collect_free_vars_expr(&body.node, param_names, outer_depth, env, captures, seen);
                }
                CatchHandler::Shorthand(fb) => {
                    collect_free_vars_expr(&fb.node, param_names, outer_depth, env, captures, seen);
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_free_vars_expr(&k.node, param_names, outer_depth, env, captures, seen);
                collect_free_vars_expr(&v.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                collect_free_vars_expr(&elem.node, param_names, outer_depth, env, captures, seen);
            }
        }
        Expr::Range { start, end, .. } => {
            collect_free_vars_expr(&start.node, param_names, outer_depth, env, captures, seen);
            collect_free_vars_expr(&end.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::Spawn { call } => {
            collect_free_vars_expr(&call.node, param_names, outer_depth, env, captures, seen);
        }
        Expr::NullPropagate { expr: inner } => {
            collect_free_vars_expr(&inner.node, param_names, outer_depth, env, captures, seen);
        }
        // Literals and other non-capturing expressions
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. } | Expr::NoneLit => {}
    }
}

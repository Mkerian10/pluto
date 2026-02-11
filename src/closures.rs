use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::typeck::env::{FuncSig, TypeEnv};
use crate::typeck::types::{PlutoType, pluto_type_to_type_expr};

/// Lift closures out of function/method bodies into top-level functions.
///
/// For each `Expr::Closure` found, this pass:
/// 1. Generates a unique function name `__closure_N`
/// 2. Creates a new top-level `Function` with an `__env: int` first param + original params
/// 3. Replaces the `Expr::Closure` with `Expr::ClosureCreate { fn_name, captures }`
/// 4. Registers the lifted function in `env.functions` and `env.closure_fns`
///
/// Returns the list of newly created functions to append to `program.functions`.
pub fn lift_closures(program: &mut Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    let mut counter = 0usize;
    let mut new_fns = Vec::new();

    // Lift from top-level function bodies
    for func in &mut program.functions {
        lift_in_block(&mut func.node.body.node, env, &mut counter, &mut new_fns)?;
    }

    // Lift from class method bodies
    for class in &mut program.classes {
        for method in &mut class.node.methods {
            lift_in_block(&mut method.node.body.node, env, &mut counter, &mut new_fns)?;
        }
    }

    // Lift from app method bodies
    if let Some(app) = &mut program.app {
        for method in &mut app.node.methods {
            lift_in_block(&mut method.node.body.node, env, &mut counter, &mut new_fns)?;
        }
    }

    // Lift from stage method bodies
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
            lift_in_block(&mut method.node.body.node, env, &mut counter, &mut new_fns)?;
        }
    }

    // Append lifted functions to the program
    for f in new_fns {
        program.functions.push(f);
    }

    Ok(())
}

fn lift_in_block(
    block: &mut Block,
    env: &mut TypeEnv,
    counter: &mut usize,
    new_fns: &mut Vec<Spanned<Function>>,
) -> Result<(), CompileError> {
    for stmt in &mut block.stmts {
        lift_in_stmt(&mut stmt.node, env, counter, new_fns)?;
    }
    Ok(())
}

fn lift_in_stmt(
    stmt: &mut Stmt,
    env: &mut TypeEnv,
    counter: &mut usize,
    new_fns: &mut Vec<Spanned<Function>>,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { value, .. } => {
            lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
        }
        Stmt::Return(Some(expr)) => {
            lift_in_expr(&mut expr.node, expr.span, env, counter, new_fns)?;
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
        }
        Stmt::FieldAssign { object, value, .. } => {
            lift_in_expr(&mut object.node, object.span, env, counter, new_fns)?;
            lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
        }
        Stmt::If { condition, then_block, else_block } => {
            lift_in_expr(&mut condition.node, condition.span, env, counter, new_fns)?;
            lift_in_block(&mut then_block.node, env, counter, new_fns)?;
            if let Some(eb) = else_block {
                lift_in_block(&mut eb.node, env, counter, new_fns)?;
            }
        }
        Stmt::While { condition, body } => {
            lift_in_expr(&mut condition.node, condition.span, env, counter, new_fns)?;
            lift_in_block(&mut body.node, env, counter, new_fns)?;
        }
        Stmt::For { iterable, body, .. } => {
            lift_in_expr(&mut iterable.node, iterable.span, env, counter, new_fns)?;
            lift_in_block(&mut body.node, env, counter, new_fns)?;
        }
        Stmt::IndexAssign { object, index, value } => {
            lift_in_expr(&mut object.node, object.span, env, counter, new_fns)?;
            lift_in_expr(&mut index.node, index.span, env, counter, new_fns)?;
            lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
        }
        Stmt::Match { expr, arms } => {
            lift_in_expr(&mut expr.node, expr.span, env, counter, new_fns)?;
            for arm in arms {
                lift_in_block(&mut arm.body.node, env, counter, new_fns)?;
            }
        }
        Stmt::Expr(expr) => {
            lift_in_expr(&mut expr.node, expr.span, env, counter, new_fns)?;
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                lift_in_expr(&mut val.node, val.span, env, counter, new_fns)?;
            }
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                lift_in_expr(&mut cap.node, cap.span, env, counter, new_fns)?;
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        lift_in_expr(&mut channel.node, channel.span, env, counter, new_fns)?;
                    }
                    SelectOp::Send { channel, value } => {
                        lift_in_expr(&mut channel.node, channel.span, env, counter, new_fns)?;
                        lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
                    }
                }
                lift_in_block(&mut arm.body.node, env, counter, new_fns)?;
            }
            if let Some(def) = default {
                lift_in_block(&mut def.node, env, counter, new_fns)?;
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                lift_in_expr(&mut seed.node, seed.span, env, counter, new_fns)?;
            }
            lift_in_block(&mut body.node, env, counter, new_fns)?;
        }
        Stmt::Yield { value, .. } => {
            lift_in_expr(&mut value.node, value.span, env, counter, new_fns)?;
        }
        Stmt::Break | Stmt::Continue => {}
    }
    Ok(())
}

fn lift_in_expr(
    expr: &mut Expr,
    span: Span,
    env: &mut TypeEnv,
    counter: &mut usize,
    new_fns: &mut Vec<Spanned<Function>>,
) -> Result<(), CompileError> {
    match expr {
        Expr::BinOp { lhs, rhs, .. } => {
            lift_in_expr(&mut lhs.node, lhs.span, env, counter, new_fns)?;
            lift_in_expr(&mut rhs.node, rhs.span, env, counter, new_fns)?;
        }
        Expr::UnaryOp { operand, .. } => {
            lift_in_expr(&mut operand.node, operand.span, env, counter, new_fns)?;
        }
        Expr::Cast { expr: inner, .. } => {
            lift_in_expr(&mut inner.node, inner.span, env, counter, new_fns)?;
        }
        Expr::Call { args, .. } => {
            for arg in args {
                lift_in_expr(&mut arg.node, arg.span, env, counter, new_fns)?;
            }
        }
        Expr::FieldAccess { object, .. } => {
            lift_in_expr(&mut object.node, object.span, env, counter, new_fns)?;
        }
        Expr::MethodCall { object, args, .. } => {
            lift_in_expr(&mut object.node, object.span, env, counter, new_fns)?;
            for arg in args {
                lift_in_expr(&mut arg.node, arg.span, env, counter, new_fns)?;
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                lift_in_expr(&mut val.node, val.span, env, counter, new_fns)?;
            }
        }
        Expr::ArrayLit { elements } => {
            for elem in elements {
                lift_in_expr(&mut elem.node, elem.span, env, counter, new_fns)?;
            }
        }
        Expr::Index { object, index } => {
            lift_in_expr(&mut object.node, object.span, env, counter, new_fns)?;
            lift_in_expr(&mut index.node, index.span, env, counter, new_fns)?;
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    lift_in_expr(&mut e.node, e.span, env, counter, new_fns)?;
                }
            }
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                lift_in_expr(&mut val.node, val.span, env, counter, new_fns)?;
            }
        }
        Expr::Closure { .. } => {
            // This is the main case — lift the closure
            // We need to take ownership of the closure fields, so use a dummy swap
            let dummy = Expr::IntLit(0);
            let old_expr = std::mem::replace(expr, dummy);

            if let Expr::Closure { params, return_type: _, body } = old_expr {
                let fn_name = format!("__closure_{}", *counter);
                *counter += 1;

                // Look up captures from typeck's closure_captures (keyed by span)
                let captures = env.closure_captures
                    .get(&(span.start, span.end))
                    .cloned()
                    .unwrap_or_default();

                let capture_names: Vec<String> = captures.iter().map(|(n, _)| n.clone()).collect();

                // Build the __env param (typed as int, since it's a raw pointer)
                let env_param = Param {
                    id: Uuid::new_v4(),
                    name: Spanned::dummy("__env".to_string()),
                    ty: Spanned::dummy(TypeExpr::Named("int".to_string())),
                    is_mut: false,
                };

                // Build the full param list: __env + original params
                let mut all_params = vec![env_param];
                all_params.extend(params.clone());

                // Determine the return type from env.functions or from the closure's inferred type
                // The typeck stored the fn type — we can look up what it inferred
                // Actually, we can compute from the captures stored in closure_captures
                // For the Function AST node, we need the return_type as Option<Spanned<TypeExpr>>
                // We can just pass None and let the codegen use the FuncSig from env.functions
                // But we need to register the FuncSig first

                // Compute param types for the FuncSig
                let mut sig_params = vec![PlutoType::Int]; // __env is I64
                for (_, ty) in &captures {
                    // Captures are not in the cranelift params — they're loaded from __env
                    let _ = ty;
                }
                for p in &params {
                    let pty = resolve_type_for_lift(&p.ty.node);
                    sig_params.push(pty);
                }

                // For return type, use the correct type from typeck when available
                let ret_type = env.closure_return_types
                    .get(&(span.start, span.end))
                    .cloned()
                    .unwrap_or_else(|| infer_return_type_from_body(&body.node));

                // Register the FuncSig in env.functions
                env.functions.insert(fn_name.clone(), FuncSig {
                    params: sig_params,
                    return_type: ret_type.clone(),
                });

                // Register captures in env.closure_fns
                env.closure_fns.insert(fn_name.clone(), captures);

                // Build the return type annotation (None → codegen will use env.functions)
                let ret_type_expr = pluto_type_to_type_expr(&ret_type);

                // IMPORTANT: Recursively lift nested closures in the body before creating the function
                // This handles cases like: (x: int) => (y: int) => x + y
                let mut lifted_body = body;
                lift_in_block(&mut lifted_body.node, env, counter, new_fns)?;

                // Create the lifted Function
                let lifted = Function {
                    id: Uuid::new_v4(),
                    name: Spanned::dummy(fn_name.clone()),
                    type_params: vec![],
                    type_param_bounds: std::collections::HashMap::new(),
                    params: all_params,
                    return_type: if ret_type == PlutoType::Void {
                        None
                    } else {
                        Some(Spanned::dummy(ret_type_expr))
                    },
                    contracts: vec![],
                    body: lifted_body,
                    is_pub: false,
                    is_override: false,
                    is_generator: false,
                };

                new_fns.push(Spanned::new(lifted, span));

                // Replace expr with ClosureCreate
                *expr = Expr::ClosureCreate {
                    fn_name,
                    captures: capture_names,
                    target_id: None,
                };
            }
        }
        Expr::Spawn { call } => {
            lift_in_expr(&mut call.node, call.span, env, counter, new_fns)?;
        }
        Expr::Propagate { expr: inner } => {
            lift_in_expr(&mut inner.node, inner.span, env, counter, new_fns)?;
        }
        Expr::Catch { expr: inner, handler } => {
            lift_in_expr(&mut inner.node, inner.span, env, counter, new_fns)?;
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    lift_in_block(&mut body.node, env, counter, new_fns)?;
                }
                CatchHandler::Shorthand(fb) => {
                    lift_in_expr(&mut fb.node, fb.span, env, counter, new_fns)?;
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                lift_in_expr(&mut k.node, k.span, env, counter, new_fns)?;
                lift_in_expr(&mut v.node, v.span, env, counter, new_fns)?;
            }
        }
        Expr::SetLit { elements, .. } => {
            for elem in elements {
                lift_in_expr(&mut elem.node, elem.span, env, counter, new_fns)?;
            }
        }
        Expr::Range { start, end, .. } => {
            lift_in_expr(&mut start.node, start.span, env, counter, new_fns)?;
            lift_in_expr(&mut end.node, end.span, env, counter, new_fns)?;
        }
        Expr::NullPropagate { expr: inner } => {
            lift_in_expr(&mut inner.node, inner.span, env, counter, new_fns)?;
        }
        // Non-capturing expressions
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } | Expr::ClosureCreate { .. }
        | Expr::NoneLit => {}
        Expr::StaticTraitCall { type_args, args, .. } => {
            for type_arg in type_args {
                // Type args don't contain closures
            }
            for arg in args {
                lift_in_expr(&mut arg.node, arg.span, env, counter, new_fns)?;
            }
        }
    }
    Ok(())
}

/// Simple type resolution for lifted closures (doesn't need full TypeEnv).
fn resolve_type_for_lift(ty: &TypeExpr) -> PlutoType {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => PlutoType::Int,
            "float" => PlutoType::Float,
            "bool" => PlutoType::Bool,
            "string" => PlutoType::String,
            "void" => PlutoType::Void,
            "byte" => PlutoType::Byte,
            "bytes" => PlutoType::Bytes,
            _ => PlutoType::Class(name.clone()),
        },
        TypeExpr::Array(inner) => PlutoType::Array(Box::new(resolve_type_for_lift(&inner.node))),
        TypeExpr::Qualified { module, name } => PlutoType::Class(format!("{}.{}", module, name)),
        TypeExpr::Fn { params, return_type } => {
            let pts: Vec<PlutoType> = params.iter().map(|p| resolve_type_for_lift(&p.node)).collect();
            let ret = resolve_type_for_lift(&return_type.node);
            PlutoType::Fn(pts, Box::new(ret))
        }
        TypeExpr::Generic { name, type_args } => {
            if name == "Map" && type_args.len() == 2 {
                let k = resolve_type_for_lift(&type_args[0].node);
                let v = resolve_type_for_lift(&type_args[1].node);
                PlutoType::Map(Box::new(k), Box::new(v))
            } else if name == "Set" && type_args.len() == 1 {
                let t = resolve_type_for_lift(&type_args[0].node);
                PlutoType::Set(Box::new(t))
            } else if name == "Task" && type_args.len() == 1 {
                let t = resolve_type_for_lift(&type_args[0].node);
                PlutoType::Task(Box::new(t))
            } else if name == "Sender" && type_args.len() == 1 {
                let t = resolve_type_for_lift(&type_args[0].node);
                PlutoType::Sender(Box::new(t))
            } else if name == "Receiver" && type_args.len() == 1 {
                let t = resolve_type_for_lift(&type_args[0].node);
                PlutoType::Receiver(Box::new(t))
            } else {
                PlutoType::Class(name.clone())
            }
        }
        TypeExpr::Nullable(inner) => PlutoType::Nullable(Box::new(resolve_type_for_lift(&inner.node))),
        TypeExpr::Stream(inner) => PlutoType::Stream(Box::new(resolve_type_for_lift(&inner.node))),
    }
}

/// Infer the return type from a closure body by looking at return statements.
fn infer_return_type_from_body(block: &Block) -> PlutoType {
    for stmt in &block.stmts {
        if let Stmt::Return(Some(expr)) = &stmt.node {
            return infer_type_from_expr(&expr.node);
        }
    }
    PlutoType::Void
}

/// Quick type inference from expression structure (used during lifting).
fn infer_type_from_expr(expr: &Expr) -> PlutoType {
    match expr {
        Expr::IntLit(_) => PlutoType::Int,
        Expr::FloatLit(_) => PlutoType::Float,
        Expr::BoolLit(_) => PlutoType::Bool,
        Expr::StringLit(_) => PlutoType::String,
        Expr::BinOp { op, lhs, .. } => {
            match op {
                BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq
                | BinOp::And | BinOp::Or => PlutoType::Bool,
                _ => infer_type_from_expr(&lhs.node),
            }
        }
        Expr::Cast { target_type, .. } => resolve_type_for_lift(&target_type.node),
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => PlutoType::Bool,
            UnaryOp::BitNot => PlutoType::Int,
            UnaryOp::Neg => infer_type_from_expr(&operand.node),
        },
        Expr::Range { .. } => PlutoType::Range,
        _ => PlutoType::Int, // fallback — typeck has already validated
    }
}

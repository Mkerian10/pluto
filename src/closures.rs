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
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before closures. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::types::PlutoType;

    fn dummy_span() -> Span {
        Span::new(0, 0)
    }

    fn spanned<T>(node: T) -> Spanned<T> {
        Spanned::new(node, dummy_span())
    }

    fn empty_type_env() -> TypeEnv {
        TypeEnv::new()
    }

    // ========== resolve_type_for_lift tests ==========

    #[test]
    fn resolve_type_primitive_int() {
        let ty = TypeExpr::Named("int".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Int);
    }

    #[test]
    fn resolve_type_primitive_float() {
        let ty = TypeExpr::Named("float".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Float);
    }

    #[test]
    fn resolve_type_primitive_bool() {
        let ty = TypeExpr::Named("bool".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Bool);
    }

    #[test]
    fn resolve_type_primitive_string() {
        let ty = TypeExpr::Named("string".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::String);
    }

    #[test]
    fn resolve_type_primitive_void() {
        let ty = TypeExpr::Named("void".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Void);
    }

    #[test]
    fn resolve_type_primitive_byte() {
        let ty = TypeExpr::Named("byte".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Byte);
    }

    #[test]
    fn resolve_type_primitive_bytes() {
        let ty = TypeExpr::Named("bytes".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Bytes);
    }

    #[test]
    fn resolve_type_class() {
        let ty = TypeExpr::Named("Point".to_string());
        assert_eq!(resolve_type_for_lift(&ty), PlutoType::Class("Point".to_string()));
    }

    #[test]
    fn resolve_type_array() {
        let ty = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Array(Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn resolve_type_nested_array() {
        let ty = TypeExpr::Array(Box::new(spanned(TypeExpr::Array(
            Box::new(spanned(TypeExpr::Named("string".to_string())))
        ))));
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Array(Box::new(PlutoType::Array(Box::new(PlutoType::String))))
        );
    }

    #[test]
    fn resolve_type_qualified() {
        let ty = TypeExpr::Qualified {
            module: "math".to_string(),
            name: "Vector".to_string(),
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Class("math.Vector".to_string())
        );
    }

    #[test]
    fn resolve_type_fn_no_params() {
        let ty = TypeExpr::Fn {
            params: vec![],
            return_type: Box::new(spanned(TypeExpr::Named("void".to_string()))),
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Fn(vec![], Box::new(PlutoType::Void))
        );
    }

    #[test]
    fn resolve_type_fn_with_params() {
        let ty = TypeExpr::Fn {
            params: vec![
                Box::new(spanned(TypeExpr::Named("int".to_string()))),
                Box::new(spanned(TypeExpr::Named("string".to_string()))),
            ],
            return_type: Box::new(spanned(TypeExpr::Named("bool".to_string()))),
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Fn(
                vec![PlutoType::Int, PlutoType::String],
                Box::new(PlutoType::Bool)
            )
        );
    }

    #[test]
    fn resolve_type_map() {
        let ty = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                spanned(TypeExpr::Named("string".to_string())),
                spanned(TypeExpr::Named("int".to_string())),
            ],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Map(Box::new(PlutoType::String), Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn resolve_type_set() {
        let ty = TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Set(Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn resolve_type_task() {
        let ty = TypeExpr::Generic {
            name: "Task".to_string(),
            type_args: vec![spanned(TypeExpr::Named("string".to_string()))],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Task(Box::new(PlutoType::String))
        );
    }

    #[test]
    fn resolve_type_sender() {
        let ty = TypeExpr::Generic {
            name: "Sender".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Sender(Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn resolve_type_receiver() {
        let ty = TypeExpr::Generic {
            name: "Receiver".to_string(),
            type_args: vec![spanned(TypeExpr::Named("float".to_string()))],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Receiver(Box::new(PlutoType::Float))
        );
    }

    #[test]
    fn resolve_type_nullable() {
        let ty = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Nullable(Box::new(PlutoType::Int))
        );
    }

    #[test]
    fn resolve_type_stream() {
        let ty = TypeExpr::Stream(Box::new(spanned(TypeExpr::Named("string".to_string()))));
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Stream(Box::new(PlutoType::String))
        );
    }

    #[test]
    fn resolve_type_generic_non_builtin() {
        let ty = TypeExpr::Generic {
            name: "CustomGeneric".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        assert_eq!(
            resolve_type_for_lift(&ty),
            PlutoType::Class("CustomGeneric".to_string())
        );
    }

    // ========== infer_type_from_expr tests ==========

    #[test]
    fn infer_int_literal() {
        let expr = Expr::IntLit(42);
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Int);
    }

    #[test]
    fn infer_float_literal() {
        let expr = Expr::FloatLit(3.14);
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Float);
    }

    #[test]
    fn infer_bool_literal() {
        let expr = Expr::BoolLit(true);
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Bool);
    }

    #[test]
    fn infer_string_literal() {
        let expr = Expr::StringLit("hello".to_string());
        assert_eq!(infer_type_from_expr(&expr), PlutoType::String);
    }

    #[test]
    fn infer_comparison_returns_bool() {
        let expr = Expr::BinOp {
            op: BinOp::Lt,
            lhs: Box::new(spanned(Expr::IntLit(1))),
            rhs: Box::new(spanned(Expr::IntLit(2))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Bool);
    }

    #[test]
    fn infer_equality_returns_bool() {
        let expr = Expr::BinOp {
            op: BinOp::Eq,
            lhs: Box::new(spanned(Expr::IntLit(1))),
            rhs: Box::new(spanned(Expr::IntLit(1))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Bool);
    }

    #[test]
    fn infer_logical_and_returns_bool() {
        let expr = Expr::BinOp {
            op: BinOp::And,
            lhs: Box::new(spanned(Expr::BoolLit(true))),
            rhs: Box::new(spanned(Expr::BoolLit(false))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Bool);
    }

    #[test]
    fn infer_arithmetic_returns_lhs_type() {
        let expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(spanned(Expr::FloatLit(1.0))),
            rhs: Box::new(spanned(Expr::FloatLit(2.0))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Float);
    }

    #[test]
    fn infer_cast() {
        let expr = Expr::Cast {
            expr: Box::new(spanned(Expr::IntLit(42))),
            target_type: spanned(TypeExpr::Named("float".to_string())),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Float);
    }

    #[test]
    fn infer_unary_not_returns_bool() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(spanned(Expr::BoolLit(true))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Bool);
    }

    #[test]
    fn infer_unary_bitnot_returns_int() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::BitNot,
            operand: Box::new(spanned(Expr::IntLit(5))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Int);
    }

    #[test]
    fn infer_unary_neg_returns_operand_type() {
        let expr = Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(spanned(Expr::FloatLit(3.14))),
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Float);
    }

    #[test]
    fn infer_range_returns_range_type() {
        let expr = Expr::Range {
            start: Box::new(spanned(Expr::IntLit(0))),
            end: Box::new(spanned(Expr::IntLit(10))),
            inclusive: false,
        };
        assert_eq!(infer_type_from_expr(&expr), PlutoType::Range);
    }

    // ========== infer_return_type_from_body tests ==========

    #[test]
    fn infer_return_type_void_empty_body() {
        let block = Block { stmts: vec![] };
        assert_eq!(infer_return_type_from_body(&block), PlutoType::Void);
    }

    #[test]
    fn infer_return_type_from_return_statement() {
        let block = Block {
            stmts: vec![spanned(Stmt::Return(Some(spanned(Expr::IntLit(42)))))],
        };
        assert_eq!(infer_return_type_from_body(&block), PlutoType::Int);
    }

    #[test]
    fn infer_return_type_first_return_wins() {
        let block = Block {
            stmts: vec![
                spanned(Stmt::Return(Some(spanned(Expr::IntLit(42))))),
                spanned(Stmt::Return(Some(spanned(Expr::StringLit("hello".to_string()))))),
            ],
        };
        assert_eq!(infer_return_type_from_body(&block), PlutoType::Int);
    }

    #[test]
    fn infer_return_type_ignores_none_return() {
        let block = Block {
            stmts: vec![spanned(Stmt::Return(None))],
        };
        assert_eq!(infer_return_type_from_body(&block), PlutoType::Void);
    }

    // ========== lift_in_expr recursion tests ==========

    #[test]
    fn lift_recurses_into_binop() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            rhs: Box::new(spanned(Expr::IntLit(1))),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Check that closure was lifted
        match expr {
            Expr::BinOp { lhs, .. } => match lhs.node {
                Expr::ClosureCreate { .. } => {
                    // Success
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn lift_recurses_into_unary_op() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::UnaryOp { operand, .. } => match operand.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected UnaryOp"),
        }
    }

    #[test]
    fn lift_recurses_into_cast() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Cast {
            expr: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            target_type: spanned(TypeExpr::Named("int".to_string())),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Cast { expr: inner, .. } => match inner.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Cast"),
        }
    }

    #[test]
    fn lift_recurses_into_call_args() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Call {
            name: spanned("foo".to_string()),
            args: vec![spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })],
            type_args: vec![],
            target_id: None,
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Call { args, .. } => match &args[0].node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Call"),
        }
    }

    #[test]
    fn lift_recurses_into_field_access() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::FieldAccess {
            object: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            field: spanned("x".to_string()),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::FieldAccess { object, .. } => match object.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected FieldAccess"),
        }
    }

    #[test]
    fn lift_recurses_into_method_call() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::MethodCall {
            object: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            method: spanned("foo".to_string()),
            args: vec![],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::MethodCall { object, .. } => match object.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected MethodCall"),
        }
    }

    #[test]
    fn lift_recurses_into_struct_lit() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::StructLit {
            name: spanned("Point".to_string()),
            type_args: vec![],
            fields: vec![(
                spanned("x".to_string()),
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
            )],
            target_id: None,
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::StructLit { fields, .. } => match &fields[0].1.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected StructLit"),
        }
    }

    #[test]
    fn lift_recurses_into_array_lit() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::ArrayLit {
            elements: vec![spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::ArrayLit { elements } => match &elements[0].node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected ArrayLit"),
        }
    }

    #[test]
    fn lift_recurses_into_index() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Index {
            object: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            index: Box::new(spanned(Expr::IntLit(0))),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Index { object, .. } => match object.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Index"),
        }
    }

    #[test]
    fn lift_recurses_into_string_interp() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::StringInterp {
            parts: vec![StringInterpPart::Expr(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }))],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::StringInterp { parts } => match &parts[0] {
                StringInterpPart::Expr(e) => match &e.node {
                    Expr::ClosureCreate { .. } => {
                        assert_eq!(new_fns.len(), 1);
                    }
                    _ => panic!("Closure should be lifted"),
                },
                _ => panic!("Expected Expr part"),
            },
            _ => panic!("Expected StringInterp"),
        }
    }

    #[test]
    fn lift_recurses_into_enum_data() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::EnumData {
            enum_name: spanned("Option".to_string()),
            variant: spanned("Some".to_string()),
            type_args: vec![],
            fields: vec![(
                spanned("value".to_string()),
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
            )],
            enum_id: None,
            variant_id: None,
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::EnumData { fields, .. } => match &fields[0].1.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected EnumData"),
        }
    }

    #[test]
    fn lift_recurses_into_spawn() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Spawn {
            call: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Spawn { call } => match call.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Spawn"),
        }
    }

    #[test]
    fn lift_recurses_into_propagate() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Propagate {
            expr: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Propagate { expr: inner } => match inner.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Propagate"),
        }
    }

    #[test]
    fn lift_recurses_into_catch_expr() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Catch {
            expr: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            handler: CatchHandler::Shorthand(Box::new(spanned(Expr::IntLit(0)))),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Catch { expr: inner, .. } => match inner.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Catch"),
        }
    }

    #[test]
    fn lift_recurses_into_catch_shorthand() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Catch {
            expr: Box::new(spanned(Expr::IntLit(0))),
            handler: CatchHandler::Shorthand(Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }))),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Catch { handler, .. } => match handler {
                CatchHandler::Shorthand(fb) => match &fb.node {
                    Expr::ClosureCreate { .. } => {
                        assert_eq!(new_fns.len(), 1);
                    }
                    _ => panic!("Closure should be lifted"),
                },
                _ => panic!("Expected Shorthand"),
            },
            _ => panic!("Expected Catch"),
        }
    }

    #[test]
    fn lift_recurses_into_map_lit() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::MapLit {
            key_type: spanned(TypeExpr::Named("int".to_string())),
            value_type: spanned(TypeExpr::Named("int".to_string())),
            entries: vec![(
                spanned(Expr::IntLit(1)),
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
            )],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::MapLit { entries, .. } => match &entries[0].1.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected MapLit"),
        }
    }

    #[test]
    fn lift_recurses_into_set_lit() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::SetLit {
            elem_type: spanned(TypeExpr::Named("int".to_string())),
            elements: vec![spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::SetLit { elements, .. } => match &elements[0].node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected SetLit"),
        }
    }

    #[test]
    fn lift_recurses_into_range() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Range {
            start: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
            end: Box::new(spanned(Expr::IntLit(10))),
            inclusive: false,
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::Range { start, .. } => match start.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Range"),
        }
    }

    #[test]
    fn lift_recurses_into_null_propagate() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::NullPropagate {
            expr: Box::new(spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::NullPropagate { expr: inner } => match inner.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected NullPropagate"),
        }
    }

    #[test]
    fn lift_recurses_into_static_trait_call() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::StaticTraitCall {
            trait_name: spanned("Trait".to_string()),
            method_name: spanned("method".to_string()),
            type_args: vec![],
            args: vec![spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            })],
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::StaticTraitCall { args, .. } => match &args[0].node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected StaticTraitCall"),
        }
    }

    // ========== lift_in_stmt recursion tests ==========

    #[test]
    fn lift_in_stmt_let() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::Let {
            name: spanned("x".to_string()),
            ty: None,
            is_mut: false,
            value: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::Let { value, .. } => match &value.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Let"),
        }
    }

    #[test]
    fn lift_in_stmt_return() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::Return(Some(spanned(Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        })));

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::Return(Some(expr)) => match &expr.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Return"),
        }
    }

    #[test]
    fn lift_in_stmt_assign() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::Assign {
            target: spanned("x".to_string()),
            value: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Assign"),
        }
    }

    #[test]
    fn lift_in_stmt_if_condition() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::If {
            condition: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
            then_block: spanned(Block { stmts: vec![] }),
            else_block: None,
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::If { condition, .. } => match &condition.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected If"),
        }
    }

    #[test]
    fn lift_in_stmt_while() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::While {
            condition: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::While { condition, .. } => match &condition.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected While"),
        }
    }

    #[test]
    fn lift_in_stmt_for() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::For {
            var: spanned("x".to_string()),
            iterable: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::For { iterable, .. } => match &iterable.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected For"),
        }
    }

    #[test]
    fn lift_in_stmt_match() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::Match {
            expr: spanned(Expr::Closure {
                params: vec![],
                return_type: None,
                body: spanned(Block { stmts: vec![] }),
            }),
            arms: vec![],
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::Match { expr, .. } => match &expr.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Match"),
        }
    }

    #[test]
    fn lift_in_stmt_raise() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut stmt = Stmt::Raise {
            error_name: spanned("MyError".to_string()),
            fields: vec![(
                spanned("msg".to_string()),
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
            )],
            error_id: None,
        };

        lift_in_stmt(&mut stmt, &mut env, &mut counter, &mut new_fns).unwrap();

        match stmt {
            Stmt::Raise { fields, .. } => match &fields[0].1.node {
                Expr::ClosureCreate { .. } => {
                    assert_eq!(new_fns.len(), 1);
                }
                _ => panic!("Closure should be lifted"),
            },
            _ => panic!("Expected Raise"),
        }
    }

    // ========== Closure lifting core tests ==========

    #[test]
    fn lift_simple_closure_no_params() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Check expr was replaced with ClosureCreate
        match expr {
            Expr::ClosureCreate { fn_name, captures, .. } => {
                assert_eq!(fn_name, "__closure_0");
                assert_eq!(captures.len(), 0);
            }
            _ => panic!("Expected ClosureCreate"),
        }

        // Check new function was created
        assert_eq!(new_fns.len(), 1);
        assert_eq!(new_fns[0].node.name.node, "__closure_0");
        // Should have __env param
        assert_eq!(new_fns[0].node.params.len(), 1);
        assert_eq!(new_fns[0].node.params[0].name.node, "__env");
    }

    #[test]
    fn lift_closure_with_params() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("x".to_string()),
                    ty: spanned(TypeExpr::Named("int".to_string())),
                    is_mut: false,
                },
                Param {
                    id: Uuid::new_v4(),
                    name: spanned("y".to_string()),
                    ty: spanned(TypeExpr::Named("string".to_string())),
                    is_mut: false,
                },
            ],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        assert_eq!(new_fns.len(), 1);
        // Should have __env + 2 params = 3 total
        assert_eq!(new_fns[0].node.params.len(), 3);
        assert_eq!(new_fns[0].node.params[0].name.node, "__env");
        assert_eq!(new_fns[0].node.params[1].name.node, "x");
        assert_eq!(new_fns[0].node.params[2].name.node, "y");
    }

    #[test]
    fn lift_closure_counter_increments() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr1 = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        let mut expr2 = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr1, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();
        lift_in_expr(&mut expr2, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr1 {
            Expr::ClosureCreate { fn_name, .. } => {
                assert_eq!(fn_name, "__closure_0");
            }
            _ => panic!("Expected ClosureCreate"),
        }

        match expr2 {
            Expr::ClosureCreate { fn_name, .. } => {
                assert_eq!(fn_name, "__closure_1");
            }
            _ => panic!("Expected ClosureCreate"),
        }

        assert_eq!(new_fns.len(), 2);
    }

    #[test]
    fn lift_nested_closure() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        // Outer closure containing inner closure
        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block {
                stmts: vec![spanned(Stmt::Return(Some(spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }))))],
            }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Should create 2 functions: inner and outer
        assert_eq!(new_fns.len(), 2);
    }

    #[test]
    fn lift_closure_with_captures() {
        let mut env = empty_type_env();
        // Add captures to the env
        env.closure_captures
            .insert((0, 0), vec![("x".to_string(), PlutoType::Int)]);
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        match expr {
            Expr::ClosureCreate { fn_name, captures, .. } => {
                assert_eq!(captures.len(), 1);
                assert_eq!(captures[0], "x");
                // Check that closure_fns was populated
                assert!(env.closure_fns.contains_key(&fn_name));
            }
            _ => panic!("Expected ClosureCreate"),
        }
    }

    #[test]
    fn lift_closure_registers_in_env() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Check that function was registered in env.functions
        assert!(env.functions.contains_key("__closure_0"));
        let sig = &env.functions["__closure_0"];
        assert_eq!(sig.params.len(), 1); // Just __env
        assert_eq!(sig.params[0], PlutoType::Int);
        assert_eq!(sig.return_type, PlutoType::Void);
    }

    #[test]
    fn lift_closure_with_return_type_inferred() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block {
                stmts: vec![spanned(Stmt::Return(Some(spanned(Expr::IntLit(42)))))],
            }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Check that return type was inferred
        let sig = &env.functions["__closure_0"];
        assert_eq!(sig.return_type, PlutoType::Int);

        // Check the generated function has return type annotation
        assert!(new_fns[0].node.return_type.is_some());
    }

    #[test]
    fn lift_closure_with_explicit_return_type_from_env() {
        let mut env = empty_type_env();
        // Set explicit return type in env
        env.closure_return_types.insert((0, 0), PlutoType::String);
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Check that return type from env was used
        let sig = &env.functions["__closure_0"];
        assert_eq!(sig.return_type, PlutoType::String);
    }

    #[test]
    fn lift_preserves_no_return_annotation_for_void() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        // Void return should not have annotation
        assert!(new_fns[0].node.return_type.is_none());
    }

    #[test]
    fn lift_multiple_closures_in_same_expr() {
        let mut env = empty_type_env();
        let mut counter = 0;
        let mut new_fns = Vec::new();

        let mut expr = Expr::Call {
            name: spanned("foo".to_string()),
            args: vec![
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
                spanned(Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                }),
            ],
            type_args: vec![],
            target_id: None,
        };

        lift_in_expr(&mut expr, dummy_span(), &mut env, &mut counter, &mut new_fns).unwrap();

        assert_eq!(new_fns.len(), 2);
        assert_eq!(new_fns[0].node.name.node, "__closure_0");
        assert_eq!(new_fns[1].node.name.node, "__closure_1");
    }
}

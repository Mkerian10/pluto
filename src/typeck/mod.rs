pub mod env;
pub mod types;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use env::{FuncSig, TypeEnv};
use types::PlutoType;

pub fn type_check(program: &Program) -> Result<TypeEnv, CompileError> {
    let mut env = TypeEnv::new();

    // Pass 1: collect function signatures
    for func in &program.functions {
        let f = &func.node;
        let mut param_types = Vec::new();
        for p in &f.params {
            param_types.push(resolve_type(&p.ty)?);
        }
        let return_type = match &f.return_type {
            Some(t) => resolve_type(t)?,
            None => PlutoType::Void,
        };
        env.functions.insert(
            f.name.node.clone(),
            FuncSig { params: param_types, return_type },
        );
    }

    // Pass 2: check function bodies
    for func in &program.functions {
        check_function(&func.node, &mut env)?;
    }

    Ok(env)
}

fn resolve_type(ty: &Spanned<TypeExpr>) -> Result<PlutoType, CompileError> {
    match &ty.node {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(PlutoType::Int),
            "float" => Ok(PlutoType::Float),
            "bool" => Ok(PlutoType::Bool),
            "string" => Ok(PlutoType::String),
            "void" => Ok(PlutoType::Void),
            _ => Err(CompileError::type_err(
                format!("unknown type '{name}'"),
                ty.span,
            )),
        },
    }
}

fn check_function(func: &Function, env: &mut TypeEnv) -> Result<(), CompileError> {
    env.push_scope();

    // Add parameters to scope
    for p in &func.params {
        let ty = resolve_type(&p.ty)?;
        env.define(p.name.node.clone(), ty);
    }

    let expected_return = env.functions.get(&func.name.node).unwrap().return_type.clone();

    // Check body
    check_block(&func.body.node, env, &expected_return)?;

    env.pop_scope();
    Ok(())
}

fn check_block(block: &Block, env: &mut TypeEnv, return_type: &PlutoType) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        check_stmt(&stmt.node, stmt.span, env, return_type)?;
    }
    Ok(())
}

fn check_stmt(
    stmt: &Stmt,
    span: crate::span::Span,
    env: &mut TypeEnv,
    return_type: &PlutoType,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { name, ty, value } => {
            let val_type = infer_expr(&value.node, value.span, env)?;
            if let Some(declared_ty) = ty {
                let expected = resolve_type(declared_ty)?;
                if expected != val_type {
                    return Err(CompileError::type_err(
                        format!("type mismatch: expected {expected}, found {val_type}"),
                        value.span,
                    ));
                }
            }
            env.define(name.node.clone(), val_type);
        }
        Stmt::Return(value) => {
            let actual = match value {
                Some(expr) => infer_expr(&expr.node, expr.span, env)?,
                None => PlutoType::Void,
            };
            if actual != *return_type {
                let err_span = value.as_ref().map_or(span, |v| v.span);
                return Err(CompileError::type_err(
                    format!("return type mismatch: expected {return_type}, found {actual}"),
                    err_span,
                ));
            }
        }
        Stmt::Assign { target, value } => {
            let var_type = env.lookup(&target.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("undefined variable '{}'", target.node),
                    target.span,
                )
            })?.clone();
            let val_type = infer_expr(&value.node, value.span, env)?;
            if var_type != val_type {
                return Err(CompileError::type_err(
                    format!("type mismatch in assignment: expected {var_type}, found {val_type}"),
                    value.span,
                ));
            }
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
            check_block(&body.node, env, return_type)?;
            env.pop_scope();
        }
        Stmt::Expr(expr) => {
            infer_expr(&expr.node, expr.span, env)?;
        }
    }
    Ok(())
}

fn infer_expr(
    expr: &Expr,
    span: crate::span::Span,
    env: &TypeEnv,
) -> Result<PlutoType, CompileError> {
    match expr {
        Expr::IntLit(_) => Ok(PlutoType::Int),
        Expr::FloatLit(_) => Ok(PlutoType::Float),
        Expr::BoolLit(_) => Ok(PlutoType::Bool),
        Expr::StringLit(_) => Ok(PlutoType::String),
        Expr::Ident(name) => {
            env.lookup(name)
                .cloned()
                .ok_or_else(|| CompileError::type_err(
                    format!("undefined variable '{name}'"),
                    span,
                ))
        }
        Expr::BinOp { op, lhs, rhs } => {
            let lt = infer_expr(&lhs.node, lhs.span, env)?;
            let rt = infer_expr(&rhs.node, rhs.span, env)?;

            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    if lt != rt {
                        return Err(CompileError::type_err(
                            format!("operand type mismatch: {lt} vs {rt}"),
                            span,
                        ));
                    }
                    match &lt {
                        PlutoType::Int | PlutoType::Float => Ok(lt),
                        _ => Err(CompileError::type_err(
                            format!("operator not supported for type {lt}"),
                            span,
                        )),
                    }
                }
                BinOp::Eq | BinOp::Neq => {
                    if lt != rt {
                        return Err(CompileError::type_err(
                            format!("cannot compare {lt} with {rt}"),
                            span,
                        ));
                    }
                    Ok(PlutoType::Bool)
                }
                BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                    if lt != rt {
                        return Err(CompileError::type_err(
                            format!("cannot compare {lt} with {rt}"),
                            span,
                        ));
                    }
                    match &lt {
                        PlutoType::Int | PlutoType::Float => Ok(PlutoType::Bool),
                        _ => Err(CompileError::type_err(
                            format!("comparison not supported for type {lt}"),
                            span,
                        )),
                    }
                }
                BinOp::And | BinOp::Or => {
                    if lt != PlutoType::Bool || rt != PlutoType::Bool {
                        return Err(CompileError::type_err(
                            format!("logical operators require bool operands, found {lt} and {rt}"),
                            span,
                        ));
                    }
                    Ok(PlutoType::Bool)
                }
            }
        }
        Expr::UnaryOp { op, operand } => {
            let t = infer_expr(&operand.node, operand.span, env)?;
            match op {
                UnaryOp::Neg => {
                    match &t {
                        PlutoType::Int | PlutoType::Float => Ok(t),
                        _ => Err(CompileError::type_err(
                            format!("cannot negate type {t}"),
                            span,
                        )),
                    }
                }
                UnaryOp::Not => {
                    if t != PlutoType::Bool {
                        return Err(CompileError::type_err(
                            format!("cannot apply '!' to type {t}"),
                            span,
                        ));
                    }
                    Ok(PlutoType::Bool)
                }
            }
        }
        Expr::Call { name, args } => {
            let sig = env.functions.get(&name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("undefined function '{}'", name.node),
                    name.span,
                )
            })?;

            if args.len() != sig.params.len() {
                return Err(CompileError::type_err(
                    format!(
                        "function '{}' expects {} arguments, got {}",
                        name.node,
                        sig.params.len(),
                        args.len()
                    ),
                    span,
                ));
            }

            for (i, (arg, expected)) in args.iter().zip(&sig.params).enumerate() {
                let actual = infer_expr(&arg.node, arg.span, env)?;
                if actual != *expected {
                    return Err(CompileError::type_err(
                        format!(
                            "argument {} of '{}': expected {expected}, found {actual}",
                            i + 1,
                            name.node
                        ),
                        arg.span,
                    ));
                }
            }

            Ok(sig.return_type.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::Parser;

    fn check(src: &str) -> Result<TypeEnv, CompileError> {
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(&tokens, src);
        let program = parser.parse_program().unwrap();
        type_check(&program)
    }

    #[test]
    fn valid_add_function() {
        check("fn add(a: int, b: int) int {\n    return a + b\n}").unwrap();
    }

    #[test]
    fn valid_main_with_call() {
        check("fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1, 2)\n}").unwrap();
    }

    #[test]
    fn type_mismatch_return() {
        let result = check("fn foo() int {\n    return true\n}");
        assert!(result.is_err());
    }

    #[test]
    fn undefined_variable() {
        let result = check("fn main() {\n    let x = y\n}");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_arg_count() {
        let result = check("fn foo(a: int) int {\n    return a\n}\n\nfn main() {\n    let x = foo(1, 2)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_arg_type() {
        let result = check("fn foo(a: int) int {\n    return a\n}\n\nfn main() {\n    let x = foo(true)\n}");
        assert!(result.is_err());
    }

    #[test]
    fn bool_condition_required() {
        let result = check("fn main() {\n    if 42 {\n        let x = 1\n    }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn valid_comparisons() {
        check("fn main() {\n    let x = 1 < 2\n    let y = 3 == 4\n}").unwrap();
    }
}

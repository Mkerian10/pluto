use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use super::env::TypeEnv;
use super::types::PlutoType;
use super::resolve::resolve_type;
use super::infer::infer_expr;
use super::types_compatible;

pub(crate) fn check_function(func: &Function, env: &mut TypeEnv, class_name: Option<&str>) -> Result<(), CompileError> {
    let prev_fn = env.current_fn.take();
    env.current_fn = Some(if let Some(cn) = class_name {
        format!("{}_{}", cn, func.name.node)
    } else {
        func.name.node.clone()
    });
    let result = check_function_body(func, env, class_name);
    env.current_fn = prev_fn;
    result
}

fn check_function_body(func: &Function, env: &mut TypeEnv, class_name: Option<&str>) -> Result<(), CompileError> {
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
        format!("{}_{}", cn, func.name.node)
    } else {
        func.name.node.clone()
    };
    let expected_return = env.functions.get(&lookup_name).ok_or_else(|| {
        CompileError::type_err(
            format!("unknown function '{}'", lookup_name),
            func.name.span,
        )
    })?.return_type.clone();

    // Check body
    check_block(&func.body.node, env, &expected_return)?;

    env.pop_scope();
    Ok(())
}

pub(crate) fn check_block(block: &Block, env: &mut TypeEnv, return_type: &PlutoType) -> Result<(), CompileError> {
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
                let expected = resolve_type(declared_ty, env)?;
                if !types_compatible(&val_type, &expected, env) {
                    return Err(CompileError::type_err(
                        format!("type mismatch: expected {expected}, found {val_type}"),
                        value.span,
                    ));
                }
                env.define(name.node.clone(), expected);
            } else {
                env.define(name.node.clone(), val_type);
            }
        }
        Stmt::Return(value) => {
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
        }
        Stmt::Assign { target, value } => {
            let var_type = env.lookup(&target.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("undefined variable '{}'", target.node),
                    target.span,
                )
            })?.clone();
            let val_type = infer_expr(&value.node, value.span, env)?;
            if !types_compatible(&val_type, &var_type, env) {
                return Err(CompileError::type_err(
                    format!("type mismatch in assignment: expected {var_type}, found {val_type}"),
                    value.span,
                ));
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
                _ => {
                    return Err(CompileError::type_err(
                        format!("for loop requires array or range, found {iter_type}"),
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
        Stmt::Raise { error_name, fields } => {
            check_raise(error_name, fields, span, env)?;
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
            infer_expr(&expr.node, expr.span, env)?;
        }
    }
    Ok(())
}

fn check_field_assign(
    object: &Spanned<Expr>,
    field: &Spanned<String>,
    value: &Spanned<Expr>,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
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
        // Accept exact match, or base generic name match (e.g., "Option" matches "Option__int")
        let arm_matches = arm.enum_name.node == enum_name
            || (env.generic_enums.contains_key(&arm.enum_name.node)
                && enum_name.starts_with(&format!("{}__", arm.enum_name.node)));
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

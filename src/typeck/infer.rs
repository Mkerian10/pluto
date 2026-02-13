use std::collections::HashMap;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use super::env::{mangle_method, TypeEnv};
use super::types::PlutoType;
use super::resolve::{resolve_type, unify, ensure_generic_func_instantiated, ensure_generic_class_instantiated, ensure_generic_enum_instantiated, validate_type_bounds};
use super::closures::infer_closure;
use super::types_compatible;

pub(crate) fn infer_expr(
    expr: &Expr,
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    match expr {
        Expr::IntLit(_) => Ok(PlutoType::Int),
        Expr::FloatLit(_) => Ok(PlutoType::Float),
        Expr::BoolLit(_) => Ok(PlutoType::Bool),
        Expr::StringLit(_) => Ok(PlutoType::String),
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    let t = infer_expr(&e.node, e.span, env)?;
                    match t {
                        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Byte => {}
                        _ => {
                            return Err(CompileError::type_err(
                                format!("cannot interpolate {} into string", t),
                                e.span,
                            ));
                        }
                    }
                }
            }
            Ok(PlutoType::String)
        }
        Expr::Ident(name) => {
            // Track variable read for unused-variable warnings
            if let Some((_, depth)) = env.lookup_with_depth(name) {
                env.variable_reads.insert((name.clone(), depth));
            }
            env.lookup(name)
                .cloned()
                .ok_or_else(|| CompileError::type_err(
                    format!("undefined variable '{name}'"),
                    span,
                ))
        }
        Expr::BinOp { op, lhs, rhs } => infer_binop(op, lhs, rhs, span, env),
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
                UnaryOp::BitNot => {
                    if t != PlutoType::Int {
                        return Err(CompileError::type_err(
                            format!("cannot apply '~' to type {t}"),
                            span,
                        ));
                    }
                    Ok(PlutoType::Int)
                }
            }
        }
        Expr::Cast { expr, target_type } => {
            let source = infer_expr(&expr.node, expr.span, env)?;
            let target = resolve_type(target_type, env)?;
            match (&source, &target) {
                (PlutoType::Int, PlutoType::Float)
                | (PlutoType::Float, PlutoType::Int)
                | (PlutoType::Int, PlutoType::Bool)
                | (PlutoType::Bool, PlutoType::Int)
                | (PlutoType::Int, PlutoType::Byte)
                | (PlutoType::Byte, PlutoType::Int) => Ok(target),
                _ => Err(CompileError::type_err(
                    format!("cannot cast from {source} to {target}"),
                    span,
                )),
            }
        }
        Expr::Call { name, args, type_args, .. } => infer_call(name, args, type_args, span, env),
        Expr::StructLit { name, fields: lit_fields, type_args, .. } => {
            infer_struct_lit(name, lit_fields, type_args, span, env)
        }
        Expr::FieldAccess { object, field } => {
            let obj_type = infer_expr(&object.node, object.span, env)?;
            match &obj_type {
                PlutoType::Class(class_name) => {
                    let class_info = env.classes.get(class_name).ok_or_else(|| {
                        CompileError::type_err(
                            format!("unknown class '{class_name}'"),
                            object.span,
                        )
                    })?;
                    class_info.fields.iter()
                        .find(|(n, _, _)| *n == field.node)
                        .map(|(_, t, _)| t.clone())
                        .ok_or_else(|| {
                            CompileError::type_err(
                                format!("class '{class_name}' has no field '{}'", field.node),
                                field.span,
                            )
                        })
                }
                PlutoType::Error if field.node == "message" && env.errors.contains_key("MathError") => {
                    Ok(PlutoType::String)
                }
                _ => Err(CompileError::type_err(
                    format!("field access on non-class type {obj_type}"),
                    object.span,
                )),
            }
        }
        Expr::ArrayLit { elements } => {
            if elements.is_empty() {
                return Err(CompileError::type_err(
                    "cannot infer type of empty array literal; add a type annotation".to_string(),
                    span,
                ));
            }
            let first_type = infer_expr(&elements[0].node, elements[0].span, env)?;
            for elem in &elements[1..] {
                let t = infer_expr(&elem.node, elem.span, env)?;
                if t != first_type {
                    return Err(CompileError::type_err(
                        format!("array element type mismatch: expected {first_type}, found {t}"),
                        elem.span,
                    ));
                }
            }
            Ok(PlutoType::Array(Box::new(first_type)))
        }
        Expr::Index { object, index } => {
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
                    Ok(*elem.clone())
                }
                PlutoType::Map(key_ty, val_ty) => {
                    let idx_type = infer_expr(&index.node, index.span, env)?;
                    if idx_type != **key_ty {
                        return Err(CompileError::type_err(
                            format!("map key type mismatch: expected {key_ty}, found {idx_type}"),
                            index.span,
                        ));
                    }
                    Ok(*val_ty.clone())
                }
                PlutoType::String => {
                    let idx_type = infer_expr(&index.node, index.span, env)?;
                    if idx_type != PlutoType::Int {
                        return Err(CompileError::type_err(
                            format!("string index must be int, found {idx_type}"),
                            index.span,
                        ));
                    }
                    Ok(PlutoType::String)
                }
                PlutoType::Bytes => {
                    let idx_type = infer_expr(&index.node, index.span, env)?;
                    if idx_type != PlutoType::Int {
                        return Err(CompileError::type_err(
                            format!("bytes index must be int, found {idx_type}"),
                            index.span,
                        ));
                    }
                    Ok(PlutoType::Byte)
                }
                _ => {
                    Err(CompileError::type_err(
                        format!("index on non-indexable type {obj_type}"),
                        object.span,
                    ))
                }
            }
        }
        Expr::EnumUnit { enum_name, variant, type_args, .. } => {
            infer_enum_unit(enum_name, variant, type_args, span, env)
        }
        Expr::EnumData { enum_name, variant, fields: lit_fields, type_args, .. } => {
            infer_enum_data(enum_name, variant, lit_fields, type_args, span, env)
        }
        Expr::Propagate { expr } => {
            let inner_type = infer_expr(&expr.node, expr.span, env)?;
            Ok(inner_type)
        }
        Expr::Catch { expr, handler } => infer_catch(expr, handler, span, env),
        Expr::MethodCall { object, method, args } => {
            infer_method_call(object, method, args, span, env)
        }
        Expr::Closure { params, return_type, body } => {
            infer_closure(params, return_type, body, span, env)
        }
        Expr::ClosureCreate { .. } => {
            Ok(PlutoType::Void)
        }
        Expr::MapLit { key_type, value_type, entries } => {
            let kt = resolve_type(key_type, env)?;
            let vt = resolve_type(value_type, env)?;
            validate_hashable_key(&kt, key_type.span)?;
            for (k, v) in entries {
                let actual_k = infer_expr(&k.node, k.span, env)?;
                if actual_k != kt {
                    return Err(CompileError::type_err(
                        format!("map key type mismatch: expected {kt}, found {actual_k}"),
                        k.span,
                    ));
                }
                let actual_v = infer_expr(&v.node, v.span, env)?;
                if actual_v != vt {
                    return Err(CompileError::type_err(
                        format!("map value type mismatch: expected {vt}, found {actual_v}"),
                        v.span,
                    ));
                }
            }
            Ok(PlutoType::Map(Box::new(kt), Box::new(vt)))
        }
        Expr::Range { start, end, .. } => {
            let start_type = infer_expr(&start.node, start.span, env)?;
            let end_type = infer_expr(&end.node, end.span, env)?;
            if start_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("range start must be int, found {start_type}"),
                    start.span,
                ));
            }
            if end_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("range end must be int, found {end_type}"),
                    end.span,
                ));
            }
            Ok(PlutoType::Range)
        }
        Expr::SetLit { elem_type, elements } => {
            let et = resolve_type(elem_type, env)?;
            validate_hashable_key(&et, elem_type.span)?;
            for elem in elements {
                let actual = infer_expr(&elem.node, elem.span, env)?;
                if actual != et {
                    return Err(CompileError::type_err(
                        format!("set element type mismatch: expected {et}, found {actual}"),
                        elem.span,
                    ));
                }
            }
            Ok(PlutoType::Set(Box::new(et)))
        }
        Expr::Spawn { call } => {
            // After desugaring, call is a Closure wrapping the original function call.
            // Infer the closure type to get the return type.
            let closure_type = infer_expr(&call.node, call.span, env)?;
            let inner_type = match &closure_type {
                PlutoType::Fn(_, ret) => *ret.clone(),
                _ => {
                    return Err(CompileError::type_err(
                        "spawn requires a function call".to_string(),
                        span,
                    ));
                }
            };
            // Extract the spawned function name by peeking inside the closure body
            if let Expr::Closure { body, .. } = &call.node {
                for stmt in &body.node.stmts {
                    if let Stmt::Return(Some(ret_expr)) = &stmt.node {
                        match &ret_expr.node {
                            Expr::Call { name, .. } => {
                                env.spawn_target_fns.insert(
                                    (span.start, span.end),
                                    name.node.clone(),
                                );
                            }
                            Expr::MethodCall { object, method, .. } => {
                                let obj_type = infer_expr(&object.node, object.span, env)?;
                                if let PlutoType::Class(class_name) = &obj_type {
                                    let mangled = super::env::mangle_method(class_name, &method.node);
                                    env.spawn_target_fns.insert(
                                        (span.start, span.end),
                                        mangled,
                                    );
                                }
                                // Non-class targets → None → conservatively fallible
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Spawn-scope safety: reject spawn if it captures scope bindings
            if !env.scope_bindings.is_empty() {
                if let Expr::Closure { body, .. } = &call.node {
                    let mut idents = std::collections::HashSet::new();
                    super::check::collect_idents_in_block(&body.node, &mut idents);
                    for name in &idents {
                        if env.scope_bindings.contains(name) {
                            return Err(CompileError::type_err(
                                    format!("cannot spawn inside scope block: task would capture scope binding '{name}'"),
                                    span,
                                ));
                        }
                    }
                }
            }
            Ok(PlutoType::Task(Box::new(inner_type)))
        }
        Expr::NoneLit => {
            // Sentinel type — Nullable(Void) means "none literal, type not yet known"
            // The actual nullable type will be determined by context (let annotation, return type, etc.)
            Ok(PlutoType::Nullable(Box::new(PlutoType::Void)))
        }
        Expr::NullPropagate { expr } => {
            let inner_type = infer_expr(&expr.node, expr.span, env)?;
            match &inner_type {
                PlutoType::Nullable(inner) => Ok(*inner.clone()),
                _ => Err(CompileError::type_err(
                    format!("'?' applied to non-nullable type {inner_type}"),
                    span,
                )),
            }
        }
        Expr::StaticTraitCall { trait_name, method_name, type_args, args } => {
            // Look up the trait and clone the information we need
            let (method_sig, is_static) = {
                let trait_info = env.traits.get(&trait_name.node).ok_or_else(|| {
                    CompileError::type_err(
                        format!("unknown trait '{}'", trait_name.node),
                        trait_name.span,
                    )
                })?;

                // Look up the method in the trait
                let method_sig = trait_info.methods.iter()
                    .find(|(name, _)| name == &method_name.node)
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("trait '{}' has no method '{}'", trait_name.node, method_name.node),
                            method_name.span,
                        )
                    })?
                    .1.clone();

                // Check if it's a static method
                let is_static = trait_info.static_methods.contains(&method_name.node);

                (method_sig, is_static)
            };

            // Verify it's a static method (no self parameter)
            if !is_static {
                return Err(CompileError::type_err(
                    format!(
                        "method '{}' on trait '{}' is not a static method (requires self parameter)",
                        method_name.node, trait_name.node
                    ),
                    method_name.span,
                ));
            }

            // TODO: Handle type arguments once generic static methods are supported
            if !type_args.is_empty() {
                // For now, type arguments are stored but not yet fully implemented
                // This will be needed for TypeInfo::kind<T>()
            }

            // Type check call arguments
            let arg_types: Result<Vec<_>, _> = args.iter().map(|a| infer_expr(&a.node, a.span, env)).collect();
            let arg_types = arg_types?;

            // Verify argument count matches
            if arg_types.len() != method_sig.params.len() {
                return Err(CompileError::type_err(
                    format!(
                        "static method '{}::{}' expects {} arguments, got {}",
                        trait_name.node, method_name.node, method_sig.params.len(), arg_types.len()
                    ),
                    span,
                ));
            }

            // Verify argument types match
            for (i, (expected, actual)) in method_sig.params.iter().zip(&arg_types).enumerate() {
                if expected != actual {
                    return Err(CompileError::type_err(
                        format!(
                            "static method '{}::{}' argument {} has type {:?}, expected {:?}",
                            trait_name.node, method_name.node, i + 1, actual, expected
                        ),
                        args[i].span,
                    ));
                }
            }

            Ok(method_sig.return_type.clone())
        }
        Expr::If { condition, then_block, else_block } => {
            // Check condition is bool
            let cond_type = infer_expr(&condition.node, condition.span, env)?;
            if cond_type != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("if condition must be bool, found {cond_type}"),
                    condition.span,
                ));
            }

            // Infer type of both branches
            env.push_scope();
            let then_type = infer_block_type(&then_block.node, env)?;
            env.pop_scope();

            env.push_scope();
            let else_type = infer_block_type(&else_block.node, env)?;
            env.pop_scope();

            // Unify branch types
            unify_branch_types(
                &then_type,
                &else_type,
                then_block.span,
                else_block.span
            )
        }
        Expr::Match { expr: match_expr, arms } => {
            use std::collections::HashSet;

            // Infer scrutinee type → must be Enum
            let scrutinee_type = infer_expr(&match_expr.node, match_expr.span, env)?;

            let enum_name = match &scrutinee_type {
                PlutoType::Enum(name) => name.clone(),
                other => {
                    return Err(CompileError::type_err(
                        format!("match requires enum type, found {}", other),
                        match_expr.span,
                    ));
                }
            };

            // Get enum info for exhaustiveness checking
            let enum_info = env.enums.get(&enum_name).ok_or_else(|| {
                CompileError::type_err(
                    format!("undefined enum '{}'", enum_name),
                    match_expr.span,
                )
            })?.clone();

            let mut arm_types = Vec::new();
            let mut covered = HashSet::new();

            // Type-check each arm
            for arm in arms {
                // Validate enum name matches (handle generics via prefix)
                let arm_enum_base = arm
                    .enum_name
                    .node
                    .split("$$")
                    .next()
                    .unwrap_or(&arm.enum_name.node);
                let scrutinee_enum_base = enum_name.split("$$").next().unwrap_or(&enum_name);

                if arm_enum_base != scrutinee_enum_base {
                    return Err(CompileError::type_err(
                        format!(
                            "match arm enum '{}' does not match scrutinee enum '{}'",
                            arm.enum_name.node, enum_name
                        ),
                        arm.enum_name.span,
                    ));
                }

                // Lookup variant
                let variant = enum_info
                    .variants
                    .iter()
                    .find(|(name, _)| name == &arm.variant_name.node)
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!(
                                "enum '{}' has no variant '{}'",
                                enum_name, arm.variant_name.node
                            ),
                            arm.variant_name.span,
                        )
                    })?;

                // Check for duplicates
                if !covered.insert(arm.variant_name.node.clone()) {
                    return Err(CompileError::type_err(
                        format!(
                            "duplicate match arm for variant '{}'",
                            arm.variant_name.node
                        ),
                        arm.variant_name.span,
                    ));
                }

                // Validate bindings
                if arm.bindings.len() != variant.1.len() {
                    return Err(CompileError::type_err(
                        format!(
                            "variant '{}' has {} fields, but {} bindings provided",
                            arm.variant_name.node,
                            variant.1.len(),
                            arm.bindings.len()
                        ),
                        arm.variant_name.span,
                    ));
                }

                // Create new scope and bind fields
                env.push_scope();

                for (binding_field, opt_rename) in arm.bindings.iter() {
                    // Find field in variant
                    let field = variant
                        .1
                        .iter()
                        .find(|(fname, _)| fname == &binding_field.node)
                        .ok_or_else(|| {
                            CompileError::type_err(
                                format!(
                                    "variant '{}' has no field '{}'",
                                    arm.variant_name.node, binding_field.node
                                ),
                                binding_field.span,
                            )
                        })?;

                    // Bind variable (use rename if provided)
                    let var_name = opt_rename
                        .as_ref()
                        .map_or(&binding_field.node, |r| &r.node);
                    env.define(var_name.clone(), field.1.clone());
                }

                // Infer arm value type
                let arm_type = infer_expr(&arm.value.node, arm.value.span, env)?;
                arm_types.push((arm_type, arm.value.span));

                env.pop_scope();
            }

            // Check exhaustiveness
            for (variant_name, _) in &enum_info.variants {
                if !covered.contains(variant_name) {
                    return Err(CompileError::type_err(
                        format!("non-exhaustive match: missing variant '{}'", variant_name),
                        span,
                    ));
                }
            }

            // Unify all arm types
            if arm_types.is_empty() {
                return Err(CompileError::type_err(
                    "match expression must have at least one arm".to_string(),
                    span,
                ));
            }

            // Start with first arm's type
            let mut unified = arm_types[0].0.clone();

            // Unify with each subsequent arm
            for (arm_type, arm_span) in &arm_types[1..] {
                unified = unify_branch_types(&unified, arm_type, arm_types[0].1, *arm_span)?;
            }

            Ok(unified)
        }
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before type checking. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
        }
    }
}

fn validate_hashable_key(ty: &PlutoType, span: crate::span::Span) -> Result<(), CompileError> {
    match ty {
        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Enum(_) | PlutoType::Byte => Ok(()),
        _ => Err(CompileError::type_err(
            format!("type {ty} cannot be used as a map/set key (must be int, float, bool, string, byte, or enum)"),
            span,
        )),
    }
}

fn infer_binop(
    op: &BinOp,
    lhs: &Spanned<Expr>,
    rhs: &Spanned<Expr>,
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
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
            if *op == BinOp::Add && lt == PlutoType::String {
                return Ok(PlutoType::String);
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
            if lt == PlutoType::Bytes || rt == PlutoType::Bytes {
                return Err(CompileError::type_err(
                    "cannot compare bytes with ==; use element-wise comparison".to_string(),
                    span,
                ));
            }
            // Allow comparing nullable types with none (Nullable(Void))
            let compatible = lt == rt
                || (matches!(&lt, PlutoType::Nullable(_)) && rt == PlutoType::Nullable(Box::new(PlutoType::Void)))
                || (lt == PlutoType::Nullable(Box::new(PlutoType::Void)) && matches!(&rt, PlutoType::Nullable(_)));
            if !compatible {
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
                PlutoType::Int | PlutoType::Float | PlutoType::Byte => Ok(PlutoType::Bool),
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
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            if lt != PlutoType::Int || rt != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("bitwise operators require int operands, found {lt} and {rt}"),
                    span,
                ));
            }
            Ok(PlutoType::Int)
        }
    }
}

fn infer_call(
    name: &Spanned<String>,
    args: &[Spanned<Expr>],
    call_type_args: &[Spanned<TypeExpr>],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    // Handle old() in ensures contracts — old(expr) has the same type as expr
    if name.node == "old" && args.len() == 1 && env.in_ensures_context {
        return infer_expr(&args[0].node, args[0].span, env);
    }

    // Reject explicit type args on builtins
    if !call_type_args.is_empty() && env.builtins.contains(&name.node) {
        return Err(CompileError::type_err(
            format!("builtin function '{}' does not accept type arguments", name.node),
            span,
        ));
    }

    // Check builtins first
    if env.builtins.contains(&name.node) {
        // Float unary math builtins: 1 float arg → float
        const FLOAT_UNARY_BUILTINS: &[&str] = &[
            "sqrt", "floor", "ceil", "round", "sin", "cos", "tan", "log",
        ];

        return match name.node.as_str() {
            "print" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("print() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                match arg_type {
                    PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Byte => {}
                    _ => {
                        return Err(CompileError::type_err(
                            format!("print() does not support type {arg_type}"),
                            args[0].span,
                        ));
                    }
                }
                Ok(PlutoType::Void)
            }
            "time_ns" | "gc_heap_size" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments, got {}", name.node, args.len()),
                        span,
                    ));
                }
                Ok(PlutoType::Int)
            }
            "bytes_new" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("bytes_new() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                Ok(PlutoType::Bytes)
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("abs() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let t = infer_expr(&args[0].node, args[0].span, env)?;
                match t {
                    PlutoType::Int | PlutoType::Float => Ok(t),
                    _ => Err(CompileError::type_err(
                        format!("abs() expects int or float, found {t}"),
                        args[0].span,
                    )),
                }
            }
            "min" | "max" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("{}() expects 2 arguments, got {}", name.node, args.len()),
                        span,
                    ));
                }
                let left = infer_expr(&args[0].node, args[0].span, env)?;
                let right = infer_expr(&args[1].node, args[1].span, env)?;
                if left != right {
                    return Err(CompileError::type_err(
                        format!("{}() requires matching argument types, found {left} and {right}", name.node),
                        span,
                    ));
                }
                match left {
                    PlutoType::Int | PlutoType::Float => Ok(left),
                    _ => Err(CompileError::type_err(
                        format!("{}() expects int or float arguments, found {left}", name.node),
                        span,
                    )),
                }
            }
            "pow" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("pow() expects 2 arguments, got {}", args.len()),
                        span,
                    ));
                }
                let base_ty = infer_expr(&args[0].node, args[0].span, env)?;
                let exp_ty = infer_expr(&args[1].node, args[1].span, env)?;
                if base_ty != exp_ty {
                    return Err(CompileError::type_err(
                        format!("pow() requires matching argument types, found {base_ty} and {exp_ty}"),
                        span,
                    ));
                }
                match base_ty {
                    PlutoType::Int => {
                        if let Some(current_fn) = &env.current_fn {
                            env.fallible_builtin_calls
                                .insert((current_fn.clone(), name.span.start));
                        }
                        Ok(PlutoType::Int)
                    }
                    PlutoType::Float => Ok(PlutoType::Float),
                    _ => Err(CompileError::type_err(
                        format!("pow() expects int,int or float,float, found {base_ty}"),
                        span,
                    )),
                }
            }
            n if FLOAT_UNARY_BUILTINS.contains(&n) => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("{}() expects 1 argument, got {}", name.node, args.len()),
                        span,
                    ));
                }
                let t = infer_expr(&args[0].node, args[0].span, env)?;
                if t != PlutoType::Float {
                    return Err(CompileError::type_err(
                        format!("{}() expects float, found {t}", name.node),
                        args[0].span,
                    ));
                }
                Ok(PlutoType::Float)
            }
            "expect" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("expect() takes exactly 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let inner_type = infer_expr(&args[0].node, args[0].span, env)?;
                Ok(inner_type)  // passthrough — returns the inner type directly
            }
            _ => Err(CompileError::type_err(
                format!("unknown builtin '{}'", name.node),
                name.span,
            )),
        };
    }

    // Check if calling a closure variable
    if let Some(PlutoType::Fn(param_types, ret_type)) = env.lookup(&name.node).cloned() {
        if args.len() != param_types.len() {
            return Err(CompileError::type_err(
                format!(
                    "'{}' expects {} arguments, got {}",
                    name.node,
                    param_types.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (i, (arg, expected)) in args.iter().zip(&param_types).enumerate() {
            let actual = infer_expr(&arg.node, arg.span, env)?;
            if !types_compatible(&actual, expected, env) {
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
        return Ok(*ret_type);
    }

    // Check if calling a generic function — infer or use explicit type args
    if let Some(gen_sig) = env.generic_functions.get(&name.node).cloned() {
        if args.len() != gen_sig.params.len() {
            return Err(CompileError::type_err(
                format!(
                    "function '{}' expects {} arguments, got {}",
                    name.node, gen_sig.params.len(), args.len()
                ),
                span,
            ));
        }
        let type_args: Vec<PlutoType> = if !call_type_args.is_empty() {
            // Explicit type args provided: resolve and validate count
            if call_type_args.len() != gen_sig.type_params.len() {
                return Err(CompileError::type_err(
                    format!(
                        "function '{}' expects {} type arguments, got {}",
                        name.node, gen_sig.type_params.len(), call_type_args.len()
                    ),
                    span,
                ));
            }
            // Still need to type-check the arguments
            for arg in args {
                infer_expr(&arg.node, arg.span, env)?;
            }
            call_type_args.iter()
                .map(|a| resolve_type(a, env))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            // Infer type args from arguments
            let mut arg_types = Vec::new();
            for arg in args {
                arg_types.push(infer_expr(&arg.node, arg.span, env)?);
            }
            let mut bindings = HashMap::new();
            for (param_ty, arg_ty) in gen_sig.params.iter().zip(&arg_types) {
                if !unify(param_ty, arg_ty, &mut bindings) {
                    return Err(CompileError::type_err(
                        format!("cannot infer type parameters for '{}'", name.node),
                        span,
                    ));
                }
            }
            // Check all type params are bound
            for tp in &gen_sig.type_params {
                if !bindings.contains_key(tp) {
                    return Err(CompileError::type_err(
                        format!("cannot infer type parameter '{}' for '{}'", tp, name.node),
                        span,
                    ));
                }
            }
            gen_sig.type_params.iter()
                .map(|tp| bindings[tp].clone())
                .collect()
        };
        // Validate type bounds before instantiation
        validate_type_bounds(&gen_sig.type_params, &type_args, &gen_sig.type_param_bounds, env, span, &name.node)?;
        let mangled = ensure_generic_func_instantiated(&name.node, &type_args, env);
        // Store rewrite
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        // Use the return type from the registered FuncSig — it has GenericInstance types resolved
        let concrete_ret = env.functions.get(&mangled)
            .expect("generic function should be registered after instantiation")
            .return_type.clone();
        return Ok(concrete_ret);
    }

    // Reject explicit type args on non-generic functions
    if !call_type_args.is_empty() {
        return Err(CompileError::type_err(
            format!("function '{}' is not generic and does not accept type arguments", name.node),
            span,
        ));
    }

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

    let sig_clone = sig.clone();
    for (i, (arg, expected)) in args.iter().zip(&sig_clone.params).enumerate() {
        let actual = infer_expr(&arg.node, arg.span, env)?;
        if !types_compatible(&actual, expected, env) {
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

    Ok(sig_clone.return_type)
}

fn infer_struct_lit(
    name: &Spanned<String>,
    lit_fields: &[(Spanned<String>, Spanned<Expr>)],
    type_args: &[Spanned<TypeExpr>],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let (class_info, effective_name) = if !type_args.is_empty() {
        let gen_info = env.generic_classes.get(&name.node).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown generic class '{}'", name.node),
                name.span,
            )
        })?.clone();
        if type_args.len() != gen_info.type_params.len() {
            return Err(CompileError::type_err(
                format!(
                    "class '{}' expects {} type arguments, got {}",
                    name.node, gen_info.type_params.len(), type_args.len()
                ),
                span,
            ));
        }
        let resolved_args: Vec<PlutoType> = type_args.iter()
            .map(|a| resolve_type(a, env))
            .collect::<Result<Vec<_>, _>>()?;
        // Validate type bounds
        validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, span, &name.node)?;
        let mangled = ensure_generic_class_instantiated(&name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ci = env.classes.get(&mangled)
            .expect("generic class should be registered after instantiation")
            .clone();
        (ci, mangled)
    } else {
        let ci = env.classes.get(&name.node).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown class '{}'", name.node),
                name.span,
            )
        })?.clone();
        (ci, name.node.clone())
    };

    // Block construction of classes with injected dependencies
    if class_info.fields.iter().any(|(_, _, inj)| *inj) {
        return Err(CompileError::type_err(
            format!("cannot manually construct class '{}' with injected dependencies", effective_name),
            span,
        ));
    }

    // Check all fields are provided
    if lit_fields.len() != class_info.fields.len() {
        return Err(CompileError::type_err(
            format!(
                "class '{}' has {} fields, but {} were provided",
                effective_name,
                class_info.fields.len(),
                lit_fields.len()
            ),
            span,
        ));
    }

    // Check each field matches
    for (lit_name, lit_val) in lit_fields {
        let field_type = class_info.fields.iter()
            .find(|(n, _, _)| *n == lit_name.node)
            .map(|(_, t, _)| t.clone())
            .ok_or_else(|| {
                CompileError::type_err(
                    format!("class '{}' has no field '{}'", effective_name, lit_name.node),
                    lit_name.span,
                )
            })?;
        let val_type = infer_expr(&lit_val.node, lit_val.span, env)?;

        // Check type compatibility with nullable coercion
        let types_match = if val_type == field_type {
            true
        } else if let PlutoType::Nullable(inner) = &field_type {
            // Allow T -> T? (auto-wrap)
            if **inner == val_type {
                true
            // Allow none -> T? (context inference: none infers as Nullable(Void))
            } else if let PlutoType::Nullable(void_box) = &val_type {
                matches!(**void_box, PlutoType::Void)
            } else {
                false
            }
        } else {
            false
        };

        if !types_match {
            return Err(CompileError::type_err(
                format!(
                    "field '{}': expected {field_type}, found {val_type}",
                    lit_name.node
                ),
                lit_val.span,
            ));
        }
    }

    Ok(PlutoType::Class(effective_name))
}

fn infer_enum_unit(
    enum_name: &Spanned<String>,
    variant: &Spanned<String>,
    type_args: &[Spanned<TypeExpr>],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let (enum_info, effective_name) = if !type_args.is_empty() {
        let gen_info = env.generic_enums.get(&enum_name.node).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown generic enum '{}'", enum_name.node),
                enum_name.span,
            )
        })?.clone();
        if type_args.len() != gen_info.type_params.len() {
            return Err(CompileError::type_err(
                format!(
                    "enum '{}' expects {} type arguments, got {}",
                    enum_name.node, gen_info.type_params.len(), type_args.len()
                ),
                span,
            ));
        }
        let resolved_args: Vec<PlutoType> = type_args.iter()
            .map(|a| resolve_type(a, env))
            .collect::<Result<Vec<_>, _>>()?;
        // Validate type bounds
        validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, span, &enum_name.node)?;
        let mangled = ensure_generic_enum_instantiated(&enum_name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ei = env.enums.get(&mangled)
            .expect("generic enum should be registered after instantiation")
            .clone();
        (ei, mangled)
    } else {
        let ei = match env.enums.get(&enum_name.node) {
            Some(ei) => ei.clone(),
            None => {
                // Fallback: if enum_name contains '.', it might be field access misinterpreted as enum
                // This handles cases where parser couldn't disambiguate (legacy EnumUnit nodes)
                if enum_name.node.contains('.') {
                    return try_as_nested_field_access(
                        &enum_name.node,
                        &variant.node,
                        enum_name.span,
                        variant.span,
                        env,
                    );
                }
                return Err(CompileError::type_err(
                    format!("unknown enum '{}'", enum_name.node),
                    enum_name.span,
                ));
            }
        };
        (ei, enum_name.node.clone())
    };
    let variant_info = enum_info.variants.iter().find(|(n, _)| *n == variant.node);
    match variant_info {
        None => Err(CompileError::type_err(
            format!("enum '{}' has no variant '{}'", effective_name, variant.node),
            variant.span,
        )),
        Some((_, fields)) if !fields.is_empty() => Err(CompileError::type_err(
            format!("variant '{}.{}' has fields; use {}.{} {{ ... }}", effective_name, variant.node, effective_name, variant.node),
            variant.span,
        )),
        Some(_) => Ok(PlutoType::Enum(effective_name)),
    }
}

/// Attempts to re-interpret a qualified enum name as nested field access.
/// Used when parser creates EnumUnit but type checker can't find the enum.
/// Example: "obj.inner" with variant "value" → obj.inner.value field access
fn try_as_nested_field_access(
    qualified_name: &str,
    final_field: &str,
    qualified_span: crate::span::Span,
    final_field_span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    // Split "obj.inner" into ["obj", "inner"]
    let segments: Vec<&str> = qualified_name.split('.').collect();

    if segments.is_empty() {
        return Err(CompileError::type_err(
            format!("invalid qualified name '{}'", qualified_name),
            qualified_span,
        ));
    }

    // Start with base variable
    let base_name = segments[0];
    let mut current_type = env.lookup(base_name)
        .ok_or_else(|| {
            CompileError::type_err(
                format!("unknown variable '{}'", base_name),
                qualified_span,
            )
        })?
        .clone();

    // Walk through intermediate fields
    for field_name in &segments[1..] {
        current_type = match &current_type {
            PlutoType::Class(class_name) => {
                let class_info = env.classes.get(class_name)
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("unknown class '{}'", class_name),
                            qualified_span,
                        )
                    })?;

                class_info.fields.iter()
                    .find(|(n, _, _)| n == field_name)
                    .map(|(_, t, _)| t.clone())
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("class '{}' has no field '{}'", class_name, field_name),
                            qualified_span,
                        )
                    })?
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("field access on non-class type"),
                    qualified_span,
                ));
            }
        };
    }

    // Resolve final field
    match &current_type {
        PlutoType::Class(class_name) => {
            let class_info = env.classes.get(class_name)
                .ok_or_else(|| {
                    CompileError::type_err(
                        format!("unknown class '{}'", class_name),
                        final_field_span,
                    )
                })?;

            class_info.fields.iter()
                .find(|(n, _, _)| n == final_field)
                .map(|(_, t, _)| t.clone())
                .ok_or_else(|| {
                    CompileError::type_err(
                        format!("class '{}' has no field '{}'", class_name, final_field),
                        final_field_span,
                    )
                })
        }
        _ => Err(CompileError::type_err(
            format!("field access on non-class type"),
            final_field_span,
        ))
    }
}

fn infer_enum_data(
    enum_name: &Spanned<String>,
    variant: &Spanned<String>,
    lit_fields: &[(Spanned<String>, Spanned<Expr>)],
    type_args: &[Spanned<TypeExpr>],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let (enum_info, effective_name) = if !type_args.is_empty() {
        let gen_info = env.generic_enums.get(&enum_name.node).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown generic enum '{}'", enum_name.node),
                enum_name.span,
            )
        })?.clone();
        if type_args.len() != gen_info.type_params.len() {
            return Err(CompileError::type_err(
                format!(
                    "enum '{}' expects {} type arguments, got {}",
                    enum_name.node, gen_info.type_params.len(), type_args.len()
                ),
                span,
            ));
        }
        let resolved_args: Vec<PlutoType> = type_args.iter()
            .map(|a| resolve_type(a, env))
            .collect::<Result<Vec<_>, _>>()?;
        // Validate type bounds
        validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, span, &enum_name.node)?;
        let mangled = ensure_generic_enum_instantiated(&enum_name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ei = env.enums.get(&mangled)
            .expect("generic enum should be registered after instantiation")
            .clone();
        (ei, mangled)
    } else {
        let ei = env.enums.get(&enum_name.node).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown enum '{}'", enum_name.node),
                enum_name.span,
            )
        })?.clone();
        (ei, enum_name.node.clone())
    };
    let variant_info = enum_info.variants.iter().find(|(n, _)| *n == variant.node);
    match variant_info {
        None => Err(CompileError::type_err(
            format!("enum '{}' has no variant '{}'", effective_name, variant.node),
            variant.span,
        )),
        Some((_, expected_fields)) => {
            if lit_fields.len() != expected_fields.len() {
                return Err(CompileError::type_err(
                    format!(
                        "variant '{}.{}' has {} fields, but {} were provided",
                        effective_name, variant.node, expected_fields.len(), lit_fields.len()
                    ),
                    span,
                ));
            }
            for (lit_name, lit_val) in lit_fields {
                let field_type = expected_fields.iter()
                    .find(|(n, _)| *n == lit_name.node)
                    .map(|(_, t)| t.clone())
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("variant '{}.{}' has no field '{}'", effective_name, variant.node, lit_name.node),
                            lit_name.span,
                        )
                    })?;
                let val_type = infer_expr(&lit_val.node, lit_val.span, env)?;

                // Check type compatibility with nullable coercion
                let types_match = if val_type == field_type {
                    true
                } else if let PlutoType::Nullable(inner) = &field_type {
                    // Allow T -> T? (auto-wrap)
                    if **inner == val_type {
                        true
                    // Allow none -> T? (context inference: none infers as Nullable(Void))
                    } else if let PlutoType::Nullable(void_box) = &val_type {
                        matches!(**void_box, PlutoType::Void)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if !types_match {
                    return Err(CompileError::type_err(
                        format!("field '{}': expected {field_type}, found {val_type}", lit_name.node),
                        lit_val.span,
                    ));
                }
            }
            Ok(PlutoType::Enum(effective_name))
        }
    }
}

fn infer_catch(
    expr: &Spanned<Expr>,
    handler: &CatchHandler,
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    let success_type = infer_expr(&expr.node, expr.span, env)?;
    let handler_type = match handler {
        CatchHandler::Wildcard { var, body } => {
            env.push_scope();
            env.define(var.node.clone(), PlutoType::Error);
            let stmts = &body.node.stmts;
            // Type-check all statements except possibly the last
            let return_type = env.current_fn.as_ref()
                .and_then(|name| env.functions.get(name).map(|f| f.return_type.clone()))
                .unwrap_or(PlutoType::Void);
            for (i, stmt) in stmts.iter().enumerate() {
                if i < stmts.len() - 1 {
                    super::check::check_block_stmt(&stmt.node, stmt.span, env, &return_type)?;
                }
            }
            // Determine result type from last statement
            let t = if let Some(last) = stmts.last() {
                match &last.node {
                    Stmt::Expr(e) => infer_expr(&e.node, e.span, env)?,
                    Stmt::Return(_) => {
                        super::check::check_block_stmt(&last.node, last.span, env, &return_type)?;
                        env.pop_scope();
                        // Diverging — skip compat check
                        return Ok(success_type);
                    }
                    _ => {
                        super::check::check_block_stmt(&last.node, last.span, env, &return_type)?;
                        PlutoType::Void
                    }
                }
            } else {
                PlutoType::Void
            };
            env.pop_scope();
            t
        }
        CatchHandler::Shorthand(fallback) => {
            infer_expr(&fallback.node, fallback.span, env)?
        }
    };
    if !types_compatible(&handler_type, &success_type, env) {
        return Err(CompileError::type_err(
            format!("catch handler type mismatch: expected {success_type}, found {handler_type}"),
            span,
        ));
    }
    Ok(success_type)
}

fn infer_method_call(
    object: &Spanned<Expr>,
    method: &Spanned<String>,
    args: &[Spanned<Expr>],
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    // Check for expect() intrinsic pattern
    if let Expr::Call { name, args: expect_args, .. } = &object.node && name.node == "expect" && expect_args.len() == 1 {
        let inner_type = infer_expr(&expect_args[0].node, expect_args[0].span, env)?;
        // Register as builtin method resolution
        if let Some(ref current) = env.current_fn {
            env.method_resolutions.insert(
                (current.clone(), method.span.start),
                super::env::MethodResolution::Builtin,
            );
        }
        match method.node.as_str() {
            "to_equal" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("to_equal() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                if inner_type == PlutoType::Bytes {
                    return Err(CompileError::type_err(
                        "cannot use to_equal() with bytes; compare elements individually".to_string(),
                        span,
                    ));
                }
                let expected_type = infer_expr(&args[0].node, args[0].span, env)?;
                if inner_type != expected_type {
                    return Err(CompileError::type_err(
                        format!("to_equal: expected type {expected_type} but expect() wraps {inner_type}"),
                        span,
                    ));
                }
                return Ok(PlutoType::Void);
            }
            "to_be_true" | "to_be_false" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments, got {}", method.node, args.len()),
                        span,
                    ));
                }
                if inner_type != PlutoType::Bool {
                    return Err(CompileError::type_err(
                        format!("{} requires bool, found {inner_type}", method.node),
                        span,
                    ));
                }
                return Ok(PlutoType::Void);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("unknown assertion method: {}", method.node),
                    method.span,
                ));
            }
        }
    }

    let obj_type = infer_expr(&object.node, object.span, env)?;
    if let PlutoType::Array(elem) = &obj_type {
        match method.node.as_str() {
            "len" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("len() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Int);
            }
            "push" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("push() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem {
                    return Err(CompileError::type_err(
                        format!("push(): expected {}, found {arg_type}", **elem),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "pop" | "last" | "first" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments, got {}", method.node, args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok((**elem).clone());
            }
            "is_empty" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("is_empty() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Bool);
            }
            "clear" | "reverse" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments, got {}", method.node, args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "remove_at" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("remove_at() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("remove_at(): expected int index, found {arg_type}"),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok((**elem).clone());
            }
            "insert_at" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("insert_at() expects 2 arguments, got {}", args.len()),
                        span,
                    ));
                }
                let idx_type = infer_expr(&args[0].node, args[0].span, env)?;
                if idx_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("insert_at(): expected int index, found {idx_type}"),
                        args[0].span,
                    ));
                }
                let val_type = infer_expr(&args[1].node, args[1].span, env)?;
                if val_type != **elem {
                    return Err(CompileError::type_err(
                        format!("insert_at(): expected {}, found {val_type}", **elem),
                        args[1].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "slice" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("slice() expects 2 arguments, got {}", args.len()),
                        span,
                    ));
                }
                let start_type = infer_expr(&args[0].node, args[0].span, env)?;
                if start_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("slice(): expected int start, found {start_type}"),
                        args[0].span,
                    ));
                }
                let end_type = infer_expr(&args[1].node, args[1].span, env)?;
                if end_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("slice(): expected int end, found {end_type}"),
                        args[1].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Array(elem.clone()));
            }
            "contains" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("contains() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem {
                    return Err(CompileError::type_err(
                        format!("contains(): expected {}, found {arg_type}", **elem),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Bool);
            }
            "index_of" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("index_of() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem {
                    return Err(CompileError::type_err(
                        format!("index_of(): expected {}, found {arg_type}", **elem),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Int);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("array has no method '{}'", method.node),
                    method.span,
                ));
            }
        }
    }
    // Map methods
    if let PlutoType::Map(key_ty, val_ty) = &obj_type {
        let builtin = |env: &mut TypeEnv, method: &Spanned<String>| {
            if let Some(ref current) = env.current_fn {
                env.method_resolutions.insert(
                    (current.clone(), method.span.start),
                    super::env::MethodResolution::Builtin,
                );
            }
        };
        match method.node.as_str() {
            "len" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("len() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Int);
            }
            "contains" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("contains() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **key_ty {
                    return Err(CompileError::type_err(
                        format!("contains(): expected {key_ty}, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Bool);
            }
            "insert" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err("insert() expects 2 arguments".to_string(), span));
                }
                let k = infer_expr(&args[0].node, args[0].span, env)?;
                if k != **key_ty {
                    return Err(CompileError::type_err(
                        format!("insert() key: expected {key_ty}, found {k}"), args[0].span,
                    ));
                }
                let v = infer_expr(&args[1].node, args[1].span, env)?;
                if v != **val_ty {
                    return Err(CompileError::type_err(
                        format!("insert() value: expected {val_ty}, found {v}"), args[1].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Void);
            }
            "remove" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("remove() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **key_ty {
                    return Err(CompileError::type_err(
                        format!("remove(): expected {key_ty}, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Void);
            }
            "keys" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("keys() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Array(key_ty.clone()));
            }
            "values" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("values() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Array(val_ty.clone()));
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("Map has no method '{}'", method.node), method.span,
                ));
            }
        }
    }
    // Set methods
    if let PlutoType::Set(elem_ty) = &obj_type {
        let builtin = |env: &mut TypeEnv, method: &Spanned<String>| {
            if let Some(ref current) = env.current_fn {
                env.method_resolutions.insert(
                    (current.clone(), method.span.start),
                    super::env::MethodResolution::Builtin,
                );
            }
        };
        match method.node.as_str() {
            "len" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("len() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Int);
            }
            "contains" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("contains() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem_ty {
                    return Err(CompileError::type_err(
                        format!("contains(): expected {elem_ty}, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Bool);
            }
            "insert" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("insert() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem_ty {
                    return Err(CompileError::type_err(
                        format!("insert(): expected {elem_ty}, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Void);
            }
            "remove" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("remove() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **elem_ty {
                    return Err(CompileError::type_err(
                        format!("remove(): expected {elem_ty}, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Void);
            }
            "to_array" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("to_array() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Array(elem_ty.clone()));
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("Set has no method '{}'", method.node), method.span,
                ));
            }
        }
    }
    // Task methods
    if let PlutoType::Task(_inner) = &obj_type {
        match method.node.as_str() {
            "get" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("get() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                // Determine spawned function for error tracking
                let spawned_fn = if let Expr::Ident(var) = &object.node {
                    env.lookup_task_origin(var).cloned()
                } else {
                    None // conservatively fallible for non-ident objects
                };
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::TaskGet { spawned_fn },
                    );
                }
                return Ok(*_inner.clone());
            }
            "detach" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("detach() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::TaskDetach,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "cancel" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("cancel() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::TaskCancel,
                    );
                }
                return Ok(PlutoType::Void);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("Task has no method '{}'", method.node),
                    method.span,
                ));
            }
        }
    }
    // Bytes methods
    if obj_type == PlutoType::Bytes {
        let builtin = |env: &mut TypeEnv, method: &Spanned<String>| {
            if let Some(ref current) = env.current_fn {
                env.method_resolutions.insert(
                    (current.clone(), method.span.start),
                    super::env::MethodResolution::Builtin,
                );
            }
        };
        match method.node.as_str() {
            "len" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("len() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::Int);
            }
            "push" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err("push() expects 1 argument".to_string(), span));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::Byte {
                    return Err(CompileError::type_err(
                        format!("push(): expected byte, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Void);
            }
            "to_string" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err("to_string() expects 0 arguments".to_string(), span));
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("bytes has no method '{}'", method.node), method.span,
                ));
            }
        }
    }
    // Sender methods
    if let PlutoType::Sender(inner) = &obj_type {
        match method.node.as_str() {
            "send" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("send() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **inner {
                    return Err(CompileError::type_err(
                        format!("send() expects {}, found {}", inner, arg_type),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::ChannelSend,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "try_send" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("try_send() expects 1 argument, got {}", args.len()),
                        span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != **inner {
                    return Err(CompileError::type_err(
                        format!("try_send() expects {}, found {}", inner, arg_type),
                        args[0].span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::ChannelTrySend,
                    );
                }
                return Ok(PlutoType::Void);
            }
            "close" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("close() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::Builtin,
                    );
                }
                return Ok(PlutoType::Void);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("Sender has no method '{}'", method.node),
                    method.span,
                ));
            }
        }
    }
    // Receiver methods
    if let PlutoType::Receiver(inner) = &obj_type {
        match method.node.as_str() {
            "recv" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("recv() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::ChannelRecv,
                    );
                }
                return Ok(*inner.clone());
            }
            "try_recv" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("try_recv() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                if let Some(ref current) = env.current_fn {
                    env.method_resolutions.insert(
                        (current.clone(), method.span.start),
                        super::env::MethodResolution::ChannelTryRecv,
                    );
                }
                return Ok(*inner.clone());
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("Receiver has no method '{}'", method.node),
                    method.span,
                ));
            }
        }
    }
    if obj_type == PlutoType::String {
        let builtin = |env: &mut TypeEnv, method: &Spanned<String>| {
            if let Some(ref current) = env.current_fn {
                env.method_resolutions.insert(
                    (current.clone(), method.span.start),
                    super::env::MethodResolution::Builtin,
                );
            }
        };
        match method.node.as_str() {
            "len" | "trim" | "to_upper" | "to_lower" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments", method.node), span,
                    ));
                }
                builtin(env, method);
                return Ok(match method.node.as_str() {
                    "len" => PlutoType::Int,
                    _ => PlutoType::String,
                });
            }
            "contains" | "starts_with" | "ends_with" | "index_of" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("{}() expects 1 argument", method.node), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::String {
                    return Err(CompileError::type_err(
                        format!("{}(): expected string, found {arg_type}", method.node), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(match method.node.as_str() {
                    "index_of" => PlutoType::Int,
                    _ => PlutoType::Bool,
                });
            }
            "char_at" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        "char_at() expects 1 argument".to_string(), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("char_at(): expected int, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            "byte_at" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        "byte_at() expects 1 argument".to_string(), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("byte_at(): expected int, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Int);
            }
            "substring" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        "substring() expects 2 arguments".to_string(), span,
                    ));
                }
                for arg in &args[..2] {
                    let arg_type = infer_expr(&arg.node, arg.span, env)?;
                    if arg_type != PlutoType::Int {
                        return Err(CompileError::type_err(
                            format!("substring(): expected int, found {arg_type}"), arg.span,
                        ));
                    }
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            "replace" => {
                if args.len() != 2 {
                    return Err(CompileError::type_err(
                        "replace() expects 2 arguments".to_string(), span,
                    ));
                }
                for arg in &args[..2] {
                    let arg_type = infer_expr(&arg.node, arg.span, env)?;
                    if arg_type != PlutoType::String {
                        return Err(CompileError::type_err(
                            format!("replace(): expected string, found {arg_type}"), arg.span,
                        ));
                    }
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            "split" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        "split() expects 1 argument".to_string(), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::String {
                    return Err(CompileError::type_err(
                        format!("split(): expected string, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Array(Box::new(PlutoType::String)));
            }
            "to_int" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        "to_int() expects 0 arguments".to_string(), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Nullable(Box::new(PlutoType::Int)));
            }
            "to_float" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        "to_float() expects 0 arguments".to_string(), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Nullable(Box::new(PlutoType::Float)));
            }
            "to_bytes" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        "to_bytes() expects 0 arguments".to_string(), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Bytes);
            }
            "trim_start" | "trim_end" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments", method.node), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            "repeat" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        "repeat() expects 1 argument".to_string(), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::Int {
                    return Err(CompileError::type_err(
                        format!("repeat(): expected int, found {arg_type}"), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::String);
            }
            "last_index_of" | "count" => {
                if args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("{}() expects 1 argument", method.node), span,
                    ));
                }
                let arg_type = infer_expr(&args[0].node, args[0].span, env)?;
                if arg_type != PlutoType::String {
                    return Err(CompileError::type_err(
                        format!("{}(): expected string, found {arg_type}", method.node), args[0].span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Int);
            }
            "is_empty" | "is_whitespace" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("{}() expects 0 arguments", method.node), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Bool);
            }
            _ => {
                return Err(CompileError::type_err(
                    format!("string has no method '{}'", method.node),
                    method.span,
                ));
            }
        }
    }

    // Trait method calls
    if let PlutoType::Trait(trait_name) = &obj_type {
        let trait_info = env.traits.get(trait_name).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown trait '{trait_name}'"),
                object.span,
            )
        })?.clone();
        let (_, method_sig) = trait_info.methods.iter()
            .find(|(n, _)| *n == method.node)
            .ok_or_else(|| {
                CompileError::type_err(
                    format!("trait '{trait_name}' has no method '{}'", method.node),
                    method.span,
                )
            })?;

        // Check non-self args
        let expected_args = method_sig.params[1..].to_vec();
        if args.len() != expected_args.len() {
            return Err(CompileError::type_err(
                format!(
                    "method '{}' expects {} arguments, got {}",
                    method.node,
                    expected_args.len(),
                    args.len()
                ),
                span,
            ));
        }
        for (i, (arg, expected)) in args.iter().zip(&expected_args).enumerate() {
            let actual = infer_expr(&arg.node, arg.span, env)?;
            if !types_compatible(&actual, expected, env) {
                return Err(CompileError::type_err(
                    format!(
                        "argument {} of '{}': expected {expected}, found {actual}",
                        i + 1,
                        method.node
                    ),
                    arg.span,
                ));
            }
        }
        if let Some(ref current) = env.current_fn {
            env.method_resolutions.insert(
                (current.clone(), method.span.start),
                super::env::MethodResolution::TraitDynamic {
                    trait_name: trait_name.clone(),
                    method_name: method.node.clone(),
                },
            );
        }
        // Check caller-side mutability for trait method calls
        if trait_info.mut_self_methods.contains(&method.node) && let Some(root) = super::check::root_variable(&object.node) && root != "self" && env.is_immutable(root) {
            return Err(CompileError::type_err(
                format!(
                    "cannot call mutating method '{}' on immutable variable '{}'; declare with 'let mut' to allow mutation",
                    method.node, root
                ),
                method.span,
            ));
        }
        return Ok(method_sig.return_type.clone());
    }

    let class_name = match &obj_type {
        PlutoType::Class(name) => name.clone(),
        _ => {
            return Err(CompileError::type_err(
                format!("method call on non-class type {obj_type}"),
                object.span,
            ));
        }
    };

    let mangled = mangle_method(&class_name, &method.node);
    if let Some(ref current) = env.current_fn {
        env.method_resolutions.insert(
            (current.clone(), method.span.start),
            super::env::MethodResolution::Class { mangled_name: mangled.clone() },
        );
    }
    // Check caller-side mutability: cannot call mut self method on immutable binding
    if env.mut_self_methods.contains(&mangled) && let Some(root) = super::check::root_variable(&object.node) && root != "self" && env.is_immutable(root) {
        return Err(CompileError::type_err(
            format!(
                "cannot call mutating method '{}' on immutable variable '{}'; declare with 'let mut' to allow mutation",
                method.node, root
            ),
            method.span,
        ));
    }
    let sig = env.functions.get(&mangled).ok_or_else(|| {
        CompileError::type_err(
            format!("class '{class_name}' has no method '{}'", method.node),
            method.span,
        )
    })?.clone();

    // params[0] is self, check the rest against args
    let expected_args = &sig.params[1..];
    if args.len() != expected_args.len() {
        return Err(CompileError::type_err(
            format!(
                "method '{}' expects {} arguments, got {}",
                method.node,
                expected_args.len(),
                args.len()
            ),
            span,
        ));
    }

    for (i, (arg, expected)) in args.iter().zip(expected_args).enumerate() {
        let actual = infer_expr(&arg.node, arg.span, env)?;
        if !types_compatible(&actual, expected, env) {
            return Err(CompileError::type_err(
                format!(
                    "argument {} of '{}': expected {expected}, found {actual}",
                    i + 1,
                    method.node
                ),
                arg.span,
            ));
        }
    }

    Ok(sig.return_type.clone())
}


/// Infer the type of a block (the last expression, or void)
fn infer_block_type(
    block: &crate::parser::ast::Block,
    env: &mut TypeEnv
) -> Result<PlutoType, CompileError> {
    use crate::parser::ast::{Block, Stmt};

    if block.stmts.is_empty() {
        return Ok(PlutoType::Void);
    }

    // Check all statements except last
    for stmt in &block.stmts[..block.stmts.len() - 1] {
        infer_stmt(&stmt.node, env)?;
    }

    // Last statement determines block type
    let last = &block.stmts[block.stmts.len() - 1];
    match &last.node {
        Stmt::Expr(expr) => {
            // Last statement is an expression → that's the block's value
            infer_expr(&expr.node, expr.span, env)
        }
        Stmt::If { condition, then_block, else_block: Some(else_block) } => {
            // If-statement with else clause can act as an expression
            // Check condition is bool
            let cond_type = infer_expr(&condition.node, condition.span, env)?;
            if cond_type != PlutoType::Bool {
                return Err(CompileError::type_err(
                    format!("if condition must be bool, found {cond_type}"),
                    condition.span,
                ));
            }

            // Infer type of both branches
            env.push_scope();
            let then_type = infer_block_type(&then_block.node, env)?;
            env.pop_scope();

            env.push_scope();
            let else_type = infer_block_type(&else_block.node, env)?;
            env.pop_scope();

            // Unify branch types
            unify_branch_types(&then_type, &else_type, then_block.span, else_block.span)
        }
        Stmt::Return(_) | Stmt::Break | Stmt::Continue | Stmt::Raise { .. } => {
            // Diverging statement → never returns
            Ok(PlutoType::Void)
        }
        _ => {
            // Last is a non-expression statement → block is void
            infer_stmt(&last.node, env)?;
            Ok(PlutoType::Void)
        }
    }
}

/// Helper to check statements (needed by infer_block_type)
fn infer_stmt(stmt: &crate::parser::ast::Stmt, env: &mut TypeEnv) -> Result<(), CompileError> {
    use crate::parser::ast::Stmt;

    // Simple inference for statements - we don't need full validation here
    // since type checking will validate everything later
    match stmt {
        Stmt::Let { value, .. } => {
            infer_expr(&value.node, value.span, env)?;
            Ok(())
        }
        Stmt::Expr(expr) => {
            infer_expr(&expr.node, expr.span, env)?;
            Ok(())
        }
        _ => Ok(())
    }
}

/// Unify two branch types for if-expression
fn unify_branch_types(
    t1: &PlutoType,
    t2: &PlutoType,
    _span1: crate::span::Span,
    span2: crate::span::Span,
) -> Result<PlutoType, CompileError> {
    // If both types are identical, return that type
    if t1 == t2 {
        return Ok(t1.clone());
    }

    // Handle none literal coercion: Nullable(Void) → any Nullable(T)
    if matches!(t1, PlutoType::Nullable(inner) if **inner == PlutoType::Void) {
        if matches!(t2, PlutoType::Nullable(_)) {
            return Ok(t2.clone());
        }
    }
    if matches!(t2, PlutoType::Nullable(inner) if **inner == PlutoType::Void) {
        if matches!(t1, PlutoType::Nullable(_)) {
            return Ok(t1.clone());
        }
    }

    // If one is T and other is T?, allow it (widen to T?)
    if let PlutoType::Nullable(inner2) = t2 {
        if t1 == inner2.as_ref() {
            return Ok(t2.clone());  // Widen to T?
        }
    }
    if let PlutoType::Nullable(inner1) = t1 {
        if t2 == inner1.as_ref() {
            return Ok(t1.clone());  // Widen to T?
        }
    }

    // Otherwise, types are incompatible
    Err(CompileError::type_err(
        format!(
            "if-expression branches have incompatible types: then-branch has type {}, else-branch has type {}",
            t1, t2
        ),
        span2,
    ))
}

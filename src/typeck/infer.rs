use std::collections::HashMap;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use super::env::TypeEnv;
use super::types::PlutoType;
use super::resolve::{resolve_type, unify, substitute_pluto_type, ensure_generic_func_instantiated, ensure_generic_class_instantiated, ensure_generic_enum_instantiated};
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
        Expr::Call { name, args } => infer_call(name, args, span, env),
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
        Expr::EnumUnit { enum_name, variant, type_args } => {
            infer_enum_unit(enum_name, variant, type_args, span, env)
        }
        Expr::EnumData { enum_name, variant, fields: lit_fields, type_args } => {
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
                        if let Expr::Call { name, .. } = &ret_expr.node {
                            env.spawn_target_fns.insert(
                                (span.start, span.end),
                                name.node.clone(),
                            );
                        }
                    }
                }
            }
            Ok(PlutoType::Task(Box::new(inner_type)))
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
    span: crate::span::Span,
    env: &mut TypeEnv,
) -> Result<PlutoType, CompileError> {
    // Handle old() in ensures contracts — old(expr) has the same type as expr
    if name.node == "old" && args.len() == 1 && env.in_ensures_context {
        return infer_expr(&args[0].node, args[0].span, env);
    }

    // Check builtins first
    if env.builtins.contains(&name.node) {
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
            "time_ns" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("time_ns() expects 0 arguments, got {}", args.len()),
                        span,
                    ));
                }
                Ok(PlutoType::Int)
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
            "sqrt" | "floor" | "ceil" | "round" | "sin" | "cos" | "tan" | "log" => {
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
            "gc_heap_size" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        format!("gc_heap_size() expects 0 arguments, got {}", args.len()),
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

    // Check if calling a generic function — infer type args from arguments
    if env.generic_functions.contains_key(&name.node) {
        let gen_sig = env.generic_functions.get(&name.node).unwrap().clone();
        if args.len() != gen_sig.params.len() {
            return Err(CompileError::type_err(
                format!(
                    "function '{}' expects {} arguments, got {}",
                    name.node, gen_sig.params.len(), args.len()
                ),
                span,
            ));
        }
        // Infer arg types and unify with generic params
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
        let type_args: Vec<PlutoType> = gen_sig.type_params.iter()
            .map(|tp| bindings[tp].clone())
            .collect();
        let mangled = ensure_generic_func_instantiated(&name.node, &type_args, env);
        // Store rewrite
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let concrete_ret = substitute_pluto_type(&gen_sig.return_type, &bindings);
        return Ok(concrete_ret);
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
        if !env.generic_classes.contains_key(&name.node) {
            return Err(CompileError::type_err(
                format!("unknown generic class '{}'", name.node),
                name.span,
            ));
        }
        let gen_info = env.generic_classes.get(&name.node).unwrap().clone();
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
        let mangled = ensure_generic_class_instantiated(&name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ci = env.classes.get(&mangled).unwrap().clone();
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
        if val_type != field_type {
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
        if !env.generic_enums.contains_key(&enum_name.node) {
            return Err(CompileError::type_err(
                format!("unknown generic enum '{}'", enum_name.node),
                enum_name.span,
            ));
        }
        let gen_info = env.generic_enums.get(&enum_name.node).unwrap().clone();
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
        let mangled = ensure_generic_enum_instantiated(&enum_name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ei = env.enums.get(&mangled).unwrap().clone();
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
        Some((_, fields)) if !fields.is_empty() => Err(CompileError::type_err(
            format!("variant '{}.{}' has fields; use {}.{} {{ ... }}", effective_name, variant.node, effective_name, variant.node),
            variant.span,
        )),
        Some(_) => Ok(PlutoType::Enum(effective_name)),
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
        if !env.generic_enums.contains_key(&enum_name.node) {
            return Err(CompileError::type_err(
                format!("unknown generic enum '{}'", enum_name.node),
                enum_name.span,
            ));
        }
        let gen_info = env.generic_enums.get(&enum_name.node).unwrap().clone();
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
        let mangled = ensure_generic_enum_instantiated(&enum_name.node, &resolved_args, env);
        env.generic_rewrites.insert((span.start, span.end), mangled.clone());
        let ei = env.enums.get(&mangled).unwrap().clone();
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
                if val_type != field_type {
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
        CatchHandler::Wildcard { body, .. } => {
            let CatchHandler::Wildcard { var, .. } = handler else { unreachable!() };
            env.push_scope();
            env.define(var.node.clone(), PlutoType::Error);
            let t = infer_expr(&body.node, body.span, env)?;
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
    if let Expr::Call { name, args: expect_args, .. } = &object.node {
        if name.node == "expect" && expect_args.len() == 1 {
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
    if let PlutoType::Task(inner) = &obj_type {
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
                return Ok(*inner.clone());
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
            "to_bytes" => {
                if !args.is_empty() {
                    return Err(CompileError::type_err(
                        "to_bytes() expects 0 arguments".to_string(), span,
                    ));
                }
                builtin(env, method);
                return Ok(PlutoType::Bytes);
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

    let mangled = format!("{}_{}", class_name, method.node);
    if let Some(ref current) = env.current_fn {
        env.method_resolutions.insert(
            (current.clone(), method.span.start),
            super::env::MethodResolution::Class { mangled_name: mangled.clone() },
        );
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

use std::collections::HashSet;

use crate::diagnostics::CompileError;
use crate::parser::ast::{Program, TypeExpr};
use crate::span::Span;
use crate::typeck::env::TypeEnv;
use crate::typeck::types::PlutoType;

/// Validates that all types crossing stage boundaries are serializable.
///
/// Runs after typeck (when all types are resolved), before codegen.
/// Walks all stage pub method signatures and recursively checks that
/// parameter and return types are serializable.
///
/// Non-serializable types:
/// - fn(...) — closures (can't serialize code)
/// - Task<T> — runtime handle
/// - Sender<T> / Receiver<T> — channel endpoints (runtime handles)
/// - Trait types — vtable pointers with no concrete type info
///
/// Serializable types:
/// - Primitives: int, float, bool, string, byte, bytes
/// - T? — if T is serializable
/// - [T], Map<K,V>, Set<T> — if element/key/value types are serializable
/// - Classes — if all data fields (excluding bracket deps) are serializable
/// - Enums — if all variant fields are serializable
pub fn validate_serializable_types(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    // Only validate if there are stages in the program
    if program.stages.is_empty() {
        return Ok(());
    }

    // Walk all stages
    for stage in &program.stages {
        // Walk all methods in each stage
        for method in &stage.node.methods {
            // Only check pub methods (these cross stage boundaries)
            if !method.node.is_pub {
                continue;
            }

            let method_name = &method.node.name.node;

            // Check all parameter types
            for param in &method.node.params {
                if param.name.node == "self" {
                    // Skip self parameter (it's the stage instance, not serialized)
                    continue;
                }

                let param_type = resolve_type_expr(&param.ty.node, env)?;
                if let Err(reason) = check_serializable(&param_type, env, &mut HashSet::new()) {
                    return Err(CompileError::type_err(
                        format!(
                            "parameter '{}' in stage pub method '{}' has non-serializable type: {}",
                            param.name.node, method_name, reason
                        ),
                        param.ty.span,
                    ));
                }
            }

            // Check return type
            if let Some(ref ret_type_expr) = method.node.return_type {
                let ret_type = resolve_type_expr(&ret_type_expr.node, env)?;
                if let Err(reason) = check_serializable(&ret_type, env, &mut HashSet::new()) {
                    return Err(CompileError::type_err(
                        format!(
                            "return type of stage pub method '{}' is not serializable: {}",
                            method_name, reason
                        ),
                        ret_type_expr.span,
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Recursively checks if a type is serializable.
/// Returns Ok(()) if serializable, Err(reason) if not.
fn check_serializable(
    ty: &PlutoType,
    env: &TypeEnv,
    visited: &mut HashSet<String>,
) -> Result<(), String> {
    match ty {
        // Primitives are always serializable
        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Byte | PlutoType::Bytes => Ok(()),

        // Void is serializable (for void-returning methods)
        PlutoType::Void => Ok(()),

        // Nullable is serializable if the inner type is
        PlutoType::Nullable(inner) => check_serializable(inner, env, visited),

        // Arrays are serializable if the element type is
        PlutoType::Array(elem_ty) => check_serializable(elem_ty, env, visited),

        // Maps are serializable if both key and value types are
        PlutoType::Map(key_ty, val_ty) => {
            check_serializable(key_ty, env, visited)?;
            check_serializable(val_ty, env, visited)
        }

        // Sets are serializable if the element type is
        PlutoType::Set(elem_ty) => check_serializable(elem_ty, env, visited),

        // Classes are serializable if all data fields are serializable
        // (bracket deps are excluded — they're injected, not serialized)
        PlutoType::Class(class_name) => {
            // Prevent infinite recursion on recursive types
            if !visited.insert(class_name.clone()) {
                return Ok(()); // Already checking this type, assume serializable
            }

            let class_info = env.classes.get(class_name).ok_or_else(|| {
                format!("class '{}' not found in type environment", class_name)
            })?;

            // Check all non-injected fields
            // ClassInfo.fields is Vec<(name, type, is_injected)>
            for (field_name, field_type, is_injected) in &class_info.fields {
                if *is_injected {
                    continue; // Skip bracket deps
                }
                check_serializable(field_type, env, visited).map_err(|reason| {
                    format!("field '{}' has type that is not serializable: {}", field_name, reason)
                })?;
            }

            visited.remove(class_name);
            Ok(())
        }

        // Enums are serializable if all variant fields are serializable
        PlutoType::Enum(enum_name) => {
            // Prevent infinite recursion
            if !visited.insert(enum_name.clone()) {
                return Ok(());
            }

            let enum_info = env.enums.get(enum_name).ok_or_else(|| {
                format!("enum '{}' not found in type environment", enum_name)
            })?;

            // Check all variants
            for (variant_name, variant_fields) in &enum_info.variants {
                for (field_name, field_type) in variant_fields {
                    check_serializable(field_type, env, visited).map_err(|reason| {
                        format!(
                            "variant '{}' field '{}' has type that is not serializable: {}",
                            variant_name, field_name, reason
                        )
                    })?;
                }
            }

            visited.remove(enum_name);
            Ok(())
        }

        // Non-serializable types
        PlutoType::Fn(_, _) => Err("closures cannot be serialized".to_string()),
        PlutoType::Task(_) => Err("Task<T> is a runtime handle and cannot be serialized".to_string()),
        PlutoType::Sender(_) => Err("Sender<T> is a runtime handle and cannot be serialized".to_string()),
        PlutoType::Receiver(_) => Err("Receiver<T> is a runtime handle and cannot be serialized".to_string()),
        PlutoType::Trait(_) => Err("trait types cannot be serialized (vtable pointer with no concrete type)".to_string()),

        // Stream is special — will be handled by streaming RPC (Phase 8), not marshaling
        PlutoType::Stream(_) => Err("stream types are not yet supported for marshaling (Phase 8)".to_string()),

        // Generic type parameters should have been resolved by this point
        PlutoType::TypeParam(name) => Err(format!("unresolved type parameter '{}' (this is a compiler bug)", name)),

        // Generic instances should have been monomorphized by this point
        PlutoType::GenericInstance(_, name, _) => Err(format!("unresolved generic instance '{}' (this is a compiler bug)", name)),

        // Range is not a user-facing type (only used in for loops)
        PlutoType::Range => Err("range type is not serializable (internal type)".to_string()),

        // Error types are not serializable directly (they're part of error handling, not data)
        PlutoType::Error => Err("error types cannot be serialized directly".to_string()),
    }
}

/// Resolves a TypeExpr to a PlutoType using the type environment.
fn resolve_type_expr(ty_expr: &TypeExpr, env: &TypeEnv) -> Result<PlutoType, CompileError> {
    match ty_expr {
        TypeExpr::Named(name) => {
            // Check if it's a primitive
            match name.as_str() {
                "int" => Ok(PlutoType::Int),
                "float" => Ok(PlutoType::Float),
                "bool" => Ok(PlutoType::Bool),
                "string" => Ok(PlutoType::String),
                "byte" => Ok(PlutoType::Byte),
                "void" => Ok(PlutoType::Void),
                _ => {
                    // Check if it's a class, enum, or trait
                    if env.classes.contains_key(name) {
                        Ok(PlutoType::Class(name.clone()))
                    } else if env.enums.contains_key(name) {
                        Ok(PlutoType::Enum(name.clone()))
                    } else if env.traits.contains_key(name) {
                        Ok(PlutoType::Trait(name.clone()))
                    } else {
                        Err(CompileError::type_err(
                            format!("unknown type '{}'", name),
                            Span { start: 0, end: 0, file_id: 0 }, // No span available in TypeExpr::Named
                        ))
                    }
                }
            }
        }

        TypeExpr::Array(elem_ty) => {
            let elem = resolve_type_expr(&elem_ty.node, env)?;
            Ok(PlutoType::Array(Box::new(elem)))
        }

        TypeExpr::Nullable(inner_ty) => {
            let inner = resolve_type_expr(&inner_ty.node, env)?;
            Ok(PlutoType::Nullable(Box::new(inner)))
        }

        TypeExpr::Generic { name, type_args } => {
            // Handle built-in generic types
            match name.as_str() {
                "Map" => {
                    if type_args.len() != 2 {
                        return Err(CompileError::type_err(
                            format!("Map requires 2 type arguments, got {}", type_args.len()),
                            Span { start: 0, end: 0, file_id: 0 },
                        ));
                    }
                    let key = resolve_type_expr(&type_args[0].node, env)?;
                    let val = resolve_type_expr(&type_args[1].node, env)?;
                    Ok(PlutoType::Map(Box::new(key), Box::new(val)))
                }
                "Set" => {
                    if type_args.len() != 1 {
                        return Err(CompileError::type_err(
                            format!("Set requires 1 type argument, got {}", type_args.len()),
                            Span { start: 0, end: 0, file_id: 0 },
                        ));
                    }
                    let elem = resolve_type_expr(&type_args[0].node, env)?;
                    Ok(PlutoType::Set(Box::new(elem)))
                }
                "Task" => {
                    if type_args.len() != 1 {
                        return Err(CompileError::type_err(
                            format!("Task requires 1 type argument, got {}", type_args.len()),
                            Span { start: 0, end: 0, file_id: 0 },
                        ));
                    }
                    let inner = resolve_type_expr(&type_args[0].node, env)?;
                    Ok(PlutoType::Task(Box::new(inner)))
                }
                "Sender" => {
                    if type_args.len() != 1 {
                        return Err(CompileError::type_err(
                            format!("Sender requires 1 type argument, got {}", type_args.len()),
                            Span { start: 0, end: 0, file_id: 0 },
                        ));
                    }
                    let inner = resolve_type_expr(&type_args[0].node, env)?;
                    Ok(PlutoType::Sender(Box::new(inner)))
                }
                "Receiver" => {
                    if type_args.len() != 1 {
                        return Err(CompileError::type_err(
                            format!("Receiver requires 1 type argument, got {}", type_args.len()),
                            Span { start: 0, end: 0, file_id: 0 },
                        ));
                    }
                    let inner = resolve_type_expr(&type_args[0].node, env)?;
                    Ok(PlutoType::Receiver(Box::new(inner)))
                }
                _ => {
                    // User-defined generic class/enum (should have been monomorphized)
                    Err(CompileError::type_err(
                        format!("generic type '{}' should have been monomorphized before serialization validation", name),
                        Span { start: 0, end: 0, file_id: 0 },
                    ))
                }
            }
        }

        TypeExpr::Fn { params: param_tys, return_type: ret_ty } => {
            let params: Result<Vec<_>, _> = param_tys.iter()
                .map(|p| resolve_type_expr(&p.node, env))
                .collect();
            let ret = resolve_type_expr(&ret_ty.node, env)?;
            Ok(PlutoType::Fn(params?, Box::new(ret)))
        }

        TypeExpr::Qualified { module, name } => {
            // Module-qualified types (e.g., math.Vector)
            // These should have been flattened by the module system to "math.Vector"
            // Try the flattened name
            let flattened = format!("{}.{}", module, name);
            if env.classes.contains_key(&flattened) {
                Ok(PlutoType::Class(flattened))
            } else if env.enums.contains_key(&flattened) {
                Ok(PlutoType::Enum(flattened))
            } else if env.traits.contains_key(&flattened) {
                Ok(PlutoType::Trait(flattened))
            } else {
                Err(CompileError::type_err(
                    format!("unknown qualified type '{}.{}'", module, name),
                    Span { start: 0, end: 0, file_id: 0 },
                ))
            }
        }

        TypeExpr::Stream(inner_ty) => {
            let inner = resolve_type_expr(&inner_ty.node, env)?;
            Ok(PlutoType::Stream(Box::new(inner)))
        }
    }
}

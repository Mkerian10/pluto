use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::TypeExpr;
use crate::span::Spanned;
use super::env::{self, ClassInfo, EnumInfo, FuncSig, InstKind, Instantiation, TypeEnv};
use super::types::PlutoType;

pub(crate) fn resolve_type(ty: &Spanned<TypeExpr>, env: &mut TypeEnv) -> Result<PlutoType, CompileError> {
    match &ty.node {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(PlutoType::Int),
            "float" => Ok(PlutoType::Float),
            "bool" => Ok(PlutoType::Bool),
            "string" => Ok(PlutoType::String),
            "void" => Ok(PlutoType::Void),
            "byte" => Ok(PlutoType::Byte),
            "bytes" => Ok(PlutoType::Bytes),
            _ => {
                if env.classes.contains_key(name) {
                    Ok(PlutoType::Class(name.clone()))
                } else if env.traits.contains_key(name) {
                    Ok(PlutoType::Trait(name.clone()))
                } else if env.enums.contains_key(name) {
                    Ok(PlutoType::Enum(name.clone()))
                } else {
                    Err(CompileError::type_err(
                        format!("unknown type '{name}'"),
                        ty.span,
                    ))
                }
            }
        },
        TypeExpr::Array(inner) => {
            let elem = resolve_type(inner, env)?;
            Ok(PlutoType::Array(Box::new(elem)))
        }
        TypeExpr::Qualified { module, name } => {
            // Flattening should have rewritten these, but as a fallback resolve as prefixed name
            let prefixed = format!("{}.{}", module, name);
            if env.classes.contains_key(&prefixed) {
                Ok(PlutoType::Class(prefixed))
            } else if env.traits.contains_key(&prefixed) {
                Ok(PlutoType::Trait(prefixed))
            } else if env.enums.contains_key(&prefixed) {
                Ok(PlutoType::Enum(prefixed))
            } else {
                Err(CompileError::type_err(
                    format!("unknown type '{}.{}'", module, name),
                    ty.span,
                ))
            }
        }
        TypeExpr::Fn { params, return_type } => {
            let param_types = params.iter()
                .map(|p| resolve_type(p, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = resolve_type(return_type, env)?;
            Ok(PlutoType::Fn(param_types, Box::new(ret)))
        }
        TypeExpr::Generic { name, type_args } => {
            // Resolve type args
            let resolved_args: Vec<PlutoType> = type_args.iter()
                .map(|a| resolve_type(a, env))
                .collect::<Result<Vec<_>, _>>()?;
            // Built-in generic types: Map<K,V> and Set<T>
            if name == "Map" {
                if resolved_args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("Map expects 2 type arguments, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                return Ok(PlutoType::Map(
                    Box::new(resolved_args[0].clone()),
                    Box::new(resolved_args[1].clone()),
                ));
            }
            if name == "Set" {
                if resolved_args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("Set expects 1 type argument, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                return Ok(PlutoType::Set(Box::new(resolved_args[0].clone())));
            }
            if name == "Task" {
                if resolved_args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("Task expects 1 type argument, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                return Ok(PlutoType::Task(Box::new(resolved_args[0].clone())));
            }
            // Check if already instantiated
            let mangled = env::mangle_name(name, &resolved_args);
            if env.classes.contains_key(&mangled) {
                Ok(PlutoType::Class(mangled))
            } else if env.enums.contains_key(&mangled) {
                Ok(PlutoType::Enum(mangled))
            } else if env.generic_classes.contains_key(name.as_str()) {
                let m = ensure_generic_class_instantiated(name, &resolved_args, env);
                Ok(PlutoType::Class(m))
            } else if env.generic_enums.contains_key(name.as_str()) {
                let m = ensure_generic_enum_instantiated(name, &resolved_args, env);
                Ok(PlutoType::Enum(m))
            } else {
                Err(CompileError::type_err(
                    format!("unknown generic type '{name}'"),
                    ty.span,
                ))
            }
        }
    }
}

pub(crate) fn resolve_type_with_params(
    ty: &Spanned<TypeExpr>,
    env: &mut TypeEnv,
    type_param_names: &HashSet<String>,
) -> Result<PlutoType, CompileError> {
    match &ty.node {
        TypeExpr::Named(name) if type_param_names.contains(name) => {
            Ok(PlutoType::TypeParam(name.clone()))
        }
        TypeExpr::Array(inner) => {
            let elem = resolve_type_with_params(inner, env, type_param_names)?;
            Ok(PlutoType::Array(Box::new(elem)))
        }
        TypeExpr::Fn { params, return_type } => {
            let param_types = params.iter()
                .map(|p| resolve_type_with_params(p, env, type_param_names))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = resolve_type_with_params(return_type, env, type_param_names)?;
            Ok(PlutoType::Fn(param_types, Box::new(ret)))
        }
        TypeExpr::Generic { name, type_args } if name == "Map" || name == "Set" || name == "Task" => {
            let resolved_args: Vec<PlutoType> = type_args.iter()
                .map(|a| resolve_type_with_params(a, env, type_param_names))
                .collect::<Result<Vec<_>, _>>()?;
            if name == "Map" {
                if resolved_args.len() != 2 {
                    return Err(CompileError::type_err(
                        format!("Map expects 2 type arguments, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                Ok(PlutoType::Map(Box::new(resolved_args[0].clone()), Box::new(resolved_args[1].clone())))
            } else if name == "Set" {
                if resolved_args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("Set expects 1 type argument, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                Ok(PlutoType::Set(Box::new(resolved_args[0].clone())))
            } else {
                // Task
                if resolved_args.len() != 1 {
                    return Err(CompileError::type_err(
                        format!("Task expects 1 type argument, got {}", resolved_args.len()),
                        ty.span,
                    ));
                }
                Ok(PlutoType::Task(Box::new(resolved_args[0].clone())))
            }
        }
        _ => resolve_type(ty, env),
    }
}

/// Resolve a TypeExpr to a PlutoType — thin wrapper for use by monomorphize.
pub(crate) fn resolve_type_for_monomorphize(ty: &Spanned<TypeExpr>, env: &mut TypeEnv) -> Result<PlutoType, CompileError> {
    resolve_type(ty, env)
}

pub(crate) fn substitute_pluto_type(ty: &PlutoType, bindings: &HashMap<String, PlutoType>) -> PlutoType {
    match ty {
        PlutoType::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| ty.clone()),
        PlutoType::Array(inner) => PlutoType::Array(Box::new(substitute_pluto_type(inner, bindings))),
        PlutoType::Fn(ps, r) => PlutoType::Fn(
            ps.iter().map(|p| substitute_pluto_type(p, bindings)).collect(),
            Box::new(substitute_pluto_type(r, bindings)),
        ),
        PlutoType::Map(k, v) => PlutoType::Map(
            Box::new(substitute_pluto_type(k, bindings)),
            Box::new(substitute_pluto_type(v, bindings)),
        ),
        PlutoType::Set(t) => PlutoType::Set(Box::new(substitute_pluto_type(t, bindings))),
        PlutoType::Task(t) => PlutoType::Task(Box::new(substitute_pluto_type(t, bindings))),
        _ => ty.clone(),
    }
}

pub(crate) fn unify(pattern: &PlutoType, concrete: &PlutoType, bindings: &mut HashMap<String, PlutoType>) -> bool {
    match pattern {
        PlutoType::TypeParam(name) => {
            if let Some(existing) = bindings.get(name) {
                existing == concrete
            } else {
                bindings.insert(name.clone(), concrete.clone());
                true
            }
        }
        PlutoType::Array(p_inner) => {
            if let PlutoType::Array(c_inner) = concrete {
                unify(p_inner, c_inner, bindings)
            } else {
                false
            }
        }
        PlutoType::Fn(pp, pr) => {
            if let PlutoType::Fn(cp, cr) = concrete {
                if pp.len() != cp.len() { return false; }
                for (p, c) in pp.iter().zip(cp.iter()) {
                    if !unify(p, c, bindings) { return false; }
                }
                unify(pr, cr, bindings)
            } else {
                false
            }
        }
        PlutoType::Map(pk, pv) => {
            if let PlutoType::Map(ck, cv) = concrete {
                unify(pk, ck, bindings) && unify(pv, cv, bindings)
            } else {
                false
            }
        }
        PlutoType::Set(pt) => {
            if let PlutoType::Set(ct) = concrete {
                unify(pt, ct, bindings)
            } else {
                false
            }
        }
        PlutoType::Task(pt) => {
            if let PlutoType::Task(ct) = concrete {
                unify(pt, ct, bindings)
            } else {
                false
            }
        }
        _ => pattern == concrete,
    }
}

pub(crate) fn ensure_generic_func_instantiated(
    base_name: &str,
    type_args: &[PlutoType],
    env: &mut TypeEnv,
) -> String {
    let mangled = env::mangle_name(base_name, type_args);
    if env.functions.contains_key(&mangled) {
        return mangled;
    }
    let gen_sig = env.generic_functions.get(base_name).unwrap().clone();
    let bindings: HashMap<String, PlutoType> = gen_sig.type_params.iter()
        .zip(type_args.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let concrete_params: Vec<PlutoType> = gen_sig.params.iter()
        .map(|p| substitute_pluto_type(p, &bindings))
        .collect();
    let concrete_ret = substitute_pluto_type(&gen_sig.return_type, &bindings);
    env.functions.insert(mangled.clone(), FuncSig {
        params: concrete_params,
        return_type: concrete_ret,
    });
    env.instantiations.insert(Instantiation {
        kind: InstKind::Function(base_name.to_string()),
        type_args: type_args.to_vec(),
    });
    mangled
}

pub(crate) fn ensure_generic_class_instantiated(
    base_name: &str,
    type_args: &[PlutoType],
    env: &mut TypeEnv,
) -> String {
    let mangled = env::mangle_name(base_name, type_args);
    if env.classes.contains_key(&mangled) {
        return mangled;
    }
    let gen_info = env.generic_classes.get(base_name).unwrap().clone();
    let bindings: HashMap<String, PlutoType> = gen_info.type_params.iter()
        .zip(type_args.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let concrete_fields: Vec<(String, PlutoType, bool)> = gen_info.fields.iter()
        .map(|(n, t, inj)| (n.clone(), substitute_pluto_type(t, &bindings), *inj))
        .collect();
    env.classes.insert(mangled.clone(), ClassInfo {
        fields: concrete_fields,
        methods: gen_info.methods.clone(),
        impl_traits: gen_info.impl_traits.clone(),
    });
    // Also register concrete method signatures
    // Need to substitute self type as well (it references the base class name)
    for (method_name, sig) in &gen_info.method_sigs {
        let concrete_params: Vec<PlutoType> = sig.params.iter()
            .map(|p| {
                if *p == PlutoType::Class(base_name.to_string()) {
                    PlutoType::Class(mangled.clone())
                } else {
                    substitute_pluto_type(p, &bindings)
                }
            })
            .collect();
        let concrete_ret = substitute_pluto_type(&sig.return_type, &bindings);
        let func_name = format!("{}_{}", mangled, method_name);
        env.functions.insert(func_name, env::FuncSig {
            params: concrete_params,
            return_type: concrete_ret,
        });
    }
    env.instantiations.insert(Instantiation {
        kind: InstKind::Class(base_name.to_string()),
        type_args: type_args.to_vec(),
    });
    mangled
}

pub(crate) fn ensure_generic_enum_instantiated(
    base_name: &str,
    type_args: &[PlutoType],
    env: &mut TypeEnv,
) -> String {
    let mangled = env::mangle_name(base_name, type_args);
    if env.enums.contains_key(&mangled) {
        return mangled;
    }
    let gen_info = env.generic_enums.get(base_name).unwrap().clone();
    let bindings: HashMap<String, PlutoType> = gen_info.type_params.iter()
        .zip(type_args.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let concrete_variants: Vec<(String, Vec<(String, PlutoType)>)> = gen_info.variants.iter()
        .map(|(vname, fields)| {
            let concrete_fields: Vec<(String, PlutoType)> = fields.iter()
                .map(|(fname, fty)| (fname.clone(), substitute_pluto_type(fty, &bindings)))
                .collect();
            (vname.clone(), concrete_fields)
        })
        .collect();
    env.enums.insert(mangled.clone(), EnumInfo {
        variants: concrete_variants,
    });
    env.instantiations.insert(Instantiation {
        kind: InstKind::Enum(base_name.to_string()),
        type_args: type_args.to_vec(),
    });
    mangled
}

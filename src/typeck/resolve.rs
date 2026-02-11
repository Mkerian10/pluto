use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::TypeExpr;
use crate::span::{Span, Spanned};
use super::env::{self, mangle_method, ClassInfo, EnumInfo, FuncSig, InstKind, Instantiation, TypeEnv};
use super::types::{GenericKind, PlutoType};

/// Try to resolve a built-in generic type (Map, Set, Task, Sender, Receiver).
/// Returns `Some(Ok(...))` on success, `Some(Err(...))` on arity mismatch, `None` if not a builtin.
fn resolve_builtin_generic(name: &str, resolved_args: &[PlutoType], span: Span) -> Option<Result<PlutoType, CompileError>> {
    match name {
        "Map" => {
            if resolved_args.len() != 2 {
                return Some(Err(CompileError::type_err(
                    format!("Map expects 2 type arguments, got {}", resolved_args.len()),
                    span,
                )));
            }
            Some(Ok(PlutoType::Map(
                Box::new(resolved_args[0].clone()),
                Box::new(resolved_args[1].clone()),
            )))
        }
        "Set" | "Task" | "Sender" | "Receiver" => {
            if resolved_args.len() != 1 {
                return Some(Err(CompileError::type_err(
                    format!("{name} expects 1 type argument, got {}", resolved_args.len()),
                    span,
                )));
            }
            let inner = Box::new(resolved_args[0].clone());
            let ty = match name {
                "Set" => PlutoType::Set(inner),
                "Task" => PlutoType::Task(inner),
                "Sender" => PlutoType::Sender(inner),
                _ => PlutoType::Receiver(inner),
            };
            Some(Ok(ty))
        }
        _ => None,
    }
}

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
            // Built-in generic types
            if let Some(result) = resolve_builtin_generic(name, &resolved_args, ty.span) {
                return result;
            }
            // Check if already instantiated
            let mangled = env::mangle_name(name, &resolved_args);
            if env.classes.contains_key(&mangled) {
                Ok(PlutoType::Class(mangled))
            } else if env.enums.contains_key(&mangled) {
                Ok(PlutoType::Enum(mangled))
            } else if env.generic_classes.contains_key(name.as_str()) {
                let gen_info = env.generic_classes.get(name.as_str()).unwrap().clone();
                validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, ty.span, name)?;
                let m = ensure_generic_class_instantiated(name, &resolved_args, env);
                Ok(PlutoType::Class(m))
            } else if env.generic_enums.contains_key(name.as_str()) {
                let gen_info = env.generic_enums.get(name.as_str()).unwrap().clone();
                validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, ty.span, name)?;
                let m = ensure_generic_enum_instantiated(name, &resolved_args, env);
                Ok(PlutoType::Enum(m))
            } else {
                Err(CompileError::type_err(
                    format!("unknown generic type '{name}'"),
                    ty.span,
                ))
            }
        }
        TypeExpr::Nullable(inner) => {
            let inner_type = resolve_type(inner, env)?;
            match &inner_type {
                PlutoType::Nullable(_) => Err(CompileError::type_err(
                    "nested nullable types (T??) are not allowed".to_string(),
                    ty.span,
                )),
                PlutoType::Void => Err(CompileError::type_err(
                    "void? is not allowed".to_string(),
                    ty.span,
                )),
                _ => Ok(PlutoType::Nullable(Box::new(inner_type))),
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
        TypeExpr::Generic { name, type_args } => {
            let resolved_args: Vec<PlutoType> = type_args.iter()
                .map(|a| resolve_type_with_params(a, env, type_param_names))
                .collect::<Result<Vec<_>, _>>()?;
            // Built-in generic types
            if let Some(result) = resolve_builtin_generic(name, &resolved_args, ty.span) {
                return result;
            }
            // User-defined generic types (e.g., Pair<A, B>)
            if resolved_args.iter().any(contains_type_param) {
                // Still has unresolved type params — store as GenericInstance
                // substitute_pluto_type will resolve when concrete types are bound
                if env.generic_classes.contains_key(name.as_str()) {
                    Ok(PlutoType::GenericInstance(GenericKind::Class, name.clone(), resolved_args))
                } else if env.generic_enums.contains_key(name.as_str()) {
                    Ok(PlutoType::GenericInstance(GenericKind::Enum, name.clone(), resolved_args))
                } else {
                    Err(CompileError::type_err(
                        format!("unknown generic type '{name}'"),
                        ty.span,
                    ))
                }
            } else {
                // All args are concrete — instantiate now
                let mangled = env::mangle_name(name, &resolved_args);
                if env.classes.contains_key(&mangled) {
                    Ok(PlutoType::Class(mangled))
                } else if env.enums.contains_key(&mangled) {
                    Ok(PlutoType::Enum(mangled))
                } else if env.generic_classes.contains_key(name.as_str()) {
                    let gen_info = env.generic_classes.get(name.as_str()).unwrap().clone();
                    validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, ty.span, name)?;
                    let m = ensure_generic_class_instantiated(name, &resolved_args, env);
                    Ok(PlutoType::Class(m))
                } else if env.generic_enums.contains_key(name.as_str()) {
                    let gen_info = env.generic_enums.get(name.as_str()).unwrap().clone();
                    validate_type_bounds(&gen_info.type_params, &resolved_args, &gen_info.type_param_bounds, env, ty.span, name)?;
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
        TypeExpr::Nullable(inner) => {
            let inner_type = resolve_type_with_params(inner, env, type_param_names)?;
            match &inner_type {
                PlutoType::Nullable(_) => Err(CompileError::type_err(
                    "nested nullable types (T??) are not allowed".to_string(),
                    ty.span,
                )),
                PlutoType::Void => Err(CompileError::type_err(
                    "void? is not allowed".to_string(),
                    ty.span,
                )),
                _ => Ok(PlutoType::Nullable(Box::new(inner_type))),
            }
        }
        _ => resolve_type(ty, env),
    }
}

fn contains_type_param(ty: &PlutoType) -> bool {
    matches!(ty, PlutoType::TypeParam(_)) || ty.any_inner_type(&contains_type_param)
}

/// Resolve a TypeExpr to a PlutoType — thin wrapper for use by monomorphize.
pub(crate) fn resolve_type_for_monomorphize(ty: &Spanned<TypeExpr>, env: &mut TypeEnv) -> Result<PlutoType, CompileError> {
    resolve_type(ty, env)
}

pub(crate) fn substitute_pluto_type(ty: &PlutoType, bindings: &HashMap<String, PlutoType>) -> PlutoType {
    if let PlutoType::TypeParam(name) = ty {
        return bindings.get(name).cloned().unwrap_or_else(|| ty.clone());
    }
    ty.map_inner_types(&|inner| substitute_pluto_type(inner, bindings))
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
        PlutoType::Sender(pt) => {
            if let PlutoType::Sender(ct) = concrete {
                unify(pt, ct, bindings)
            } else {
                false
            }
        }
        PlutoType::Receiver(pt) => {
            if let PlutoType::Receiver(ct) = concrete {
                unify(pt, ct, bindings)
            } else {
                false
            }
        }
        PlutoType::Nullable(p_inner) => {
            if let PlutoType::Nullable(c_inner) = concrete {
                unify(p_inner, c_inner, bindings)
            } else {
                false
            }
        }
        PlutoType::GenericInstance(pk, pn, pargs) => {
            if let PlutoType::GenericInstance(ck, cn, cargs) = concrete {
                if pk != ck || pn != cn || pargs.len() != cargs.len() { return false; }
                for (p, c) in pargs.iter().zip(cargs.iter()) {
                    if !unify(p, c, bindings) { return false; }
                }
                true
            } else {
                false
            }
        }
        _ => pattern == concrete,
    }
}

/// Walk a PlutoType and resolve any fully-concrete GenericInstance types
/// by instantiating the corresponding generic class/enum in env.
pub(crate) fn resolve_generic_instances(ty: &PlutoType, env: &mut TypeEnv) -> PlutoType {
    match ty {
        PlutoType::GenericInstance(kind, name, args) => {
            // Recursively resolve args first
            let resolved_args: Vec<PlutoType> = args.iter()
                .map(|a| resolve_generic_instances(a, env))
                .collect();
            if resolved_args.iter().any(contains_type_param) {
                PlutoType::GenericInstance(kind.clone(), name.clone(), resolved_args)
            } else {
                match kind {
                    GenericKind::Class => {
                        let m = ensure_generic_class_instantiated(name, &resolved_args, env);
                        PlutoType::Class(m)
                    }
                    GenericKind::Enum => {
                        let m = ensure_generic_enum_instantiated(name, &resolved_args, env);
                        PlutoType::Enum(m)
                    }
                }
            }
        }
        PlutoType::Array(inner) => PlutoType::Array(Box::new(resolve_generic_instances(inner, env))),
        PlutoType::Fn(ps, r) => PlutoType::Fn(
            ps.iter().map(|p| resolve_generic_instances(p, env)).collect(),
            Box::new(resolve_generic_instances(r, env)),
        ),
        PlutoType::Map(k, v) => PlutoType::Map(
            Box::new(resolve_generic_instances(k, env)),
            Box::new(resolve_generic_instances(v, env)),
        ),
        PlutoType::Set(t) => PlutoType::Set(Box::new(resolve_generic_instances(t, env))),
        PlutoType::Task(t) => PlutoType::Task(Box::new(resolve_generic_instances(t, env))),
        PlutoType::Sender(t) => PlutoType::Sender(Box::new(resolve_generic_instances(t, env))),
        PlutoType::Receiver(t) => PlutoType::Receiver(Box::new(resolve_generic_instances(t, env))),
        PlutoType::Nullable(inner) => PlutoType::Nullable(Box::new(resolve_generic_instances(inner, env))),
        _ => ty.clone(),
    }
}

/// Validate that concrete type arguments satisfy their type parameter bounds.
/// Each type parameter may have bounds like `T: Trait1 + Trait2`, meaning the
/// concrete type must be a class that implements all the required traits.
pub(crate) fn validate_type_bounds(
    type_params: &[String],
    type_args: &[PlutoType],
    bounds: &HashMap<String, Vec<String>>,
    env: &TypeEnv,
    span: crate::span::Span,
    generic_name: &str,
) -> Result<(), CompileError> {
    for (param, arg) in type_params.iter().zip(type_args.iter()) {
        if let Some(required_traits) = bounds.get(param) {
            for trait_name in required_traits {
                let satisfies = match arg {
                    PlutoType::Class(class_name) => env.class_implements_trait(class_name, trait_name),
                    _ => false,
                };
                if !satisfies {
                    return Err(CompileError::type_err(
                        format!(
                            "type {} does not satisfy bound '{}: {}' required by '{}'",
                            arg, param, trait_name, generic_name
                        ),
                        span,
                    ));
                }
            }
        }
    }
    Ok(())
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
    let gen_sig = env.generic_functions.get(base_name)
        .unwrap_or_else(|| panic!("ICE: unknown generic function '{base_name}'"))
        .clone();
    let bindings: HashMap<String, PlutoType> = gen_sig.type_params.iter()
        .zip(type_args.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let concrete_params: Vec<PlutoType> = gen_sig.params.iter()
        .map(|p| resolve_generic_instances(&substitute_pluto_type(p, &bindings), env))
        .collect();
    let concrete_ret = resolve_generic_instances(&substitute_pluto_type(&gen_sig.return_type, &bindings), env);
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
    let gen_info = env.generic_classes.get(base_name)
        .unwrap_or_else(|| panic!("ICE: unknown generic class '{base_name}'"))
        .clone();
    let bindings: HashMap<String, PlutoType> = gen_info.type_params.iter()
        .zip(type_args.iter())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let concrete_fields: Vec<(String, PlutoType, bool)> = gen_info.fields.iter()
        .map(|(n, t, inj)| (n.clone(), resolve_generic_instances(&substitute_pluto_type(t, &bindings), env), *inj))
        .collect();
    env.classes.insert(mangled.clone(), ClassInfo {
        fields: concrete_fields,
        methods: gen_info.methods.clone(),
        impl_traits: gen_info.impl_traits.clone(),
        lifecycle: gen_info.lifecycle,
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
        let func_name = mangle_method(&mangled, method_name);
        // Propagate mut self from generic class info
        if gen_info.mut_self_methods.contains(method_name) {
            env.mut_self_methods.insert(func_name.clone());
        }
        env.functions.insert(func_name, env::FuncSig {
            params: concrete_params,
            return_type: concrete_ret,
        });
    }
    // Register default trait methods for the concrete class
    for trait_name in &gen_info.impl_traits {
        if let Some(trait_info) = env.traits.get(trait_name).cloned() {
            for (method_name, trait_sig) in &trait_info.methods {
                if !gen_info.methods.contains(method_name) && trait_info.default_methods.contains(method_name) {
                    let method_mangled = mangle_method(&mangled, method_name);
                    if !env.functions.contains_key(&method_mangled) {
                        let mut params = trait_sig.params.clone();
                        // Replace the Void placeholder self param with the concrete class type
                        if !params.is_empty() {
                            params[0] = PlutoType::Class(mangled.clone());
                        }
                        env.functions.insert(
                            method_mangled.clone(),
                            env::FuncSig {
                                params,
                                return_type: trait_sig.return_type.clone(),
                            },
                        );
                        if trait_info.mut_self_methods.contains(method_name) {
                            env.mut_self_methods.insert(method_mangled);
                        }
                        // Add default method to class info
                        if let Some(info) = env.classes.get_mut(&mangled) {
                            if !info.methods.contains(method_name) {
                                info.methods.push(method_name.clone());
                            }
                        }
                    }
                }
            }
        }
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
    let gen_info = env.generic_enums.get(base_name)
        .unwrap_or_else(|| panic!("ICE: unknown generic enum '{base_name}'"))
        .clone();
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

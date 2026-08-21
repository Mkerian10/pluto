//! Skolem checking of generic templates (issue #159).
//!
//! Generic bodies used to be type-checked only at instantiation time inside
//! monomorphize — so a template that was never called was never checked at
//! all, and errors that were caught surfaced with span-offset diagnostics.
//!
//! This pass checks every generic function and class template once, before
//! monomorphization, by substituting each type parameter with a fresh opaque
//! *skolem* class (named `%T` — `%` cannot appear in user identifiers) that
//! implements exactly the parameter's declared bounds. The substituted clone
//! is then run through the ordinary concrete body checker: an error found
//! under a skolem holds for *every* instantiation (the body must be well-typed
//! for all `T`, not just the `T`s that happen to be used), and spans are the
//! original template spans.
//!
//! Side effects on the environment (skolem classes, eagerly-instantiated
//! `Box$$%T` entries, instantiation records) are deliberately left in place
//! through error inference/enforcement — method resolutions recorded here are
//! keyed under template names, which is what lets those passes see into
//! generic bodies — and are swept out by `sweep_skolems` before
//! monomorphization and reflection run.

use std::collections::HashMap;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use super::check::check_function;
use super::env::{mangle_method, ClassInfo, FuncSig, TypeEnv};
use super::resolve::{
    ensure_generic_class_instantiated, resolve_generic_instances, substitute_pluto_type,
};
use super::types::PlutoType;
use crate::monomorphize::{build_type_expr_bindings, substitute_in_function};

fn skolem_name(param: &str) -> String {
    format!("%{param}")
}

/// Register an opaque skolem class for each type parameter, implementing the
/// parameter's declared bounds (trait method signatures included, so bounded
/// method calls resolve). Returns the skolem class names for cleanup.
fn register_skolems(
    type_params: &[String],
    bounds: &HashMap<String, Vec<String>>,
    env: &mut TypeEnv,
) -> Vec<String> {
    let mut names = Vec::new();
    for tp in type_params {
        let name = skolem_name(tp);
        let tp_bounds = bounds.get(tp).cloned().unwrap_or_default();
        let mut methods = Vec::new();
        for bound in &tp_bounds {
            if let Some(trait_info) = env.traits.get(bound).cloned() {
                for (m, sig) in &trait_info.methods {
                    let mut params = sig.params.clone();
                    // Trait sigs use a Void placeholder for self
                    if !params.is_empty() && params[0] == PlutoType::Void {
                        params[0] = PlutoType::Class(name.clone());
                    }
                    let mangled = mangle_method(&name, m);
                    if trait_info.mut_self_methods.contains(m) {
                        env.mut_self_methods.insert(mangled.clone());
                    }
                    env.functions.insert(mangled, FuncSig {
                        params,
                        return_type: sig.return_type.clone(),
                    });
                    if !methods.contains(m) {
                        methods.push(m.clone());
                    }
                }
            }
        }
        env.classes.insert(name.clone(), ClassInfo {
            fields: Vec::new(),
            methods,
            impl_traits: tp_bounds,
            lifecycle: Lifecycle::Singleton,
        });
        names.push(name);
    }
    names
}

/// Remove the per-template skolem classes so the next template's parameters
/// (which may share names but carry different bounds) start fresh. Entries
/// derived from them (skolem method sigs, eager `Box$$%T` instantiations)
/// stay until `sweep_skolems`.
fn unregister_skolems(names: &[String], env: &mut TypeEnv) {
    for name in names {
        env.classes.remove(name);
    }
}

/// Check every generic function and class template under skolem substitution.
pub(crate) fn check_generic_templates(
    program: &Program,
    env: &mut TypeEnv,
) -> Result<(), CompileError> {
    for func in &program.functions {
        if func.node.type_params.is_empty() {
            continue;
        }
        check_function_template(&func.node, env)?;
    }
    for class in &program.classes {
        if class.node.type_params.is_empty() {
            continue;
        }
        check_class_template(&class.node, env)?;
    }
    Ok(())
}

fn check_function_template(func: &Function, env: &mut TypeEnv) -> Result<(), CompileError> {
    let name = &func.name.node;
    let type_params: Vec<String> = func.type_params.iter().map(|tp| tp.node.clone()).collect();
    let bounds = env
        .generic_functions
        .get(name)
        .map(|g| g.type_param_bounds.clone())
        .unwrap_or_default();

    let skolems = register_skolems(&type_params, &bounds, env);
    let skolem_args: Vec<PlutoType> = type_params
        .iter()
        .map(|tp| PlutoType::Class(skolem_name(tp)))
        .collect();
    let bindings = build_type_expr_bindings(&type_params, &skolem_args);

    // The body checker reads the expected return type from env.functions under
    // the function's own name; register a skolem-substituted signature there
    // for the duration of the check. (Keeping the template's own name also
    // makes method resolutions and fallible-builtin records land under the
    // keys error inference and enforcement use.)
    if let Some(gen_sig) = env.generic_functions.get(name).cloned() {
        let type_bindings: HashMap<String, PlutoType> = type_params
            .iter()
            .cloned()
            .zip(skolem_args.iter().cloned())
            .collect();
        let params: Vec<PlutoType> = gen_sig
            .params
            .iter()
            .map(|p| resolve_generic_instances(&substitute_pluto_type(p, &type_bindings), env))
            .collect();
        let return_type =
            resolve_generic_instances(&substitute_pluto_type(&gen_sig.return_type, &type_bindings), env);
        env.functions.insert(name.clone(), FuncSig { params, return_type });
    }

    let mut clone = func.clone();
    clone.type_params.clear();
    substitute_in_function(&mut clone, &bindings);
    let result = check_function(&clone, env, None);

    // The template must not look like a concrete function afterwards.
    env.functions.remove(name);
    unregister_skolems(&skolems, env);
    result
}

fn check_class_template(class: &ClassDecl, env: &mut TypeEnv) -> Result<(), CompileError> {
    let name = &class.name.node;
    let type_params: Vec<String> = class.type_params.iter().map(|tp| tp.node.clone()).collect();
    let bounds = env
        .generic_classes
        .get(name)
        .map(|g| g.type_param_bounds.clone())
        .unwrap_or_default();

    let skolems = register_skolems(&type_params, &bounds, env);
    let skolem_args: Vec<PlutoType> = type_params
        .iter()
        .map(|tp| PlutoType::Class(skolem_name(tp)))
        .collect();

    // Register the skolem instance (fields, method sigs, trait defaults)
    // through the ordinary eager-instantiation path. Self-referencing types in
    // method bodies (`Box<T>` inside `class Box<T>`) substitute to `Box<%T>`
    // and resolve to this same instance, so they unify with `self` naturally.
    let mangled = ensure_generic_class_instantiated(name, &skolem_args, env);
    let bindings = build_type_expr_bindings(&type_params, &skolem_args);

    let mut result = Ok(());
    for method in &class.methods {
        let mut m = method.node.clone();
        substitute_in_function(&mut m, &bindings);
        if let Err(e) = check_function(&m, env, Some(&mangled)) {
            result = Err(e);
            break;
        }
    }

    // Invariants on generic classes were previously never type-checked.
    if result.is_ok() && !class.invariants.is_empty() {
        env.push_scope();
        env.define_unchecked("self".to_string(), PlutoType::Class(mangled.clone()));
        for inv in &class.invariants {
            match super::infer::infer_expr(&inv.node.expr.node, inv.node.expr.span, env, None) {
                Ok(inv_type) => {
                    if inv_type != PlutoType::Bool {
                        result = Err(CompileError::type_err(
                            format!("invariant expression must be bool, found {inv_type}"),
                            inv.node.expr.span,
                        ));
                        break;
                    }
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        env.pop_scope();
    }

    unregister_skolems(&skolems, env);
    result
}

fn type_contains_skolem(ty: &PlutoType) -> bool {
    match ty {
        PlutoType::Class(n) | PlutoType::Enum(n) | PlutoType::Trait(n) => n.contains('%'),
        PlutoType::Array(t)
        | PlutoType::Nullable(t)
        | PlutoType::Set(t)
        | PlutoType::Task(t)
        | PlutoType::Sender(t)
        | PlutoType::Receiver(t)
        | PlutoType::Stream(t) => type_contains_skolem(t),
        PlutoType::Map(k, v) => type_contains_skolem(k) || type_contains_skolem(v),
        PlutoType::Fn(ps, r, _) => ps.iter().any(type_contains_skolem) || type_contains_skolem(r),
        PlutoType::GenericInstance(_, _, args) => args.iter().any(type_contains_skolem),
        _ => false,
    }
}

/// Remove every skolem-derived artifact from the environment before
/// monomorphization and reflection: instantiations with skolem type args must
/// not produce concrete copies, `%`-named classes/enums/functions must not get
/// reflection impls or marshalers, and skolem rewrites must not survive into
/// diagnostics surfaces.
pub(crate) fn sweep_skolems(env: &mut TypeEnv) {
    env.instantiations
        .retain(|inst| !inst.type_args.iter().any(type_contains_skolem));
    env.classes.retain(|k, _| !k.contains('%'));
    env.enums.retain(|k, _| !k.contains('%'));
    env.functions.retain(|k, _| !k.contains('%'));
    env.fn_errors.retain(|k, _| !k.contains('%'));
    env.mut_self_methods.retain(|k| !k.contains('%'));
    env.generic_rewrites.retain(|_, v| !v.contains('%'));
}

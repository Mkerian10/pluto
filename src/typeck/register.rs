use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use super::env::{self, mangle_method, ClassInfo, EnumInfo, ErrorInfo, FuncSig, GenericClassInfo, GenericEnumInfo, GenericFuncSig, TraitInfo, TypeEnv};
use super::types::PlutoType;
use super::resolve::{resolve_type, resolve_type_with_params};
use super::check::{check_function, all_paths_return};
use crate::parser::ast::ContractKind;

/// Type-check requires contracts on a function or method.
/// Push scope with params, infer each requires expr, assert bool.
fn check_function_contracts(
    func: &Function,
    env: &mut TypeEnv,
    class_name: Option<&str>,
) -> Result<(), CompileError> {
    if func.contracts.is_empty() {
        return Ok(());
    }

    // Check requires clauses
    let has_requires = func.contracts.iter().any(|c| c.node.kind == ContractKind::Requires);
    if has_requires {
        env.push_scope();
        // Define params (including self for methods)
        for p in &func.params {
            if p.name.node == "self" {
                if let Some(cn) = class_name {
                    env.define("self".to_string(), PlutoType::Class(cn.to_string()));
                }
            } else {
                let ty = resolve_type(&p.ty, env)?;
                env.define(p.name.node.clone(), ty);
            }
        }
        for contract in &func.contracts {
            if contract.node.kind == ContractKind::Requires {
                let ty = super::infer::infer_expr(&contract.node.expr.node, contract.node.expr.span, env)?;
                if ty != PlutoType::Bool {
                    return Err(CompileError::type_err(
                        format!("requires expression must be bool, found {ty}"),
                        contract.node.expr.span,
                    ));
                }
            }
        }
        env.pop_scope();
    }

    Ok(())
}

/// Pass 0: Register trait names and store raw AST type expressions for methods.
/// Does NOT resolve types yet — allows forward references to classes/enums.
pub(crate) fn register_trait_names(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for trait_decl in &program.traits {
        let t = &trait_decl.node;

        // Check for duplicate trait declarations
        if env.traits.contains_key(&t.name.node) {
            return Err(CompileError::type_err(
                format!("trait '{}' is already declared", t.name.node),
                t.name.span,
            ));
        }

        let mut default_methods = Vec::new();
        let mut mut_self_methods = HashSet::new();
        let mut static_methods = HashSet::new();
        let mut method_contracts = HashMap::new();
        let mut method_type_exprs = HashMap::new();

        for m in &t.methods {
            if m.body.is_some() {
                default_methods.push(m.name.node.clone());
            }

            // Check if method has self parameter
            let has_self = !m.params.is_empty() && m.params[0].name.node == "self";

            if has_self && m.params[0].is_mut {
                mut_self_methods.insert(m.name.node.clone());
            }

            if !has_self {
                static_methods.insert(m.name.node.clone());
            }

            if !m.contracts.is_empty() {
                method_contracts.insert(m.name.node.clone(), m.contracts.clone());
            }

            // Store raw AST type expressions for later resolution
            let param_type_exprs: Vec<Spanned<TypeExpr>> = m.params.iter()
                .filter(|p| p.name.node != "self")
                .map(|p| p.ty.clone())
                .collect();
            method_type_exprs.insert(
                m.name.node.clone(),
                (param_type_exprs, m.return_type.clone())
            );
        }

        env.traits.insert(t.name.node.clone(), TraitInfo {
            methods: Vec::new(),  // Will be populated in Pass 1
            default_methods,
            mut_self_methods,
            static_methods,
            method_contracts,
            method_type_exprs,
        });
    }
    Ok(())
}

/// Pass 1: Resolve trait method signatures now that all classes/enums are registered.
pub(crate) fn resolve_trait_signatures(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for trait_decl in &program.traits {
        let t = &trait_decl.node;
        let trait_name = &t.name.node;

        let mut methods = Vec::new();
        for m in &t.methods {
            // Trait methods can be instance methods (with self) or static methods (without self)
            let _has_self = !m.params.is_empty() && m.params[0].name.node == "self";

            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Void); // placeholder for self
                } else {
                    param_types.push(resolve_type(&p.ty, env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(rt) => resolve_type(rt, env)?,
                None => PlutoType::Void,
            };
            methods.push((m.name.node.clone(), FuncSig { params: param_types, return_type }));
        }

        // Update the TraitInfo with resolved method signatures
        if let Some(trait_info) = env.traits.get_mut(trait_name) {
            trait_info.methods = methods;
            trait_info.method_type_exprs.clear();  // No longer needed
        }
    }
    Ok(())
}

/// Pass 0: Register enum names and store raw AST type expressions for variant fields.
/// Does NOT resolve types yet — allows forward references to classes/enums.
pub(crate) fn register_enum_names(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for enum_decl in &program.enums {
        let e = &enum_decl.node;

        // Check for duplicate enum declarations
        if env.enums.contains_key(&e.name.node) || env.generic_enums.contains_key(&e.name.node) {
            return Err(CompileError::type_err(
                format!("enum '{}' is already declared", e.name.node),
                e.name.span,
            ));
        }

        // Check for cross-kind collision with traits
        if env.traits.contains_key(&e.name.node) {
            return Err(CompileError::type_err(
                format!("enum '{}' is already declared as a trait", e.name.node),
                e.name.span,
            ));
        }

        // Check for duplicate variant names within this enum
        let mut seen_variants = HashSet::new();
        for v in &e.variants {
            if !seen_variants.insert(&v.name.node) {
                return Err(CompileError::type_err(
                    format!("variant '{}' is already declared in enum '{}'", v.name.node, e.name.node),
                    v.name.span,
                ));
            }
        }

        if !e.type_params.is_empty() {
            // Check for duplicate type parameters
            let mut seen_tparams = HashSet::new();
            for tp in &e.type_params {
                if !seen_tparams.insert(&tp.node) {
                    return Err(CompileError::type_err(
                        format!("type parameter '{}' is already declared in enum '{}'", tp.node, e.name.node),
                        tp.span,
                    ));
                }
            }

            // Generic enum — register in generic_enums with TypeParam types
            // (These still need immediate resolution because TypeParam can be used)
            let tp_names: std::collections::HashSet<String> = e.type_params.iter().map(|tp| tp.node.clone()).collect();
            let mut variants = Vec::new();
            for v in &e.variants {
                let mut fields = Vec::new();
                for f in &v.fields {
                    let ty = resolve_type_with_params(&f.ty, env, &tp_names)?;
                    fields.push((f.name.node.clone(), ty));
                }
                variants.push((v.name.node.clone(), fields));
            }
            // Extract and validate type param bounds
            let enum_bounds: HashMap<String, Vec<String>> = e.type_param_bounds.iter()
                .map(|(tp, traits)| {
                    (tp.clone(), traits.iter().map(|t| t.node.clone()).collect())
                })
                .collect();
            for (tp, trait_names) in &e.type_param_bounds {
                if !tp_names.contains(tp) {
                    continue;
                }
                for trait_name in trait_names {
                    if !env.traits.contains_key(&trait_name.node) {
                        return Err(CompileError::type_err(
                            format!("unknown trait '{}' in type bound for '{}'", trait_name.node, tp),
                            trait_name.span,
                        ));
                    }
                }
            }
            env.generic_enums.insert(e.name.node.clone(), GenericEnumInfo {
                type_params: e.type_params.iter().map(|tp| tp.node.clone()).collect(),
                type_param_bounds: enum_bounds,
                variants,
            });
            continue;
        }

        // Non-generic enum: store raw AST type expressions for later resolution
        let mut variant_type_exprs = Vec::new();
        for v in &e.variants {
            let field_type_exprs: Vec<(String, Spanned<TypeExpr>)> = v.fields.iter()
                .map(|f| (f.name.node.clone(), f.ty.clone()))
                .collect();
            variant_type_exprs.push((v.name.node.clone(), field_type_exprs));
        }

        env.enums.insert(e.name.node.clone(), EnumInfo {
            variants: Vec::new(),  // Will be populated in Pass 1
            variant_type_exprs,
        });
    }
    Ok(())
}

/// Pass 1: Resolve enum variant field types now that all classes/enums are registered.
pub(crate) fn resolve_enum_fields(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for enum_decl in &program.enums {
        let e = &enum_decl.node;
        if !e.type_params.is_empty() {
            // Generic enums already resolved in register_enum_names
            continue;
        }

        let enum_name = &e.name.node;
        let mut variants = Vec::new();
        for v in &e.variants {
            let mut fields = Vec::new();
            for f in &v.fields {
                let ty = resolve_type(&f.ty, env)?;
                fields.push((f.name.node.clone(), ty));
            }
            variants.push((v.name.node.clone(), fields));
        }

        // Update the EnumInfo with resolved variants
        if let Some(enum_info) = env.enums.get_mut(enum_name) {
            enum_info.variants = variants;
            enum_info.variant_type_exprs.clear();  // No longer needed
        }
    }
    Ok(())
}

pub(crate) fn register_app_placeholder(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = app.name.node.clone();

        // Conflict check: app + top-level main
        if program.functions.iter().any(|f| f.node.name.node == "main") {
            return Err(CompileError::type_err(
                "cannot have both an app declaration and a top-level main function".to_string(),
                app_spanned.span,
            ));
        }

        // We register the app as a class so method mangling/self resolution works identically.
        // Inject fields are resolved later (after classes are registered) in a second pass.
        // For now, insert a placeholder ClassInfo with no fields.
        env.classes.insert(
            app_name.clone(),
            ClassInfo {
                fields: Vec::new(),
                methods: Vec::new(),
                impl_traits: Vec::new(),
                lifecycle: Lifecycle::Singleton,
            },
        );
    }
    Ok(())
}

pub(crate) fn register_stage_placeholders(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = stage.name.node.clone();

        // Conflict check: stage + top-level main
        if program.functions.iter().any(|f| f.node.name.node == "main") {
            return Err(CompileError::type_err(
                "cannot have both a stage declaration and a top-level main function".to_string(),
                stage_spanned.span,
            ));
        }

        // Register stage as a class so method mangling/self resolution works identically.
        env.classes.insert(
            stage_name.clone(),
            ClassInfo {
                fields: Vec::new(),
                methods: Vec::new(),
                impl_traits: Vec::new(),
                lifecycle: Lifecycle::Singleton,
            },
        );
    }
    Ok(())
}

pub(crate) fn register_errors(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for error_decl in &program.errors {
        let e = &error_decl.node;

        // Check for duplicate error declarations
        if env.errors.contains_key(&e.name.node) {
            return Err(CompileError::type_err(
                format!("error '{}' is already declared", e.name.node),
                e.name.span,
            ));
        }

        // Check for cross-kind collisions with traits and enums
        if env.traits.contains_key(&e.name.node) {
            return Err(CompileError::type_err(
                format!("error '{}' is already declared as a trait", e.name.node),
                e.name.span,
            ));
        }
        if env.enums.contains_key(&e.name.node) || env.generic_enums.contains_key(&e.name.node) {
            return Err(CompileError::type_err(
                format!("error '{}' is already declared as an enum", e.name.node),
                e.name.span,
            ));
        }

        let mut fields = Vec::new();
        for f in &e.fields {
            let ty = resolve_type(&f.ty, env)?;
            fields.push((f.name.node.clone(), ty));
        }
        env.errors.insert(e.name.node.clone(), ErrorInfo { fields });
    }
    Ok(())
}

pub(crate) fn register_class_names(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    // Build set of import binding names for collision checks
    let import_names: HashSet<&str> = program.imports.iter()
        .map(|i| i.node.binding_name())
        .collect();

    for class in &program.classes {
        let c = &class.node;

        // Check for duplicate class declarations
        if env.classes.contains_key(&c.name.node) {
            return Err(CompileError::type_err(
                format!("class '{}' is already declared", c.name.node),
                c.name.span,
            ));
        }

        // Check for cross-kind collisions with traits, enums, errors
        if env.traits.contains_key(&c.name.node) {
            return Err(CompileError::type_err(
                format!("class '{}' is already declared as a trait", c.name.node),
                c.name.span,
            ));
        }
        if env.enums.contains_key(&c.name.node) || env.generic_enums.contains_key(&c.name.node) {
            return Err(CompileError::type_err(
                format!("class '{}' is already declared as an enum", c.name.node),
                c.name.span,
            ));
        }
        if env.errors.contains_key(&c.name.node) {
            return Err(CompileError::type_err(
                format!("class '{}' is already declared as an error", c.name.node),
                c.name.span,
            ));
        }

        // Check for collision with builtin names
        if env.builtins.contains(&c.name.node) {
            return Err(CompileError::type_err(
                format!("class '{}' cannot shadow builtin '{}'", c.name.node, c.name.node),
                c.name.span,
            ));
        }

        // Check for collision with import names
        if import_names.contains(c.name.node.as_str()) {
            return Err(CompileError::type_err(
                format!("class '{}' is already declared as an imported module name", c.name.node),
                c.name.span,
            ));
        }

        if !c.type_params.is_empty() {
            // Generic class — skip concrete registration (handled in resolve_class_fields)
            continue;
        }
        env.classes.insert(
            c.name.node.clone(),
            ClassInfo {
                fields: Vec::new(),
                methods: Vec::new(),
                impl_traits: Vec::new(),
                lifecycle: c.lifecycle,
            },
        );
    }
    Ok(())
}

pub(crate) fn resolve_class_fields(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() {
            // Generic class — register in generic_classes

            // Check for duplicate generic class declarations
            if env.generic_classes.contains_key(&c.name.node) {
                return Err(CompileError::type_err(
                    format!("class '{}' is already declared", c.name.node),
                    c.name.span,
                ));
            }

            // Check for duplicate type parameters
            let mut seen_tparams = HashSet::new();
            for tp in &c.type_params {
                if !seen_tparams.insert(&tp.node) {
                    return Err(CompileError::type_err(
                        format!("type parameter '{}' is already declared in class '{}'", tp.node, c.name.node),
                        tp.span,
                    ));
                }
            }

            // Validate trait names for generic classes
            for trait_name in &c.impl_traits {
                if !env.traits.contains_key(&trait_name.node) {
                    return Err(CompileError::type_err(
                        format!("unknown trait '{}'", trait_name.node),
                        trait_name.span,
                    ));
                }
            }
            // Check for duplicate field names
            let mut seen_fields = HashSet::new();
            for f in &c.fields {
                if !seen_fields.insert(&f.name.node) {
                    return Err(CompileError::type_err(
                        format!("duplicate field '{}' in class '{}'", f.name.node, c.name.node),
                        f.name.span,
                    ));
                }
            }
            // Check for duplicate method names
            let mut seen_methods = HashSet::new();
            for m in &c.methods {
                if !seen_methods.insert(&m.node.name.node) {
                    return Err(CompileError::type_err(
                        format!("duplicate method '{}' in class '{}'", m.node.name.node, c.name.node),
                        m.node.name.span,
                    ));
                }
            }
            let tp_names: std::collections::HashSet<String> = c.type_params.iter().map(|tp| tp.node.clone()).collect();
            // Extract and validate type param bounds
            let bounds: HashMap<String, Vec<String>> = c.type_param_bounds.iter()
                .map(|(tp, traits)| {
                    (tp.clone(), traits.iter().map(|t| t.node.clone()).collect())
                })
                .collect();
            for (tp, trait_names) in &c.type_param_bounds {
                if !tp_names.contains(tp) {
                    continue;
                }
                for trait_name in trait_names {
                    if !env.traits.contains_key(&trait_name.node) {
                        return Err(CompileError::type_err(
                            format!("unknown trait '{}' in type bound for '{}'", trait_name.node, tp),
                            trait_name.span,
                        ));
                    }
                }
            }
            let mut fields = Vec::new();
            for f in &c.fields {
                let ty = resolve_type_with_params(&f.ty, env, &tp_names)?;
                fields.push((f.name.node.clone(), ty, f.is_injected));
            }
            let method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();
            // Build method signatures with TypeParam types
            let mut method_sigs = HashMap::new();
            for m in &c.methods {
                let mut param_types = Vec::new();
                for p in &m.node.params {
                    if p.name.node == "self" {
                        // self will be substituted to the concrete class type during instantiation
                        param_types.push(PlutoType::Class(c.name.node.clone()));
                    } else {
                        param_types.push(resolve_type_with_params(&p.ty, env, &tp_names)?);
                    }
                }
                let return_type = match &m.node.return_type {
                    Some(t) => resolve_type_with_params(t, env, &tp_names)?,
                    None => PlutoType::Void,
                };
                method_sigs.insert(m.node.name.node.clone(), env::FuncSig {
                    params: param_types,
                    return_type,
                });
            }
            let mut generic_mut_self = HashSet::new();
            for m in &c.methods {
                if !m.node.params.is_empty() && m.node.params[0].name.node == "self" && m.node.params[0].is_mut {
                    generic_mut_self.insert(m.node.name.node.clone());
                }
            }
            env.generic_classes.insert(c.name.node.clone(), GenericClassInfo {
                type_params: c.type_params.iter().map(|tp| tp.node.clone()).collect(),
                type_param_bounds: bounds,
                fields,
                methods: method_names,
                method_sigs,
                impl_traits: c.impl_traits.iter().map(|t| t.node.clone()).collect(),
                mut_self_methods: generic_mut_self,
                lifecycle: c.lifecycle,
            });
            continue;
        }
        // Check for duplicate field names
        let mut seen_fields = HashSet::new();
        for f in &c.fields {
            if !seen_fields.insert(&f.name.node) {
                return Err(CompileError::type_err(
                    format!("duplicate field '{}' in class '{}'", f.name.node, c.name.node),
                    f.name.span,
                ));
            }
        }
        // Check for duplicate method names
        let mut seen_methods = HashSet::new();
        for m in &c.methods {
            if !seen_methods.insert(&m.node.name.node) {
                return Err(CompileError::type_err(
                    format!("duplicate method '{}' in class '{}'", m.node.name.node, c.name.node),
                    m.node.name.span,
                ));
            }
        }
        let mut fields = Vec::new();
        for f in &c.fields {
            let ty = resolve_type(&f.ty, env)?;
            fields.push((f.name.node.clone(), ty, f.is_injected));
        }

        // Validate trait names
        let mut impl_trait_names = Vec::new();
        for trait_name in &c.impl_traits {
            if !env.traits.contains_key(&trait_name.node) {
                return Err(CompileError::type_err(
                    format!("unknown trait '{}'", trait_name.node),
                    trait_name.span,
                ));
            }
            impl_trait_names.push(trait_name.node.clone());
        }

        if let Some(info) = env.classes.get_mut(&c.name.node) {
            info.fields = fields;
            info.impl_traits = impl_trait_names;
        }
    }
    Ok(())
}

pub(crate) fn register_extern_fns(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for ext in &program.extern_fns {
        let e = &ext.node;

        // Validate only primitive types allowed
        let mut param_types = Vec::new();
        for p in &e.params {
            let ty = resolve_type(&p.ty, env)?;
            match &ty {
                PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Void | PlutoType::Array(_) => {}
                _ => {
                    return Err(CompileError::type_err(
                        format!("extern functions only support primitive types and arrays (int, float, bool, string, array), got '{}'", ty),
                        p.ty.span,
                    ));
                }
            }
            param_types.push(ty);
        }

        let return_type = match &e.return_type {
            Some(t) => {
                let ty = resolve_type(t, env)?;
                match &ty {
                    PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Void | PlutoType::Array(_) => {}
                    _ => {
                        return Err(CompileError::type_err(
                            format!("extern functions only support primitive types and arrays (int, float, bool, string, array), got '{}'", ty),
                            t.span,
                        ));
                    }
                }
                ty
            }
            None => PlutoType::Void,
        };

        env.functions.insert(
            e.name.node.clone(),
            FuncSig { params: param_types, return_type },
        );
        env.extern_fns.insert(e.name.node.clone());
    }
    Ok(())
}

pub(crate) fn register_functions(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for func in &program.functions {
        let f = &func.node;

        if env.builtins.contains(&f.name.node) {
            return Err(CompileError::type_err(
                format!("function '{}' cannot shadow builtin '{}'", f.name.node, f.name.node),
                f.name.span,
            ));
        }

        // Check for conflict with extern fn
        if env.extern_fns.contains(&f.name.node) {
            return Err(CompileError::type_err(
                format!("duplicate function name '{}': defined as both fn and extern fn", f.name.node),
                f.name.span,
            ));
        }

        // Check for duplicate function declarations
        if env.functions.contains_key(&f.name.node) || env.generic_functions.contains_key(&f.name.node) {
            return Err(CompileError::type_err(
                format!("function '{}' is already declared", f.name.node),
                f.name.span,
            ));
        }

        // Check for cross-kind collision with classes
        if env.classes.contains_key(&f.name.node) || env.generic_classes.contains_key(&f.name.node) {
            return Err(CompileError::type_err(
                format!("function '{}' is already declared as a class", f.name.node),
                f.name.span,
            ));
        }

        // Check for duplicate parameters
        let mut seen_params: HashSet<&str> = HashSet::new();
        for p in &f.params {
            if !seen_params.insert(&p.name.node) {
                return Err(CompileError::type_err(
                    format!("parameter '{}' is already declared in function '{}'", p.name.node, f.name.node),
                    p.name.span,
                ));
            }
        }

        if !f.type_params.is_empty() {
            // Check for duplicate type parameters
            let mut seen_tparams: HashSet<&str> = HashSet::new();
            for tp in &f.type_params {
                if !seen_tparams.insert(&tp.node) {
                    return Err(CompileError::type_err(
                        format!("type parameter '{}' is already declared in function '{}'", tp.node, f.name.node),
                        tp.span,
                    ));
                }
                // Check that type parameter doesn't shadow a class name
                if env.classes.contains_key(&tp.node) || env.generic_classes.contains_key(&tp.node) {
                    return Err(CompileError::type_err(
                        format!("type parameter '{}' in function '{}' shadows class '{}'", tp.node, f.name.node, tp.node),
                        tp.span,
                    ));
                }
            }

            // Generic function — register in generic_functions with TypeParam types
            let tp_names: std::collections::HashSet<String> = f.type_params.iter().map(|tp| tp.node.clone()).collect();
            let mut param_types = Vec::new();
            for p in &f.params {
                param_types.push(resolve_type_with_params(&p.ty, env, &tp_names)?);
            }
            let return_type = match &f.return_type {
                Some(t) => resolve_type_with_params(t, env, &tp_names)?,
                None => PlutoType::Void,
            };

            // Verify non-void generic functions have a return or raise on every control flow path
            if !matches!(return_type, PlutoType::Void) && !all_paths_return(&f.body.node) {
                return Err(CompileError::type_err(
                    format!("missing return statement in function with return type {}", return_type),
                    f.body.span,
                ));
            }

            // Extract and validate type param bounds
            let bounds: HashMap<String, Vec<String>> = f.type_param_bounds.iter()
                .map(|(tp, traits)| {
                    (tp.clone(), traits.iter().map(|t| t.node.clone()).collect())
                })
                .collect();
            for (tp, trait_names) in &f.type_param_bounds {
                if !tp_names.contains(tp) {
                    continue; // shouldn't happen from parser, but defensive
                }
                for trait_name in trait_names {
                    if !env.traits.contains_key(&trait_name.node) {
                        return Err(CompileError::type_err(
                            format!("unknown trait '{}' in type bound for '{}'", trait_name.node, tp),
                            trait_name.span,
                        ));
                    }
                }
            }
            env.generic_functions.insert(f.name.node.clone(), GenericFuncSig {
                type_params: f.type_params.iter().map(|tp| tp.node.clone()).collect(),
                type_param_bounds: bounds,
                params: param_types,
                return_type,
            });
            continue;
        }

        let mut param_types = Vec::new();
        for p in &f.params {
            param_types.push(resolve_type(&p.ty, env)?);
        }
        let return_type = match &f.return_type {
            Some(t) => resolve_type(t, env)?,
            None => PlutoType::Void,
        };
        if matches!(&return_type, PlutoType::Stream(_)) {
            env.generators.insert(f.name.node.clone());
        }
        env.functions.insert(
            f.name.node.clone(),
            FuncSig { params: param_types, return_type },
        );
    }
    Ok(())
}

pub(crate) fn register_method_sigs(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() { continue; } // Skip generic classes
        let class_name = &c.name.node;
        let mut method_names = Vec::new();
        for method in &c.methods {
            let m = &method.node;
            let mangled = mangle_method(class_name, &m.name.node);
            method_names.push(m.name.node.clone());

            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Class(class_name.clone()));
                } else {
                    param_types.push(resolve_type(&p.ty, env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(t) => resolve_type(t, env)?,
                None => PlutoType::Void,
            };
            // Track mut self methods
            if !m.params.is_empty() && m.params[0].name.node == "self" && m.params[0].is_mut {
                env.mut_self_methods.insert(mangled.clone());
            }
            env.functions.insert(
                mangled,
                FuncSig { params: param_types, return_type },
            );
        }
        // Update the ClassInfo with method names
        if let Some(info) = env.classes.get_mut(class_name) {
            info.methods = method_names;
        }
    }
    Ok(())
}

pub(crate) fn register_app_fields_and_methods(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = app.name.node.clone();

        // Resolve inject field types
        let mut fields = Vec::new();
        for f in &app.inject_fields {
            let ty = resolve_type(&f.ty, env)?;
            fields.push((f.name.node.clone(), ty, f.is_injected));
        }

        // Update the ClassInfo with resolved fields
        if let Some(info) = env.classes.get_mut(&app_name) {
            info.fields = fields.clone();
        }

        // Store in env.app
        env.app = Some((app_name.clone(), ClassInfo {
            fields: fields.clone(),
            methods: Vec::new(),
            impl_traits: Vec::new(),
            lifecycle: Lifecycle::Singleton,
        }));

        // Populate ambient_types and validate each is a known class
        for ambient_type in &app.ambient_types {
            if !env.classes.contains_key(&ambient_type.node) {
                return Err(CompileError::type_err(
                    format!("ambient type '{}' is not a known class", ambient_type.node),
                    ambient_type.span,
                ));
            }
            env.ambient_types.insert(ambient_type.node.clone());
        }

        // Register app methods (mangled as AppName_methodname)
        let mut method_names = Vec::new();
        let mut has_main = false;
        for method in &app.methods {
            let m = &method.node;
            let mangled = mangle_method(&app_name, &m.name.node);
            method_names.push(m.name.node.clone());

            if m.name.node == "main" {
                has_main = true;
                // Verify main has self as first param
                if m.params.is_empty() || m.params[0].name.node != "self" {
                    return Err(CompileError::type_err(
                        "app main method must take 'self' as first parameter".to_string(),
                        m.name.span,
                    ));
                }
                // Verify main returns void (no return type annotation)
                if m.return_type.is_some() {
                    return Err(CompileError::type_err(
                        "app main method must not have a return type".to_string(),
                        m.name.span,
                    ));
                }
            }

            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Class(app_name.clone()));
                } else {
                    param_types.push(resolve_type(&p.ty, env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(t) => resolve_type(t, env)?,
                None => PlutoType::Void,
            };
            // Track mut self methods
            if !m.params.is_empty() && m.params[0].name.node == "self" && m.params[0].is_mut {
                env.mut_self_methods.insert(mangled.clone());
            }
            env.functions.insert(
                mangled,
                FuncSig { params: param_types, return_type },
            );
        }

        if !has_main {
            return Err(CompileError::type_err(
                "app must have a 'main' method".to_string(),
                app.name.span,
            ));
        }

        // Update class info with method names
        if let Some(info) = env.classes.get_mut(&app_name) {
            info.methods = method_names.clone();
        }
        // Also update env.app
        if let Some((_, ref mut app_info)) = env.app {
            app_info.methods = method_names;
        }
    }
    Ok(())
}

pub(crate) fn register_stage_fields_and_methods(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = stage.name.node.clone();

        // Resolve inject field types
        let mut fields = Vec::new();
        for f in &stage.inject_fields {
            let ty = resolve_type(&f.ty, env)?;
            fields.push((f.name.node.clone(), ty, f.is_injected));
        }

        // Update the ClassInfo with resolved fields
        if let Some(info) = env.classes.get_mut(&stage_name) {
            info.fields = fields.clone();
        }

        // Populate ambient_types and validate each is a known class
        for ambient_type in &stage.ambient_types {
            if !env.classes.contains_key(&ambient_type.node) {
                return Err(CompileError::type_err(
                    format!("ambient type '{}' is not a known class", ambient_type.node),
                    ambient_type.span,
                ));
            }
            env.ambient_types.insert(ambient_type.node.clone());
        }

        // Register stage methods (mangled as StageName_methodname)
        let mut method_names = Vec::new();
        let mut has_main = false;
        for method in &stage.methods {
            let m = &method.node;
            let mangled = mangle_method(&stage_name, &m.name.node);
            method_names.push(m.name.node.clone());

            if m.name.node == "main" {
                has_main = true;
                if m.params.is_empty() || m.params[0].name.node != "self" {
                    return Err(CompileError::type_err(
                        "stage main method must take 'self' as first parameter".to_string(),
                        m.name.span,
                    ));
                }
                if m.return_type.is_some() {
                    return Err(CompileError::type_err(
                        "stage main method must not have a return type".to_string(),
                        m.name.span,
                    ));
                }
            }

            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Class(stage_name.clone()));
                } else {
                    param_types.push(resolve_type(&p.ty, env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(t) => resolve_type(t, env)?,
                None => PlutoType::Void,
            };
            if !m.params.is_empty() && m.params[0].name.node == "self" && m.params[0].is_mut {
                env.mut_self_methods.insert(mangled.clone());
            }
            env.functions.insert(
                mangled,
                FuncSig { params: param_types, return_type },
            );
        }

        if !has_main {
            return Err(CompileError::type_err(
                "stage must have a 'main' method".to_string(),
                stage.name.span,
            ));
        }

        // Update class info with method names
        if let Some(info) = env.classes.get_mut(&stage_name) {
            info.methods = method_names.clone();
        }

        // Store in env.stages
        env.stages.push((stage_name.clone(), ClassInfo {
            fields: fields.clone(),
            methods: method_names,
            impl_traits: Vec::new(),
            lifecycle: Lifecycle::Singleton,
        }));
    }
    Ok(())
}

pub(crate) fn validate_di_graph(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    use std::collections::{HashMap as DMap, HashSet as DSet, VecDeque};

    // Validate `uses` on classes — each used type must be declared `ambient` in the app or stage
    for class in &program.classes {
        let c = &class.node;
        if c.uses.is_empty() {
            continue;
        }
        if program.app.is_none() && program.stages.is_empty() {
            return Err(CompileError::type_err(
                format!("class '{}' uses ambient types, but no app or stage declaration exists", c.name.node),
                class.span,
            ));
        }
        for used_type in &c.uses {
            if !env.ambient_types.contains(&used_type.node) {
                return Err(CompileError::type_err(
                    format!("class '{}' uses ambient type '{}', but '{}' is not declared ambient in the app",
                        c.name.node, used_type.node, used_type.node),
                    used_type.span,
                ));
            }
        }
    }

    // Build adjacency: class -> list of dep types
    let mut graph: DMap<String, Vec<String>> = DMap::new();
    let mut all_di_classes = DSet::new();

    // Collect app and stage names to exclude from DI graph
    let mut excluded_names = DSet::new();
    if let Some(app_spanned) = &program.app {
        excluded_names.insert(app_spanned.node.name.node.clone());
    }
    for stage_spanned in &program.stages {
        excluded_names.insert(stage_spanned.node.name.node.clone());
    }

    for (class_name, class_info) in &env.classes {
        // Skip app and stages - they're top-level containers, not DI dependencies
        if excluded_names.contains(class_name) {
            continue;
        }

        let deps: Vec<String> = class_info.fields.iter()
            .filter(|(_, _, inj)| *inj)
            .map(|(_, ty, _)| {
                match ty {
                    PlutoType::Class(name) => name.clone(),
                    _ => String::new(),
                }
            })
            .filter(|n| !n.is_empty())
            .collect();
        if !deps.is_empty() {
            all_di_classes.insert(class_name.clone());
            for d in &deps {
                all_di_classes.insert(d.clone());
            }
            graph.insert(class_name.clone(), deps);
        }
    }

    // Add app dependencies to the graph
    if let Some(app_spanned) = &program.app {
        for field in &app_spanned.node.inject_fields {
            if let crate::parser::ast::TypeExpr::Named(ref type_name) = field.ty.node {
                all_di_classes.insert(type_name.clone());
            }
        }
    }

    // Add stage dependencies to the graph
    for stage_spanned in &program.stages {
        for field in &stage_spanned.node.inject_fields {
            if let crate::parser::ast::TypeExpr::Named(ref type_name) = field.ty.node {
                all_di_classes.insert(type_name.clone());
            }
        }
    }

    // Also add classes that are deps but have no deps themselves
    for c in &all_di_classes {
        graph.entry(c.clone()).or_default();
    }

    // Verify all injected types are known classes
    for (class_name, deps) in &graph {
        for dep in deps {
            if !env.classes.contains_key(dep) {
                // Find the span for better error reporting
                let span = if let Some(app_spanned) = &program.app {
                    if app_spanned.node.name.node == *class_name {
                        app_spanned.span
                    } else {
                        program.classes.iter()
                            .find(|c| c.node.name.node == *class_name)
                            .map(|c| c.span)
                            .unwrap_or(app_spanned.span)
                    }
                } else {
                    program.classes.iter()
                        .find(|c| c.node.name.node == *class_name)
                        .map(|c| c.span)
                        .unwrap_or(crate::span::Span { start: 0, end: 0, file_id: 0 })
                };
                return Err(CompileError::type_err(
                    format!(
                        "injected dependency '{}' in class '{}' is not a known class; \
                         check spelling or ensure '{}' is declared with pub visibility if imported",
                        dep, class_name, dep
                    ),
                    span,
                ));
            }
        }
    }

    // Topological sort (Kahn's algorithm)
    if !all_di_classes.is_empty() {
        let mut in_degree: DMap<String, usize> = DMap::new();
        for c in &all_di_classes {
            in_degree.insert(c.clone(), 0);
        }
        for deps in graph.values() {
            for dep in deps {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Note: in_degree counts how many classes DEPEND ON this class,
        // but for topological sort we want "dependents" direction.
        // Actually, let's redo: edge A->B means A depends on B.
        // For topo sort (creation order), B must be created before A.
        // in_degree[X] = number of classes X depends on (graph[X].len())
        let mut in_degree2: DMap<String, usize> = DMap::new();
        for c in &all_di_classes {
            in_degree2.insert(c.clone(), graph.get(c).map_or(0, |v| v.len()));
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for (c, deg) in &in_degree2 {
            if *deg == 0 {
                queue.push_back(c.clone());
            }
        }

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            // For each class that depends on `node`, decrement its in_degree
            for (class, deps) in &graph {
                if deps.contains(&node) && let Some(deg) = in_degree2.get_mut(class) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(class.clone());
                    }
                }
            }
        }

        if order.len() != all_di_classes.len() {
            // Cycle detected — find the cycle for the error message
            let remaining: Vec<String> = all_di_classes.iter()
                .filter(|c| !order.contains(c))
                .cloned()
                .collect();
            let cycle_str = remaining.join(" -> ");
            let span = program.app.as_ref()
                .map(|a| a.span)
                .or_else(|| program.stages.first().map(|s| s.span))
                .or_else(|| program.classes.first().map(|c| c.span))
                .unwrap_or(crate::span::Span { start: 0, end: 0, file_id: 0 });
            return Err(CompileError::type_err(
                format!("circular dependency detected: {}", cycle_str),
                span,
            ));
        }

        // Lifecycle inference: propagate scoped/transient upward through dependency graph.
        // A class's inferred lifecycle = min(its declared lifecycle, min of dep lifecycles)
        // where Transient < Scoped < Singleton.
        // Process in topological order so deps are resolved before dependents.
        for class_name in &order {
            let deps = graph.get(class_name).cloned().unwrap_or_default();
            let mut inferred = env.classes.get(class_name)
                .map(|ci| ci.lifecycle)
                .unwrap_or(Lifecycle::Singleton);

            for dep_name in &deps {
                if let Some(dep_info) = env.classes.get(dep_name) {
                    inferred = min_lifecycle(inferred, dep_info.lifecycle);
                }
            }

            if let Some(info) = env.classes.get_mut(class_name) {
                info.lifecycle = inferred;
            }
        }

        // Filter out the app/stage names from di_order (app/stage are wired separately)
        let app_name_opt = env.app.as_ref().map(|(n, _)| n.clone());
        let stage_names: HashSet<String> = env.stages.iter().map(|(n, _)| n.clone()).collect();
        env.di_order = order.into_iter()
            .filter(|n| Some(n) != app_name_opt.as_ref() && !stage_names.contains(n))
            .collect();
    }

    // Apply app-level lifecycle overrides (runs even if no DI classes in graph)
    if let Some(app_spanned) = &program.app {
        for (class_name, target_lifecycle) in &app_spanned.node.lifecycle_overrides {
            // Verify class exists
            let class_info = env.classes.get(&class_name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("lifecycle override: unknown class '{}'", class_name.node),
                    class_name.span,
                )
            })?;

            // Verify shortening only (Singleton→Scoped OK, Scoped→Transient OK; reverse is error)
            let current = class_info.lifecycle;
            let lifecycle_rank = |l: Lifecycle| -> u8 {
                match l {
                    Lifecycle::Transient => 0,
                    Lifecycle::Scoped => 1,
                    Lifecycle::Singleton => 2,
                }
            };
            if lifecycle_rank(*target_lifecycle) > lifecycle_rank(current) {
                return Err(CompileError::type_err(
                    format!(
                        "lifecycle override: cannot lengthen lifecycle of '{}' from {} to {}; \
                         overrides can only shorten lifecycle (singleton -> scoped -> transient)",
                        class_name.node, current, *target_lifecycle
                    ),
                    class_name.span,
                ));
            }

            // Apply the override
            if let Some(info) = env.classes.get_mut(&class_name.node) {
                info.lifecycle = *target_lifecycle;
            }
            env.lifecycle_overridden.insert(class_name.node.clone());
        }

        // Re-run lifecycle inference to propagate overrides to dependents
        let di_order_snapshot = env.di_order.clone();
        for class_name in &di_order_snapshot {
            if let Some(class_info) = env.classes.get(class_name) {
                let deps: Vec<String> = class_info.fields.iter()
                    .filter(|(_, _, inj)| *inj)
                    .filter_map(|(_, ty, _)| {
                        if let PlutoType::Class(name) = ty { Some(name.clone()) } else { None }
                    })
                    .collect();
                let mut inferred = class_info.lifecycle;
                for dep_name in &deps {
                    if let Some(dep_info) = env.classes.get(dep_name) {
                        inferred = min_lifecycle(inferred, dep_info.lifecycle);
                    }
                }
                if let Some(info) = env.classes.get_mut(class_name) {
                    if inferred != info.lifecycle {
                        info.lifecycle = inferred;
                        env.lifecycle_overridden.insert(class_name.clone());
                    }
                }
            }
        }

        // Validate app bracket deps don't reference overridden classes
        for field in &app_spanned.node.inject_fields {
            if let crate::parser::ast::TypeExpr::Named(ref type_name) = field.ty.node {
                if env.lifecycle_overridden.contains(type_name) {
                    return Err(CompileError::type_err(
                        format!(
                            "app bracket dependency '{}' has overridden lifecycle; use scope blocks to access scoped/transient instances",
                            field.name.node
                        ),
                        field.ty.span,
                    ));
                }
            }
        }

        // Remove overridden classes from di_order
        env.di_order.retain(|n| !env.lifecycle_overridden.contains(n));
    }

    // Apply stage-level lifecycle overrides (parallel to app lifecycle overrides)
    for stage_spanned in &program.stages {
        for (class_name, target_lifecycle) in &stage_spanned.node.lifecycle_overrides {
            let class_info = env.classes.get(&class_name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("lifecycle override: unknown class '{}'", class_name.node),
                    class_name.span,
                )
            })?;

            let current = class_info.lifecycle;
            let lifecycle_rank = |l: Lifecycle| -> u8 {
                match l {
                    Lifecycle::Transient => 0,
                    Lifecycle::Scoped => 1,
                    Lifecycle::Singleton => 2,
                }
            };
            if lifecycle_rank(*target_lifecycle) > lifecycle_rank(current) {
                return Err(CompileError::type_err(
                    format!(
                        "lifecycle override: cannot lengthen lifecycle of '{}' from {} to {}; \
                         overrides can only shorten lifecycle (singleton -> scoped -> transient)",
                        class_name.node, current, *target_lifecycle
                    ),
                    class_name.span,
                ));
            }

            if let Some(info) = env.classes.get_mut(&class_name.node) {
                info.lifecycle = *target_lifecycle;
            }
            env.lifecycle_overridden.insert(class_name.node.clone());
        }

        // Re-run lifecycle inference to propagate overrides
        let di_order_snapshot = env.di_order.clone();
        for class_name in &di_order_snapshot {
            if let Some(class_info) = env.classes.get(class_name) {
                let deps: Vec<String> = class_info.fields.iter()
                    .filter(|(_, _, inj)| *inj)
                    .filter_map(|(_, ty, _)| {
                        if let PlutoType::Class(name) = ty { Some(name.clone()) } else { None }
                    })
                    .collect();
                let mut inferred = class_info.lifecycle;
                for dep_name in &deps {
                    if let Some(dep_info) = env.classes.get(dep_name) {
                        inferred = min_lifecycle(inferred, dep_info.lifecycle);
                    }
                }
                if let Some(info) = env.classes.get_mut(class_name) {
                    if inferred != info.lifecycle {
                        info.lifecycle = inferred;
                        env.lifecycle_overridden.insert(class_name.clone());
                    }
                }
            }
        }

        // Validate stage bracket deps don't reference overridden classes
        for field in &stage_spanned.node.inject_fields {
            if let crate::parser::ast::TypeExpr::Named(ref type_name) = field.ty.node {
                if env.lifecycle_overridden.contains(type_name) {
                    return Err(CompileError::type_err(
                        format!(
                            "stage bracket dependency '{}' has overridden lifecycle; use scope blocks to access scoped/transient instances",
                            field.name.node
                        ),
                        field.ty.span,
                    ));
                }
            }
        }

        env.di_order.retain(|n| !env.lifecycle_overridden.contains(n));
    }

    Ok(())
}

pub(crate) fn check_trait_conformance(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() { continue; } // Skip generic classes
        let class_name = &c.name.node;
        let class_info = env.classes.get(class_name).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown class '{}'", class_name),
                class.span,
            )
        })?.clone();

        // Validate no duplicate traits in impl list
        {
            let mut seen_traits = HashSet::new();
            for trait_name_spanned in &c.impl_traits {
                let trait_name = &trait_name_spanned.node;
                if !seen_traits.insert(trait_name.clone()) {
                    return Err(CompileError::type_err(
                        format!("trait '{}' appears multiple times in impl list for class '{}'", trait_name, class_name),
                        trait_name_spanned.span,
                    ));
                }
            }
        }

        // Multi-trait collision guard: reject if two traits define the same method
        // and at least one has contracts on it
        {
            let mut method_contract_traits: HashMap<String, Vec<String>> = HashMap::new();
            for trait_name_spanned in &c.impl_traits {
                let trait_name = &trait_name_spanned.node;
                if let Some(trait_info) = env.traits.get(trait_name) {
                    for (method_name, _) in &trait_info.methods {
                        if trait_info.method_contracts.contains_key(method_name) {
                            method_contract_traits.entry(method_name.clone())
                                .or_default()
                                .push(trait_name.clone());
                        }
                    }
                }
            }
            for (method_name, trait_names) in &method_contract_traits {
                if trait_names.len() > 1 {
                    return Err(CompileError::type_err(
                        format!(
                            "class '{}' implements traits {} which both define method '{}' with contracts; this is not supported",
                            class_name,
                            trait_names.join(" and "),
                            method_name
                        ),
                        class.span,
                    ));
                }
            }
        }

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            let trait_info = env.traits.get(trait_name).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown trait '{}'", trait_name),
                    trait_name_spanned.span,
                )
            })?.clone();

            for (method_name, trait_sig) in &trait_info.methods {
                let mangled = mangle_method(class_name, method_name);

                if class_info.methods.contains(method_name) {
                    // Class has this method — verify signature matches
                    let class_sig = env.functions.get(&mangled).ok_or_else(|| {
                        CompileError::type_err(
                            format!("missing method signature for '{}.{}'", class_name, method_name),
                            trait_name_spanned.span,
                        )
                    })?;
                    // Compare non-self params
                    let trait_non_self = &trait_sig.params[1..];
                    let class_non_self = &class_sig.params[1..];
                    if trait_non_self.len() != class_non_self.len() {
                        return Err(CompileError::type_err(
                            format!(
                                "method '{}' of class '{}' has wrong number of parameters for trait '{}'",
                                method_name, class_name, trait_name
                            ),
                            trait_name_spanned.span,
                        ));
                    }
                    for (i, (tp, cp)) in trait_non_self.iter().zip(class_non_self).enumerate() {
                        if tp != cp {
                            return Err(CompileError::type_err(
                                format!(
                                    "method '{}' parameter {} type mismatch: trait '{}' expects {}, class '{}' has {}",
                                    method_name, i + 1, trait_name, tp, class_name, cp
                                ),
                                trait_name_spanned.span,
                            ));
                        }
                    }
                    if trait_sig.return_type != class_sig.return_type {
                        return Err(CompileError::type_err(
                            format!(
                                "method '{}' return type mismatch: trait '{}' expects {}, class '{}' returns {}",
                                method_name, trait_name, trait_sig.return_type, class_name, class_sig.return_type
                            ),
                            trait_name_spanned.span,
                        ));
                    }
                    // Check mut self conformance
                    let trait_mut = trait_info.mut_self_methods.contains(method_name);
                    let class_mut = env.mut_self_methods.contains(&mangled);
                    if trait_mut && !class_mut {
                        return Err(CompileError::type_err(
                            format!(
                                "method '{}' in trait '{}' declares 'mut self', but class '{}' does not",
                                method_name, trait_name, class_name
                            ),
                            trait_name_spanned.span,
                        ));
                    }
                    if !trait_mut && class_mut {
                        return Err(CompileError::type_err(
                            format!(
                                "method '{}' in trait '{}' declares 'self', but class '{}' declares 'mut self'",
                                method_name, trait_name, class_name
                            ),
                            trait_name_spanned.span,
                        ));
                    }
                    // Liskov: class methods implementing a trait MUST NOT add requires clauses
                    // (a trait method with no requires effectively has "requires true";
                    //  adding requires would weaken the precondition and break substitutability)
                    let class_method_ast = c.methods.iter().find(|m| m.node.name.node == *method_name);
                    if let Some(cm) = class_method_ast {
                        let has_class_requires = cm.node.contracts.iter()
                            .any(|ct| ct.node.kind == ContractKind::Requires);
                        if has_class_requires {
                            return Err(CompileError::type_err(
                                format!(
                                    "method '{}' on class '{}' cannot add 'requires' clauses: \
                                     it implements trait '{}' and adding preconditions would \
                                     violate the Liskov Substitution Principle",
                                    method_name, class_name, trait_name
                                ),
                                cm.node.name.span,
                            ));
                        }
                    }
                } else if trait_info.default_methods.contains(method_name) {
                    // Default implementation — register under mangled name
                    let mut params = trait_sig.params.clone();
                    // Replace the Void placeholder with the actual class type
                    if !params.is_empty() {
                        params[0] = PlutoType::Class(class_name.clone());
                    }
                    env.functions.insert(
                        mangled.clone(),
                        FuncSig {
                            params,
                            return_type: trait_sig.return_type.clone(),
                        },
                    );
                    // Propagate mut self from trait default method
                    if trait_info.mut_self_methods.contains(method_name) {
                        env.mut_self_methods.insert(mangled.clone());
                    }
                    // Add method name to class info
                    if let Some(info) = env.classes.get_mut(class_name) {
                        info.methods.push(method_name.clone());
                    }
                } else {
                    return Err(CompileError::type_err(
                        format!(
                            "class '{}' does not implement required method '{}' from trait '{}'",
                            class_name, method_name, trait_name
                        ),
                        trait_name_spanned.span,
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn check_all_bodies(program: &Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    // Check function bodies and contracts
    for func in &program.functions {
        if !func.node.type_params.is_empty() { continue; } // Skip generic functions
        check_function(&func.node, env, None)?;
        check_function_contracts(&func.node, env, None)?;
    }

    // Check method bodies and contracts
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() { continue; } // Skip generic classes
        for method in &c.methods {
            check_function(&method.node, env, Some(&c.name.node))?;
            check_function_contracts(&method.node, env, Some(&c.name.node))?;
        }
        // Type-check class invariants
        if !c.invariants.is_empty() {
            env.push_scope();
            env.define("self".to_string(), PlutoType::Class(c.name.node.clone()));
            for inv in &c.invariants {
                let inv_type = super::infer::infer_expr(&inv.node.expr.node, inv.node.expr.span, env)?;
                if inv_type != PlutoType::Bool {
                    return Err(CompileError::type_err(
                        format!("invariant expression must be bool, found {inv_type}"),
                        inv.node.expr.span,
                    ));
                }
            }
            env.pop_scope();
        }
    }

    // Type-check trait method contracts (requires on abstract trait methods)
    for trait_decl in &program.traits {
        let t = &trait_decl.node;
        for m in &t.methods {
            if m.contracts.is_empty() {
                continue;
            }
            // Resolve param types and return type
            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(("self".to_string(), PlutoType::Void));
                } else {
                    let ty = resolve_type(&p.ty, env)?;
                    param_types.push((p.name.node.clone(), ty));
                }
            }
            let _return_type = match &m.return_type {
                Some(rt) => resolve_type(rt, env)?,
                None => PlutoType::Void,
            };

            // Check requires clauses
            let has_requires = m.contracts.iter().any(|c| c.node.kind == ContractKind::Requires);
            if has_requires {
                env.push_scope();
                for (name, ty) in &param_types {
                    env.define(name.clone(), ty.clone());
                }
                for contract in &m.contracts {
                    if contract.node.kind == ContractKind::Requires {
                        let ty = super::infer::infer_expr(&contract.node.expr.node, contract.node.expr.span, env)?;
                        if ty != PlutoType::Bool {
                            return Err(CompileError::type_err(
                                format!("requires expression must be bool, found {ty}"),
                                contract.node.expr.span,
                            ));
                        }
                    }
                }
                env.pop_scope();
            }

        }
    }

    // Type-check default method bodies for classes that inherit them
    for class in &program.classes {
        let c = &class.node;
        if !c.type_params.is_empty() { continue; } // Skip generic classes
        let class_name = &c.name.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            // Find the trait's default methods in the AST
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if let Some(body) = &trait_method.body
                            && !class_method_names.contains(&trait_method.name.node)
                        {
                            // This class inherits this default method — type check it
                            let tmp_func = Function {
                                id: Uuid::new_v4(),
                                name: trait_method.name.clone(),
                                type_params: vec![],
                                type_param_bounds: HashMap::new(),
                                params: trait_method.params.clone(),
                                return_type: trait_method.return_type.clone(),
                                contracts: trait_method.contracts.clone(),
                                body: body.clone(),
                                is_pub: false,
                                is_override: false,
                                is_generator: false,
                            };
                            check_function(&tmp_func, env, Some(class_name))?;
                        }
                    }
                }
            }
        }
    }

    // Type-check app method bodies and contracts
    if let Some(app_spanned) = &program.app {
        let app = &app_spanned.node;
        let app_name = &app.name.node;
        for method in &app.methods {
            check_function(&method.node, env, Some(app_name))?;
            check_function_contracts(&method.node, env, Some(app_name))?;
        }
    }

    // Type-check stage method bodies and contracts
    for stage_spanned in &program.stages {
        let stage = &stage_spanned.node;
        let stage_name = &stage.name.node;
        for method in &stage.methods {
            check_function(&method.node, env, Some(stage_name))?;
            check_function_contracts(&method.node, env, Some(stage_name))?;
        }
    }
    Ok(())
}

/// Returns the shorter of two lifecycles.
/// Ordering: Transient < Scoped < Singleton.
fn min_lifecycle(a: Lifecycle, b: Lifecycle) -> Lifecycle {
    match (a, b) {
        (Lifecycle::Transient, _) | (_, Lifecycle::Transient) => Lifecycle::Transient,
        (Lifecycle::Scoped, _) | (_, Lifecycle::Scoped) => Lifecycle::Scoped,
        _ => Lifecycle::Singleton,
    }
}

pub mod env;
pub mod types;

use std::collections::{HashMap, HashSet};

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::Spanned;
use env::{ClassInfo, EnumInfo, ErrorInfo, FuncSig, TraitInfo, TypeEnv};
use types::PlutoType;

fn types_compatible(actual: &PlutoType, expected: &PlutoType, env: &TypeEnv) -> bool {
    if actual == expected {
        return true;
    }
    if let (PlutoType::Class(cn), PlutoType::Trait(tn)) = (actual, expected) {
        return env.class_implements_trait(cn, tn);
    }
    false
}

pub fn type_check(program: &Program) -> Result<TypeEnv, CompileError> {
    let mut env = TypeEnv::new();

    // Pass 0: Register traits
    for trait_decl in &program.traits {
        let t = &trait_decl.node;
        let mut methods = Vec::new();
        let mut default_methods = Vec::new();

        for m in &t.methods {
            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Void); // placeholder for self
                } else {
                    param_types.push(resolve_type(&p.ty, &env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(rt) => resolve_type(rt, &env)?,
                None => PlutoType::Void,
            };
            methods.push((m.name.node.clone(), FuncSig { params: param_types, return_type }));
            if m.body.is_some() {
                default_methods.push(m.name.node.clone());
            }
        }

        env.traits.insert(t.name.node.clone(), TraitInfo { methods, default_methods });
    }

    // Pass 0b: Register enums
    for enum_decl in &program.enums {
        let e = &enum_decl.node;
        let mut variants = Vec::new();
        for v in &e.variants {
            let mut fields = Vec::new();
            for f in &v.fields {
                let ty = resolve_type(&f.ty, &env)?;
                fields.push((f.name.node.clone(), ty));
            }
            variants.push((v.name.node.clone(), fields));
        }
        env.enums.insert(e.name.node.clone(), EnumInfo { variants });
    }

    // Pass 0c: Register errors
    for error_decl in &program.errors {
        let e = &error_decl.node;
        let mut fields = Vec::new();
        for f in &e.fields {
            let ty = resolve_type(&f.ty, &env)?;
            fields.push((f.name.node.clone(), ty));
        }
        env.errors.insert(e.name.node.clone(), ErrorInfo { fields });
    }

    // Pass 1a: Register class field definitions
    for class in &program.classes {
        let c = &class.node;
        let mut fields = Vec::new();
        for f in &c.fields {
            let ty = resolve_type(&f.ty, &env)?;
            fields.push((f.name.node.clone(), ty));
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

        env.classes.insert(
            c.name.node.clone(),
            ClassInfo {
                fields,
                methods: Vec::new(),
                impl_traits: impl_trait_names,
            },
        );
    }

    // Pass 1b: Register extern fn signatures (before regular fns so conflict checks work)
    for ext in &program.extern_fns {
        let e = &ext.node;

        // Validate only primitive types allowed
        let mut param_types = Vec::new();
        for p in &e.params {
            let ty = resolve_type(&p.ty, &env)?;
            match &ty {
                PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Void => {}
                _ => {
                    return Err(CompileError::type_err(
                        format!("extern functions only support primitive types (int, float, bool, string), got '{}'", ty),
                        p.ty.span,
                    ));
                }
            }
            param_types.push(ty);
        }

        let return_type = match &e.return_type {
            Some(t) => {
                let ty = resolve_type(t, &env)?;
                match &ty {
                    PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String | PlutoType::Void => {}
                    _ => {
                        return Err(CompileError::type_err(
                            format!("extern functions only support primitive types (int, float, bool, string), got '{}'", ty),
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

    // Pass 1b2: Collect top-level function signatures
    for func in &program.functions {
        let f = &func.node;

        // Check for conflict with extern fn
        if env.extern_fns.contains(&f.name.node) {
            return Err(CompileError::type_err(
                format!("duplicate function name '{}': defined as both fn and extern fn", f.name.node),
                f.name.span,
            ));
        }

        let mut param_types = Vec::new();
        for p in &f.params {
            param_types.push(resolve_type(&p.ty, &env)?);
        }
        let return_type = match &f.return_type {
            Some(t) => resolve_type(t, &env)?,
            None => PlutoType::Void,
        };
        env.functions.insert(
            f.name.node.clone(),
            FuncSig { params: param_types, return_type },
        );
    }

    // Pass 1c: Collect method signatures (mangled name: ClassName_method)
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let mut method_names = Vec::new();
        for method in &c.methods {
            let m = &method.node;
            let mangled = format!("{}_{}", class_name, m.name.node);
            method_names.push(m.name.node.clone());

            let mut param_types = Vec::new();
            for p in &m.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Class(class_name.clone()));
                } else {
                    param_types.push(resolve_type(&p.ty, &env)?);
                }
            }
            let return_type = match &m.return_type {
                Some(t) => resolve_type(t, &env)?,
                None => PlutoType::Void,
            };
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

    // Pass 1d: Trait conformance checking
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let class_info = env.classes.get(class_name).ok_or_else(|| {
            CompileError::type_err(
                format!("unknown class '{}'", class_name),
                class.span,
            )
        })?.clone();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            let trait_info = env.traits.get(trait_name).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown trait '{}'", trait_name),
                    trait_name_spanned.span,
                )
            })?.clone();

            for (method_name, trait_sig) in &trait_info.methods {
                let mangled = format!("{}_{}", class_name, method_name);

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
                } else if trait_info.default_methods.contains(method_name) {
                    // Default implementation — register under mangled name
                    let mut params = trait_sig.params.clone();
                    // Replace the Void placeholder with the actual class type
                    if !params.is_empty() {
                        params[0] = PlutoType::Class(class_name.clone());
                    }
                    env.functions.insert(
                        mangled,
                        FuncSig {
                            params,
                            return_type: trait_sig.return_type.clone(),
                        },
                    );
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

    // Pass 2: Check function bodies
    for func in &program.functions {
        check_function(&func.node, &mut env, None)?;
    }

    // Pass 2b: Check method bodies
    for class in &program.classes {
        let c = &class.node;
        for method in &c.methods {
            check_function(&method.node, &mut env, Some(&c.name.node))?;
        }
    }

    // Pass 2c: Type-check default method bodies for classes that inherit them
    for class in &program.classes {
        let c = &class.node;
        let class_name = &c.name.node;
        let class_method_names: Vec<String> = c.methods.iter().map(|m| m.node.name.node.clone()).collect();

        for trait_name_spanned in &c.impl_traits {
            let trait_name = &trait_name_spanned.node;
            // Find the trait's default methods in the AST
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == *trait_name {
                    for trait_method in &trait_decl.node.methods {
                        if trait_method.body.is_some() && !class_method_names.contains(&trait_method.name.node) {
                            // This class inherits this default method — type check it
                            let body = trait_method.body.as_ref().unwrap();
                            let tmp_func = Function {
                                name: trait_method.name.clone(),
                                params: trait_method.params.clone(),
                                return_type: trait_method.return_type.clone(),
                                body: body.clone(),
                                is_pub: false,
                            };
                            check_function(&tmp_func, &mut env, Some(class_name))?;
                        }
                    }
                }
            }
        }
    }

    // Pass 3: Error inference — compute per-function error sets
    infer_error_sets(program, &mut env);

    // Pass 4: Error handling enforcement
    enforce_error_handling(program, &env)?;

    Ok(env)
}

fn resolve_type(ty: &Spanned<TypeExpr>, env: &TypeEnv) -> Result<PlutoType, CompileError> {
    match &ty.node {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(PlutoType::Int),
            "float" => Ok(PlutoType::Float),
            "bool" => Ok(PlutoType::Bool),
            "string" => Ok(PlutoType::String),
            "void" => Ok(PlutoType::Void),
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
    }
}

fn check_function(func: &Function, env: &mut TypeEnv, class_name: Option<&str>) -> Result<(), CompileError> {
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
                .find(|(n, _)| *n == field.node)
                .map(|(_, t)| t.clone())
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
        Stmt::For { var, iterable, body } => {
            let iter_type = infer_expr(&iterable.node, iterable.span, env)?;
            let elem_type = match iter_type {
                PlutoType::Array(elem) => *elem,
                _ => {
                    return Err(CompileError::type_err(
                        format!("for loop requires array, found {iter_type}"),
                        iterable.span,
                    ));
                }
            };
            env.push_scope();
            env.define(var.node.clone(), elem_type);
            check_block(&body.node, env, return_type)?;
            env.pop_scope();
        }
        Stmt::IndexAssign { object, index, value } => {
            let obj_type = infer_expr(&object.node, object.span, env)?;
            let elem_type = match &obj_type {
                PlutoType::Array(elem) => *elem.clone(),
                _ => {
                    return Err(CompileError::type_err(
                        format!("index assignment on non-array type {obj_type}"),
                        object.span,
                    ));
                }
            };
            let idx_type = infer_expr(&index.node, index.span, env)?;
            if idx_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("array index must be int, found {idx_type}"),
                    index.span,
                ));
            }
            let val_type = infer_expr(&value.node, value.span, env)?;
            if val_type != elem_type {
                return Err(CompileError::type_err(
                    format!("index assignment: expected {elem_type}, found {val_type}"),
                    value.span,
                ));
            }
        }
        Stmt::Match { expr, arms } => {
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
                if arm.enum_name.node != enum_name {
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
        }
        Stmt::Raise { error_name, fields } => {
            // Validate that the error type exists
            let error_info = env.errors.get(&error_name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown error type '{}'", error_name.node),
                    error_name.span,
                )
            })?.clone();
            // Validate field count
            if fields.len() != error_info.fields.len() {
                return Err(CompileError::type_err(
                    format!(
                        "error '{}' has {} fields, but {} were provided",
                        error_name.node, error_info.fields.len(), fields.len()
                    ),
                    span,
                ));
            }
            // Validate each field
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
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    let t = infer_expr(&e.node, e.span, env)?;
                    match t {
                        PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String => {}
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
                            PlutoType::Int | PlutoType::Float | PlutoType::Bool | PlutoType::String => {}
                            _ => {
                                return Err(CompileError::type_err(
                                    format!("print() does not support type {arg_type}"),
                                    args[0].span,
                                ));
                            }
                        }
                        Ok(PlutoType::Void)
                    }
                    _ => Err(CompileError::type_err(
                        format!("unknown builtin '{}'", name.node),
                        name.span,
                    )),
                };
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
        Expr::StructLit { name, fields: lit_fields } => {
            let class_info = env.classes.get(&name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown class '{}'", name.node),
                    name.span,
                )
            })?;

            // Check all fields are provided
            if lit_fields.len() != class_info.fields.len() {
                return Err(CompileError::type_err(
                    format!(
                        "class '{}' has {} fields, but {} were provided",
                        name.node,
                        class_info.fields.len(),
                        lit_fields.len()
                    ),
                    span,
                ));
            }

            // Check each field matches
            for (lit_name, lit_val) in lit_fields {
                let field_type = class_info.fields.iter()
                    .find(|(n, _)| *n == lit_name.node)
                    .map(|(_, t)| t.clone())
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("class '{}' has no field '{}'", name.node, lit_name.node),
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

            Ok(PlutoType::Class(name.node.clone()))
        }
        Expr::FieldAccess { object, field } => {
            let obj_type = infer_expr(&object.node, object.span, env)?;
            let class_name = match &obj_type {
                PlutoType::Class(name) => name.clone(),
                _ => {
                    return Err(CompileError::type_err(
                        format!("field access on non-class type {obj_type}"),
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
            class_info.fields.iter()
                .find(|(n, _)| *n == field.node)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| {
                    CompileError::type_err(
                        format!("class '{class_name}' has no field '{}'", field.node),
                        field.span,
                    )
                })
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
            let elem_type = match &obj_type {
                PlutoType::Array(elem) => *elem.clone(),
                _ => {
                    return Err(CompileError::type_err(
                        format!("index on non-array type {obj_type}"),
                        object.span,
                    ));
                }
            };
            let idx_type = infer_expr(&index.node, index.span, env)?;
            if idx_type != PlutoType::Int {
                return Err(CompileError::type_err(
                    format!("array index must be int, found {idx_type}"),
                    index.span,
                ));
            }
            Ok(elem_type)
        }
        Expr::EnumUnit { enum_name, variant } => {
            let enum_info = env.enums.get(&enum_name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown enum '{}'", enum_name.node),
                    enum_name.span,
                )
            })?;
            let variant_info = enum_info.variants.iter().find(|(n, _)| *n == variant.node);
            match variant_info {
                None => Err(CompileError::type_err(
                    format!("enum '{}' has no variant '{}'", enum_name.node, variant.node),
                    variant.span,
                )),
                Some((_, fields)) if !fields.is_empty() => Err(CompileError::type_err(
                    format!("variant '{}.{}' has fields; use {}.{} {{ ... }}", enum_name.node, variant.node, enum_name.node, variant.node),
                    variant.span,
                )),
                Some(_) => Ok(PlutoType::Enum(enum_name.node.clone())),
            }
        }
        Expr::EnumData { enum_name, variant, fields: lit_fields } => {
            let enum_info = env.enums.get(&enum_name.node).ok_or_else(|| {
                CompileError::type_err(
                    format!("unknown enum '{}'", enum_name.node),
                    enum_name.span,
                )
            })?.clone();
            let variant_info = enum_info.variants.iter().find(|(n, _)| *n == variant.node);
            match variant_info {
                None => Err(CompileError::type_err(
                    format!("enum '{}' has no variant '{}'", enum_name.node, variant.node),
                    variant.span,
                )),
                Some((_, expected_fields)) => {
                    if lit_fields.len() != expected_fields.len() {
                        return Err(CompileError::type_err(
                            format!(
                                "variant '{}.{}' has {} fields, but {} were provided",
                                enum_name.node, variant.node, expected_fields.len(), lit_fields.len()
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
                                    format!("variant '{}.{}' has no field '{}'", enum_name.node, variant.node, lit_name.node),
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
                    Ok(PlutoType::Enum(enum_name.node.clone()))
                }
            }
        }
        Expr::Propagate { expr } => {
            // The inner expression must be a call-like expression
            let inner_type = infer_expr(&expr.node, expr.span, env)?;
            // In MVP, ! just returns the success type (enforcement pass will check fallibility)
            Ok(inner_type)
        }
        Expr::Catch { expr, handler } => {
            let success_type = infer_expr(&expr.node, expr.span, env)?;
            // Type check the handler (without wildcard scope binding — that needs
            // mutable env, done in check_stmt for let/expr statements)
            let handler_type = match handler {
                CatchHandler::Wildcard { body, .. } => {
                    // In infer_expr we can't push_scope (env is &), so just infer body type.
                    // The wildcard var won't resolve, but this path is only hit for nested
                    // catch in expressions. Full checking happens via check_stmt → check_catch.
                    infer_expr(&body.node, body.span, env)?
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
        Expr::MethodCall { object, method, args } => {
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
            if obj_type == PlutoType::String {
                if method.node == "len" && args.is_empty() {
                    return Ok(PlutoType::Int);
                }
                return Err(CompileError::type_err(
                    format!("string has no method '{}'", method.node),
                    method.span,
                ));
            }

            // Trait method calls
            if let PlutoType::Trait(trait_name) = &obj_type {
                let trait_info = env.traits.get(trait_name).ok_or_else(|| {
                    CompileError::type_err(
                        format!("unknown trait '{trait_name}'"),
                        object.span,
                    )
                })?;
                let (_, method_sig) = trait_info.methods.iter()
                    .find(|(n, _)| *n == method.node)
                    .ok_or_else(|| {
                        CompileError::type_err(
                            format!("trait '{trait_name}' has no method '{}'", method.node),
                            method.span,
                        )
                    })?;

                // Check non-self args
                let expected_args = &method_sig.params[1..];
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
            let sig = env.functions.get(&mangled).ok_or_else(|| {
                CompileError::type_err(
                    format!("class '{class_name}' has no method '{}'", method.node),
                    method.span,
                )
            })?;

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
    }
}

// ── Phase 2b: Error inference ─────────────────────────────────────────────────

fn infer_error_sets(program: &Program, env: &mut TypeEnv) {
    let mut direct_errors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut propagation_edges: HashMap<String, HashSet<String>> = HashMap::new();

    // Collect effects from top-level functions
    for func in &program.functions {
        let name = func.node.name.node.clone();
        let (directs, edges) = collect_block_effects(&func.node.body.node);
        direct_errors.insert(name.clone(), directs);
        propagation_edges.insert(name, edges);
    }

    // Collect effects from class methods
    for class in &program.classes {
        let class_name = &class.node.name.node;
        for method in &class.node.methods {
            let mangled = format!("{}_{}", class_name, method.node.name.node);
            let (directs, edges) = collect_block_effects(&method.node.body.node);
            direct_errors.insert(mangled.clone(), directs);
            propagation_edges.insert(mangled, edges);
        }
    }

    // Collect effects from inherited default trait methods
    for class in &program.classes {
        let class_name = &class.node.name.node;
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            let mangled = format!("{}_{}", class_name, tm.name.node);
                            let (directs, edges) =
                                collect_block_effects(&tm.body.as_ref().unwrap().node);
                            direct_errors.insert(mangled.clone(), directs);
                            propagation_edges.insert(mangled, edges);
                        }
                    }
                }
            }
        }
    }

    // Fixed-point iteration: propagate error sets through call edges
    let mut fn_errors: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, directs) in &direct_errors {
        fn_errors.insert(name.clone(), directs.clone());
    }

    loop {
        let mut changed = false;
        for (fn_name, edges) in &propagation_edges {
            let mut new_errors = fn_errors.get(fn_name).cloned().unwrap_or_default();
            for callee in edges {
                if let Some(callee_errors) = fn_errors.get(callee) {
                    for e in callee_errors {
                        if new_errors.insert(e.clone()) {
                            changed = true;
                        }
                    }
                }
            }
            fn_errors.insert(fn_name.clone(), new_errors);
        }
        if !changed {
            break;
        }
    }

    env.fn_errors = fn_errors;
}

/// Collect direct error raises and propagation edges from a block.
fn collect_block_effects(block: &Block) -> (HashSet<String>, HashSet<String>) {
    let mut direct_errors = HashSet::new();
    let mut edges = HashSet::new();
    for stmt in &block.stmts {
        collect_stmt_effects(&stmt.node, &mut direct_errors, &mut edges);
    }
    (direct_errors, edges)
}

fn collect_stmt_effects(
    stmt: &Stmt,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Raise { error_name, fields } => {
            direct_errors.insert(error_name.node.clone());
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Stmt::Let { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::Expr(expr) => {
            collect_expr_effects(&expr.node, direct_errors, edges);
        }
        Stmt::Return(Some(expr)) => {
            collect_expr_effects(&expr.node, direct_errors, edges);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::FieldAssign { object, value, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::IndexAssign { object, index, value } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&index.node, direct_errors, edges);
            collect_expr_effects(&value.node, direct_errors, edges);
        }
        Stmt::If { condition, then_block, else_block } => {
            collect_expr_effects(&condition.node, direct_errors, edges);
            for s in &then_block.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
            if let Some(eb) = else_block {
                for s in &eb.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_effects(&condition.node, direct_errors, edges);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
        }
        Stmt::For { iterable, body, .. } => {
            collect_expr_effects(&iterable.node, direct_errors, edges);
            for s in &body.node.stmts {
                collect_stmt_effects(&s.node, direct_errors, edges);
            }
        }
        Stmt::Match { expr, arms } => {
            collect_expr_effects(&expr.node, direct_errors, edges);
            for arm in arms {
                for s in &arm.body.node.stmts {
                    collect_stmt_effects(&s.node, direct_errors, edges);
                }
            }
        }
    }
}

fn collect_expr_effects(
    expr: &Expr,
    direct_errors: &mut HashSet<String>,
    edges: &mut HashSet<String>,
) {
    match expr {
        Expr::Propagate { expr: inner } => {
            // ! propagates errors from the inner call
            match &inner.node {
                Expr::Call { name, args } => {
                    edges.insert(name.node.clone());
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    // MVP: can't resolve method mangled name without type info
                    collect_expr_effects(&object.node, direct_errors, edges);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges),
            }
        }
        Expr::Catch { expr: inner, handler } => {
            // catch handles errors — don't add propagation edge, but recurse into args
            match &inner.node {
                Expr::Call { args, .. } => {
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    collect_expr_effects(&object.node, direct_errors, edges);
                    for arg in args {
                        collect_expr_effects(&arg.node, direct_errors, edges);
                    }
                }
                _ => collect_expr_effects(&inner.node, direct_errors, edges),
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    collect_expr_effects(&body.node, direct_errors, edges);
                }
                CatchHandler::Shorthand(fb) => {
                    collect_expr_effects(&fb.node, direct_errors, edges);
                }
            }
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr_effects(&lhs.node, direct_errors, edges);
            collect_expr_effects(&rhs.node, direct_errors, edges);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_expr_effects(&operand.node, direct_errors, edges);
        }
        Expr::Call { args, .. } => {
            // Bare call — no propagation edge (errors not propagated)
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            for arg in args {
                collect_expr_effects(&arg.node, direct_errors, edges);
            }
        }
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Expr::FieldAccess { object, .. } => {
            collect_expr_effects(&object.node, direct_errors, edges);
        }
        Expr::ArrayLit { elements } => {
            for e in elements {
                collect_expr_effects(&e.node, direct_errors, edges);
            }
        }
        Expr::Index { object, index } => {
            collect_expr_effects(&object.node, direct_errors, edges);
            collect_expr_effects(&index.node, direct_errors, edges);
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                collect_expr_effects(&val.node, direct_errors, edges);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_expr_effects(&e.node, direct_errors, edges);
                }
            }
        }
        // Leaf nodes
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } => {}
    }
}

// ── Phase 2c: Error handling enforcement ──────────────────────────────────────

fn enforce_error_handling(program: &Program, env: &TypeEnv) -> Result<(), CompileError> {
    for func in &program.functions {
        enforce_block(&func.node.body.node, env)?;
    }
    for class in &program.classes {
        for method in &class.node.methods {
            enforce_block(&method.node.body.node, env)?;
        }
    }
    // Also enforce in inherited default trait method bodies
    for class in &program.classes {
        let class_method_names: Vec<String> =
            class.node.methods.iter().map(|m| m.node.name.node.clone()).collect();
        for trait_name in &class.node.impl_traits {
            for trait_decl in &program.traits {
                if trait_decl.node.name.node == trait_name.node {
                    for tm in &trait_decl.node.methods {
                        if tm.body.is_some() && !class_method_names.contains(&tm.name.node) {
                            enforce_block(&tm.body.as_ref().unwrap().node, env)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn enforce_block(block: &Block, env: &TypeEnv) -> Result<(), CompileError> {
    for stmt in &block.stmts {
        enforce_stmt(&stmt.node, stmt.span, env)?;
    }
    Ok(())
}

fn enforce_stmt(
    stmt: &Stmt,
    _span: crate::span::Span,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { value, .. } => enforce_expr(&value.node, value.span, env),
        Stmt::Expr(expr) => enforce_expr(&expr.node, expr.span, env),
        Stmt::Return(Some(expr)) => enforce_expr(&expr.node, expr.span, env),
        Stmt::Return(None) => Ok(()),
        Stmt::Assign { value, .. } => enforce_expr(&value.node, value.span, env),
        Stmt::FieldAssign { object, value, .. } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&value.node, value.span, env)
        }
        Stmt::IndexAssign { object, index, value } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&index.node, index.span, env)?;
            enforce_expr(&value.node, value.span, env)
        }
        Stmt::If { condition, then_block, else_block } => {
            enforce_expr(&condition.node, condition.span, env)?;
            enforce_block(&then_block.node, env)?;
            if let Some(eb) = else_block {
                enforce_block(&eb.node, env)?;
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            enforce_expr(&condition.node, condition.span, env)?;
            enforce_block(&body.node, env)
        }
        Stmt::For { iterable, body, .. } => {
            enforce_expr(&iterable.node, iterable.span, env)?;
            enforce_block(&body.node, env)
        }
        Stmt::Match { expr, arms } => {
            enforce_expr(&expr.node, expr.span, env)?;
            for arm in arms {
                enforce_block(&arm.body.node, env)?;
            }
            Ok(())
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
    }
}

fn enforce_expr(
    expr: &Expr,
    span: crate::span::Span,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    match expr {
        Expr::Call { name, args } => {
            for arg in args {
                enforce_expr(&arg.node, arg.span, env)?;
            }
            if env.is_fn_fallible(&name.node) {
                return Err(CompileError::type_err(
                    format!(
                        "call to fallible function '{}' must be handled with ! or catch",
                        name.node
                    ),
                    span,
                ));
            }
            Ok(())
        }
        Expr::MethodCall { object, args, .. } => {
            enforce_expr(&object.node, object.span, env)?;
            for arg in args {
                enforce_expr(&arg.node, arg.span, env)?;
            }
            // MVP: method fallibility enforcement deferred (needs type resolution)
            Ok(())
        }
        Expr::Propagate { expr: inner } => match &inner.node {
            Expr::Call { name, args } => {
                for arg in args {
                    enforce_expr(&arg.node, arg.span, env)?;
                }
                if !env.is_fn_fallible(&name.node) {
                    return Err(CompileError::type_err(
                        format!("'!' applied to infallible function '{}'", name.node),
                        span,
                    ));
                }
                Ok(())
            }
            Expr::MethodCall { object, args, .. } => {
                enforce_expr(&object.node, object.span, env)?;
                for arg in args {
                    enforce_expr(&arg.node, arg.span, env)?;
                }
                // MVP: allow ! on method calls without fallibility check
                Ok(())
            }
            _ => Err(CompileError::type_err(
                "! can only be applied to function calls",
                inner.span,
            )),
        },
        Expr::Catch { expr: inner, handler } => {
            match &inner.node {
                Expr::Call { name, args } => {
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, env)?;
                    }
                    if !env.is_fn_fallible(&name.node) {
                        return Err(CompileError::type_err(
                            format!("catch applied to infallible function '{}'", name.node),
                            span,
                        ));
                    }
                }
                Expr::MethodCall { object, args, .. } => {
                    enforce_expr(&object.node, object.span, env)?;
                    for arg in args {
                        enforce_expr(&arg.node, arg.span, env)?;
                    }
                    // MVP: allow catch on method calls without fallibility check
                }
                _ => {
                    return Err(CompileError::type_err(
                        "catch can only be applied to function calls",
                        inner.span,
                    ));
                }
            }
            match handler {
                CatchHandler::Wildcard { body, .. } => enforce_expr(&body.node, body.span, env),
                CatchHandler::Shorthand(fb) => enforce_expr(&fb.node, fb.span, env),
            }
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            enforce_expr(&lhs.node, lhs.span, env)?;
            enforce_expr(&rhs.node, rhs.span, env)
        }
        Expr::UnaryOp { operand, .. } => enforce_expr(&operand.node, operand.span, env),
        Expr::StructLit { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
        Expr::FieldAccess { object, .. } => enforce_expr(&object.node, object.span, env),
        Expr::ArrayLit { elements } => {
            for e in elements {
                enforce_expr(&e.node, e.span, env)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            enforce_expr(&object.node, object.span, env)?;
            enforce_expr(&index.node, index.span, env)
        }
        Expr::EnumData { fields, .. } => {
            for (_, val) in fields {
                enforce_expr(&val.node, val.span, env)?;
            }
            Ok(())
        }
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    enforce_expr(&e.node, e.span, env)?;
                }
            }
            Ok(())
        }
        // Leaf nodes
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::Ident(_) | Expr::EnumUnit { .. } => Ok(()),
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

    // Class tests

    #[test]
    fn valid_class_construction() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n}").unwrap();
    }

    #[test]
    fn valid_field_access() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n    let v = p.x\n}").unwrap();
    }

    #[test]
    fn valid_method_call() {
        check("class Point {\n    x: int\n    y: int\n\n    fn get_x(self) int {\n        return self.x\n    }\n}\n\nfn main() {\n    let p = Point { x: 1, y: 2 }\n    let v = p.get_x()\n}").unwrap();
    }

    #[test]
    fn wrong_field_type_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: true, y: 2 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn missing_field_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn unknown_field_rejected() {
        let result = check("class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let p = Point { x: 1, z: 2 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn class_as_param() {
        check("class Point {\n    x: int\n    y: int\n}\n\nfn get_x(p: Point) int {\n    return p.x\n}\n\nfn main() {\n    let p = Point { x: 42, y: 0 }\n    let v = get_x(p)\n}").unwrap();
    }

    // Trait tests

    #[test]
    fn valid_trait_basic() {
        check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn trait_missing_method_rejected() {
        let result = check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn trait_unknown_rejected() {
        let result = check("class X impl NonExistent {\n    val: int\n}\n\nfn main() {\n}");
        assert!(result.is_err());
    }

    #[test]
    fn trait_as_param() {
        check("trait Foo {\n    fn bar(self) int\n}\n\nclass X impl Foo {\n    val: int\n\n    fn bar(self) int {\n        return self.val\n    }\n}\n\nfn process(f: Foo) int {\n    return f.bar()\n}\n\nfn main() {\n    let x = X { val: 42 }\n    let r = process(x)\n}").unwrap();
    }

    #[test]
    fn trait_default_method() {
        check("trait Foo {\n    fn bar(self) int {\n        return 0\n    }\n}\n\nclass X impl Foo {\n    val: int\n}\n\nfn main() {\n}").unwrap();
    }

    // Enum tests

    #[test]
    fn enum_registration() {
        let env = check("enum Color {\n    Red\n    Green\n    Blue\n}\n\nfn main() {\n}").unwrap();
        assert!(env.enums.contains_key("Color"));
        assert_eq!(env.enums["Color"].variants.len(), 3);
    }

    #[test]
    fn enum_unit_construction() {
        check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n}").unwrap();
    }

    #[test]
    fn enum_data_construction() {
        check("enum Status {\n    Active\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { reason: \"banned\" }\n}").unwrap();
    }

    #[test]
    fn enum_exhaustive_match() {
        check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            let x = 1\n        }\n        Color.Blue {\n            let x = 2\n        }\n    }\n}").unwrap();
    }

    #[test]
    fn enum_non_exhaustive_rejected() {
        let result = check("enum Color {\n    Red\n    Blue\n}\n\nfn main() {\n    let c = Color.Red\n    match c {\n        Color.Red {\n            let x = 1\n        }\n    }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn enum_wrong_field_name_rejected() {
        let result = check("enum Status {\n    Suspended { reason: string }\n}\n\nfn main() {\n    let s = Status.Suspended { msg: \"banned\" }\n}");
        assert!(result.is_err());
    }

    // Error handling tests

    #[test]
    fn error_decl_registered() {
        let env = check("error NotFound {\n    msg: string\n}\n\nfn main() {\n}").unwrap();
        assert!(env.errors.contains_key("NotFound"));
        assert_eq!(env.errors["NotFound"].fields.len(), 1);
    }

    #[test]
    fn raise_valid() {
        check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n}").unwrap();
    }

    #[test]
    fn raise_unknown_error_rejected() {
        let result = check("fn main() {\n    raise Oops { msg: \"bad\" }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn raise_wrong_field_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn main() {\n    raise Oops { code: 42 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn raise_wrong_field_type_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn main() {\n    raise Oops { msg: 42 }\n}");
        assert!(result.is_err());
    }

    #[test]
    fn propagate_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n    fail()!\n}").unwrap();
    }

    #[test]
    fn propagate_on_infallible_fn_rejected() {
        let result = check("fn safe() {\n}\n\nfn main() {\n    safe()!\n}");
        assert!(result.is_err());
    }

    #[test]
    fn bare_call_to_fallible_fn_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn fail() {\n    raise Oops { msg: \"bad\" }\n}\n\nfn main() {\n    fail()\n}");
        assert!(result.is_err());
    }

    #[test]
    fn catch_shorthand_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get() catch 0\n}").unwrap();
    }

    #[test]
    fn catch_wildcard_on_fallible_fn_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get() catch err { 0 }\n}").unwrap();
    }

    #[test]
    fn catch_on_infallible_fn_rejected() {
        let result = check("fn safe() int {\n    return 42\n}\n\nfn main() {\n    let x = safe() catch 0\n}");
        assert!(result.is_err());
    }

    #[test]
    fn error_propagation_transitive() {
        // a raises, b propagates from a, c propagates from b
        let env = check("error Oops {\n    msg: string\n}\n\nfn a() {\n    raise Oops { msg: \"a\" }\n}\n\nfn b() {\n    a()!\n}\n\nfn c() {\n    b()!\n}\n\nfn main() {\n    c()!\n}").unwrap();
        assert!(env.is_fn_fallible("a"));
        assert!(env.is_fn_fallible("b"));
        assert!(env.is_fn_fallible("c"));
    }

    #[test]
    fn catch_stops_propagation() {
        // a raises, b catches — b should NOT be fallible
        let env = check("error Oops {\n    msg: string\n}\n\nfn a() int {\n    raise Oops { msg: \"a\" }\n    return 0\n}\n\nfn b() {\n    let x = a() catch 0\n}\n\nfn main() {\n    b()\n}").unwrap();
        assert!(env.is_fn_fallible("a"));
        assert!(!env.is_fn_fallible("b"));
    }

    #[test]
    fn let_with_propagation_ok() {
        check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get()!\n}").unwrap();
    }

    #[test]
    fn let_bare_call_to_fallible_rejected() {
        let result = check("error Oops {\n    msg: string\n}\n\nfn get() int {\n    raise Oops { msg: \"bad\" }\n    return 0\n}\n\nfn main() {\n    let x = get()\n}");
        assert!(result.is_err());
    }
}

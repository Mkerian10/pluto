use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::typeck::env::{mangle_method, mangle_name, InstKind, Instantiation, TypeEnv};
use crate::typeck::types::{PlutoType, pluto_type_to_type_expr};

/// Span offset multiplier for monomorphized bodies. Each iteration gets unique
/// spans to avoid closure capture key collisions. Must exceed any realistic
/// source file size.
const SPAN_OFFSET_MULTIPLIER: usize = 10_000_000;

/// Monomorphize generic items: instantiate concrete copies, type-check their bodies,
/// rewrite call sites via the rewrite map, then remove generic templates.
pub fn monomorphize(program: &mut Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    // Phase 1: Instantiate generic bodies (fixed-point loop)
    let mut processed: HashSet<Instantiation> = HashSet::new();
    let mut iteration = 0;

    loop {
        let pending: Vec<Instantiation> = env
            .instantiations
            .iter()
            .filter(|inst| !processed.contains(*inst))
            .cloned()
            .collect();

        if pending.is_empty() {
            break;
        }

        for inst in pending {
            iteration += 1;
            let span_offset = iteration * SPAN_OFFSET_MULTIPLIER;
            let mangled = mangle_name(
                match &inst.kind {
                    InstKind::Function(n) | InstKind::Class(n) | InstKind::Enum(n) => n.as_str(),
                },
                &inst.type_args,
            );

            match &inst.kind {
                InstKind::Function(name) => {
                    instantiate_function(program, env, name, &inst.type_args, &mangled, span_offset)?;
                }
                InstKind::Class(name) => {
                    instantiate_class(program, env, name, &inst.type_args, &mangled, span_offset)?;
                }
                InstKind::Enum(name) => {
                    instantiate_enum(program, name, &inst.type_args, &mangled, span_offset);
                }
            }

            processed.insert(inst);
        }
    }

    // Phase 2: Rewrite call sites using the rewrite map
    rewrite_program(program, &env.generic_rewrites);

    // Phase 2b: Resolve all TypeExpr::Generic nodes to TypeExpr::Named(mangled)
    resolve_all_generic_type_exprs(program, env)?;

    // Phase 3: Remove generic templates
    program.functions.retain(|f| f.node.type_params.is_empty());
    program.classes.retain(|c| c.node.type_params.is_empty());
    program.enums.retain(|e| e.node.type_params.is_empty());

    Ok(())
}

// ── Phase 1: Instantiation ──────────────────────────────────────────

fn instantiate_function(
    program: &mut Program,
    env: &mut TypeEnv,
    name: &str,
    type_args: &[PlutoType],
    mangled: &str,
    span_offset: usize,
) -> Result<(), CompileError> {
    // Find the generic function template
    let template = program
        .functions
        .iter()
        .find(|f| f.node.name.node == name && !f.node.type_params.is_empty())
        .ok_or_else(|| CompileError::type_err(format!("generic function '{}' not found", name), Span::dummy()))?
        .clone();

    let type_params: Vec<String> = template.node.type_params.iter().map(|tp| tp.node.clone()).collect();
    let bindings = build_type_expr_bindings(&type_params, type_args);

    // Clone and substitute
    let mut func = template.node.clone();
    func.id = Uuid::new_v4();
    reassign_function_uuids(&mut func);
    func.name = Spanned::new(mangled.to_string(), template.node.name.span);
    func.type_params.clear();
    substitute_in_function(&mut func, &bindings);
    offset_function_spans(&mut func, span_offset);

    // Add to program (preserve template's file_id for DeclKeyMap)
    let spanned_func = Spanned::new(func.clone(), Span::with_file(
        template.span.start + span_offset,
        template.span.end + span_offset,
        template.span.file_id,
    ));
    program.functions.push(spanned_func);

    // Type-check the body to discover transitive instantiations
    crate::typeck::check_function(&func, env, None)?;

    Ok(())
}

fn instantiate_class(
    program: &mut Program,
    env: &mut TypeEnv,
    name: &str,
    type_args: &[PlutoType],
    mangled: &str,
    span_offset: usize,
) -> Result<(), CompileError> {
    let template = program
        .classes
        .iter()
        .find(|c| c.node.name.node == name && !c.node.type_params.is_empty())
        .ok_or_else(|| CompileError::type_err(format!("generic class '{}' not found", name), Span::dummy()))?
        .clone();

    let type_params: Vec<String> = template.node.type_params.iter().map(|tp| tp.node.clone()).collect();
    let bindings = build_type_expr_bindings(&type_params, type_args);

    let mut class = template.node.clone();
    class.id = Uuid::new_v4();
    reassign_class_uuids(&mut class);
    class.name = Spanned::new(mangled.to_string(), template.node.name.span);
    class.type_params.clear();
    substitute_in_class(&mut class, &bindings);
    offset_class_spans(&mut class, span_offset);

    // Add to program (preserve template's file_id for DeclKeyMap)
    let spanned_class = Spanned::new(class.clone(), Span::with_file(
        template.span.start + span_offset,
        template.span.end + span_offset,
        template.span.file_id,
    ));
    program.classes.push(spanned_class);

    // Type-check methods to discover transitive instantiations
    for method in &class.methods {
        // Register method signature if not already registered
        let method_name = mangle_method(&mangled, &method.node.name.node);
        if !env.functions.contains_key(&method_name) {
            // Build the FuncSig for this method
            let mut param_types = Vec::new();
            for p in &method.node.params {
                if p.name.node == "self" {
                    param_types.push(PlutoType::Class(mangled.to_string()));
                } else {
                    let ty = crate::typeck::resolve_type_for_monomorphize(&p.ty, env)?;
                    param_types.push(ty);
                }
            }
            let return_type = match &method.node.return_type {
                Some(rt) => crate::typeck::resolve_type_for_monomorphize(rt, env)?,
                None => PlutoType::Void,
            };
            // Propagate mut self
            if !method.node.params.is_empty()
                && method.node.params[0].name.node == "self"
                && method.node.params[0].is_mut
            {
                env.mut_self_methods.insert(method_name.clone());
            }
            env.functions.insert(method_name, crate::typeck::env::FuncSig {
                params: param_types,
                return_type,
            });
        }
    }

    // Register default trait methods for monomorphized classes with impl_traits
    let class_method_names: Vec<String> = class.methods.iter().map(|m| m.node.name.node.clone()).collect();
    for trait_name_spanned in &class.impl_traits {
        let trait_name = &trait_name_spanned.node;
        if let Some(trait_info) = env.traits.get(trait_name).cloned() {
            for (method_name, trait_sig) in &trait_info.methods {
                if !class_method_names.contains(method_name) && trait_info.default_methods.contains(method_name) {
                    let method_mangled = mangle_method(mangled, method_name);
                    if !env.functions.contains_key(&method_mangled) {
                        let mut params = trait_sig.params.clone();
                        // Replace the Void placeholder self param with the concrete class type
                        if !params.is_empty() {
                            params[0] = PlutoType::Class(mangled.to_string());
                        }
                        env.functions.insert(
                            method_mangled.clone(),
                            crate::typeck::env::FuncSig {
                                params,
                                return_type: trait_sig.return_type.clone(),
                            },
                        );
                        // Propagate mut self from trait default method
                        if trait_info.mut_self_methods.contains(method_name) {
                            env.mut_self_methods.insert(method_mangled);
                        }
                        // Add method name to class info
                        if let Some(info) = env.classes.get_mut(mangled) {
                            if !info.methods.contains(method_name) {
                                info.methods.push(method_name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Type-check method bodies
    for method in &class.methods {
        crate::typeck::check_function(&method.node, env, Some(mangled))?;
    }

    Ok(())
}

fn instantiate_enum(
    program: &mut Program,
    name: &str,
    _type_args: &[PlutoType],
    mangled: &str,
    span_offset: usize,
) {
    let template = match program
        .enums
        .iter()
        .find(|e| e.node.name.node == name && !e.node.type_params.is_empty())
    {
        Some(t) => t.clone(),
        None => return, // Already instantiated or not found
    };

    let type_params: Vec<String> = template.node.type_params.iter().map(|tp| tp.node.clone()).collect();
    let bindings = build_type_expr_bindings(&type_params, _type_args);

    let mut edecl = template.node.clone();
    edecl.id = Uuid::new_v4();
    reassign_enum_uuids(&mut edecl);
    edecl.name = Spanned::new(mangled.to_string(), template.node.name.span);
    edecl.type_params.clear();
    substitute_in_enum(&mut edecl, &bindings);
    offset_enum_spans(&mut edecl, span_offset);

    let spanned_enum = Spanned::new(edecl, Span::with_file(
        template.span.start + span_offset,
        template.span.end + span_offset,
        template.span.file_id,
    ));
    program.enums.push(spanned_enum);
}

// ── Type substitution helpers ───────────────────────────────────────

/// Build a map from type param names to concrete TypeExpr values.
fn build_type_expr_bindings(type_params: &[String], type_args: &[PlutoType]) -> HashMap<String, TypeExpr> {
    type_params
        .iter()
        .zip(type_args.iter())
        .map(|(name, ty)| (name.clone(), pluto_type_to_type_expr(ty)))
        .collect()
}

fn substitute_in_type_expr(te: &mut TypeExpr, bindings: &HashMap<String, TypeExpr>) {
    match te {
        TypeExpr::Named(name) => {
            if let Some(replacement) = bindings.get(name) {
                *te = replacement.clone();
            }
        }
        TypeExpr::Array(inner) => {
            substitute_in_type_expr(&mut inner.node, bindings);
        }
        TypeExpr::Qualified { .. } => {}
        TypeExpr::Fn { params, return_type } => {
            for p in params.iter_mut() {
                substitute_in_type_expr(&mut p.node, bindings);
            }
            substitute_in_type_expr(&mut return_type.node, bindings);
        }
        TypeExpr::Generic { type_args, .. } => {
            for arg in type_args.iter_mut() {
                substitute_in_type_expr(&mut arg.node, bindings);
            }
        }
        TypeExpr::Nullable(inner) => {
            substitute_in_type_expr(&mut inner.node, bindings);
        }
    }
}

/// Reassign UUIDs for all nested declarations within a Function (params).
fn reassign_function_uuids(func: &mut Function) {
    for p in &mut func.params {
        p.id = Uuid::new_v4();
    }
}

/// Reassign UUIDs for all nested declarations within a ClassDecl (fields, methods, params).
fn reassign_class_uuids(class: &mut ClassDecl) {
    for f in &mut class.fields {
        f.id = Uuid::new_v4();
    }
    for method in &mut class.methods {
        method.node.id = Uuid::new_v4();
        reassign_function_uuids(&mut method.node);
    }
}

/// Reassign UUIDs for all nested declarations within an EnumDecl (variants, fields).
fn reassign_enum_uuids(edecl: &mut EnumDecl) {
    for variant in &mut edecl.variants {
        variant.id = Uuid::new_v4();
        for f in &mut variant.fields {
            f.id = Uuid::new_v4();
        }
    }
}

fn substitute_in_function(func: &mut Function, bindings: &HashMap<String, TypeExpr>) {
    // Substitute in parameter types
    for p in &mut func.params {
        substitute_in_type_expr(&mut p.ty.node, bindings);
    }
    // Substitute in return type
    if let Some(ref mut rt) = func.return_type {
        substitute_in_type_expr(&mut rt.node, bindings);
    }
    // Substitute in body
    substitute_in_block(&mut func.body.node, bindings);
}

fn substitute_in_class(class: &mut ClassDecl, bindings: &HashMap<String, TypeExpr>) {
    for field in &mut class.fields {
        substitute_in_type_expr(&mut field.ty.node, bindings);
    }
    for method in &mut class.methods {
        substitute_in_function(&mut method.node, bindings);
    }
}

fn substitute_in_enum(edecl: &mut EnumDecl, bindings: &HashMap<String, TypeExpr>) {
    for variant in &mut edecl.variants {
        for field in &mut variant.fields {
            substitute_in_type_expr(&mut field.ty.node, bindings);
        }
    }
}

fn substitute_in_block(block: &mut Block, bindings: &HashMap<String, TypeExpr>) {
    for stmt in &mut block.stmts {
        substitute_in_stmt(&mut stmt.node, bindings);
    }
}

fn substitute_in_stmt(stmt: &mut Stmt, bindings: &HashMap<String, TypeExpr>) {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(t) = ty {
                substitute_in_type_expr(&mut t.node, bindings);
            }
            substitute_in_expr(&mut value.node, bindings);
        }
        Stmt::Return(Some(expr)) => {
            substitute_in_expr(&mut expr.node, bindings);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            substitute_in_expr(&mut value.node, bindings);
        }
        Stmt::FieldAssign { object, value, .. } => {
            substitute_in_expr(&mut object.node, bindings);
            substitute_in_expr(&mut value.node, bindings);
        }
        Stmt::If { condition, then_block, else_block } => {
            substitute_in_expr(&mut condition.node, bindings);
            substitute_in_block(&mut then_block.node, bindings);
            if let Some(eb) = else_block {
                substitute_in_block(&mut eb.node, bindings);
            }
        }
        Stmt::While { condition, body } => {
            substitute_in_expr(&mut condition.node, bindings);
            substitute_in_block(&mut body.node, bindings);
        }
        Stmt::For { iterable, body, .. } => {
            substitute_in_expr(&mut iterable.node, bindings);
            substitute_in_block(&mut body.node, bindings);
        }
        Stmt::IndexAssign { object, index, value } => {
            substitute_in_expr(&mut object.node, bindings);
            substitute_in_expr(&mut index.node, bindings);
            substitute_in_expr(&mut value.node, bindings);
        }
        Stmt::Match { expr, arms } => {
            substitute_in_expr(&mut expr.node, bindings);
            for arm in arms.iter_mut() {
                substitute_in_block(&mut arm.body.node, bindings);
                for ta in &mut arm.type_args {
                    substitute_in_type_expr(&mut ta.node, bindings);
                }
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, expr) in fields.iter_mut() {
                substitute_in_expr(&mut expr.node, bindings);
            }
        }
        Stmt::Expr(expr) => {
            substitute_in_expr(&mut expr.node, bindings);
        }
        Stmt::LetChan { elem_type, capacity, .. } => {
            substitute_in_type_expr(&mut elem_type.node, bindings);
            if let Some(cap) = capacity {
                substitute_in_expr(&mut cap.node, bindings);
            }
        }
        Stmt::Scope { seeds, bindings: scope_bindings, body } => {
            for seed in seeds {
                substitute_in_expr(&mut seed.node, bindings);
            }
            for sb in scope_bindings {
                substitute_in_type_expr(&mut sb.ty.node, bindings);
            }
            substitute_in_block(&mut body.node, bindings);
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        substitute_in_expr(&mut channel.node, bindings);
                    }
                    SelectOp::Send { channel, value } => {
                        substitute_in_expr(&mut channel.node, bindings);
                        substitute_in_expr(&mut value.node, bindings);
                    }
                }
                substitute_in_block(&mut arm.body.node, bindings);
            }
            if let Some(def) = default {
                substitute_in_block(&mut def.node, bindings);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn substitute_in_expr(expr: &mut Expr, bindings: &HashMap<String, TypeExpr>) {
    match expr {
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
        | Expr::StringLit(_) | Expr::Ident(_) | Expr::NoneLit => {}
        Expr::NullPropagate { expr } => {
            substitute_in_expr(&mut expr.node, bindings);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            substitute_in_expr(&mut lhs.node, bindings);
            substitute_in_expr(&mut rhs.node, bindings);
        }
        Expr::UnaryOp { operand, .. } => {
            substitute_in_expr(&mut operand.node, bindings);
        }
        Expr::Cast { expr: inner, target_type } => {
            substitute_in_expr(&mut inner.node, bindings);
            substitute_in_type_expr(&mut target_type.node, bindings);
        }
        Expr::Call { args, type_args, .. } => {
            for arg in args.iter_mut() {
                substitute_in_expr(&mut arg.node, bindings);
            }
            for ta in type_args.iter_mut() {
                substitute_in_type_expr(&mut ta.node, bindings);
            }
        }
        Expr::FieldAccess { object, .. } => {
            substitute_in_expr(&mut object.node, bindings);
        }
        Expr::MethodCall { object, args, .. } => {
            substitute_in_expr(&mut object.node, bindings);
            for arg in args.iter_mut() {
                substitute_in_expr(&mut arg.node, bindings);
            }
        }
        Expr::StructLit { type_args, fields, .. } => {
            for ta in type_args.iter_mut() {
                substitute_in_type_expr(&mut ta.node, bindings);
            }
            for (_, fexpr) in fields.iter_mut() {
                substitute_in_expr(&mut fexpr.node, bindings);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements.iter_mut() {
                substitute_in_expr(&mut el.node, bindings);
            }
        }
        Expr::Index { object, index } => {
            substitute_in_expr(&mut object.node, bindings);
            substitute_in_expr(&mut index.node, bindings);
        }
        Expr::EnumUnit { type_args, .. } => {
            for ta in type_args.iter_mut() {
                substitute_in_type_expr(&mut ta.node, bindings);
            }
        }
        Expr::EnumData { type_args, fields, .. } => {
            for ta in type_args.iter_mut() {
                substitute_in_type_expr(&mut ta.node, bindings);
            }
            for (_, fexpr) in fields.iter_mut() {
                substitute_in_expr(&mut fexpr.node, bindings);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts.iter_mut() {
                if let StringInterpPart::Expr(e) = part {
                    substitute_in_expr(&mut e.node, bindings);
                }
            }
        }
        Expr::Closure { params, return_type, body } => {
            for p in params.iter_mut() {
                substitute_in_type_expr(&mut p.ty.node, bindings);
            }
            if let Some(rt) = return_type {
                substitute_in_type_expr(&mut rt.node, bindings);
            }
            substitute_in_block(&mut body.node, bindings);
        }
        Expr::MapLit { key_type, value_type, entries } => {
            substitute_in_type_expr(&mut key_type.node, bindings);
            substitute_in_type_expr(&mut value_type.node, bindings);
            for (k, v) in entries.iter_mut() {
                substitute_in_expr(&mut k.node, bindings);
                substitute_in_expr(&mut v.node, bindings);
            }
        }
        Expr::SetLit { elem_type, elements } => {
            substitute_in_type_expr(&mut elem_type.node, bindings);
            for el in elements.iter_mut() {
                substitute_in_expr(&mut el.node, bindings);
            }
        }
        Expr::ClosureCreate { .. } => {}
        Expr::Range { start, end, .. } => {
            substitute_in_expr(&mut start.node, bindings);
            substitute_in_expr(&mut end.node, bindings);
        }
        Expr::Propagate { expr } => {
            substitute_in_expr(&mut expr.node, bindings);
        }
        Expr::Catch { expr, handler } => {
            substitute_in_expr(&mut expr.node, bindings);
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    substitute_in_block(&mut body.node, bindings);
                }
                CatchHandler::Shorthand(body) => {
                    substitute_in_expr(&mut body.node, bindings);
                }
            }
        }
        Expr::Spawn { call } => {
            substitute_in_expr(&mut call.node, bindings);
        }
    }
}

// ── Span offsetting ─────────────────────────────────────────────────

fn offset_span(span: &mut Span, offset: usize) {
    span.start += offset;
    span.end += offset;
}

fn offset_spanned<T>(s: &mut Spanned<T>, offset: usize) {
    offset_span(&mut s.span, offset);
}

fn offset_function_spans(func: &mut Function, offset: usize) {
    offset_spanned(&mut func.name, offset);
    for tp in &mut func.type_params {
        offset_spanned(tp, offset);
    }
    for p in &mut func.params {
        offset_spanned(&mut p.name, offset);
        offset_spanned(&mut p.ty, offset);
        offset_type_expr_spans(&mut p.ty.node, offset);
    }
    if let Some(ref mut rt) = func.return_type {
        offset_spanned(rt, offset);
        offset_type_expr_spans(&mut rt.node, offset);
    }
    offset_spanned(&mut func.body, offset);
    offset_block_spans(&mut func.body.node, offset);
}

fn offset_class_spans(class: &mut ClassDecl, offset: usize) {
    offset_spanned(&mut class.name, offset);
    for f in &mut class.fields {
        offset_spanned(&mut f.name, offset);
        offset_spanned(&mut f.ty, offset);
        offset_type_expr_spans(&mut f.ty.node, offset);
    }
    for method in &mut class.methods {
        offset_spanned(method, offset);
        offset_function_spans(&mut method.node, offset);
    }
}

fn offset_enum_spans(edecl: &mut EnumDecl, offset: usize) {
    offset_spanned(&mut edecl.name, offset);
    for variant in &mut edecl.variants {
        offset_spanned(&mut variant.name, offset);
        for field in &mut variant.fields {
            offset_spanned(&mut field.name, offset);
            offset_spanned(&mut field.ty, offset);
            offset_type_expr_spans(&mut field.ty.node, offset);
        }
    }
}

fn offset_type_expr_spans(te: &mut TypeExpr, offset: usize) {
    match te {
        TypeExpr::Named(_) | TypeExpr::Qualified { .. } => {}
        TypeExpr::Array(inner) => {
            offset_spanned(inner, offset);
            offset_type_expr_spans(&mut inner.node, offset);
        }
        TypeExpr::Fn { params, return_type } => {
            for p in params.iter_mut() {
                offset_spanned(p, offset);
                offset_type_expr_spans(&mut p.node, offset);
            }
            offset_spanned(return_type, offset);
            offset_type_expr_spans(&mut return_type.node, offset);
        }
        TypeExpr::Generic { type_args, .. } => {
            for arg in type_args.iter_mut() {
                offset_spanned(arg, offset);
                offset_type_expr_spans(&mut arg.node, offset);
            }
        }
        TypeExpr::Nullable(inner) => {
            offset_spanned(inner, offset);
            offset_type_expr_spans(&mut inner.node, offset);
        }
    }
}

fn offset_block_spans(block: &mut Block, offset: usize) {
    for stmt in &mut block.stmts {
        offset_spanned(stmt, offset);
        offset_stmt_spans(&mut stmt.node, offset);
    }
}

fn offset_stmt_spans(stmt: &mut Stmt, offset: usize) {
    match stmt {
        Stmt::Let { name, ty, value, .. } => {
            offset_spanned(name, offset);
            if let Some(t) = ty {
                offset_spanned(t, offset);
                offset_type_expr_spans(&mut t.node, offset);
            }
            offset_spanned(value, offset);
            offset_expr_spans(&mut value.node, offset);
        }
        Stmt::Return(Some(expr)) => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { target, value } => {
            offset_spanned(target, offset);
            offset_spanned(value, offset);
            offset_expr_spans(&mut value.node, offset);
        }
        Stmt::FieldAssign { object, field, value } => {
            offset_spanned(object, offset);
            offset_expr_spans(&mut object.node, offset);
            offset_spanned(field, offset);
            offset_spanned(value, offset);
            offset_expr_spans(&mut value.node, offset);
        }
        Stmt::If { condition, then_block, else_block } => {
            offset_spanned(condition, offset);
            offset_expr_spans(&mut condition.node, offset);
            offset_spanned(then_block, offset);
            offset_block_spans(&mut then_block.node, offset);
            if let Some(eb) = else_block {
                offset_spanned(eb, offset);
                offset_block_spans(&mut eb.node, offset);
            }
        }
        Stmt::While { condition, body } => {
            offset_spanned(condition, offset);
            offset_expr_spans(&mut condition.node, offset);
            offset_spanned(body, offset);
            offset_block_spans(&mut body.node, offset);
        }
        Stmt::For { var, iterable, body } => {
            offset_spanned(var, offset);
            offset_spanned(iterable, offset);
            offset_expr_spans(&mut iterable.node, offset);
            offset_spanned(body, offset);
            offset_block_spans(&mut body.node, offset);
        }
        Stmt::IndexAssign { object, index, value } => {
            offset_spanned(object, offset);
            offset_expr_spans(&mut object.node, offset);
            offset_spanned(index, offset);
            offset_expr_spans(&mut index.node, offset);
            offset_spanned(value, offset);
            offset_expr_spans(&mut value.node, offset);
        }
        Stmt::Match { expr, arms } => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
            for arm in arms.iter_mut() {
                offset_spanned(&mut arm.enum_name, offset);
                offset_spanned(&mut arm.variant_name, offset);
                for ta in &mut arm.type_args {
                    offset_spanned(ta, offset);
                    offset_type_expr_spans(&mut ta.node, offset);
                }
                for (fname, binding) in &mut arm.bindings {
                    offset_spanned(fname, offset);
                    if let Some(b) = binding {
                        offset_spanned(b, offset);
                    }
                }
                offset_spanned(&mut arm.body, offset);
                offset_block_spans(&mut arm.body.node, offset);
            }
        }
        Stmt::Raise { error_name, fields, .. } => {
            offset_spanned(error_name, offset);
            for (fname, fexpr) in fields.iter_mut() {
                offset_spanned(fname, offset);
                offset_spanned(fexpr, offset);
                offset_expr_spans(&mut fexpr.node, offset);
            }
        }
        Stmt::Expr(expr) => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
        }
        Stmt::LetChan { sender, receiver, elem_type, capacity } => {
            offset_spanned(sender, offset);
            offset_spanned(receiver, offset);
            offset_spanned(elem_type, offset);
            offset_type_expr_spans(&mut elem_type.node, offset);
            if let Some(cap) = capacity {
                offset_spanned(cap, offset);
                offset_expr_spans(&mut cap.node, offset);
            }
        }
        Stmt::Scope { seeds, bindings, body } => {
            for seed in seeds {
                offset_spanned(seed, offset);
                offset_expr_spans(&mut seed.node, offset);
            }
            for binding in bindings {
                offset_spanned(&mut binding.name, offset);
                offset_spanned(&mut binding.ty, offset);
                offset_type_expr_spans(&mut binding.ty.node, offset);
            }
            offset_spanned(body, offset);
            offset_block_spans(&mut body.node, offset);
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { binding, channel } => {
                        offset_spanned(binding, offset);
                        offset_spanned(channel, offset);
                        offset_expr_spans(&mut channel.node, offset);
                    }
                    SelectOp::Send { channel, value } => {
                        offset_spanned(channel, offset);
                        offset_expr_spans(&mut channel.node, offset);
                        offset_spanned(value, offset);
                        offset_expr_spans(&mut value.node, offset);
                    }
                }
                offset_spanned(&mut arm.body, offset);
                offset_block_spans(&mut arm.body.node, offset);
            }
            if let Some(def) = default {
                offset_spanned(def, offset);
                offset_block_spans(&mut def.node, offset);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn offset_expr_spans(expr: &mut Expr, offset: usize) {
    match expr {
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
        | Expr::StringLit(_) | Expr::Ident(_) | Expr::ClosureCreate { .. }
        | Expr::NoneLit => {}
        Expr::NullPropagate { expr } => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            offset_spanned(lhs, offset);
            offset_expr_spans(&mut lhs.node, offset);
            offset_spanned(rhs, offset);
            offset_expr_spans(&mut rhs.node, offset);
        }
        Expr::Range { start, end, .. } => {
            offset_spanned(start, offset);
            offset_expr_spans(&mut start.node, offset);
            offset_spanned(end, offset);
            offset_expr_spans(&mut end.node, offset);
        }
        Expr::UnaryOp { operand, .. } => {
            offset_spanned(operand, offset);
            offset_expr_spans(&mut operand.node, offset);
        }
        Expr::Cast { expr: inner, target_type } => {
            offset_spanned(inner, offset);
            offset_expr_spans(&mut inner.node, offset);
            offset_spanned(target_type, offset);
            offset_type_expr_spans(&mut target_type.node, offset);
        }
        Expr::Call { name, args, .. } => {
            offset_spanned(name, offset);
            for arg in args.iter_mut() {
                offset_spanned(arg, offset);
                offset_expr_spans(&mut arg.node, offset);
            }
        }
        Expr::FieldAccess { object, field } => {
            offset_spanned(object, offset);
            offset_expr_spans(&mut object.node, offset);
            offset_spanned(field, offset);
        }
        Expr::MethodCall { object, method, args } => {
            offset_spanned(object, offset);
            offset_expr_spans(&mut object.node, offset);
            offset_spanned(method, offset);
            for arg in args.iter_mut() {
                offset_spanned(arg, offset);
                offset_expr_spans(&mut arg.node, offset);
            }
        }
        Expr::StructLit { name, type_args, fields, .. } => {
            offset_spanned(name, offset);
            for ta in type_args.iter_mut() {
                offset_spanned(ta, offset);
                offset_type_expr_spans(&mut ta.node, offset);
            }
            for (fname, fexpr) in fields.iter_mut() {
                offset_spanned(fname, offset);
                offset_spanned(fexpr, offset);
                offset_expr_spans(&mut fexpr.node, offset);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements.iter_mut() {
                offset_spanned(el, offset);
                offset_expr_spans(&mut el.node, offset);
            }
        }
        Expr::Index { object, index } => {
            offset_spanned(object, offset);
            offset_expr_spans(&mut object.node, offset);
            offset_spanned(index, offset);
            offset_expr_spans(&mut index.node, offset);
        }
        Expr::EnumUnit { enum_name, variant, type_args, .. } => {
            offset_spanned(enum_name, offset);
            offset_spanned(variant, offset);
            for ta in type_args.iter_mut() {
                offset_spanned(ta, offset);
                offset_type_expr_spans(&mut ta.node, offset);
            }
        }
        Expr::EnumData { enum_name, variant, type_args, fields, .. } => {
            offset_spanned(enum_name, offset);
            offset_spanned(variant, offset);
            for ta in type_args.iter_mut() {
                offset_spanned(ta, offset);
                offset_type_expr_spans(&mut ta.node, offset);
            }
            for (fname, fexpr) in fields.iter_mut() {
                offset_spanned(fname, offset);
                offset_spanned(fexpr, offset);
                offset_expr_spans(&mut fexpr.node, offset);
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts.iter_mut() {
                if let StringInterpPart::Expr(e) = part {
                    offset_spanned(e, offset);
                    offset_expr_spans(&mut e.node, offset);
                }
            }
        }
        Expr::Closure { params, return_type, body } => {
            for p in params.iter_mut() {
                offset_spanned(&mut p.name, offset);
                offset_spanned(&mut p.ty, offset);
                offset_type_expr_spans(&mut p.ty.node, offset);
            }
            if let Some(rt) = return_type {
                offset_spanned(rt, offset);
                offset_type_expr_spans(&mut rt.node, offset);
            }
            offset_spanned(body, offset);
            offset_block_spans(&mut body.node, offset);
        }
        Expr::Propagate { expr } => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
        }
        Expr::Catch { expr, handler } => {
            offset_spanned(expr, offset);
            offset_expr_spans(&mut expr.node, offset);
            match handler {
                CatchHandler::Wildcard { var, body } => {
                    offset_spanned(var, offset);
                    offset_spanned(body, offset);
                    offset_block_spans(&mut body.node, offset);
                }
                CatchHandler::Shorthand(body) => {
                    offset_spanned(body, offset);
                    offset_expr_spans(&mut body.node, offset);
                }
            }
        }
        Expr::MapLit { key_type, value_type, entries } => {
            offset_spanned(key_type, offset);
            offset_type_expr_spans(&mut key_type.node, offset);
            offset_spanned(value_type, offset);
            offset_type_expr_spans(&mut value_type.node, offset);
            for (k, v) in entries.iter_mut() {
                offset_spanned(k, offset);
                offset_expr_spans(&mut k.node, offset);
                offset_spanned(v, offset);
                offset_expr_spans(&mut v.node, offset);
            }
        }
        Expr::SetLit { elem_type, elements } => {
            offset_spanned(elem_type, offset);
            offset_type_expr_spans(&mut elem_type.node, offset);
            for el in elements.iter_mut() {
                offset_spanned(el, offset);
                offset_expr_spans(&mut el.node, offset);
            }
        }
        Expr::Spawn { call } => {
            offset_spanned(call, offset);
            offset_expr_spans(&mut call.node, offset);
        }
    }
}

// ── Phase 2: Rewrite call sites ─────────────────────────────────────

fn rewrite_program(program: &mut Program, rewrites: &HashMap<(usize, usize), String>) {
    if rewrites.is_empty() {
        return;
    }

    for func in &mut program.functions {
        rewrite_block(&mut func.node.body.node, rewrites);
    }
    for class in &mut program.classes {
        for method in &mut class.node.methods {
            rewrite_block(&mut method.node.body.node, rewrites);
        }
    }
    if let Some(ref mut app) = program.app {
        for method in &mut app.node.methods {
            rewrite_block(&mut method.node.body.node, rewrites);
        }
    }
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
            rewrite_block(&mut method.node.body.node, rewrites);
        }
    }
}

fn rewrite_block(block: &mut Block, rewrites: &HashMap<(usize, usize), String>) {
    for stmt in &mut block.stmts {
        rewrite_stmt(&mut stmt.node, rewrites);
    }
}

fn rewrite_stmt(stmt: &mut Stmt, rewrites: &HashMap<(usize, usize), String>) {
    match stmt {
        Stmt::Let { value, .. } => {
            rewrite_expr(&mut value.node, value.span.start, value.span.end, rewrites);
        }
        Stmt::Return(Some(expr)) => {
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
        }
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => {
            rewrite_expr(&mut value.node, value.span.start, value.span.end, rewrites);
        }
        Stmt::FieldAssign { object, value, .. } => {
            rewrite_expr(&mut object.node, object.span.start, object.span.end, rewrites);
            rewrite_expr(&mut value.node, value.span.start, value.span.end, rewrites);
        }
        Stmt::If { condition, then_block, else_block } => {
            rewrite_expr(&mut condition.node, condition.span.start, condition.span.end, rewrites);
            rewrite_block(&mut then_block.node, rewrites);
            if let Some(eb) = else_block {
                rewrite_block(&mut eb.node, rewrites);
            }
        }
        Stmt::While { condition, body } => {
            rewrite_expr(&mut condition.node, condition.span.start, condition.span.end, rewrites);
            rewrite_block(&mut body.node, rewrites);
        }
        Stmt::For { iterable, body, .. } => {
            rewrite_expr(&mut iterable.node, iterable.span.start, iterable.span.end, rewrites);
            rewrite_block(&mut body.node, rewrites);
        }
        Stmt::IndexAssign { object, index, value } => {
            rewrite_expr(&mut object.node, object.span.start, object.span.end, rewrites);
            rewrite_expr(&mut index.node, index.span.start, index.span.end, rewrites);
            rewrite_expr(&mut value.node, value.span.start, value.span.end, rewrites);
        }
        Stmt::Match { expr, arms } => {
            let match_span = (expr.span.start, expr.span.end);
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
            // Rewrite enum names in match arms
            for arm in arms.iter_mut() {
                // Check if the match statement has a rewrite for the arm's enum name
                // Use the arm's enum_name span to look up rewrites
                let arm_key = (arm.enum_name.span.start, arm.enum_name.span.end);
                if let Some(mangled) = rewrites.get(&arm_key) {
                    arm.enum_name.node = mangled.clone();
                }
                // Also check if match expr span maps to a rewrite
                if let Some(mangled) = rewrites.get(&match_span) {
                    arm.enum_name.node = mangled.clone();
                }
                rewrite_block(&mut arm.body.node, rewrites);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, fexpr) in fields.iter_mut() {
                rewrite_expr(&mut fexpr.node, fexpr.span.start, fexpr.span.end, rewrites);
            }
        }
        Stmt::Expr(expr) => {
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
        }
        Stmt::LetChan { capacity, .. } => {
            if let Some(cap) = capacity {
                rewrite_expr(&mut cap.node, cap.span.start, cap.span.end, rewrites);
            }
        }
        Stmt::Scope { seeds, body, .. } => {
            for seed in seeds {
                rewrite_expr(&mut seed.node, seed.span.start, seed.span.end, rewrites);
            }
            rewrite_block(&mut body.node, rewrites);
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        rewrite_expr(&mut channel.node, channel.span.start, channel.span.end, rewrites);
                    }
                    SelectOp::Send { channel, value } => {
                        rewrite_expr(&mut channel.node, channel.span.start, channel.span.end, rewrites);
                        rewrite_expr(&mut value.node, value.span.start, value.span.end, rewrites);
                    }
                }
                rewrite_block(&mut arm.body.node, rewrites);
            }
            if let Some(def) = default {
                rewrite_block(&mut def.node, rewrites);
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
}

fn rewrite_expr(expr: &mut Expr, start: usize, end: usize, rewrites: &HashMap<(usize, usize), String>) {
    match expr {
        Expr::Call { name, args, type_args, .. } => {
            // Check if this call site should be rewritten
            if let Some(mangled) = rewrites.get(&(start, end)) {
                name.node = mangled.clone();
                type_args.clear();
            }
            for arg in args.iter_mut() {
                rewrite_expr(&mut arg.node, arg.span.start, arg.span.end, rewrites);
            }
        }
        Expr::StructLit { name, type_args, fields, .. } => {
            if let Some(mangled) = rewrites.get(&(start, end)) {
                name.node = mangled.clone();
                type_args.clear();
            }
            for (_, fexpr) in fields.iter_mut() {
                rewrite_expr(&mut fexpr.node, fexpr.span.start, fexpr.span.end, rewrites);
            }
        }
        Expr::EnumUnit { enum_name, type_args, .. } => {
            if let Some(mangled) = rewrites.get(&(start, end)) {
                enum_name.node = mangled.clone();
                type_args.clear();
            }
        }
        Expr::EnumData { enum_name, type_args, fields, .. } => {
            if let Some(mangled) = rewrites.get(&(start, end)) {
                enum_name.node = mangled.clone();
                type_args.clear();
            }
            for (_, fexpr) in fields.iter_mut() {
                rewrite_expr(&mut fexpr.node, fexpr.span.start, fexpr.span.end, rewrites);
            }
        }
        // Recurse into sub-expressions
        Expr::BinOp { lhs, rhs, .. } => {
            rewrite_expr(&mut lhs.node, lhs.span.start, lhs.span.end, rewrites);
            rewrite_expr(&mut rhs.node, rhs.span.start, rhs.span.end, rewrites);
        }
        Expr::Range { start, end, .. } => {
            rewrite_expr(&mut start.node, start.span.start, start.span.end, rewrites);
            rewrite_expr(&mut end.node, end.span.start, end.span.end, rewrites);
        }
        Expr::UnaryOp { operand, .. } => {
            rewrite_expr(&mut operand.node, operand.span.start, operand.span.end, rewrites);
        }
        Expr::Cast { expr: inner, .. } => {
            rewrite_expr(&mut inner.node, inner.span.start, inner.span.end, rewrites);
        }
        Expr::FieldAccess { object, .. } => {
            rewrite_expr(&mut object.node, object.span.start, object.span.end, rewrites);
        }
        Expr::MethodCall { object, args, .. } => {
            rewrite_expr(&mut object.node, object.span.start, object.span.end, rewrites);
            for arg in args.iter_mut() {
                rewrite_expr(&mut arg.node, arg.span.start, arg.span.end, rewrites);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements.iter_mut() {
                rewrite_expr(&mut el.node, el.span.start, el.span.end, rewrites);
            }
        }
        Expr::Index { object, index } => {
            rewrite_expr(&mut object.node, object.span.start, object.span.end, rewrites);
            rewrite_expr(&mut index.node, index.span.start, index.span.end, rewrites);
        }
        Expr::StringInterp { parts } => {
            for part in parts.iter_mut() {
                if let StringInterpPart::Expr(e) = part {
                    rewrite_expr(&mut e.node, e.span.start, e.span.end, rewrites);
                }
            }
        }
        Expr::Closure { body, .. } => {
            rewrite_block(&mut body.node, rewrites);
        }
        Expr::Propagate { expr } => {
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
        }
        Expr::Catch { expr, handler } => {
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
            match handler {
                CatchHandler::Wildcard { body, .. } => {
                    rewrite_block(&mut body.node, rewrites);
                }
                CatchHandler::Shorthand(body) => {
                    rewrite_expr(&mut body.node, body.span.start, body.span.end, rewrites);
                }
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries.iter_mut() {
                rewrite_expr(&mut k.node, k.span.start, k.span.end, rewrites);
                rewrite_expr(&mut v.node, v.span.start, v.span.end, rewrites);
            }
        }
        Expr::SetLit { elements, .. } => {
            for el in elements.iter_mut() {
                rewrite_expr(&mut el.node, el.span.start, el.span.end, rewrites);
            }
        }
        Expr::Spawn { call } => {
            rewrite_expr(&mut call.node, call.span.start, call.span.end, rewrites);
        }
        Expr::NullPropagate { expr } => {
            rewrite_expr(&mut expr.node, expr.span.start, expr.span.end, rewrites);
        }
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
        | Expr::StringLit(_) | Expr::Ident(_) | Expr::ClosureCreate { .. }
        | Expr::NoneLit => {}
    }
}

// ── Phase 2b: Resolve TypeExpr::Generic to TypeExpr::Named ──────────

/// Resolve all remaining TypeExpr::Generic nodes in the program to TypeExpr::Named(mangled_name).
/// This handles cases where generic types appear in non-generic code (e.g., `fn foo(x: Box<int>)`).
fn resolve_all_generic_type_exprs(program: &mut Program, env: &mut TypeEnv) -> Result<(), CompileError> {
    for func in &mut program.functions {
        resolve_generic_te_in_function(&mut func.node, env)?;
    }
    for class in &mut program.classes {
        for field in &mut class.node.fields {
            resolve_generic_te(&mut field.ty.node, env)?;
        }
        for method in &mut class.node.methods {
            resolve_generic_te_in_function(&mut method.node, env)?;
        }
    }
    if let Some(ref mut app) = program.app {
        for method in &mut app.node.methods {
            resolve_generic_te_in_function(&mut method.node, env)?;
        }
    }
    for stage in &mut program.stages {
        for method in &mut stage.node.methods {
            resolve_generic_te_in_function(&mut method.node, env)?;
        }
    }
    Ok(())
}

fn resolve_generic_te_in_function(func: &mut Function, env: &mut TypeEnv) -> Result<(), CompileError> {
    for p in &mut func.params {
        resolve_generic_te(&mut p.ty.node, env)?;
    }
    if let Some(ref mut rt) = func.return_type {
        resolve_generic_te(&mut rt.node, env)?;
    }
    resolve_generic_te_in_block(&mut func.body.node, env)?;
    Ok(())
}

fn resolve_generic_te_in_block(block: &mut Block, env: &mut TypeEnv) -> Result<(), CompileError> {
    for stmt in &mut block.stmts {
        resolve_generic_te_in_stmt(&mut stmt.node, env)?;
    }
    Ok(())
}

fn resolve_generic_te_in_stmt(stmt: &mut Stmt, env: &mut TypeEnv) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let { ty, value, .. } => {
            if let Some(t) = ty {
                resolve_generic_te(&mut t.node, env)?;
            }
            resolve_generic_te_in_expr(&mut value.node, env)?;
        }
        Stmt::Return(Some(expr)) => resolve_generic_te_in_expr(&mut expr.node, env)?,
        Stmt::Return(None) => {}
        Stmt::Assign { value, .. } => resolve_generic_te_in_expr(&mut value.node, env)?,
        Stmt::FieldAssign { object, value, .. } => {
            resolve_generic_te_in_expr(&mut object.node, env)?;
            resolve_generic_te_in_expr(&mut value.node, env)?;
        }
        Stmt::If { condition, then_block, else_block } => {
            resolve_generic_te_in_expr(&mut condition.node, env)?;
            resolve_generic_te_in_block(&mut then_block.node, env)?;
            if let Some(eb) = else_block {
                resolve_generic_te_in_block(&mut eb.node, env)?;
            }
        }
        Stmt::While { condition, body } => {
            resolve_generic_te_in_expr(&mut condition.node, env)?;
            resolve_generic_te_in_block(&mut body.node, env)?;
        }
        Stmt::For { iterable, body, .. } => {
            resolve_generic_te_in_expr(&mut iterable.node, env)?;
            resolve_generic_te_in_block(&mut body.node, env)?;
        }
        Stmt::IndexAssign { object, index, value } => {
            resolve_generic_te_in_expr(&mut object.node, env)?;
            resolve_generic_te_in_expr(&mut index.node, env)?;
            resolve_generic_te_in_expr(&mut value.node, env)?;
        }
        Stmt::Match { expr, arms } => {
            resolve_generic_te_in_expr(&mut expr.node, env)?;
            for arm in arms.iter_mut() {
                resolve_generic_te_in_block(&mut arm.body.node, env)?;
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, fexpr) in fields.iter_mut() {
                resolve_generic_te_in_expr(&mut fexpr.node, env)?;
            }
        }
        Stmt::Expr(expr) => resolve_generic_te_in_expr(&mut expr.node, env)?,
        Stmt::LetChan { elem_type, capacity, .. } => {
            resolve_generic_te(&mut elem_type.node, env)?;
            if let Some(cap) = capacity {
                resolve_generic_te_in_expr(&mut cap.node, env)?;
            }
        }
        Stmt::Scope { seeds, bindings, body } => {
            for seed in seeds {
                resolve_generic_te_in_expr(&mut seed.node, env)?;
            }
            for binding in bindings {
                resolve_generic_te(&mut binding.ty.node, env)?;
            }
            resolve_generic_te_in_block(&mut body.node, env)?;
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => {
                        resolve_generic_te_in_expr(&mut channel.node, env)?;
                    }
                    SelectOp::Send { channel, value } => {
                        resolve_generic_te_in_expr(&mut channel.node, env)?;
                        resolve_generic_te_in_expr(&mut value.node, env)?;
                    }
                }
                resolve_generic_te_in_block(&mut arm.body.node, env)?;
            }
            if let Some(def) = default {
                resolve_generic_te_in_block(&mut def.node, env)?;
            }
        }
        Stmt::Break | Stmt::Continue => {}
    }
    Ok(())
}

fn resolve_generic_te_in_expr(expr: &mut Expr, env: &mut TypeEnv) -> Result<(), CompileError> {
    match expr {
        Expr::Closure { params, return_type, body } => {
            for p in params.iter_mut() {
                resolve_generic_te(&mut p.ty.node, env)?;
            }
            if let Some(rt) = return_type {
                resolve_generic_te(&mut rt.node, env)?;
            }
            resolve_generic_te_in_block(&mut body.node, env)?;
        }
        Expr::Call { args, .. } => {
            for arg in args.iter_mut() {
                resolve_generic_te_in_expr(&mut arg.node, env)?;
            }
        }
        Expr::MethodCall { object, args, .. } => {
            resolve_generic_te_in_expr(&mut object.node, env)?;
            for arg in args.iter_mut() {
                resolve_generic_te_in_expr(&mut arg.node, env)?;
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            resolve_generic_te_in_expr(&mut lhs.node, env)?;
            resolve_generic_te_in_expr(&mut rhs.node, env)?;
        }
        Expr::Range { start, end, .. } => {
            resolve_generic_te_in_expr(&mut start.node, env)?;
            resolve_generic_te_in_expr(&mut end.node, env)?;
        }
        Expr::UnaryOp { operand, .. } => {
            resolve_generic_te_in_expr(&mut operand.node, env)?;
        }
        Expr::Cast { expr: inner, target_type } => {
            resolve_generic_te_in_expr(&mut inner.node, env)?;
            resolve_generic_te(&mut target_type.node, env)?;
        }
        Expr::FieldAccess { object, .. } => {
            resolve_generic_te_in_expr(&mut object.node, env)?;
        }
        Expr::StructLit { fields, .. } => {
            for (_, fexpr) in fields.iter_mut() {
                resolve_generic_te_in_expr(&mut fexpr.node, env)?;
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements.iter_mut() {
                resolve_generic_te_in_expr(&mut el.node, env)?;
            }
        }
        Expr::Index { object, index } => {
            resolve_generic_te_in_expr(&mut object.node, env)?;
            resolve_generic_te_in_expr(&mut index.node, env)?;
        }
        Expr::EnumData { fields, .. } => {
            for (_, fexpr) in fields.iter_mut() {
                resolve_generic_te_in_expr(&mut fexpr.node, env)?;
            }
        }
        Expr::StringInterp { parts } => {
            for part in parts.iter_mut() {
                if let StringInterpPart::Expr(e) = part {
                    resolve_generic_te_in_expr(&mut e.node, env)?;
                }
            }
        }
        Expr::Propagate { expr } => {
            resolve_generic_te_in_expr(&mut expr.node, env)?;
        }
        Expr::Catch { expr, handler } => {
            resolve_generic_te_in_expr(&mut expr.node, env)?;
            match handler {
                CatchHandler::Wildcard { body, .. } => resolve_generic_te_in_block(&mut body.node, env)?,
                CatchHandler::Shorthand(body) => resolve_generic_te_in_expr(&mut body.node, env)?,
            }
        }
        Expr::Spawn { call } => {
            resolve_generic_te_in_expr(&mut call.node, env)?;
        }
        Expr::NullPropagate { expr } => {
            resolve_generic_te_in_expr(&mut expr.node, env)?;
        }
        _ => {}
    }
    Ok(())
}

/// Resolve a single TypeExpr::Generic to TypeExpr::Named(mangled_name).
/// Also ensures the instantiation is registered.
fn resolve_generic_te(te: &mut TypeExpr, env: &mut TypeEnv) -> Result<(), CompileError> {
    match te {
        TypeExpr::Generic { name, type_args } => {
            // Built-in generic types (Map, Set) are kept as-is — no monomorphization needed
            if name == "Map" || name == "Set" || name == "Task" || name == "Sender" || name == "Receiver" {
                for arg in type_args.iter_mut() {
                    resolve_generic_te(&mut arg.node, env)?;
                }
                return Ok(());
            }
            // Resolve type args recursively first
            for arg in type_args.iter_mut() {
                resolve_generic_te(&mut arg.node, env)?;
            }
            // Convert type args to PlutoType for mangling
            let resolved_args: Vec<PlutoType> = type_args.iter()
                .map(|ta| type_expr_to_pluto_type(&ta.node, env))
                .collect::<Result<Vec<_>, _>>()?;

            let mangled = if env.generic_classes.contains_key(name.as_str())
                || env.generic_enums.contains_key(name.as_str())
            {
                crate::typeck::env::mangle_name(name, &resolved_args)
            } else {
                return Err(CompileError::type_err(
                    format!("unknown generic type '{}'", name),
                    Span::dummy(),
                ));
            };
            *te = TypeExpr::Named(mangled);
        }
        TypeExpr::Array(inner) => resolve_generic_te(&mut inner.node, env)?,
        TypeExpr::Fn { params, return_type } => {
            for p in params.iter_mut() {
                resolve_generic_te(&mut p.node, env)?;
            }
            resolve_generic_te(&mut return_type.node, env)?;
        }
        TypeExpr::Named(_) | TypeExpr::Qualified { .. } => {}
        TypeExpr::Nullable(inner) => resolve_generic_te(&mut inner.node, env)?,
    }
    Ok(())
}

/// Convert a TypeExpr to a PlutoType (simple case for already-resolved exprs).
fn type_expr_to_pluto_type(te: &TypeExpr, env: &TypeEnv) -> Result<PlutoType, CompileError> {
    match te {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(PlutoType::Int),
            "float" => Ok(PlutoType::Float),
            "bool" => Ok(PlutoType::Bool),
            "string" => Ok(PlutoType::String),
            "void" => Ok(PlutoType::Void),
            "byte" => Ok(PlutoType::Byte),
            "bytes" => Ok(PlutoType::Bytes),
            _ => {
                if env.classes.contains_key(name) || env.generic_classes.contains_key(name) {
                    Ok(PlutoType::Class(name.clone()))
                } else if env.enums.contains_key(name) || env.generic_enums.contains_key(name) {
                    Ok(PlutoType::Enum(name.clone()))
                } else if env.traits.contains_key(name) {
                    Ok(PlutoType::Trait(name.clone()))
                } else {
                    Ok(PlutoType::Class(name.clone())) // Assume class for mangled names
                }
            }
        },
        TypeExpr::Array(inner) => {
            Ok(PlutoType::Array(Box::new(type_expr_to_pluto_type(&inner.node, env)?)))
        }
        TypeExpr::Fn { params, return_type } => {
            let param_types: Vec<PlutoType> = params.iter()
                .map(|p| type_expr_to_pluto_type(&p.node, env))
                .collect::<Result<Vec<_>, _>>()?;
            let ret = type_expr_to_pluto_type(&return_type.node, env)?;
            Ok(PlutoType::Fn(param_types, Box::new(ret)))
        }
        TypeExpr::Qualified { module, name } => {
            Ok(PlutoType::Class(format!("{}.{}", module, name)))
        }
        TypeExpr::Generic { name, type_args } => {
            let resolved_args: Vec<PlutoType> = type_args.iter()
                .map(|ta| type_expr_to_pluto_type(&ta.node, env))
                .collect::<Result<Vec<_>, _>>()?;
            if name == "Sender" && resolved_args.len() == 1 {
                return Ok(PlutoType::Sender(Box::new(resolved_args[0].clone())));
            }
            if name == "Receiver" && resolved_args.len() == 1 {
                return Ok(PlutoType::Receiver(Box::new(resolved_args[0].clone())));
            }
            if name == "Map" && resolved_args.len() == 2 {
                return Ok(PlutoType::Map(Box::new(resolved_args[0].clone()), Box::new(resolved_args[1].clone())));
            }
            if name == "Set" && resolved_args.len() == 1 {
                return Ok(PlutoType::Set(Box::new(resolved_args[0].clone())));
            }
            if name == "Task" && resolved_args.len() == 1 {
                return Ok(PlutoType::Task(Box::new(resolved_args[0].clone())));
            }
            let mangled = crate::typeck::env::mangle_name(name, &resolved_args);
            Ok(PlutoType::Class(mangled))
        }
        TypeExpr::Nullable(inner) => {
            let inner_type = type_expr_to_pluto_type(&inner.node, env)?;
            Ok(PlutoType::Nullable(Box::new(inner_type)))
        }
    }
}

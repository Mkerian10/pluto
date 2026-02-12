use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::diagnostics::CompileError;
use crate::parser::ast::*;
use crate::span::{Span, Spanned};
use crate::typeck::env::{mangle_method, mangle_name, InstKind, Instantiation, TypeEnv};
use crate::typeck::types::{PlutoType, pluto_type_to_type_expr};
use crate::visit::{walk_block_mut, walk_expr_mut, walk_stmt_mut, walk_type_expr_mut, VisitMut};

/// Span offset multiplier for monomorphized bodies. Each iteration gets unique
/// spans to avoid closure capture key collisions. Must exceed any realistic
/// source file size.
const SPAN_OFFSET_MULTIPLIER: usize = 10_000_000;

/// Visitor that resolves generic type instances (TypeExpr::Generic) in AST nodes.
/// Replaces TypeExpr::Generic with mangled concrete type names and ensures
/// instantiations are registered.
struct GenericTypeResolver<'a> {
    env: &'a mut TypeEnv,
}

impl VisitMut for GenericTypeResolver<'_> {
    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        // Resolve this type expression
        if let Err(_e) = resolve_generic_te(&mut te.node, self.env) {
            // Silently continue on error - caller will detect issues later
        }
        // Then recurse into children
        walk_type_expr_mut(self, te);
    }
}

/// Visitor for offsetting all spans in an AST subtree by a fixed amount.
/// Used during monomorphization to give each instantiation unique spans.
struct SpanOffsetter {
    offset: usize,
}

impl VisitMut for SpanOffsetter {
    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        te.span.start += self.offset;
        te.span.end += self.offset;
        walk_type_expr_mut(self, te);
    }

    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        expr.span.start += self.offset;
        expr.span.end += self.offset;
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        stmt.span.start += self.offset;
        stmt.span.end += self.offset;
        walk_stmt_mut(self, stmt);
    }

    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        block.span.start += self.offset;
        block.span.end += self.offset;
        walk_block_mut(self, block);
    }
}

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
        TypeExpr::Stream(inner) => {
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
        Stmt::Yield { value, .. } => {
            substitute_in_expr(&mut value.node, bindings);
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
        Expr::StaticTraitCall { type_args, args, .. } => {
            for type_arg in type_args {
                substitute_in_type_expr(&mut type_arg.node, bindings);
            }
            for arg in args {
                substitute_in_expr(&mut arg.node, bindings);
            }
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
        Expr::QualifiedAccess { segments } => {
            panic!(
                "QualifiedAccess should be resolved by module flattening before monomorphize. Segments: {:?}",
                segments.iter().map(|s| &s.node).collect::<Vec<_>>()
            )
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
    let mut offsetter = SpanOffsetter { offset };
    offset_spanned(&mut func.name, offset);
    for tp in &mut func.type_params {
        offset_spanned(tp, offset);
    }
    for p in &mut func.params {
        offset_spanned(&mut p.name, offset);
        offsetter.visit_type_expr_mut(&mut p.ty);
    }
    if let Some(ref mut rt) = func.return_type {
        offsetter.visit_type_expr_mut(rt);
    }
    offsetter.visit_block_mut(&mut func.body);
}

fn offset_class_spans(class: &mut ClassDecl, offset: usize) {
    let mut offsetter = SpanOffsetter { offset };
    offset_spanned(&mut class.name, offset);
    for f in &mut class.fields {
        offset_spanned(&mut f.name, offset);
        offsetter.visit_type_expr_mut(&mut f.ty);
    }
    for method in &mut class.methods {
        offset_spanned(method, offset);
        offset_function_spans(&mut method.node, offset);
    }
}

fn offset_enum_spans(edecl: &mut EnumDecl, offset: usize) {
    let mut offsetter = SpanOffsetter { offset };
    offset_spanned(&mut edecl.name, offset);
    for variant in &mut edecl.variants {
        offset_spanned(&mut variant.name, offset);
        for field in &mut variant.fields {
            offset_spanned(&mut field.name, offset);
            offsetter.visit_type_expr_mut(&mut field.ty);
        }
    }
}

fn offset_block_spans(block: &mut Block, offset: usize) {
    let mut offsetter = SpanOffsetter { offset };
    for stmt in &mut block.stmts {
        offsetter.visit_stmt_mut(stmt);
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

struct MonomorphizeRewriter<'a> {
    rewrites: &'a HashMap<(usize, usize), String>,
}

impl VisitMut for MonomorphizeRewriter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        let span_key = (expr.span.start, expr.span.end);

        // Handle expressions that need name rewriting
        match &mut expr.node {
            Expr::Call { name, type_args, .. } => {
                // Check if this call site should be rewritten
                if let Some(mangled) = self.rewrites.get(&span_key) {
                    name.node = mangled.clone();
                    type_args.clear();
                }
            }
            Expr::StructLit { name, type_args, .. } => {
                if let Some(mangled) = self.rewrites.get(&span_key) {
                    name.node = mangled.clone();
                    type_args.clear();
                }
            }
            Expr::EnumUnit { enum_name, type_args, .. } => {
                if let Some(mangled) = self.rewrites.get(&span_key) {
                    enum_name.node = mangled.clone();
                    type_args.clear();
                }
            }
            Expr::EnumData { enum_name, type_args, .. } => {
                if let Some(mangled) = self.rewrites.get(&span_key) {
                    enum_name.node = mangled.clone();
                    type_args.clear();
                }
            }
            Expr::QualifiedAccess { segments } => {
                panic!(
                    "QualifiedAccess should be resolved by module flattening before monomorphize. Segments: {:?}",
                    segments.iter().map(|s| &s.node).collect::<Vec<_>>()
                )
            }
            Expr::StringInterp { parts } => {
                for part in parts.iter_mut() {
                    if let StringInterpPart::Expr(e) = part {
                        self.visit_expr_mut(e);
                    }
                }
                return;
            }
            _ => {}
        }
        // Recurse into sub-expressions
        walk_expr_mut(self, expr);
    }

    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        // Handle Match arms: rewrite enum names
        if let Stmt::Match { expr, arms } = &mut stmt.node {
            let match_span = (expr.span.start, expr.span.end);
            // Visit the match expression
            self.visit_expr_mut(expr);

            // Rewrite enum names in match arms
            for arm in arms.iter_mut() {
                let arm_key = (arm.enum_name.span.start, arm.enum_name.span.end);
                if let Some(mangled) = self.rewrites.get(&arm_key) {
                    arm.enum_name.node = mangled.clone();
                }
                // Also check if match expr span maps to a rewrite
                if let Some(mangled) = self.rewrites.get(&match_span) {
                    arm.enum_name.node = mangled.clone();
                }
                self.visit_block_mut(&mut arm.body);
            }
            return;
        }

        // Recurse into sub-statements
        walk_stmt_mut(self, stmt);
    }
}

fn rewrite_block(block: &mut Block, rewrites: &HashMap<(usize, usize), String>) {
    let mut rewriter = MonomorphizeRewriter { rewrites };
    for stmt in &mut block.stmts {
        rewriter.visit_stmt_mut(stmt);
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
    let mut visitor = GenericTypeResolver { env };
    // Visit param types
    for p in &mut func.params {
        visitor.visit_type_expr_mut(&mut p.ty);
    }
    // Visit return type
    if let Some(ref mut rt) = func.return_type {
        visitor.visit_type_expr_mut(rt);
    }
    // Visit function body (which contains type exprs in casts, closures, etc.)
    visitor.visit_block_mut(&mut func.body);
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
        TypeExpr::Stream(inner) => resolve_generic_te(&mut inner.node, env)?,
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
        TypeExpr::Stream(inner) => {
            let inner_type = type_expr_to_pluto_type(&inner.node, env)?;
            Ok(PlutoType::Stream(Box::new(inner_type)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{Field, TypeExpr, Param, EnumVariant, Function, ClassDecl, EnumDecl, Block, Lifecycle};
    use crate::typeck::types::PlutoType;
    use crate::span::{Span, Spanned};
    use std::collections::HashMap;

    fn dummy_span() -> Span {
        Span { start: 0, end: 0, file_id: 0 }
    }

    fn spanned<T>(node: T) -> Spanned<T> {
        Spanned { node, span: dummy_span() }
    }

    // ── build_type_expr_bindings tests ──────────────────────────────────

    #[test]
    fn test_build_type_expr_bindings_single() {
        let type_params = vec!["T".to_string()];
        let type_args = vec![PlutoType::Int];
        let bindings = build_type_expr_bindings(&type_params, &type_args);
        
        assert_eq!(bindings.len(), 1);
        assert!(matches!(bindings.get("T"), Some(TypeExpr::Named(n)) if n == "int"));
    }

    #[test]
    fn test_build_type_expr_bindings_multiple() {
        let type_params = vec!["T".to_string(), "U".to_string()];
        let type_args = vec![PlutoType::Int, PlutoType::String];
        let bindings = build_type_expr_bindings(&type_params, &type_args);
        
        assert_eq!(bindings.len(), 2);
        assert!(matches!(bindings.get("T"), Some(TypeExpr::Named(n)) if n == "int"));
        assert!(matches!(bindings.get("U"), Some(TypeExpr::Named(n)) if n == "string"));
    }

    #[test]
    fn test_build_type_expr_bindings_class_type() {
        let type_params = vec!["T".to_string()];
        let type_args = vec![PlutoType::Class("User".to_string())];
        let bindings = build_type_expr_bindings(&type_params, &type_args);
        
        assert!(matches!(bindings.get("T"), Some(TypeExpr::Named(n)) if n == "User"));
    }

    // ── substitute_in_type_expr tests ───────────────────────────────────

    #[test]
    fn test_substitute_named_match() {
        let mut te = TypeExpr::Named("T".to_string());
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), TypeExpr::Named("int".to_string()));
        
        substitute_in_type_expr(&mut te, &bindings);
        assert!(matches!(te, TypeExpr::Named(n) if n == "int"));
    }

    #[test]
    fn test_substitute_named_no_match() {
        let mut te = TypeExpr::Named("int".to_string());
        let bindings = HashMap::new();
        
        substitute_in_type_expr(&mut te, &bindings);
        assert!(matches!(te, TypeExpr::Named(n) if n == "int"));
    }

    #[test]
    fn test_substitute_array() {
        let mut te = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("T".to_string()))));
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), TypeExpr::Named("int".to_string()));
        
        substitute_in_type_expr(&mut te, &bindings);
        if let TypeExpr::Array(inner) = te {
            assert!(matches!(inner.node, TypeExpr::Named(n) if n == "int"));
        } else {
            panic!("Expected Array type");
        }
    }

    #[test]
    fn test_substitute_nullable() {
        let mut te = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("T".to_string()))));
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), TypeExpr::Named("string".to_string()));
        
        substitute_in_type_expr(&mut te, &bindings);
        if let TypeExpr::Nullable(inner) = te {
            assert!(matches!(inner.node, TypeExpr::Named(n) if n == "string"));
        } else {
            panic!("Expected Nullable type");
        }
    }

    #[test]
    fn test_substitute_generic_args() {
        let mut te = TypeExpr::Generic {
            name: "Box".to_string(),
            type_args: vec![spanned(TypeExpr::Named("T".to_string()))],
        };
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), TypeExpr::Named("int".to_string()));

        substitute_in_type_expr(&mut te, &bindings);
        if let TypeExpr::Generic { type_args, .. } = te {
            assert!(matches!(&type_args[0].node, TypeExpr::Named(n) if n == "int"));
        } else {
            panic!("Expected Generic type");
        }
    }

    #[test]
    fn test_substitute_fn_types() {
        let mut te = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("T".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("U".to_string()))),
        };
        let mut bindings = HashMap::new();
        bindings.insert("T".to_string(), TypeExpr::Named("int".to_string()));
        bindings.insert("U".to_string(), TypeExpr::Named("string".to_string()));

        substitute_in_type_expr(&mut te, &bindings);
        if let TypeExpr::Fn { params, return_type } = te {
            assert!(matches!(&params[0].node, TypeExpr::Named(n) if n == "int"));
            assert!(matches!(&return_type.node, TypeExpr::Named(n) if n == "string"));
        } else {
            panic!("Expected Fn type");
        }
    }

    // ── offset_span tests ───────────────────────────────────────────────

    #[test]
    fn test_offset_span_basic() {
        let mut span = Span { start: 10, end: 20, file_id: 0 };
        offset_span(&mut span, 100);
        assert_eq!(span.start, 110);
        assert_eq!(span.end, 120);
    }

    #[test]
    fn test_offset_span_zero() {
        let mut span = Span { start: 10, end: 20, file_id: 0 };
        offset_span(&mut span, 0);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
    }

    #[test]
    fn test_offset_spanned() {
        let mut spanned = Spanned {
            node: 42,
            span: Span { start: 5, end: 15, file_id: 0 },
        };
        offset_spanned(&mut spanned, 50);
        assert_eq!(spanned.span.start, 55);
        assert_eq!(spanned.span.end, 65);
        assert_eq!(spanned.node, 42); // Node unchanged
    }

    // ── reassign_*_uuids tests ──────────────────────────────────────────

    #[test]
    fn test_reassign_function_uuids() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        let mut func = Function {
            id: Uuid::new_v4(),
            name: spanned("foo".to_string()),
            type_params: vec![],
            type_param_bounds: HashMap::new(),
            params: vec![
                Param { id: uuid1, name: spanned("x".to_string()), ty: spanned(TypeExpr::Named("int".to_string())), is_mut: false },
                Param { id: uuid2, name: spanned("y".to_string()), ty: spanned(TypeExpr::Named("int".to_string())), is_mut: false },
            ],
            return_type: None,
            body: spanned(Block { stmts: vec![] }),
            contracts: vec![],
            is_pub: false,
            is_override: false,
            is_generator: false,
        };

        reassign_function_uuids(&mut func);

        // UUIDs should be different after reassignment
        assert_ne!(func.params[0].id, uuid1);
        assert_ne!(func.params[1].id, uuid2);
        // Each param should have a unique UUID
        assert_ne!(func.params[0].id, func.params[1].id);
    }

    #[test]
    fn test_reassign_class_uuids() {
        let field_uuid = Uuid::new_v4();
        let method_uuid = Uuid::new_v4();
        let mut class = ClassDecl {
            id: Uuid::new_v4(),
            name: spanned("User".to_string()),
            type_params: vec![],
            type_param_bounds: HashMap::new(),
            fields: vec![
                Field {
                    id: field_uuid,
                    name: spanned("name".to_string()),
                    ty: spanned(TypeExpr::Named("string".to_string())),
                    is_injected: false,
                    is_ambient: false,
                },
            ],
            methods: vec![
                spanned(Function {
                    id: method_uuid,
                    name: spanned("get_name".to_string()),
                    type_params: vec![],
                    type_param_bounds: HashMap::new(),
                    params: vec![],
                    return_type: None,
                    body: spanned(Block { stmts: vec![] }),
                    contracts: vec![],
                    is_pub: false,
                    is_override: false,
                    is_generator: false,
                }),
            ],
            impl_traits: vec![],
            uses: vec![],
            is_pub: false,
            lifecycle: Lifecycle::Singleton,
            invariants: vec![],
        };

        reassign_class_uuids(&mut class);

        assert_ne!(class.fields[0].id, field_uuid);
        assert_ne!(class.methods[0].node.id, method_uuid);
    }

    #[test]
    fn test_reassign_enum_uuids() {
        let variant_uuid = Uuid::new_v4();
        let field_uuid = Uuid::new_v4();
        let mut edecl = EnumDecl {
            id: Uuid::new_v4(),
            name: spanned("Option".to_string()),
            type_params: vec![],
            type_param_bounds: HashMap::new(),
            variants: vec![
                EnumVariant {
                    id: variant_uuid,
                    name: spanned("Some".to_string()),
                    fields: vec![
                        Field {
                            id: field_uuid,
                            name: spanned("value".to_string()),
                            ty: spanned(TypeExpr::Named("int".to_string())),
                            is_injected: false,
                            is_ambient: false,
                        },
                    ],
                },
            ],
            is_pub: false,
        };

        reassign_enum_uuids(&mut edecl);

        assert_ne!(edecl.variants[0].id, variant_uuid);
        assert_ne!(edecl.variants[0].fields[0].id, field_uuid);
    }

    // ── type_expr_to_pluto_type tests ───────────────────────────────────

    #[test]
    fn test_type_expr_to_pluto_type_primitives() {
        let env = TypeEnv::new();
        
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("int".to_string()), &env), Ok(PlutoType::Int)));
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("float".to_string()), &env), Ok(PlutoType::Float)));
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("bool".to_string()), &env), Ok(PlutoType::Bool)));
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("string".to_string()), &env), Ok(PlutoType::String)));
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("void".to_string()), &env), Ok(PlutoType::Void)));
        assert!(matches!(type_expr_to_pluto_type(&TypeExpr::Named("byte".to_string()), &env), Ok(PlutoType::Byte)));
    }

    #[test]
    fn test_type_expr_to_pluto_type_array() {
        let env = TypeEnv::new();
        let te = TypeExpr::Array(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        
        if let Ok(PlutoType::Array(inner)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*inner, PlutoType::Int));
        } else {
            panic!("Expected Array<int>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_nullable() {
        let env = TypeEnv::new();
        let te = TypeExpr::Nullable(Box::new(spanned(TypeExpr::Named("int".to_string()))));
        
        if let Ok(PlutoType::Nullable(inner)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*inner, PlutoType::Int));
        } else {
            panic!("Expected Nullable<int>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_fn() {
        let env = TypeEnv::new();
        let te = TypeExpr::Fn {
            params: vec![Box::new(spanned(TypeExpr::Named("int".to_string())))],
            return_type: Box::new(spanned(TypeExpr::Named("string".to_string()))),
        };

        if let Ok(PlutoType::Fn(params, ret)) = type_expr_to_pluto_type(&te, &env) {
            assert_eq!(params.len(), 1);
            assert!(matches!(params[0], PlutoType::Int));
            assert!(matches!(*ret, PlutoType::String));
        } else {
            panic!("Expected Fn type");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_map() {
        let env = TypeEnv::new();
        let te = TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                spanned(TypeExpr::Named("string".to_string())),
                spanned(TypeExpr::Named("int".to_string())),
            ],
        };
        
        if let Ok(PlutoType::Map(key, val)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*key, PlutoType::String));
            assert!(matches!(*val, PlutoType::Int));
        } else {
            panic!("Expected Map<string, int>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_set() {
        let env = TypeEnv::new();
        let te = TypeExpr::Generic {
            name: "Set".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        
        if let Ok(PlutoType::Set(elem)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*elem, PlutoType::Int));
        } else {
            panic!("Expected Set<int>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_task() {
        let env = TypeEnv::new();
        let te = TypeExpr::Generic {
            name: "Task".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        
        if let Ok(PlutoType::Task(inner)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*inner, PlutoType::Int));
        } else {
            panic!("Expected Task<int>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_sender() {
        let env = TypeEnv::new();
        let te = TypeExpr::Generic {
            name: "Sender".to_string(),
            type_args: vec![spanned(TypeExpr::Named("string".to_string()))],
        };
        
        if let Ok(PlutoType::Sender(elem)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*elem, PlutoType::String));
        } else {
            panic!("Expected Sender<string>");
        }
    }

    #[test]
    fn test_type_expr_to_pluto_type_receiver() {
        let env = TypeEnv::new();
        let te = TypeExpr::Generic {
            name: "Receiver".to_string(),
            type_args: vec![spanned(TypeExpr::Named("int".to_string()))],
        };
        
        if let Ok(PlutoType::Receiver(elem)) = type_expr_to_pluto_type(&te, &env) {
            assert!(matches!(*elem, PlutoType::Int));
        } else {
            panic!("Expected Receiver<int>");
        }
    }
}

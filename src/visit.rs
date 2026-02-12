//! AST visitor pattern infrastructure
//!
//! This module provides two visitor traits and corresponding walk functions for traversing
//! the Pluto AST:
//!
//! - `Visitor` — immutable reference traversal (for analysis/collection passes)
//! - `VisitMut` — mutable reference traversal (for in-place rewriting passes)
//!
//! ## Usage
//!
//! Implement the visitor trait for your pass, overriding only the methods you need.
//! Call the corresponding `walk_*` function inside your override to get default recursion.
//!
//! ```rust
//! use crate::visit::{Visitor, walk_expr};
//! use crate::parser::ast::Expr;
//! use crate::span::Spanned;
//! use std::collections::HashSet;
//!
//! struct IdentCollector {
//!     names: HashSet<String>,
//! }
//!
//! impl Visitor for IdentCollector {
//!     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
//!         if let Expr::Ident(name) = &expr.node {
//!             self.names.insert(name.clone());
//!         }
//!         walk_expr(self, expr); // Continue recursion
//!     }
//! }
//! ```
//!
//! ## When to Use
//!
//! Use `Visitor`/`VisitMut` for passes where >50% of match arms would be pure recursion.
//! Use manual `match` blocks for passes where >50% of arms have custom logic (like codegen, typeck core).

use crate::parser::ast::*;
use crate::span::Spanned;

// ============================================================================
// Visitor Trait (Read-Only)
// ============================================================================

/// Read-only AST visitor. Default implementations recurse into all children.
/// Override specific methods to intercept nodes of interest.
///
/// Call the corresponding `walk_*` function inside your override to continue
/// the default recursion after your custom logic. Omit the walk call to prune
/// traversal at that node.
pub trait Visitor: Sized {
    // Program-level
    fn visit_program(&mut self, program: &Program) {
        walk_program(self, program);
    }

    // Block-level
    fn visit_block(&mut self, block: &Spanned<Block>) {
        walk_block(self, block);
    }

    // Declaration-level
    fn visit_function(&mut self, func: &Spanned<Function>) {
        walk_function(self, func);
    }

    fn visit_class(&mut self, class: &Spanned<ClassDecl>) {
        walk_class(self, class);
    }

    fn visit_trait(&mut self, trait_decl: &Spanned<TraitDecl>) {
        walk_trait(self, trait_decl);
    }

    fn visit_enum(&mut self, enum_decl: &Spanned<EnumDecl>) {
        walk_enum(self, enum_decl);
    }

    fn visit_error(&mut self, error_decl: &Spanned<ErrorDecl>) {
        walk_error(self, error_decl);
    }

    fn visit_app(&mut self, app: &Spanned<AppDecl>) {
        walk_app(self, app);
    }

    fn visit_stage(&mut self, stage: &Spanned<StageDecl>) {
        walk_stage(self, stage);
    }

    fn visit_system(&mut self, system: &Spanned<SystemDecl>) {
        walk_system(self, system);
    }

    fn visit_extern_fn(&mut self, extern_fn: &Spanned<ExternFnDecl>) {
        walk_extern_fn(self, extern_fn);
    }

    fn visit_import(&mut self, import: &Spanned<ImportDecl>) {
        walk_import(self, import);
    }

    // Statement/Expression-level
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        walk_expr(self, expr);
    }

    fn visit_type_expr(&mut self, te: &Spanned<TypeExpr>) {
        walk_type_expr(self, te);
    }
}

// ============================================================================
// Walk Functions (Read-Only)
// ============================================================================

pub fn walk_program<V: Visitor>(v: &mut V, program: &Program) {
    // Visit all top-level declarations in order
    for import in &program.imports {
        v.visit_import(import);
    }
    for func in &program.functions {
        v.visit_function(func);
    }
    for extern_fn in &program.extern_fns {
        v.visit_extern_fn(extern_fn);
    }
    for class in &program.classes {
        v.visit_class(class);
    }
    for trait_decl in &program.traits {
        v.visit_trait(trait_decl);
    }
    for enum_decl in &program.enums {
        v.visit_enum(enum_decl);
    }
    if let Some(app) = &program.app {
        v.visit_app(app);
    }
    for stage in &program.stages {
        v.visit_stage(stage);
    }
    if let Some(system) = &program.system {
        v.visit_system(system);
    }
    for error_decl in &program.errors {
        v.visit_error(error_decl);
    }
    // Note: test_info and tests are synthetic metadata, not walked
}

pub fn walk_import<V: Visitor>(_v: &mut V, _import: &Spanned<ImportDecl>) {
    // ImportDecl has no nested AST nodes to visit
}

pub fn walk_extern_fn<V: Visitor>(v: &mut V, extern_fn: &Spanned<ExternFnDecl>) {
    // Visit param types
    for param in &extern_fn.node.params {
        v.visit_type_expr(&param.ty);
    }

    // Visit return type
    if let Some(rt) = &extern_fn.node.return_type {
        v.visit_type_expr(rt);
    }
}

pub fn walk_function<V: Visitor>(v: &mut V, func: &Spanned<Function>) {
    // Visit param types
    for param in &func.node.params {
        v.visit_type_expr(&param.ty);
    }

    // Visit return type
    if let Some(rt) = &func.node.return_type {
        v.visit_type_expr(rt);
    }

    // Visit body (ALWAYS present — not optional)
    v.visit_block(&func.node.body);

    // Visit contracts
    for contract in &func.node.contracts {
        v.visit_expr(&contract.node.expr);
    }
}

pub fn walk_class<V: Visitor>(v: &mut V, class: &Spanned<ClassDecl>) {
    // Visit field types (includes both regular and injected fields)
    for field in &class.node.fields {
        v.visit_type_expr(&field.ty);
    }

    // Visit methods
    for method in &class.node.methods {
        v.visit_function(method);
    }

    // Visit invariants
    for invariant in &class.node.invariants {
        v.visit_expr(&invariant.node.expr);
    }
}

pub fn walk_trait<V: Visitor>(v: &mut V, trait_decl: &Spanned<TraitDecl>) {
    for method in &trait_decl.node.methods {
        // Visit param types
        for param in &method.params {
            v.visit_type_expr(&param.ty);
        }

        // Visit return type
        if let Some(rt) = &method.return_type {
            v.visit_type_expr(rt);
        }

        // Visit default body
        if let Some(body) = &method.body {
            v.visit_block(body);
        }

        // Visit contracts
        for contract in &method.contracts {
            v.visit_expr(&contract.node.expr);
        }
    }
}

pub fn walk_enum<V: Visitor>(v: &mut V, enum_decl: &Spanned<EnumDecl>) {
    for variant in &enum_decl.node.variants {
        for field in &variant.fields {
            v.visit_type_expr(&field.ty);
        }
    }
}

pub fn walk_error<V: Visitor>(v: &mut V, error_decl: &Spanned<ErrorDecl>) {
    for field in &error_decl.node.fields {
        v.visit_type_expr(&field.ty);
    }
}

pub fn walk_app<V: Visitor>(v: &mut V, app: &Spanned<AppDecl>) {
    // Visit injected field types
    for field in &app.node.inject_fields {
        v.visit_type_expr(&field.ty);
    }

    // Visit methods
    for method in &app.node.methods {
        v.visit_function(method);
    }
}

pub fn walk_stage<V: Visitor>(v: &mut V, stage: &Spanned<StageDecl>) {
    // Visit injected field types
    for field in &stage.node.inject_fields {
        v.visit_type_expr(&field.ty);
    }

    // Visit required methods
    for required_method in &stage.node.required_methods {
        for param in &required_method.node.params {
            v.visit_type_expr(&param.ty);
        }
        if let Some(rt) = &required_method.node.return_type {
            v.visit_type_expr(rt);
        }
    }

    // Visit methods
    for method in &stage.node.methods {
        v.visit_function(method);
    }
}

pub fn walk_system<V: Visitor>(_v: &mut V, _system: &Spanned<SystemDecl>) {
    // SystemDecl has no nested AST nodes to visit (only names)
}

pub fn walk_block<V: Visitor>(v: &mut V, block: &Spanned<Block>) {
    for stmt in &block.node.stmts {
        v.visit_stmt(stmt);
    }
}

pub fn walk_stmt<V: Visitor>(v: &mut V, stmt: &Spanned<Stmt>) {
    match &stmt.node {
        Stmt::Let { ty, value, .. } => {
            if let Some(te) = ty {
                v.visit_type_expr(te);
            }
            v.visit_expr(value);
        }
        Stmt::Return(Some(expr)) => v.visit_expr(expr),
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
        Stmt::Assign { value, .. } => v.visit_expr(value),
        Stmt::FieldAssign { object, value, .. } => {
            v.visit_expr(object);
            v.visit_expr(value);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            v.visit_expr(condition);
            v.visit_block(then_block);
            if let Some(eb) = else_block {
                v.visit_block(eb);
            }
        }
        Stmt::While { condition, body } => {
            v.visit_expr(condition);
            v.visit_block(body);
        }
        Stmt::For { iterable, body, .. } => {
            v.visit_expr(iterable);
            v.visit_block(body);
        }
        Stmt::IndexAssign {
            object,
            index,
            value,
        } => {
            v.visit_expr(object);
            v.visit_expr(index);
            v.visit_expr(value);
        }
        Stmt::Match { expr, arms } => {
            v.visit_expr(expr);
            for arm in arms {
                for te in &arm.type_args {
                    v.visit_type_expr(te);
                }
                v.visit_block(&arm.body);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                v.visit_expr(val);
            }
        }
        Stmt::LetChan {
            elem_type,
            capacity,
            ..
        } => {
            v.visit_type_expr(elem_type);
            if let Some(cap) = capacity {
                v.visit_expr(cap);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &arm.op {
                    SelectOp::Recv { channel, .. } => v.visit_expr(channel),
                    SelectOp::Send { channel, value } => {
                        v.visit_expr(channel);
                        v.visit_expr(value);
                    }
                }
                v.visit_block(&arm.body);
            }
            if let Some(d) = default {
                v.visit_block(d);
            }
        }
        Stmt::Scope {
            seeds,
            bindings,
            body,
        } => {
            for seed in seeds {
                v.visit_expr(seed);
            }
            for binding in bindings {
                v.visit_type_expr(&binding.ty);
            }
            v.visit_block(body);
        }
        Stmt::Yield { value } => v.visit_expr(value),
        Stmt::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn walk_expr<V: Visitor>(v: &mut V, expr: &Spanned<Expr>) {
    match &expr.node {
        // Leaves — no children
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::NoneLit
        | Expr::Ident(_)
        | Expr::ClosureCreate { .. } => {}

        // Unary wrappers
        Expr::UnaryOp { operand, .. } => v.visit_expr(operand),
        Expr::Propagate { expr: inner } => v.visit_expr(inner),
        Expr::NullPropagate { expr: inner } => v.visit_expr(inner),
        Expr::Spawn { call } => v.visit_expr(call),
        Expr::Cast {
            expr: inner,
            target_type,
        } => {
            v.visit_expr(inner);
            v.visit_type_expr(target_type);
        }
        Expr::FieldAccess { object, .. } => v.visit_expr(object),

        // Binary
        Expr::BinOp { lhs, rhs, .. } => {
            v.visit_expr(lhs);
            v.visit_expr(rhs);
        }
        Expr::Index { object, index } => {
            v.visit_expr(object);
            v.visit_expr(index);
        }
        Expr::Range { start, end, .. } => {
            v.visit_expr(start);
            v.visit_expr(end);
        }

        // Calls
        Expr::Call {
            args, type_args, ..
        } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for arg in args {
                v.visit_expr(arg);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            v.visit_expr(object);
            for arg in args {
                v.visit_expr(arg);
            }
        }
        Expr::StaticTraitCall {
            type_args, args, ..
        } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for arg in args {
                v.visit_expr(arg);
            }
        }

        // Compound literals
        Expr::StructLit {
            type_args, fields, ..
        } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for (_, val) in fields {
                v.visit_expr(val);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements {
                v.visit_expr(el);
            }
        }
        Expr::EnumUnit { type_args, .. } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
        }
        Expr::EnumData {
            type_args, fields, ..
        } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for (_, val) in fields {
                v.visit_expr(val);
            }
        }
        Expr::MapLit {
            key_type,
            value_type,
            entries,
        } => {
            v.visit_type_expr(key_type);
            v.visit_type_expr(value_type);
            for (k, val) in entries {
                v.visit_expr(k);
                v.visit_expr(val);
            }
        }
        Expr::SetLit {
            elem_type,
            elements,
        } => {
            v.visit_type_expr(elem_type);
            for el in elements {
                v.visit_expr(el);
            }
        }

        // String interpolation
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    v.visit_expr(e);
                }
            }
        }

        // Closures
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            for p in params {
                v.visit_type_expr(&p.ty);
            }
            if let Some(rt) = return_type {
                v.visit_type_expr(rt);
            }
            v.visit_block(body);
        }

        // Error handling
        Expr::Catch { expr: inner, handler } => {
            v.visit_expr(inner);
            match handler {
                CatchHandler::Wildcard { body, .. } => v.visit_block(body),
                CatchHandler::Shorthand(fallback) => v.visit_expr(fallback),
            }
        }

        // QualifiedAccess (no children — just names)
        Expr::QualifiedAccess { .. } => {}
    }
}

pub fn walk_type_expr<V: Visitor>(v: &mut V, te: &Spanned<TypeExpr>) {
    match &te.node {
        TypeExpr::Named(_) | TypeExpr::Qualified { .. } => {}
        TypeExpr::Array(inner) => v.visit_type_expr(inner),
        TypeExpr::Nullable(inner) => v.visit_type_expr(inner),
        TypeExpr::Stream(inner) => v.visit_type_expr(inner),
        TypeExpr::Fn {
            params,
            return_type,
        } => {
            for p in params {
                v.visit_type_expr(p);
            }
            v.visit_type_expr(return_type);
        }
        TypeExpr::Generic { type_args, .. } => {
            for ta in type_args {
                v.visit_type_expr(ta);
            }
        }
    }
}

// ============================================================================
// VisitMut Trait (In-Place Mutation)
// ============================================================================

/// Mutable AST visitor for in-place transformation passes.
/// Structurally identical to `Visitor` but takes `&mut` references.
pub trait VisitMut: Sized {
    // Program-level
    fn visit_program_mut(&mut self, program: &mut Program) {
        walk_program_mut(self, program);
    }

    // Block-level
    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        walk_block_mut(self, block);
    }

    // Declaration-level
    fn visit_function_mut(&mut self, func: &mut Spanned<Function>) {
        walk_function_mut(self, func);
    }

    fn visit_class_mut(&mut self, class: &mut Spanned<ClassDecl>) {
        walk_class_mut(self, class);
    }

    fn visit_trait_mut(&mut self, trait_decl: &mut Spanned<TraitDecl>) {
        walk_trait_mut(self, trait_decl);
    }

    fn visit_enum_mut(&mut self, enum_decl: &mut Spanned<EnumDecl>) {
        walk_enum_mut(self, enum_decl);
    }

    fn visit_error_mut(&mut self, error_decl: &mut Spanned<ErrorDecl>) {
        walk_error_mut(self, error_decl);
    }

    fn visit_app_mut(&mut self, app: &mut Spanned<AppDecl>) {
        walk_app_mut(self, app);
    }

    fn visit_stage_mut(&mut self, stage: &mut Spanned<StageDecl>) {
        walk_stage_mut(self, stage);
    }

    fn visit_system_mut(&mut self, system: &mut Spanned<SystemDecl>) {
        walk_system_mut(self, system);
    }

    fn visit_extern_fn_mut(&mut self, extern_fn: &mut Spanned<ExternFnDecl>) {
        walk_extern_fn_mut(self, extern_fn);
    }

    fn visit_import_mut(&mut self, import: &mut Spanned<ImportDecl>) {
        walk_import_mut(self, import);
    }

    // Statement/Expression-level
    fn visit_stmt_mut(&mut self, stmt: &mut Spanned<Stmt>) {
        walk_stmt_mut(self, stmt);
    }

    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        walk_expr_mut(self, expr);
    }

    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        walk_type_expr_mut(self, te);
    }
}

// ============================================================================
// Walk Functions (Mutable) — structurally identical to Visitor versions
// ============================================================================

pub fn walk_program_mut<V: VisitMut>(v: &mut V, program: &mut Program) {
    for import in &mut program.imports {
        v.visit_import_mut(import);
    }
    for func in &mut program.functions {
        v.visit_function_mut(func);
    }
    for extern_fn in &mut program.extern_fns {
        v.visit_extern_fn_mut(extern_fn);
    }
    for class in &mut program.classes {
        v.visit_class_mut(class);
    }
    for trait_decl in &mut program.traits {
        v.visit_trait_mut(trait_decl);
    }
    for enum_decl in &mut program.enums {
        v.visit_enum_mut(enum_decl);
    }
    if let Some(app) = &mut program.app {
        v.visit_app_mut(app);
    }
    for stage in &mut program.stages {
        v.visit_stage_mut(stage);
    }
    if let Some(system) = &mut program.system {
        v.visit_system_mut(system);
    }
    for error_decl in &mut program.errors {
        v.visit_error_mut(error_decl);
    }
}

pub fn walk_import_mut<V: VisitMut>(_v: &mut V, _import: &mut Spanned<ImportDecl>) {
    // No nested AST nodes
}

pub fn walk_extern_fn_mut<V: VisitMut>(v: &mut V, extern_fn: &mut Spanned<ExternFnDecl>) {
    for param in &mut extern_fn.node.params {
        v.visit_type_expr_mut(&mut param.ty);
    }
    if let Some(rt) = &mut extern_fn.node.return_type {
        v.visit_type_expr_mut(rt);
    }
}

pub fn walk_function_mut<V: VisitMut>(v: &mut V, func: &mut Spanned<Function>) {
    for param in &mut func.node.params {
        v.visit_type_expr_mut(&mut param.ty);
    }
    if let Some(rt) = &mut func.node.return_type {
        v.visit_type_expr_mut(rt);
    }
    v.visit_block_mut(&mut func.node.body);
    for contract in &mut func.node.contracts {
        v.visit_expr_mut(&mut contract.node.expr);
    }
}

pub fn walk_class_mut<V: VisitMut>(v: &mut V, class: &mut Spanned<ClassDecl>) {
    for field in &mut class.node.fields {
        v.visit_type_expr_mut(&mut field.ty);
    }
    for method in &mut class.node.methods {
        v.visit_function_mut(method);
    }
    for invariant in &mut class.node.invariants {
        v.visit_expr_mut(&mut invariant.node.expr);
    }
}

pub fn walk_trait_mut<V: VisitMut>(v: &mut V, trait_decl: &mut Spanned<TraitDecl>) {
    for method in &mut trait_decl.node.methods {
        for param in &mut method.params {
            v.visit_type_expr_mut(&mut param.ty);
        }
        if let Some(rt) = &mut method.return_type {
            v.visit_type_expr_mut(rt);
        }
        if let Some(body) = &mut method.body {
            v.visit_block_mut(body);
        }
        for contract in &mut method.contracts {
            v.visit_expr_mut(&mut contract.node.expr);
        }
    }
}

pub fn walk_enum_mut<V: VisitMut>(v: &mut V, enum_decl: &mut Spanned<EnumDecl>) {
    for variant in &mut enum_decl.node.variants {
        for field in &mut variant.fields {
            v.visit_type_expr_mut(&mut field.ty);
        }
    }
}

pub fn walk_error_mut<V: VisitMut>(v: &mut V, error_decl: &mut Spanned<ErrorDecl>) {
    for field in &mut error_decl.node.fields {
        v.visit_type_expr_mut(&mut field.ty);
    }
}

pub fn walk_app_mut<V: VisitMut>(v: &mut V, app: &mut Spanned<AppDecl>) {
    for field in &mut app.node.inject_fields {
        v.visit_type_expr_mut(&mut field.ty);
    }
    for method in &mut app.node.methods {
        v.visit_function_mut(method);
    }
}

pub fn walk_stage_mut<V: VisitMut>(v: &mut V, stage: &mut Spanned<StageDecl>) {
    for field in &mut stage.node.inject_fields {
        v.visit_type_expr_mut(&mut field.ty);
    }
    for required_method in &mut stage.node.required_methods {
        for param in &mut required_method.node.params {
            v.visit_type_expr_mut(&mut param.ty);
        }
        if let Some(rt) = &mut required_method.node.return_type {
            v.visit_type_expr_mut(rt);
        }
    }
    for method in &mut stage.node.methods {
        v.visit_function_mut(method);
    }
}

pub fn walk_system_mut<V: VisitMut>(_v: &mut V, _system: &mut Spanned<SystemDecl>) {
    // No nested AST nodes
}

pub fn walk_block_mut<V: VisitMut>(v: &mut V, block: &mut Spanned<Block>) {
    for stmt in &mut block.node.stmts {
        v.visit_stmt_mut(stmt);
    }
}

pub fn walk_stmt_mut<V: VisitMut>(v: &mut V, stmt: &mut Spanned<Stmt>) {
    match &mut stmt.node {
        Stmt::Let { ty, value, .. } => {
            if let Some(te) = ty {
                v.visit_type_expr_mut(te);
            }
            v.visit_expr_mut(value);
        }
        Stmt::Return(Some(expr)) => v.visit_expr_mut(expr),
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
        Stmt::Assign { value, .. } => v.visit_expr_mut(value),
        Stmt::FieldAssign { object, value, .. } => {
            v.visit_expr_mut(object);
            v.visit_expr_mut(value);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            v.visit_expr_mut(condition);
            v.visit_block_mut(then_block);
            if let Some(eb) = else_block {
                v.visit_block_mut(eb);
            }
        }
        Stmt::While { condition, body } => {
            v.visit_expr_mut(condition);
            v.visit_block_mut(body);
        }
        Stmt::For { iterable, body, .. } => {
            v.visit_expr_mut(iterable);
            v.visit_block_mut(body);
        }
        Stmt::IndexAssign {
            object,
            index,
            value,
        } => {
            v.visit_expr_mut(object);
            v.visit_expr_mut(index);
            v.visit_expr_mut(value);
        }
        Stmt::Match { expr, arms } => {
            v.visit_expr_mut(expr);
            for arm in arms {
                for te in &mut arm.type_args {
                    v.visit_type_expr_mut(te);
                }
                v.visit_block_mut(&mut arm.body);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields {
                v.visit_expr_mut(val);
            }
        }
        Stmt::LetChan {
            elem_type,
            capacity,
            ..
        } => {
            v.visit_type_expr_mut(elem_type);
            if let Some(cap) = capacity {
                v.visit_expr_mut(cap);
            }
        }
        Stmt::Select { arms, default } => {
            for arm in arms {
                match &mut arm.op {
                    SelectOp::Recv { channel, .. } => v.visit_expr_mut(channel),
                    SelectOp::Send { channel, value } => {
                        v.visit_expr_mut(channel);
                        v.visit_expr_mut(value);
                    }
                }
                v.visit_block_mut(&mut arm.body);
            }
            if let Some(d) = default {
                v.visit_block_mut(d);
            }
        }
        Stmt::Scope {
            seeds,
            bindings,
            body,
        } => {
            for seed in seeds {
                v.visit_expr_mut(seed);
            }
            for binding in bindings {
                v.visit_type_expr_mut(&mut binding.ty);
            }
            v.visit_block_mut(body);
        }
        Stmt::Yield { value } => v.visit_expr_mut(value),
        Stmt::Expr(expr) => v.visit_expr_mut(expr),
    }
}

pub fn walk_expr_mut<V: VisitMut>(v: &mut V, expr: &mut Spanned<Expr>) {
    match &mut expr.node {
        Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::NoneLit
        | Expr::Ident(_)
        | Expr::ClosureCreate { .. } => {}

        Expr::UnaryOp { operand, .. } => v.visit_expr_mut(operand),
        Expr::Propagate { expr: inner } => v.visit_expr_mut(inner),
        Expr::NullPropagate { expr: inner } => v.visit_expr_mut(inner),
        Expr::Spawn { call } => v.visit_expr_mut(call),
        Expr::Cast {
            expr: inner,
            target_type,
        } => {
            v.visit_expr_mut(inner);
            v.visit_type_expr_mut(target_type);
        }
        Expr::FieldAccess { object, .. } => v.visit_expr_mut(object),

        Expr::BinOp { lhs, rhs, .. } => {
            v.visit_expr_mut(lhs);
            v.visit_expr_mut(rhs);
        }
        Expr::Index { object, index } => {
            v.visit_expr_mut(object);
            v.visit_expr_mut(index);
        }
        Expr::Range { start, end, .. } => {
            v.visit_expr_mut(start);
            v.visit_expr_mut(end);
        }

        Expr::Call {
            args, type_args, ..
        } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for arg in args {
                v.visit_expr_mut(arg);
            }
        }
        Expr::MethodCall { object, args, .. } => {
            v.visit_expr_mut(object);
            for arg in args {
                v.visit_expr_mut(arg);
            }
        }
        Expr::StaticTraitCall {
            type_args, args, ..
        } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for arg in args {
                v.visit_expr_mut(arg);
            }
        }

        Expr::StructLit {
            type_args, fields, ..
        } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for (_, val) in fields {
                v.visit_expr_mut(val);
            }
        }
        Expr::ArrayLit { elements } => {
            for el in elements {
                v.visit_expr_mut(el);
            }
        }
        Expr::EnumUnit { type_args, .. } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
        }
        Expr::EnumData {
            type_args, fields, ..
        } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for (_, val) in fields {
                v.visit_expr_mut(val);
            }
        }
        Expr::MapLit {
            key_type,
            value_type,
            entries,
        } => {
            v.visit_type_expr_mut(key_type);
            v.visit_type_expr_mut(value_type);
            for (k, val) in entries {
                v.visit_expr_mut(k);
                v.visit_expr_mut(val);
            }
        }
        Expr::SetLit {
            elem_type,
            elements,
        } => {
            v.visit_type_expr_mut(elem_type);
            for el in elements {
                v.visit_expr_mut(el);
            }
        }

        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    v.visit_expr_mut(e);
                }
            }
        }

        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            for p in params {
                v.visit_type_expr_mut(&mut p.ty);
            }
            if let Some(rt) = return_type {
                v.visit_type_expr_mut(rt);
            }
            v.visit_block_mut(body);
        }

        Expr::Catch { expr: inner, handler } => {
            v.visit_expr_mut(inner);
            match handler {
                CatchHandler::Wildcard { body, .. } => v.visit_block_mut(body),
                CatchHandler::Shorthand(fallback) => v.visit_expr_mut(fallback),
            }
        }

        Expr::QualifiedAccess { .. } => {}
    }
}

pub fn walk_type_expr_mut<V: VisitMut>(v: &mut V, te: &mut Spanned<TypeExpr>) {
    match &mut te.node {
        TypeExpr::Named(_) | TypeExpr::Qualified { .. } => {}
        TypeExpr::Array(inner) => v.visit_type_expr_mut(inner),
        TypeExpr::Nullable(inner) => v.visit_type_expr_mut(inner),
        TypeExpr::Stream(inner) => v.visit_type_expr_mut(inner),
        TypeExpr::Fn {
            params,
            return_type,
        } => {
            for p in params {
                v.visit_type_expr_mut(p);
            }
            v.visit_type_expr_mut(return_type);
        }
        TypeExpr::Generic { type_args, .. } => {
            for ta in type_args {
                v.visit_type_expr_mut(ta);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ============================================================================
    // Test Visitor: Collects all visited expression types
    // ============================================================================

    #[derive(Default)]
    struct ExprCollector {
        visited: HashSet<String>,
    }

    impl Visitor for ExprCollector {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            let expr_type = match &expr.node {
                Expr::IntLit(_) => "IntLit",
                Expr::FloatLit(_) => "FloatLit",
                Expr::BoolLit(_) => "BoolLit",
                Expr::StringLit(_) => "StringLit",
                Expr::NoneLit => "NoneLit",
                Expr::Ident(_) => "Ident",
                Expr::BinOp { .. } => "BinOp",
                Expr::UnaryOp { .. } => "UnaryOp",
                Expr::Call { .. } => "Call",
                Expr::MethodCall { .. } => "MethodCall",
                Expr::StructLit { .. } => "StructLit",
                Expr::FieldAccess { .. } => "FieldAccess",
                Expr::ArrayLit { .. } => "ArrayLit",
                Expr::Index { .. } => "Index",
                Expr::EnumUnit { .. } => "EnumUnit",
                Expr::EnumData { .. } => "EnumData",
                Expr::Closure { .. } => "Closure",
                Expr::Catch { .. } => "Catch",
                Expr::Propagate { .. } => "Propagate",
                Expr::Cast { .. } => "Cast",
                Expr::StringInterp { .. } => "StringInterp",
                Expr::Range { .. } => "Range",
                Expr::ClosureCreate { .. } => "ClosureCreate",
                Expr::Spawn { .. } => "Spawn",
                Expr::MapLit { .. } => "MapLit",
                Expr::SetLit { .. } => "SetLit",
                Expr::NullPropagate { .. } => "NullPropagate",
                Expr::StaticTraitCall { .. } => "StaticTraitCall",
                Expr::QualifiedAccess { .. } => "QualifiedAccess",
            };
            self.visited.insert(expr_type.to_string());
            walk_expr(self, expr);
        }
    }

    // Helper to create dummy spanned nodes
    fn dummy<T>(node: T) -> Spanned<T> {
        Spanned::dummy(node)
    }

    // ============================================================================
    // Test: walk_expr visits BinOp children
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_binop_children() {
        let lhs = Box::new(dummy(Expr::IntLit(1)));
        let rhs = Box::new(dummy(Expr::IntLit(2)));
        let binop = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs,
            rhs,
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&binop);

        // Should visit BinOp and both IntLit children
        assert!(collector.visited.contains("BinOp"));
        assert!(collector.visited.contains("IntLit"));
        assert_eq!(collector.visited.len(), 2);
    }

    // ============================================================================
    // Test: walk_expr visits Call args
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_call_args() {
        let call = dummy(Expr::Call {
            name: dummy("func".to_string()),
            type_args: vec![],
            args: vec![dummy(Expr::IntLit(10)), dummy(Expr::IntLit(20))],
            target_id: None,
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&call);

        // Should visit Call and IntLit args
        assert!(collector.visited.contains("Call"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits nested structures
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_nested_structures() {
        // Build: StructLit { field: BinOp { lhs: IntLit(1), rhs: IntLit(2) } }
        let binop = dummy(Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(dummy(Expr::IntLit(1))),
            rhs: Box::new(dummy(Expr::IntLit(2))),
        });
        let struct_lit = dummy(Expr::StructLit {
            name: dummy("Foo".to_string()),
            type_args: vec![],
            fields: vec![(dummy("value".to_string()), binop)],
            target_id: None,
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&struct_lit);

        // Should visit StructLit, BinOp, and IntLit
        assert!(collector.visited.contains("StructLit"));
        assert!(collector.visited.contains("BinOp"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits ArrayLit elements
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_array_elements() {
        let array = dummy(Expr::ArrayLit {
            elements: vec![
                dummy(Expr::IntLit(1)),
                dummy(Expr::IntLit(2)),
                dummy(Expr::IntLit(3)),
            ],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&array);

        assert!(collector.visited.contains("ArrayLit"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits MethodCall object and args
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_method_call() {
        let method_call = dummy(Expr::MethodCall {
            object: Box::new(dummy(Expr::Ident("obj".to_string()))),
            method: dummy("foo".to_string()),
            args: vec![dummy(Expr::IntLit(42))],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&method_call);

        assert!(collector.visited.contains("MethodCall"));
        assert!(collector.visited.contains("Ident"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits Index object and index
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_index() {
        let index_expr = dummy(Expr::Index {
            object: Box::new(dummy(Expr::Ident("arr".to_string()))),
            index: Box::new(dummy(Expr::IntLit(0))),
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&index_expr);

        assert!(collector.visited.contains("Index"));
        assert!(collector.visited.contains("Ident"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits MapLit entries
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_map_lit() {
        let map = dummy(Expr::MapLit {
            key_type: dummy(TypeExpr::Named("int".to_string())),
            value_type: dummy(TypeExpr::Named("string".to_string())),
            entries: vec![
                (dummy(Expr::IntLit(1)), dummy(Expr::StringLit("a".to_string()))),
                (dummy(Expr::IntLit(2)), dummy(Expr::StringLit("b".to_string()))),
            ],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&map);

        assert!(collector.visited.contains("MapLit"));
        assert!(collector.visited.contains("IntLit"));
        assert!(collector.visited.contains("StringLit"));
    }

    // ============================================================================
    // Test: walk_expr visits SetLit elements
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_set_lit() {
        let set = dummy(Expr::SetLit {
            elem_type: dummy(TypeExpr::Named("int".to_string())),
            elements: vec![dummy(Expr::IntLit(1)), dummy(Expr::IntLit(2))],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&set);

        assert!(collector.visited.contains("SetLit"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits Range start and end
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_range() {
        let range = dummy(Expr::Range {
            start: Box::new(dummy(Expr::IntLit(0))),
            end: Box::new(dummy(Expr::IntLit(10))),
            inclusive: false,
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&range);

        assert!(collector.visited.contains("Range"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits Propagate inner expression
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_propagate() {
        let propagate = dummy(Expr::Propagate {
            expr: Box::new(dummy(Expr::Call {
                name: dummy("foo".to_string()),
                type_args: vec![],
                args: vec![],
                target_id: None,
            })),
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&propagate);

        assert!(collector.visited.contains("Propagate"));
        assert!(collector.visited.contains("Call"));
    }

    // ============================================================================
    // Test: walk_expr visits NullPropagate inner expression
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_null_propagate() {
        let null_prop = dummy(Expr::NullPropagate {
            expr: Box::new(dummy(Expr::Ident("x".to_string()))),
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&null_prop);

        assert!(collector.visited.contains("NullPropagate"));
        assert!(collector.visited.contains("Ident"));
    }

    // ============================================================================
    // Test: walk_expr visits Cast expression and target type
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_cast() {
        let cast = dummy(Expr::Cast {
            expr: Box::new(dummy(Expr::IntLit(42))),
            target_type: dummy(TypeExpr::Named("float".to_string())),
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&cast);

        assert!(collector.visited.contains("Cast"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr handles StringInterp parts
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_string_interp() {
        let string_interp = dummy(Expr::StringInterp {
            parts: vec![
                StringInterpPart::Lit("Hello ".to_string()),
                StringInterpPart::Expr(dummy(Expr::Ident("name".to_string()))),
                StringInterpPart::Lit("!".to_string()),
            ],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&string_interp);

        assert!(collector.visited.contains("StringInterp"));
        assert!(collector.visited.contains("Ident"));
    }

    // ============================================================================
    // Test: walk_expr visits EnumData fields
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_enum_data() {
        let enum_data = dummy(Expr::EnumData {
            enum_name: dummy("Option".to_string()),
            variant: dummy("Some".to_string()),
            type_args: vec![],
            fields: vec![(dummy("value".to_string()), dummy(Expr::IntLit(42)))],
            enum_id: None,
            variant_id: None,
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&enum_data);

        assert!(collector.visited.contains("EnumData"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_expr visits StaticTraitCall args
    // ============================================================================

    #[test]
    fn test_walk_expr_visits_static_trait_call() {
        let static_call = dummy(Expr::StaticTraitCall {
            trait_name: dummy("TypeInfo".to_string()),
            method_name: dummy("type_name".to_string()),
            type_args: vec![dummy(TypeExpr::Named("int".to_string()))],
            args: vec![],
        });

        let mut collector = ExprCollector::default();
        collector.visit_expr(&static_call);

        assert!(collector.visited.contains("StaticTraitCall"));
    }

    // ============================================================================
    // Test: walk_stmt visits all statement types
    // ============================================================================

    #[test]
    fn test_walk_stmt_visits_if_branches() {
        let if_stmt = dummy(Stmt::If {
            condition: dummy(Expr::BoolLit(true)),
            then_block: dummy(Block {
                stmts: vec![dummy(Stmt::Return(Some(dummy(Expr::IntLit(1)))))],
            }),
            else_block: Some(dummy(Block {
                stmts: vec![dummy(Stmt::Return(Some(dummy(Expr::IntLit(2)))))],
            })),
        });

        let mut collector = ExprCollector::default();
        collector.visit_stmt(&if_stmt);

        // Should visit condition and return expressions in both branches
        assert!(collector.visited.contains("BoolLit"));
        assert!(collector.visited.contains("IntLit"));
    }

    #[test]
    fn test_walk_stmt_visits_let_value() {
        let let_stmt = dummy(Stmt::Let {
            name: dummy("x".to_string()),
            ty: None,
            value: dummy(Expr::IntLit(42)),
            is_mut: false,
        });

        let mut collector = ExprCollector::default();
        collector.visit_stmt(&let_stmt);

        assert!(collector.visited.contains("IntLit"));
    }

    #[test]
    fn test_walk_stmt_visits_match_arms() {
        let match_stmt = dummy(Stmt::Match {
            expr: dummy(Expr::Ident("x".to_string())),
            arms: vec![
                MatchArm {
                    enum_name: dummy("Option".to_string()),
                    variant_name: dummy("Some".to_string()),
                    type_args: vec![],
                    bindings: vec![],
                    enum_id: None,
                    variant_id: None,
                    body: dummy(Block {
                        stmts: vec![dummy(Stmt::Return(Some(dummy(Expr::IntLit(1)))))],
                    }),
                },
                MatchArm {
                    enum_name: dummy("Option".to_string()),
                    variant_name: dummy("None".to_string()),
                    type_args: vec![],
                    bindings: vec![],
                    enum_id: None,
                    variant_id: None,
                    body: dummy(Block {
                        stmts: vec![dummy(Stmt::Return(Some(dummy(Expr::IntLit(0)))))],
                    }),
                },
            ],
        });

        let mut collector = ExprCollector::default();
        collector.visit_stmt(&match_stmt);

        assert!(collector.visited.contains("Ident"));
        assert!(collector.visited.contains("IntLit"));
    }

    // ============================================================================
    // Test: walk_type_expr visits all type expression types
    // ============================================================================

    #[test]
    fn test_walk_type_expr_visits_generic_args() {
        let generic_te = dummy(TypeExpr::Generic {
            name: "Map".to_string(),
            type_args: vec![
                dummy(TypeExpr::Named("int".to_string())),
                dummy(TypeExpr::Named("string".to_string())),
            ],
        });

        #[derive(Default)]
        struct TypeExprCollector {
            count: usize,
        }

        impl Visitor for TypeExprCollector {
            fn visit_type_expr(&mut self, _te: &Spanned<TypeExpr>) {
                self.count += 1;
                walk_type_expr(self, _te);
            }
        }

        let mut collector = TypeExprCollector::default();
        collector.visit_type_expr(&generic_te);

        // Should visit Generic + 2 Named type args = 3 total
        assert_eq!(collector.count, 3);
    }

    #[test]
    fn test_walk_type_expr_visits_array_element() {
        let array_te = dummy(TypeExpr::Array(Box::new(dummy(TypeExpr::Named(
            "int".to_string(),
        )))));

        #[derive(Default)]
        struct TypeExprCollector {
            count: usize,
        }

        impl Visitor for TypeExprCollector {
            fn visit_type_expr(&mut self, _te: &Spanned<TypeExpr>) {
                self.count += 1;
                walk_type_expr(self, _te);
            }
        }

        let mut collector = TypeExprCollector::default();
        collector.visit_type_expr(&array_te);

        // Should visit Array + Named = 2 total
        assert_eq!(collector.count, 2);
    }

    #[test]
    fn test_walk_type_expr_visits_fn_params_and_return() {
        let fn_te = dummy(TypeExpr::Fn {
            params: vec![
                Box::new(dummy(TypeExpr::Named("int".to_string()))),
                Box::new(dummy(TypeExpr::Named("float".to_string()))),
            ],
            return_type: Box::new(dummy(TypeExpr::Named("string".to_string()))),
        });

        #[derive(Default)]
        struct TypeExprCollector {
            count: usize,
        }

        impl Visitor for TypeExprCollector {
            fn visit_type_expr(&mut self, _te: &Spanned<TypeExpr>) {
                self.count += 1;
                walk_type_expr(self, _te);
            }
        }

        let mut collector = TypeExprCollector::default();
        collector.visit_type_expr(&fn_te);

        // Should visit Fn + 2 params + 1 return = 4 total
        assert_eq!(collector.count, 4);
    }
}

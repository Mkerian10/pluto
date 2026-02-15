# Visitor Pattern Implementation Plan

**Status:** Implementation Ready (Corrected)
**Date:** 2026-02-12
**Companion to:** `rfc-visitor-pattern.md`

---

## Overview

This document provides detailed technical implementation steps for the visitor pattern RFC. Each phase is designed to be independently reviewable and testable.

**Key Principles:**
- All code samples use exact AST field names from `src/parser/ast.rs`
- All type signatures match actual AST types (including `Spanned<T>` wrappers)
- Preserve current behavior before optimizing
- Every code sample is copy-pasteable into the actual codebase

---

## Phase 0: Infrastructure (1-2 days, ~500 LOC)

### Goal
Create the core visitor infrastructure without touching existing code. Zero risk — this phase only adds new code, no modifications.

### Implementation Steps

#### Step 0.1: Create `src/visit.rs`

**File structure:**
```rust
// src/visit.rs

use crate::parser::ast::*;
use crate::span::Spanned;

// ============================================================================
// Visitor Trait (Read-Only)
// ============================================================================

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
        Stmt::If { condition, then_block, else_block } => {
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
        Stmt::IndexAssign { object, index, value } => {
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
        Stmt::LetChan { elem_type, capacity, .. } => {
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
        Stmt::Scope { seeds, bindings, body } => {
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
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
        | Expr::StringLit(_) | Expr::NoneLit
        | Expr::Ident(_) | Expr::ClosureCreate { .. } => {}

        // Unary wrappers
        Expr::UnaryOp { operand, .. } => v.visit_expr(operand),
        Expr::Propagate { expr: inner } => v.visit_expr(inner),
        Expr::NullPropagate { expr: inner } => v.visit_expr(inner),
        Expr::Spawn { call } => v.visit_expr(call),
        Expr::Cast { expr: inner, target_type } => {
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
        Expr::Call { args, type_args, .. } => {
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
        Expr::StaticTraitCall { type_args, args, .. } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for arg in args {
                v.visit_expr(arg);
            }
        }

        // Compound literals
        Expr::StructLit { type_args, fields, .. } => {
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
        Expr::EnumData { type_args, fields, .. } => {
            for te in type_args {
                v.visit_type_expr(te);
            }
            for (_, val) in fields {
                v.visit_expr(val);
            }
        }
        Expr::MapLit { key_type, value_type, entries } => {
            v.visit_type_expr(key_type);
            v.visit_type_expr(value_type);
            for (k, val) in entries {
                v.visit_expr(k);
                v.visit_expr(val);
            }
        }
        Expr::SetLit { elem_type, elements } => {
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
        Expr::Closure { params, return_type, body } => {
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
        TypeExpr::Fn { params, return_type } => {
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

// Walk functions for VisitMut — structurally identical to Visitor versions but with &mut

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
        Stmt::If { condition, then_block, else_block } => {
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
        Stmt::IndexAssign { object, index, value } => {
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
        Stmt::LetChan { elem_type, capacity, .. } => {
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
        Stmt::Scope { seeds, bindings, body } => {
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
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
        | Expr::StringLit(_) | Expr::NoneLit
        | Expr::Ident(_) | Expr::ClosureCreate { .. } => {}

        Expr::UnaryOp { operand, .. } => v.visit_expr_mut(operand),
        Expr::Propagate { expr: inner } => v.visit_expr_mut(inner),
        Expr::NullPropagate { expr: inner } => v.visit_expr_mut(inner),
        Expr::Spawn { call } => v.visit_expr_mut(call),
        Expr::Cast { expr: inner, target_type } => {
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

        Expr::Call { args, type_args, .. } => {
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
        Expr::StaticTraitCall { type_args, args, .. } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for arg in args {
                v.visit_expr_mut(arg);
            }
        }

        Expr::StructLit { type_args, fields, .. } => {
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
        Expr::EnumData { type_args, fields, .. } => {
            for te in type_args {
                v.visit_type_expr_mut(te);
            }
            for (_, val) in fields {
                v.visit_expr_mut(val);
            }
        }
        Expr::MapLit { key_type, value_type, entries } => {
            v.visit_type_expr_mut(key_type);
            v.visit_type_expr_mut(value_type);
            for (k, val) in entries {
                v.visit_expr_mut(k);
                v.visit_expr_mut(val);
            }
        }
        Expr::SetLit { elem_type, elements } => {
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

        Expr::Closure { params, return_type, body } => {
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
        TypeExpr::Fn { params, return_type } => {
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
```

**Testing:**
```rust
// tests/integration/visit.rs

#[cfg(test)]
mod tests {
    use pluto::visit::{Visitor, walk_expr};
    use pluto::parser::*;
    use pluto::span::{Span, Spanned};

    #[test]
    fn test_visitor_visits_all_expr_types() {
        // Parse a program with diverse expression types
        let source = r#"
            fn main() {
                let x = 42
                let y = x + 1
                let z = Map<int, int> { 1: 2 }
                let s = Set<int> { 1, 2 }
            }
        "#;

        let tokens = lex(source).unwrap();
        let program = parse(&tokens).unwrap();

        struct CountingVisitor {
            expr_count: usize,
            map_lit_count: usize,
            set_lit_count: usize,
        }

        impl Visitor for CountingVisitor {
            fn visit_expr(&mut self, expr: &Spanned<crate::parser::ast::Expr>) {
                self.expr_count += 1;

                use crate::parser::ast::Expr;
                match &expr.node {
                    Expr::MapLit { .. } => self.map_lit_count += 1,
                    Expr::SetLit { .. } => self.set_lit_count += 1,
                    _ => {}
                }

                walk_expr(self, expr);
            }
        }

        let mut visitor = CountingVisitor {
            expr_count: 0,
            map_lit_count: 0,
            set_lit_count: 0,
        };
        visitor.visit_program(&program);

        assert!(visitor.expr_count > 0, "Should visit expressions");
        assert_eq!(visitor.map_lit_count, 1, "Should find MapLit");
        assert_eq!(visitor.set_lit_count, 1, "Should find SetLit");
    }

    #[test]
    fn test_visitor_can_prune_traversal() {
        let source = r#"
            fn main() {
                spawn compute()
            }
            fn compute() int {
                return 42
            }
        "#;

        let tokens = lex(source).unwrap();
        let program = parse(&tokens).unwrap();

        struct PruningVisitor {
            found_return_inside_spawn: bool,
        }

        impl Visitor for PruningVisitor {
            fn visit_expr(&mut self, expr: &Spanned<crate::parser::ast::Expr>) {
                use crate::parser::ast::Expr;
                if matches!(expr.node, Expr::Spawn { .. }) {
                    // Don't recurse into spawn — prune here
                    return;
                }
                walk_expr(self, expr);
            }

            fn visit_stmt(&mut self, stmt: &Spanned<crate::parser::ast::Stmt>) {
                use crate::parser::ast::Stmt;
                if matches!(stmt.node, Stmt::Return(_)) {
                    self.found_return_inside_spawn = true;
                }
                walk_stmt(self, stmt);
            }
        }

        let mut visitor = PruningVisitor { found_return_inside_spawn: false };
        visitor.visit_program(&program);

        // The return is inside compute(), which is NOT inside spawn (spawn only contains call)
        // So we should NOT find it because we pruned at Spawn
        assert!(!visitor.found_return_inside_spawn, "Should not traverse into spawn");
    }
}
```

#### Step 0.2: Add `src/visit.rs` to `src/lib.rs`

```rust
// src/lib.rs
pub mod visit;  // Add this line
```

#### Step 0.3: Run tests

```bash
cargo test --test visit
```

Expected: Both integration tests pass.

### Deliverable

- `src/visit.rs` with `Visitor` and `VisitMut` traits (~500 LOC)
- All walk functions with exhaustive matches (no `_ => {}`)
- 2 integration tests demonstrating visitor behavior
- PR with title "Add visitor pattern infrastructure"

### Success Criteria

- No new warnings in modified files
- All integration tests pass
- Zero changes to existing compiler code
- walk_* functions cover all Program fields (imports, functions, extern_fns, classes, traits, enums, app, stages, system, errors)

---

## Phase 1: Bug-Fixing Conversions (2-3 days, 4 PRs)

### Goal
Convert the 4 walkers with known bugs to visitor implementations. Each conversion is its own PR. Preserve current behavior first, then fix the bug.

### PR 1.1: Fix monomorphize.rs::resolve_generic_te_in_expr

**Current bug:** Catch-all at line 1487 skips `MapLit`, `SetLit`, `StaticTraitCall`.

**Current code structure:** `resolve_generic_te_in_expr` takes `&mut Expr` and walks it, calling `resolve_generic_te` on each nested `TypeExpr`.

**Implementation:**

```rust
// src/monomorphize.rs

use crate::visit::{VisitMut, walk_type_expr_mut};
use crate::span::Spanned;

struct GenericTypeResolver<'a> {
    env: &'a mut TypeEnv,
}

impl VisitMut for GenericTypeResolver<'_> {
    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        // Resolve this type expression
        resolve_generic_te(&mut te.node, self.env).unwrap();
        // Then recurse into children
        walk_type_expr_mut(self, te);
    }
}

// Replace the body of resolve_generic_instances_in_body
fn resolve_generic_instances_in_body(body: &mut Spanned<Block>, env: &mut TypeEnv) -> Result<(), CompileError> {
    let mut visitor = GenericTypeResolver { env };
    visitor.visit_block_mut(body);
    Ok(())
}
```

Then delete `resolve_generic_te_in_expr` and `resolve_generic_te_in_stmt` (they're no longer needed).

**Testing:**
- Add test: `Map<T, int> {}` inside a generic function body
- Add test: `StaticTraitCall` with `type_args` inside a generic function
- Verify both compile without errors

**Lines changed:** -80 (delete old walkers), +15 (new visitor impl) = **-65 net**

**Deliverable:** PR with title "Fix generic type resolution for MapLit/SetLit/StaticTraitCall"

---

### PR 1.2: Fix typeck/errors.rs::contains_propagate

**Current bug:** Catch-all misses `StaticTraitCall` args.

**Implementation:**

```rust
// src/typeck/errors.rs

use crate::visit::{Visitor, walk_expr};
use crate::span::Spanned;

struct PropagateDetector {
    found: bool,
}

impl Visitor for PropagateDetector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if matches!(expr.node, Expr::Propagate { .. }) {
            self.found = true;
            // No need to recurse once found (optimization)
            return;
        }
        walk_expr(self, expr);
    }
}

fn contains_propagate(expr: &Spanned<Expr>) -> bool {
    let mut detector = PropagateDetector { found: false };
    detector.visit_expr(expr);
    detector.found
}
```

Delete the old `contains_propagate` function (28 lines) and `stmt_contains_propagate` (17 lines).

**Note:** Current callers pass `&Expr`, but they already have `&Spanned<Expr>` in their context. Update call sites to pass the Spanned version.

**Testing:**
- Add test: `TypeInfo::kind<User>()!` should be detected as containing propagate
- Verify existing tests still pass

**Lines changed:** -45, +20 = **-25 net**

**Deliverable:** PR with title "Fix propagate detection for StaticTraitCall"

---

### PR 1.3: Fix derived.rs::collect_deps_from_expr

**Current bug:** Doesn't collect `StaticTraitCall` trait method references.

**Current behavior:** Recursively calls `collect_test_dependencies(fn_name, program, visited, deps)` for each function call found.

**Implementation (preserving transitive recursion):**

```rust
// src/derived.rs

use crate::visit::{Visitor, walk_expr};
use crate::span::Spanned;
use std::collections::HashSet;

struct DependencyCollector<'a> {
    program: &'a Program,
    visited: &'a mut HashSet<String>,
    deps: &'a mut Vec<String>,
}

impl Visitor for DependencyCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Call { name, .. } => {
                let fn_name = &name.node;
                // CRITICAL: Preserve transitive dependency collection
                collect_test_dependencies(fn_name, self.program, self.visited, self.deps);
            }
            Expr::StaticTraitCall { trait_name, method_name, .. } => {
                // NEW: Collect trait method as dependency
                let dep_name = format!("{}::{}", trait_name.node, method_name.node);
                if !self.visited.contains(&dep_name) {
                    self.visited.insert(dep_name.clone());
                    self.deps.push(dep_name);
                    // Note: Trait methods don't have bodies to recurse into (interface only)
                }
            }
            Expr::StructLit { name, .. } => {
                // Track class usage
                let class_name = &name.node;
                if !self.visited.contains(class_name) {
                    self.visited.insert(class_name.clone());
                    self.deps.push(class_name.clone());
                }
            }
            Expr::EnumUnit { enum_name, .. } | Expr::EnumData { enum_name, .. } => {
                // Track enum usage
                let enum_name_str = &enum_name.node;
                if !self.visited.contains(enum_name_str) {
                    self.visited.insert(enum_name_str.clone());
                    self.deps.push(enum_name_str.clone());
                }
            }
            _ => {}
        }
        // Always recurse to find nested dependencies
        walk_expr(self, expr);
    }
}

// Replace collect_deps_from_block and collect_deps_from_expr with:
fn collect_deps_from_function_body(
    func: &Function,
    program: &Program,
    visited: &mut HashSet<String>,
    deps: &mut Vec<String>,
) {
    let mut collector = DependencyCollector { program, visited, deps };
    collector.visit_block(&func.body);
}
```

Update `collect_test_dependencies` to call `collect_deps_from_function_body` instead of `collect_deps_from_block`.

**Testing:**
- Add test: test function that calls `TypeInfo::kind<User>()` should list that trait method as a dependency

**Lines changed:** -120 (delete collect_deps_from_block + collect_deps_from_expr), +40 = **-80 net**

**Deliverable:** PR with title "Fix test dependency tracking for StaticTraitCall"

---

### PR 1.4: Fix typeck/check.rs::check_stmt_for_self_mutation

**Current bug:** Misses `IndexAssign` (e.g., `self.array[i] = x`).

**Current behavior:** Returns `Result<(), CompileError>` with specific error messages. Recursively checks nested blocks and method calls.

**Implementation:**

```rust
// src/typeck/check.rs

use crate::visit::{Visitor, walk_stmt};
use crate::span::Spanned;

struct SelfMutationChecker<'a> {
    class_name: &'a str,
    env: &'a TypeEnv,
    error: Option<CompileError>,
}

impl Visitor for SelfMutationChecker<'_> {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        // Short-circuit if we already found an error
        if self.error.is_some() {
            return;
        }

        match &stmt.node {
            Stmt::FieldAssign { object, field, .. } => {
                if matches!(&object.node, Expr::Ident(name) if name == "self") {
                    self.error = Some(CompileError::type_err(
                        format!(
                            "cannot assign to 'self.{}' in a non-mut method; declare 'mut self' to modify fields",
                            field.node
                        ),
                        stmt.span,
                    ));
                    return;
                }
            }
            Stmt::IndexAssign { object, .. } => {
                // NEW: Handle self.array[i] = x and self.field.array[i] = x
                if is_mutation_on_self(&object.node) {
                    self.error = Some(CompileError::type_err(
                        "cannot mutate self's data in a non-mut method; declare 'mut self'".to_string(),
                        stmt.span,
                    ));
                    return;
                }
            }
            Stmt::Expr(expr) => {
                // Check for method calls on self with mut self methods
                if let Err(e) = check_expr_for_mut_method_call(&expr.node, expr.span, self.class_name, self.env) {
                    self.error = Some(e);
                    return;
                }
            }
            _ => {}
        }

        // Recurse into nested blocks
        walk_stmt(self, stmt);
    }
}

// Helper: detect mutations rooted at self (self.field[i], self[i], etc.)
fn is_mutation_on_self(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) if name == "self" => true,
        Expr::FieldAccess { object, .. } => is_mutation_on_self(&object.node),
        Expr::Index { object, .. } => is_mutation_on_self(&object.node),
        _ => false,
    }
}

// Separate checker for method calls (preserve existing logic)
fn check_expr_for_mut_method_call(
    expr: &Expr,
    span: crate::span::Span,
    class_name: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    if let Expr::MethodCall { object, method, .. } = expr {
        if matches!(&object.node, Expr::Ident(name) if name == "self") {
            let mangled = mangle_method(class_name, &method.node);
            if env.mut_self_methods.contains(&mangled) {
                return Err(CompileError::type_err(
                    format!(
                        "cannot call 'mut self' method '{}' on self in a non-mut method; declare 'mut self'",
                        method.node
                    ),
                    span,
                ));
            }
        }
    }
    Ok(())
}

// Replace check_body_for_self_mutation and check_stmt_for_self_mutation
fn check_body_for_self_mutation(
    body: &Block,
    class_name: &str,
    env: &TypeEnv,
) -> Result<(), CompileError> {
    let body_spanned = Spanned::dummy(body.clone());
    let mut checker = SelfMutationChecker {
        class_name,
        env,
        error: None
    };

    for stmt in &body.stmts {
        checker.visit_stmt(stmt);
        if let Some(err) = checker.error {
            return Err(err);
        }
    }

    Ok(())
}
```

Delete old `check_stmt_for_self_mutation` and `check_expr_for_self_mutation`.

**Testing:**
- Add test: method with `self.items[0] = x` should fail on non-mut receiver
- Add test: method with `self.map["key"] = value` should fail on non-mut receiver
- Verify existing tests still pass

**Lines changed:** -135 (delete 2 old functions), +60 = **-75 net**

**Deliverable:** PR with title "Fix self-mutation detection for IndexAssign"

---

### Phase 1 Summary

- 4 PRs, each fixing one bug
- Total lines removed: ~320
- Total lines added: ~135
- **Net reduction: -185 lines**
- All 4 bugs fixed
- Pattern established for future conversions

---

## Phase 2: High-Recursion Walkers (3-4 days, 6 PRs)

### Goal
Convert walkers where >85% of code is pure recursion. Highest code reduction.

### PR 2.1: Convert offset_*_spans (monomorphize.rs)

**Current:** 3 functions (`offset_type_expr_spans`, `offset_stmt_spans`, `offset_expr_spans`), ~200 lines, 95% pure recursion.

**Implementation:**

```rust
struct SpanOffsetter {
    offset: usize,
}

impl VisitMut for SpanOffsetter {
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

    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        te.span.start += self.offset;
        te.span.end += self.offset;
        walk_type_expr_mut(self, te);
    }
}

fn offset_body_spans(body: &mut Spanned<Block>, offset: usize) {
    let mut offsetter = SpanOffsetter { offset };
    offsetter.visit_block_mut(body);
}
```

Delete the 3 old functions. Update call sites to use `offset_body_spans`.

**Lines:** -200, +25 = **-175 net**

---

### PR 2.2: Convert collect_spawn_closure_names (codegen/mod.rs)

**Current:** ~140 lines, 90% pure recursion.

**Implementation:**

```rust
struct SpawnClosureCollector<'a> {
    names: &'a mut HashSet<String>,
}

impl Visitor for SpawnClosureCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Spawn { call } = &expr.node {
            if let Expr::ClosureCreate { fn_name, .. } = &call.node {
                self.names.insert(fn_name.clone());
            }
        }
        walk_expr(self, expr);
    }
}

fn collect_spawn_closure_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut collector = SpawnClosureCollector { names: &mut names };
    collector.visit_program(program);
    names
}
```

**Lines:** -140, +20 = **-120 net**

---

### PR 2.3: Convert spawn desugaring (spawn.rs)

**Current:** 2 functions, ~100 lines, 85% pure recursion.

**Implementation:**

```rust
struct SpawnDesugarer;

impl VisitMut for SpawnDesugarer {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // First recurse to handle nested spawns
        walk_expr_mut(self, expr);

        // Then desugar this node if it's a Spawn (bottom-up)
        if let Expr::Spawn { call } = &expr.node {
            // Wrap in closure
            let closure_expr = Expr::Closure {
                params: vec![],
                return_type: None,
                body: Spanned::dummy(Block {
                    stmts: vec![
                        Spanned::dummy(Stmt::Return(Some(call.clone())))
                    ],
                }),
            };
            *expr = Spanned::new(closure_expr, expr.span);
        }
    }
}

pub fn desugar_spawn(program: &mut Program) {
    let mut desugarer = SpawnDesugarer;
    desugarer.visit_program_mut(program);
}
```

**Lines:** -100, +25 = **-75 net**

---

### PR 2.4: Convert ambient rewriting (ambient.rs)

**Current:** 2 functions, ~100 lines, 80% pure recursion.

**Lines:** -100, +25 = **-75 net**

---

### PR 2.5: Convert collect_idents (typeck/check.rs)

**Current:** 2 functions, ~60 lines.

**Lines:** -60, +15 = **-45 net**

---

### PR 2.6: Convert free variable collection (typeck/closures.rs)

**Current:** 2 functions, ~80 lines.

**Special consideration:** This walker tracks scope depth and closure nesting. Needs state:

```rust
struct FreeVarCollector<'a> {
    param_names: &'a mut HashSet<String>,
    env: &'a TypeEnv,
    captures: &'a mut HashMap<String, PlutoType>,
    current_depth: usize,
}

impl Visitor for FreeVarCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Ident(name) => {
                // Check if it's a free variable
                if !self.param_names.contains(name) && self.current_depth > 0 {
                    // Capture it
                    if let Some(ty) = self.env.get_var_type(name) {
                        self.captures.insert(name.clone(), ty);
                    }
                }
            }
            Expr::Closure { .. } => {
                // Push scope depth
                self.current_depth += 1;
                walk_expr(self, expr);
                self.current_depth -= 1;
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        if let Stmt::For { var, .. } = &stmt.node {
            // Bind loop variable temporarily
            let was_present = self.param_names.insert(var.node.clone());
            walk_stmt(self, stmt);
            if !was_present {
                self.param_names.remove(&var.node);
            }
        } else {
            walk_stmt(self, stmt);
        }
    }
}
```

**Lines:** -80, +50 = **-30 net**

---

### Phase 2 Summary

- 6 PRs
- Total reduction: ~520 lines
- Establishes pattern for scope-sensitive walkers

---

## Phase 3: Medium-Recursion Walkers (4-5 days, ~8 PRs)

### Goal
Convert walkers with 60-85% pure recursion.

### Walkers to convert:

1. `rewrite_*_for_module` (modules.rs) — 2 funcs, ~150 lines → ~50 lines (**-100**)
2. `rewrite_*` for module qualified access (modules.rs) — 2 funcs, ~120 lines → ~40 lines (**-80**)
3. `rewrite_*` in monomorphize.rs — 2 funcs, ~120 lines → ~40 lines (**-80**)
4. `resolve_*` in xref.rs — 2 funcs, ~100 lines → ~35 lines (**-65**)
5. `collect_*_accesses` (concurrency.rs) — 2 funcs, ~130 lines → ~50 lines (**-80**)
6. `lift_in_*` (closures.rs) — 2 funcs, ~150 lines → ~60 lines (**-90**)
7. Narrow-purpose codegen utilities (3 funcs, ~120 lines → ~40 lines, **-80**)

**Special note on lift_in_*:** This transforms the AST (Closure → ClosureCreate). Requires bottom-up ordering:

```rust
struct ClosureLifter<'a> {
    env: &'a mut TypeEnv,
    closure_counter: &'a mut usize,
    lifted_funcs: &'a mut Vec<Spanned<Function>>,
}

impl VisitMut for ClosureLifter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // First recurse to lift nested closures (bottom-up)
        walk_expr_mut(self, expr);

        // Then lift this closure
        if let Expr::Closure { params, return_type, body } = &expr.node {
            // Generate lifted function
            let fn_name = format!("__closure_{}", self.closure_counter);
            *self.closure_counter += 1;

            // Collect captures (use existing logic)
            let captures = collect_free_vars(params, body, self.env);

            // Create ClosureCreate to replace this Closure
            expr.node = Expr::ClosureCreate {
                fn_name: fn_name.clone(),
                captures: captures.keys().cloned().collect(),
                target_id: None,
            };

            // Add lifted function
            let lifted_fn = Function {
                id: Uuid::new_v4(),
                name: Spanned::dummy(fn_name),
                type_params: vec![],
                type_param_bounds: HashMap::new(),
                params: /* params + __env */,
                return_type: return_type.clone(),
                contracts: vec![],
                body: body.clone(),
                is_pub: false,
                is_override: false,
                is_generator: false,
            };
            self.lifted_funcs.push(Spanned::dummy(lifted_fn));
        }
    }
}
```

### Phase 3 Summary

- ~8 PRs
- Total reduction: ~575 lines
- Covers all "medium complexity" walkers

---

## Phase 4: Document Walkers To Keep (1 day)

### Goal
Document why the remaining walkers (core passes) should stay as manual matches.

### Core walkers to keep:

| Walker | File | Lines | Reason to keep manual |
|--------|------|-------|----------------------|
| `check_stmt` | typeck/check.rs | ~650 | 80% custom type-checking logic per arm |
| `infer_expr` | typeck/infer.rs | ~800 | 85% custom type-inference logic |
| `lower_stmt` | codegen/lower.rs | ~1400 | 95% custom Cranelift IR emission |
| `lower_expr` | codegen/lower.rs | ~2700 | 95% custom Cranelift IR emission |
| `infer_type_for_expr` | codegen/lower.rs | ~600 | Tightly coupled to codegen context |
| `emit_stmt` | pretty.rs | ~240 | 90% custom formatting |
| `emit_expr` | pretty.rs | ~800 | 90% custom formatting |
| `emit_type_expr` | pretty.rs | ~100 | 90% custom formatting |

### Deliverable

Add a section to this document:

```markdown
## Walkers Intentionally Kept Manual

The following walkers use manual `match` statements because they have >70% custom logic per arm. Converting them to visitors would add indirection without reducing complexity.

### Typeck Core (`src/typeck/check.rs`, `src/typeck/infer.rs`)

- **`check_stmt`**: 650 lines, 80% custom type-checking logic
- **`infer_expr`**: 800 lines, 85% custom inference logic per variant

Example: `Expr::Call` requires function lookup, arg type checking, return type inference, error-ability propagation — all custom logic.

### Codegen (`src/codegen/lower.rs`)

- **`lower_stmt`**: 1400 lines, 95% custom Cranelift IR emission
- **`lower_expr`**: 2700 lines, 95% custom IR emission per variant

Example: `Expr::BinOp` emits `iadd`/`fadd`/`imul` based on operand types — deeply coupled to FunctionBuilder state.

### Pretty Printer (`src/pretty.rs`)

- **`emit_stmt`**: 240 lines, 90% custom formatting
- **`emit_expr`**: 800 lines, 90% custom precedence/parenthesization logic

Example: `Expr::BinOp` requires precedence-aware parenthesis insertion — custom logic per operator.

### Verification

All these functions use **exhaustive matching** (no `_ => {}`). They will get compiler errors if new AST variants are added, same as visitor-based code.
```

---

## Enforcement (Manual Review)

### Recommendation: Manual Code Review

**Avoid automated enforcement** via CI grep or Clippy — these approaches have false positives and don't understand context.

Instead, **add to code review checklist**:

When reviewing a new AST walker:
- [ ] If >50% of match arms are pure recursion, suggest using Visitor/VisitMut
- [ ] If the walker is one of the core passes (typeck, codegen, pretty), manual match is OK
- [ ] Verify exhaustive matching (no `_ => {}` catch-alls)

### Update CLAUDE.md

Add section:

```markdown
## AST Walking Convention

When adding a new pass that walks the AST:

- **Use `Visitor` or `VisitMut`** if >50% of match arms would be pure recursion
- **Use manual `match`** if >50% of arms have custom logic (like codegen, typeck core)
- **Never use `_ => {}` on AST enums** — either use exhaustive matching or the visitor trait

Core passes that use manual `match` blocks: `check_stmt`, `infer_expr`, `lower_stmt`, `lower_expr`, `emit_*` (pretty printer).

Visitor infrastructure is in `src/visit.rs`. Import with:
```rust
use crate::visit::{Visitor, walk_expr};
// or
use crate::visit::{VisitMut, walk_expr_mut};
```
```

---

## Timeline Summary

| Phase | Duration | PRs | Lines Removed | Lines Added | Net |
|-------|----------|-----|---------------|-------------|-----|
| Phase 0: Infrastructure | 1-2 days | 1 | 0 | 500 | +500 |
| Phase 1: Bug fixes | 2-3 days | 4 | 320 | 135 | **-185** |
| Phase 2: High recursion | 3-4 days | 6 | 680 | 160 | **-520** |
| Phase 3: Medium recursion | 4-5 days | 8 | 775 | 200 | **-575** |
| Phase 4: Documentation | 1 day | 1 | 0 | 100 | +100 |
| **Total** | **~2 weeks** | **20 PRs** | **1775** | **1095** | **-680** |

Final state:
- **58 walkers** reduced to **~30 walkers + visitor infrastructure**
- **4 bugs fixed**
- **~680 net lines removed**
- All future AST variants automatically handled by visitor infrastructure

---

## Risk Mitigation

### Testing Strategy

Each conversion PR must include:

1. **Existing test verification** — run full test suite, must be green
2. **New test for the bug** (Phase 1 only) — test the specific bug that was fixed
3. **Smoke test** — compile a non-trivial Pluto program (e.g., `examples/channels`)

### Rollback Plan

Each phase is independently revertible:
- Phase 0: Delete `src/visit.rs`, revert `src/lib.rs` change
- Phase 1-3: Each PR can be reverted individually without affecting others

### Code Review Checklist

For each conversion PR, verify:

- [ ] Old walker function(s) deleted
- [ ] New visitor implementation added
- [ ] No `_ => {}` patterns in visitor (unless intentional pruning with comment)
- [ ] All existing tests pass
- [ ] Conversion adds test for the specific bug (if Phase 1) or edge case
- [ ] Visitor state fields documented if non-obvious (e.g., `current_depth` for closures)
- [ ] Behavior preserved before optimization (check against old implementation)

---

## Open Questions for User

1. **Do you want strict policy that new non-core AST walkers MUST use Visitor/VisitMut?** Or is it a strong convention with opt-out for justified cases?

2. **Should we convert `substitute_in_*` (monomorphize.rs)?** These produce new AST nodes (fold pattern) rather than mutating in place. Options:
   - Keep as manual walkers (low maintenance burden, only 3 functions)
   - Add a `Fold` trait (more complexity)
   - Clone-then-mutate with `VisitMut` (adds clone cost)

3. **Timeline preference:** Should Phase 1 (bug fixes) be prioritized for immediate merge, with Phase 2-3 as follow-up work? Or proceed linearly?

4. **Scope-sensitive walker pattern:** Should we add examples to Phase 0 tests showing the recommended pattern for walkers that need to track scope depth or push/pop context? (e.g., `collect_free_vars`, `ambient` rewriting)

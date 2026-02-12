# Visitor Pattern Implementation Plan

**Status:** Implementation Ready
**Date:** 2026-02-12
**Companion to:** `rfc-visitor-pattern.md`

---

## Overview

This document provides detailed technical implementation steps for the visitor pattern RFC. Each phase is designed to be independently reviewable and testable.

---

## Phase 0: Infrastructure (1-2 days, ~300 LOC)

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
    // Block-level
    fn visit_program(&mut self, program: &Program) {
        walk_program(self, program);
    }

    fn visit_block(&mut self, block: &Spanned<Block>) {
        walk_block(self, block);
    }

    // Declaration-level
    fn visit_function(&mut self, func: &Function) {
        walk_function(self, func);
    }

    fn visit_class(&mut self, class: &ClassDecl) {
        walk_class(self, class);
    }

    fn visit_trait(&mut self, trait_decl: &TraitDecl) {
        walk_trait(self, trait_decl);
    }

    fn visit_enum(&mut self, enum_decl: &EnumDecl) {
        walk_enum(self, enum_decl);
    }

    fn visit_error(&mut self, error_decl: &ErrorDecl) {
        walk_error(self, error_decl);
    }

    fn visit_app(&mut self, app: &AppDecl) {
        walk_app(self, app);
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
    for func in &program.functions {
        v.visit_function(func);
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
    for error_decl in &program.errors {
        v.visit_error(error_decl);
    }
    if let Some(app) = &program.app {
        v.visit_app(app);
    }
}

pub fn walk_function<V: Visitor>(v: &mut V, func: &Function) {
    // Visit param types
    for param in &func.params {
        v.visit_type_expr(&param.ty);
    }

    // Visit return type
    if let Some(rt) = &func.return_type {
        v.visit_type_expr(rt);
    }

    // Visit body
    if let Some(body) = &func.body {
        v.visit_block(body);
    }

    // Visit contracts
    for contract in &func.contracts {
        v.visit_expr(&contract.node.expr);
    }
}

pub fn walk_class<V: Visitor>(v: &mut V, class: &ClassDecl) {
    // Visit bracket deps (injected fields)
    for (_, ty) in &class.bracket_deps {
        v.visit_type_expr(ty);
    }

    // Visit field types
    for field in &class.fields {
        v.visit_type_expr(&field.ty);
    }

    // Visit methods
    for method in &class.methods {
        v.visit_function(method);
    }

    // Visit invariants
    for invariant in &class.invariants {
        v.visit_expr(&invariant.node.expr);
    }
}

pub fn walk_trait<V: Visitor>(v: &mut V, trait_decl: &TraitDecl) {
    for method in &trait_decl.methods {
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

pub fn walk_enum<V: Visitor>(v: &mut V, enum_decl: &EnumDecl) {
    for variant in &enum_decl.variants {
        for field in &variant.fields {
            v.visit_type_expr(&field.ty);
        }
    }
}

pub fn walk_error<V: Visitor>(v: &mut V, error_decl: &ErrorDecl) {
    for field in &error_decl.fields {
        v.visit_type_expr(&field.ty);
    }
}

pub fn walk_app<V: Visitor>(v: &mut V, app: &AppDecl) {
    // Visit bracket deps
    for (_, ty) in &app.bracket_deps {
        v.visit_type_expr(ty);
    }

    // Visit methods
    for method in &app.methods {
        v.visit_function(method);
    }
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
        Expr::QualifiedAccess { object, .. } => v.visit_expr(object),

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
    // Block-level
    fn visit_program_mut(&mut self, program: &mut Program) {
        walk_program_mut(self, program);
    }

    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        walk_block_mut(self, block);
    }

    // Declaration-level
    fn visit_function_mut(&mut self, func: &mut Function) {
        walk_function_mut(self, func);
    }

    fn visit_class_mut(&mut self, class: &mut ClassDecl) {
        walk_class_mut(self, class);
    }

    fn visit_trait_mut(&mut self, trait_decl: &mut TraitDecl) {
        walk_trait_mut(self, trait_decl);
    }

    fn visit_enum_mut(&mut self, enum_decl: &mut EnumDecl) {
        walk_enum_mut(self, enum_decl);
    }

    fn visit_error_mut(&mut self, error_decl: &mut ErrorDecl) {
        walk_error_mut(self, error_decl);
    }

    fn visit_app_mut(&mut self, app: &mut AppDecl) {
        walk_app_mut(self, app);
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
// (Implementation follows same pattern as above, with &mut references)
// [Abbreviated here for brevity — full implementation would be ~200 more lines]

pub fn walk_program_mut<V: VisitMut>(v: &mut V, program: &mut Program) {
    // Same structure as walk_program, but &mut references
    for func in &mut program.functions {
        v.visit_function_mut(func);
    }
    // ... etc
}

// ... (remaining walk_*_mut functions)
```

**Testing:**
```rust
// tests/unit/visit.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::*;

    #[test]
    fn test_visitor_visits_all_exprs() {
        // Construct a minimal AST with one of each Expr variant
        let ast = /* ... */;

        struct CountingVisitor {
            count: usize,
        }

        impl Visitor for CountingVisitor {
            fn visit_expr(&mut self, expr: &Spanned<Expr>) {
                self.count += 1;
                walk_expr(self, expr);
            }
        }

        let mut visitor = CountingVisitor { count: 0 };
        visitor.visit_block(&ast);

        // Verify all exprs were visited
        assert_eq!(visitor.count, 29); // One per Expr variant
    }

    #[test]
    fn test_visitor_can_prune_traversal() {
        // Verify that not calling walk_* stops recursion
        struct PruningVisitor {
            visited_after_spawn: bool,
        }

        impl Visitor for PruningVisitor {
            fn visit_expr(&mut self, expr: &Spanned<Expr>) {
                if matches!(expr.node, Expr::Spawn { .. }) {
                    // Don't call walk_expr — prune here
                    return;
                }
                walk_expr(self, expr);
            }

            fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
                // If we get here after seeing a Spawn, the pruning failed
                if matches!(stmt.node, Stmt::Return(_)) {
                    self.visited_after_spawn = true;
                }
                walk_stmt(self, stmt);
            }
        }

        // AST: spawn func() where func has "return 42"
        let ast = /* ... */;
        let mut visitor = PruningVisitor { visited_after_spawn: false };
        visitor.visit_block(&ast);

        assert!(!visitor.visited_after_spawn, "Pruning failed — visited inside spawn");
    }

    #[test]
    fn test_visit_mut_can_rewrite() {
        struct RewriteIdentVisitor;

        impl VisitMut for RewriteIdentVisitor {
            fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
                if let Expr::Ident(name) = &mut expr.node {
                    if name == "old" {
                        *name = "new".to_string();
                    }
                }
                walk_expr_mut(self, expr);
            }
        }

        let mut ast = /* Expr::Ident("old") */;
        let mut visitor = RewriteIdentVisitor;
        visitor.visit_expr_mut(&mut ast);

        assert_eq!(ast.node, Expr::Ident("new".to_string()));
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
cargo test --lib visit
```

Expected: All 3 unit tests pass.

### Deliverable

- `src/visit.rs` with `Visitor` and `VisitMut` traits
- All walk functions with exhaustive matches (no `_ => {}`)
- 3 unit tests demonstrating basic visitor behavior
- PR with title "Phase 0: Add visitor pattern infrastructure"

### Success Criteria

- Compiles without warnings
- All unit tests pass
- Zero changes to existing compiler code
- walk_* functions use `#[deny(unreachable_patterns)]` (Rust's exhaustiveness checking)

---

## Phase 1: Bug-Fixing Conversions (2-3 days, 4 PRs)

### Goal
Convert the 4 walkers with known bugs to visitor implementations. Each conversion is its own PR.

### PR 1.1: Fix monomorphize.rs::resolve_generic_te_in_expr

**Current bug:** Catch-all at line 1487 skips `MapLit`, `SetLit`, `StaticTraitCall`, `QualifiedAccess`.

**Implementation:**

```rust
// src/monomorphize.rs

use crate::visit::{Visitor, walk_expr};

struct GenericTypeResolver<'a> {
    env: &'a mut TypeEnv,
}

impl Visitor for GenericTypeResolver<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // No custom pre-visit logic needed — just recurse
        walk_expr(self, expr);
    }

    fn visit_type_expr(&mut self, te: &Spanned<TypeExpr>) {
        // This is where the work happens
        resolve_generic_te(&mut te.node, self.env).unwrap();
        walk_type_expr(self, te);
    }
}

// Replace resolve_generic_te_in_expr with:
fn resolve_generic_te_in_expr(expr: &mut Expr, env: &mut TypeEnv) -> Result<(), CompileError> {
    let mut visitor = GenericTypeResolver { env };
    visitor.visit_expr(&Spanned { node: expr.clone(), span: Span::default() });
    // Note: This requires making expr mutable, or using VisitMut
    Ok(())
}
```

**Wait, problem:** The current function takes `&mut Expr` and mutates in place. Visitor takes `&Expr` (immutable). Need to use `VisitMut`:

```rust
struct GenericTypeResolverMut<'a> {
    env: &'a mut TypeEnv,
}

impl VisitMut for GenericTypeResolverMut<'_> {
    fn visit_type_expr_mut(&mut self, te: &mut Spanned<TypeExpr>) {
        resolve_generic_te(&mut te.node, self.env).unwrap();
        walk_type_expr_mut(self, te);
    }
}

fn resolve_generic_te_in_expr(expr: &mut Expr, env: &mut TypeEnv) -> Result<(), CompileError> {
    let mut spanned_expr = Spanned { node: expr.clone(), span: Span::default() };
    let mut visitor = GenericTypeResolverMut { env };
    visitor.visit_expr_mut(&mut spanned_expr);
    *expr = spanned_expr.node;
    Ok(())
}
```

**Actually, simpler:** Just call the visitor from the top-level function that already has a Spanned<Expr>:

```rust
// In resolve_generic_instances_in_body (the caller)
fn resolve_generic_instances_in_body(body: &mut Spanned<Block>, env: &mut TypeEnv) -> Result<(), CompileError> {
    let mut visitor = GenericTypeResolverMut { env };
    visitor.visit_block_mut(body);
    Ok(())
}
```

Then delete the old `resolve_generic_te_in_expr` and `resolve_generic_te_in_stmt` functions entirely.

**Testing:**
- Add a test case: `Map<T, int> {}` inside a generic function body
- Add a test case: `StaticTraitCall` with `type_args` inside a generic function
- Verify both compile without errors

**Lines changed:** -80 (delete old walker), +15 (new visitor impl) = **-65 net**

**Deliverable:** PR with title "Fix generic type resolution for MapLit/SetLit/StaticTraitCall"

---

### PR 1.2: Fix typeck/errors.rs::contains_propagate

**Current bug:** Catch-all misses `StaticTraitCall` args.

**Implementation:**

```rust
// src/typeck/errors.rs

use crate::visit::{Visitor, walk_expr};

struct PropagateDetector {
    found: bool,
}

impl Visitor for PropagateDetector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if matches!(expr.node, Expr::Propagate { .. }) {
            self.found = true;
            // No need to recurse further once found
            return;
        }
        walk_expr(self, expr);
    }
}

fn contains_propagate(expr: &Expr) -> bool {
    let spanned_expr = Spanned { node: expr.clone(), span: Span::default() };
    let mut detector = PropagateDetector { found: false };
    detector.visit_expr(&spanned_expr);
    detector.found
}
```

Delete the old `contains_propagate` function (28 lines) and the now-unused `stmt_contains_propagate` (17 lines).

**Testing:**
- Add test: `StaticTraitCall { args: [some_call()!] }` should return `true`
- Verify existing tests still pass

**Lines changed:** -45, +20 = **-25 net**

**Deliverable:** PR with title "Fix propagate detection for StaticTraitCall"

---

### PR 1.3: Fix derived.rs::collect_deps_from_expr

**Current bug:** Doesn't collect `StaticTraitCall` trait method references.

**Implementation:**

```rust
// src/derived.rs

use crate::visit::{Visitor, walk_expr};

struct DependencyCollector<'a> {
    program: &'a Program,
    visited: &'a mut HashSet<String>,
    deps: &'a mut Vec<String>,
}

impl Visitor for DependencyCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Call { name, .. } => {
                if !self.visited.contains(&name.node) {
                    self.visited.insert(name.node.clone());
                    self.deps.push(name.node.clone());
                }
            }
            Expr::StaticTraitCall { trait_name, method_name, .. } => {
                // Collect the trait method as a dependency
                let dep_name = format!("{}::{}", trait_name.node, method_name.node);
                if !self.visited.contains(&dep_name) {
                    self.visited.insert(dep_name.clone());
                    self.deps.push(dep_name);
                }
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}
```

**Testing:**
- Add test: test that calls a static trait method should have it as a dependency

**Lines changed:** -120 (delete old collect_deps_from_block + collect_deps_from_expr), +30 = **-90 net**

**Deliverable:** PR with title "Fix test dependency tracking for StaticTraitCall"

---

### PR 1.4: Fix typeck/check.rs::check_stmt_for_self_mutation

**Current bug:** Misses `IndexAssign` (e.g., `self.array[i] = x`).

**Implementation:**

```rust
// src/typeck/check.rs

use crate::visit::{Visitor, walk_stmt, walk_expr};

struct SelfMutationChecker<'a> {
    class_name: &'a str,
    env: &'a TypeEnv,
    found_mutation: bool,
}

impl Visitor for SelfMutationChecker<'_> {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        match &stmt.node {
            Stmt::FieldAssign { object, .. } => {
                if let Expr::Ident(name) = &object.node {
                    if name == "self" {
                        self.found_mutation = true;
                        return; // No need to recurse
                    }
                }
            }
            Stmt::IndexAssign { object, .. } => {
                // NEW: Handle self.array[i] = x
                if let Expr::FieldAccess { object: inner, .. } = &object.node {
                    if let Expr::Ident(name) = &inner.node {
                        if name == "self" {
                            self.found_mutation = true;
                            return;
                        }
                    }
                }
            }
            // Don't recurse into closures — they capture self by value
            Stmt::Expr(expr) if matches!(expr.node, Expr::Closure { .. }) => return,
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}

fn check_stmt_for_self_mutation(stmt: &Spanned<Stmt>, class_name: &str, env: &TypeEnv) -> bool {
    let mut checker = SelfMutationChecker { class_name, env, found_mutation: false };
    checker.visit_stmt(stmt);
    checker.found_mutation
}
```

Delete old `check_stmt_for_self_mutation` and `check_expr_for_self_mutation`.

**Testing:**
- Add test: method with `self.items[0] = x` should fail on non-mut receiver

**Lines changed:** -135 (delete 2 old functions), +40 = **-95 net**

**Deliverable:** PR with title "Fix self-mutation detection for IndexAssign"

---

### Phase 1 Summary

- 4 PRs, each fixing one bug
- Total lines removed: ~275
- Total lines added: ~105
- **Net reduction: -170 lines**
- All 4 bugs fixed
- Pattern established for future conversions

---

## Phase 2: High-Recursion Walkers (3-4 days, 7 PRs)

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

Delete the 3 old functions.

**Lines:** -200, +25 = **-175 net**

---

### PR 2.2: Convert collect_spawn_closure_names (codegen/mod.rs)

**Current:** 3 functions, ~140 lines, 90% pure recursion.

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

        // Then desugar this node if it's a Spawn
        if let Expr::Spawn { call } = &expr.node {
            // Transform into Closure wrapper
            // ... (same desugaring logic as before)
        }
    }
}

pub fn desugar_spawn(program: &mut Program) {
    let mut desugarer = SpawnDesugarer;
    desugarer.visit_program_mut(program);
}
```

**Lines:** -100, +20 = **-80 net**

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
    param_names: &'a HashSet<String>,
    outer_depth: usize,
    env: &'a TypeEnv,
    captures: &'a mut HashMap<String, PlutoType>,
    seen: &'a mut HashSet<String>,
    current_depth: usize, // Track depth as we descend
}

impl Visitor for FreeVarCollector<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Ident(name) => {
                // Check if it's a free variable
                if !self.param_names.contains(name) && self.current_depth > self.outer_depth {
                    // ... capture logic
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

**Lines:** -80, +40 = **-40 net**

---

### Phase 2 Summary

- 7 PRs
- Total reduction: ~535 lines
- Establishes pattern for scope-sensitive walkers

---

## Phase 3: Medium-Recursion Walkers (4-5 days, ~10 PRs)

### Goal
Convert walkers with 60-85% pure recursion.

### Walkers to convert:

1. `rewrite_*_for_module` (modules.rs) — 2 funcs, ~150 lines → ~50 lines (**-100**)
2. `rewrite_*` for module qualified access (modules.rs) — 2 funcs, ~120 lines → ~40 lines (**-80**)
3. `rewrite_*` in monomorphize.rs — 2 funcs, ~120 lines → ~40 lines (**-80**)
4. `resolve_*` in xref.rs — 2 funcs, ~100 lines → ~35 lines (**-65**)
5. `collect_*_accesses` (concurrency.rs) — 2 funcs, ~130 lines → ~50 lines (**-80**)
6. `lift_in_*` (closures.rs) — 2 funcs, ~150 lines → ~50 lines (**-100**)
7. Narrow-purpose codegen utilities (3 funcs, ~120 lines → ~40 lines, **-80**)

**Special note on lift_in_*:** This transforms the AST (Closure → ClosureCreate). Requires careful ordering:

```rust
struct ClosureLifter<'a> {
    env: &'a mut TypeEnv,
    closure_counter: &'a mut usize,
    lifted_funcs: &'a mut Vec<Function>,
}

impl VisitMut for ClosureLifter<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Spanned<Expr>) {
        // First recurse to lift nested closures
        walk_expr_mut(self, expr);

        // Then lift this closure (bottom-up)
        if let Expr::Closure { params, return_type, body } = &expr.node {
            // Generate lifted function
            let fn_name = format!("__closure_{}", self.closure_counter);
            *self.closure_counter += 1;

            // Create ClosureCreate to replace this Closure
            expr.node = Expr::ClosureCreate {
                fn_name,
                captures: /* ... */,
                target_id: None,
            };

            // Add lifted function
            self.lifted_funcs.push(/* ... */);
        }
    }
}
```

### Phase 3 Summary

- ~10 PRs
- Total reduction: ~585 lines
- Covers all "medium complexity" walkers

---

## Phase 4: Evaluate Remaining Walkers (1 day)

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

Add a section to `docs/design/visitor-implementation-plan.md` documenting these walkers and the decision to keep them manual. Include:

- Percentage breakdown of custom vs structural logic
- Specific examples of why the visitor would add indirection without reducing complexity
- Verification that they use exhaustive matching (no `_ => {}`)

---

## Enforcement (Ongoing)

### Step E.1: Add Clippy lint configuration

Create `.clippy.toml`:
```toml
# Deny wildcard matches on AST enums
disallowed-types = [
    { path = "crate::parser::ast::Expr", reason = "Use exhaustive match or Visitor trait" },
    { path = "crate::parser::ast::Stmt", reason = "Use exhaustive match or Visitor trait" },
]
```

### Step E.2: Add CI check

In `.github/workflows/ci.yml`:
```yaml
- name: Check for wildcard AST matches
  run: |
    # Fail if any file has "match expr { ... _ => {} }" on AST enums
    # except for allowed files (codegen, typeck core, pretty printer)
    if git grep -n "_ =>" src/ | grep -v "src/codegen/lower" | grep -v "src/typeck/check.rs" | grep -v "src/typeck/infer.rs" | grep -v "src/pretty.rs"; then
      echo "Error: Found wildcard match on AST enum outside core passes"
      exit 1
    fi
```

### Step E.3: Update CLAUDE.md

Add section:
```markdown
## AST Walking Convention

When adding a new pass that walks the AST:

- **Use `Visitor` or `VisitMut`** if >50% of match arms would be pure recursion
- **Use manual `match`** if >50% of arms have custom logic (like codegen, typeck core)
- **Never use `_ => {}` on AST enums** — either use exhaustive matching or the visitor trait

Core passes that use manual `match` blocks: `check_stmt`, `infer_expr`, `lower_stmt`, `lower_expr`, `emit_*` (pretty printer).
```

---

## Timeline Summary

| Phase | Duration | PRs | Lines Removed | Lines Added | Net |
|-------|----------|-----|---------------|-------------|-----|
| Phase 0: Infrastructure | 1-2 days | 1 | 0 | 300 | +300 |
| Phase 1: Bug fixes | 2-3 days | 4 | 275 | 105 | **-170** |
| Phase 2: High recursion | 3-4 days | 7 | 735 | 200 | **-535** |
| Phase 3: Medium recursion | 4-5 days | 10 | 785 | 200 | **-585** |
| Phase 4: Documentation | 1 day | 1 | 0 | 50 | +50 |
| **Total** | **~2 weeks** | **23 PRs** | **1795** | **855** | **-940** |

Final state:
- **58 walkers** reduced to **~24 walkers + visitor infrastructure**
- **4 bugs fixed**
- **~940 net lines removed**
- All future AST variants automatically handled by visitor

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

---

## Open Questions for User

1. **Do you want strict policy that new non-core AST walkers MUST use Visitor/VisitMut?** Or is it a strong convention with opt-out for justified cases?

2. **Should we convert `substitute_in_*` (monomorphize.rs)?** These produce new AST nodes (fold pattern) rather than mutating in place. Options:
   - Keep as manual walkers (low maintenance burden, only 3 functions)
   - Add a `Fold` trait (more complexity)
   - Clone-then-mutate with `VisitMut` (adds clone cost)

3. **Enforcement timing:** Should the CI check for `_ => {}` be added in Phase 0 (strict from the start) or Phase 4 (after all conversions are done)?

4. **Scope-sensitive walker pattern:** Should we add a section to the RFC showing the recommended pattern for walkers that need to track scope depth or push/pop context? (e.g., `collect_free_vars`, `ambient` rewriting)

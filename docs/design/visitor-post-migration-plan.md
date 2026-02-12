# Visitor Pattern Post-Migration Plan

**Date:** 2026-02-12
**Status:** Active roadmap
**Related:** `rfc-visitor-pattern.md`, `visitor-phase4-assessment.md`

---

## Executive Summary

The visitor pattern migration (Phases 0-4) successfully converted 24 walker functions and established clear patterns for AST traversal. This document outlines **12 follow-up initiatives** to strengthen the infrastructure, improve extensibility, and prepare for future compiler features.

**Goals:**
1. **Robustness** — Comprehensive testing, CI enforcement, bug fixes
2. **Extensibility** — Utilities and patterns for common visitor use cases
3. **Documentation** — Clear examples and guidelines for contributors
4. **Future-proofing** — Infrastructure for upcoming features (RPC, tooling, static analysis)

---

## 1. Fix Catch-All Patterns in Error System

**Priority:** HIGH (bug prevention)
**Effort:** 2-3 hours
**Owner:** Next available contributor

### Problem
Four walkers in `src/typeck/errors.rs` use `_ => {}` catch-all patterns:
- `collect_raise_effects` (line 389)
- `collect_call_effects` (line 587)
- `enforce_call_handling` (line 764)
- `enforce_stmt_handling` (line 672)

These catch-all patterns are the root cause of Bugs 1-4 identified in the RFC. When new AST variants are added, these walkers silently skip them instead of forcing explicit handling.

### Solution
Convert catch-all patterns to exhaustive matching:

```rust
// Before
_ => {}

// After
Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_)
| Expr::StringLit(_) | Expr::NoneLit | Expr::Ident(_)
| Expr::ClosureCreate { .. } | Expr::EnumUnit { .. } => {}
```

This provides compile-time exhaustiveness checking without requiring visitor conversion.

### Verification
- Compile with new AST variant added (e.g., `Expr::AwaitExpr { .. }`) — should produce compiler errors in all 4 walkers
- Run `cargo test` — all tests should still pass
- Check `git diff` — only catch-all patterns changed, no logic changes

### Deliverable
Single PR: "Fix catch-all patterns in error system walkers"

---

## 2. Add CI Enforcement for Catch-All Patterns

**Priority:** MEDIUM (long-term prevention)
**Effort:** 3-4 hours
**Owner:** Next available contributor

### Problem
Without enforcement, developers may introduce new catch-all patterns in walker functions, reintroducing the bug class we just eliminated.

### Solution
Add a CI check that rejects new catch-all patterns on AST enum matches.

#### Option A: Simple grep-based check (recommended for MVP)

```yaml
# .github/workflows/lint.yml
name: AST Walker Lint

on: [pull_request]

jobs:
  check-catchall:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Check for catch-all patterns in AST walkers
        run: |
          # Find lines that match on AST enums and use catch-all
          VIOLATIONS=$(rg 'match.*&?.*\.node.*\{' src/ --type rust -A 30 | \
                       rg '^\s*_ => \{\}' --files-with-matches || true)

          if [ -n "$VIOLATIONS" ]; then
            echo "ERROR: Found catch-all patterns in AST walkers:"
            echo "$VIOLATIONS"
            echo ""
            echo "Use exhaustive matching or the visitor pattern instead."
            echo "See docs/design/visitor-phase4-assessment.md for guidance."
            exit 1
          fi
```

**Pros:** Simple, fast, no dependencies
**Cons:** Heuristic-based (false positives possible on non-AST matches)

#### Option B: AST-based check using syn (more precise)

Create `scripts/check_ast_walkers.rs`:
```rust
// Parse Rust files with syn, find match expressions on Expr/Stmt/TypeExpr,
// check for _ => patterns, report violations
```

**Pros:** Precise, no false positives
**Cons:** Requires proc-macro infrastructure, slower CI

#### Recommendation
Start with Option A (grep-based). If false positives become a problem, upgrade to Option B.

### Allowlist for Legitimate Catch-Alls
Some non-walker code may legitimately use catch-all patterns on AST enums (e.g., fallback formatting in error messages). Add an allowlist:

```rust
// check_ast_walkers: allow
match expr.node {
    Expr::Call { .. } => format!("in call"),
    _ => format!("in expression"), // Legitimate: just for error messages
}
```

The CI script can skip lines with `// check_ast_walkers: allow` comments.

### Deliverable
PR: "Add CI enforcement for AST walker catch-all patterns"

---

## 3. Comprehensive Unit Tests for Visitor Infrastructure

**Priority:** HIGH (correctness guarantee)
**Effort:** 6-8 hours
**Owner:** Next available contributor

### Problem
The `walk_*` functions in `src/visit.rs` are the single source of truth for structural recursion. If they have bugs (e.g., forgetting to visit a child node), all visitor-based passes inherit the bug.

Currently, there are **no unit tests** for the visitor infrastructure itself.

### Solution
Add unit tests that verify `walk_*` functions visit every child node.

#### 3.1 Test: walk_expr visits all child expressions

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    struct ExprCollector {
        visited: HashSet<String>,
    }

    impl Visitor for ExprCollector {
        fn visit_expr(&mut self, expr: &Spanned<Expr>) {
            // Record this expression
            self.visited.insert(format!("{:?}", expr.node));
            // Continue walking
            walk_expr(self, expr);
        }
    }

    #[test]
    fn test_walk_expr_visits_binop_children() {
        let lhs = Spanned::dummy(Expr::IntLit(1));
        let rhs = Spanned::dummy(Expr::IntLit(2));
        let binop = Spanned::dummy(Expr::BinOp {
            op: Spanned::dummy(BinOp::Add),
            lhs: Box::new(lhs.clone()),
            rhs: Box::new(rhs.clone()),
        });

        let mut collector = ExprCollector { visited: HashSet::new() };
        collector.visit_expr(&binop);

        // Should visit BinOp, lhs, and rhs
        assert_eq!(collector.visited.len(), 3);
        assert!(collector.visited.contains("IntLit(1)"));
        assert!(collector.visited.contains("IntLit(2)"));
    }

    #[test]
    fn test_walk_expr_visits_call_args() {
        let arg1 = Spanned::dummy(Expr::IntLit(10));
        let arg2 = Spanned::dummy(Expr::IntLit(20));
        let call = Spanned::dummy(Expr::Call {
            name: Spanned::dummy("func".to_string()),
            type_args: vec![],
            args: vec![arg1.clone(), arg2.clone()],
            target_id: None,
        });

        let mut collector = ExprCollector { visited: HashSet::new() };
        collector.visit_expr(&call);

        // Should visit Call, arg1, arg2
        assert_eq!(collector.visited.len(), 3);
    }

    #[test]
    fn test_walk_expr_visits_nested_structures() {
        // Build: StructLit { field: BinOp { lhs: IntLit(1), rhs: IntLit(2) } }
        let lhs = Spanned::dummy(Expr::IntLit(1));
        let rhs = Spanned::dummy(Expr::IntLit(2));
        let binop = Spanned::dummy(Expr::BinOp {
            op: Spanned::dummy(BinOp::Add),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
        let struct_lit = Spanned::dummy(Expr::StructLit {
            name: Spanned::dummy("Foo".to_string()),
            type_args: vec![],
            fields: vec![("value".to_string(), binop)],
            target_id: None,
        });

        let mut collector = ExprCollector { visited: HashSet::new() };
        collector.visit_expr(&struct_lit);

        // Should visit StructLit, BinOp, IntLit(1), IntLit(2)
        assert_eq!(collector.visited.len(), 4);
    }

    // Add similar tests for all Expr variants with children:
    // - MethodCall
    // - Index
    // - ArrayLit
    // - MapLit
    // - SetLit
    // - Closure
    // - Match (via Catch)
    // - StringInterp
    // - Cast
    // - Spawn
    // - Propagate
    // - NullPropagate
    // - EnumData
}
```

#### 3.2 Test: walk_stmt visits all child statements and expressions

```rust
#[test]
fn test_walk_stmt_visits_if_branches() {
    let cond = Spanned::dummy(Expr::BoolLit(true));
    let then_stmt = Spanned::dummy(Stmt::Return(Some(Spanned::dummy(Expr::IntLit(1)))));
    let else_stmt = Spanned::dummy(Stmt::Return(Some(Spanned::dummy(Expr::IntLit(2)))));

    let if_stmt = Spanned::dummy(Stmt::If {
        condition: cond,
        then_block: Spanned::dummy(Block { stmts: vec![then_stmt] }),
        else_block: Some(Spanned::dummy(Block { stmts: vec![else_stmt] })),
    });

    let mut collector = ExprCollector { visited: HashSet::new() };
    collector.visit_stmt(&if_stmt);

    // Should visit condition, then return value, else return value
    assert_eq!(collector.visited.len(), 3);
}

// Add tests for all Stmt variants with children
```

#### 3.3 Test: walk_type_expr visits all child type expressions

```rust
#[test]
fn test_walk_type_expr_visits_generic_args() {
    let arg1 = Spanned::dummy(TypeExpr::Named("int".to_string()));
    let arg2 = Spanned::dummy(TypeExpr::Named("string".to_string()));
    let generic = Spanned::dummy(TypeExpr::Generic {
        name: "Map".to_string(),
        type_args: vec![arg1, arg2],
    });

    let mut collector = TypeExprCollector { visited: HashSet::new() };
    collector.visit_type_expr(&generic);

    // Should visit Generic, Named("int"), Named("string")
    assert_eq!(collector.visited.len(), 3);
}
```

#### 3.4 Property-based tests (optional, high value)

Use `proptest` to generate random AST fragments and verify invariants:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn walk_expr_visits_all_nodes(expr in arb_expr()) {
        let mut counter = CountingVisitor { count: 0 };
        counter.visit_expr(&expr);

        // Property: visitor should be called at least once (for the root)
        assert!(counter.count > 0);

        // Property: visitor count should equal manual node count
        let manual_count = count_expr_nodes(&expr);
        assert_eq!(counter.count, manual_count);
    }
}

fn arb_expr() -> impl Strategy<Value = Spanned<Expr>> {
    // Generate random expressions with bounded depth
    // (needs custom strategy builder)
}
```

### Deliverable
PR: "Add unit tests for visitor infrastructure" (~200-300 lines of tests)

---

## 4. Visitor Documentation and Examples

**Priority:** MEDIUM (contributor onboarding)
**Effort:** 4-5 hours
**Owner:** Next available contributor

### Problem
`src/visit.rs` currently has minimal documentation. Contributors need clear examples of:
- How to implement a visitor
- Common patterns (scope tracking, error collection)
- When to use visitor vs. manual match
- How to handle special cases (spawn opacity, non-standard traversal)

### Solution
Add comprehensive module-level documentation to `src/visit.rs`.

#### 4.1 Module header documentation

```rust
//! AST visitor pattern for traversing and transforming the Pluto AST.
//!
//! This module provides two visitor traits and their corresponding walk functions:
//! - [`Visitor`] — read-only traversal for analysis and collection passes
//! - [`VisitMut`] — mutable traversal for in-place AST transformation
//!
//! # When to use visitors
//!
//! Use the visitor pattern when:
//! - >50% of your walker's match arms are pure structural recursion
//! - You're implementing an analysis/collection pass (use [`Visitor`])
//! - You're implementing a rewriting pass (use [`VisitMut`])
//!
//! Use manual `match` blocks when:
//! - >50% of each arm is domain-specific logic (typeck, codegen, pretty printing)
//! - You need very fine-grained control over traversal order
//! - See `docs/design/visitor-phase4-assessment.md` for detailed guidance
//!
//! # Quick Start Example
//!
//! ```rust
//! use crate::visit::{Visitor, walk_expr};
//! use crate::parser::ast::{Expr, Stmt};
//! use crate::span::Spanned;
//! use std::collections::HashSet;
//!
//! // Collect all variable names used in expressions
//! struct VariableCollector {
//!     vars: HashSet<String>,
//! }
//!
//! impl Visitor for VariableCollector {
//!     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
//!         if let Expr::Ident(name) = &expr.node {
//!             self.vars.insert(name.clone());
//!         }
//!         // Continue walking into child expressions
//!         walk_expr(self, expr);
//!     }
//! }
//!
//! // Usage:
//! let mut collector = VariableCollector { vars: HashSet::new() };
//! collector.visit_block(&function_body);
//! println!("Variables used: {:?}", collector.vars);
//! ```
//!
//! # Common Patterns
//!
//! ## Pattern 1: Scope tracking
//!
//! Many passes need to track scope information (which variables are in scope,
//! nesting depth, etc.). Use a stack in your visitor state:
//!
//! ```rust
//! struct ScopeTracker {
//!     scopes: Vec<HashSet<String>>, // Stack of scopes
//! }
//!
//! impl Visitor for ScopeTracker {
//!     fn visit_block(&mut self, block: &Spanned<Block>) {
//!         // Enter new scope
//!         self.scopes.push(HashSet::new());
//!
//!         // Walk children
//!         walk_block(self, block);
//!
//!         // Exit scope
//!         self.scopes.pop();
//!     }
//!
//!     fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
//!         if let Stmt::Let { name, .. } = &stmt.node {
//!             // Add variable to current scope
//!             if let Some(scope) = self.scopes.last_mut() {
//!                 scope.insert(name.node.clone());
//!             }
//!         }
//!         walk_stmt(self, stmt);
//!     }
//! }
//! ```
//!
//! ## Pattern 2: Error collection
//!
//! Collect multiple errors without stopping traversal:
//!
//! ```rust
//! struct ErrorCollector {
//!     errors: Vec<CompileError>,
//! }
//!
//! impl Visitor for ErrorCollector {
//!     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
//!         // Validate and collect errors
//!         if let Expr::Cast { expr: inner, target_type } = &expr.node {
//!             if !is_valid_cast(inner, target_type) {
//!                 self.errors.push(CompileError::InvalidCast {
//!                     span: expr.span,
//!                     // ...
//!                 });
//!             }
//!         }
//!         walk_expr(self, expr);
//!     }
//! }
//! ```
//!
//! ## Pattern 3: Counting/statistics
//!
//! ```rust
//! struct ComplexityCounter {
//!     call_depth: usize,
//!     max_depth: usize,
//! }
//!
//! impl Visitor for ComplexityCounter {
//!     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
//!         if matches!(expr.node, Expr::Call { .. } | Expr::MethodCall { .. }) {
//!             self.call_depth += 1;
//!             self.max_depth = self.max_depth.max(self.call_depth);
//!         }
//!
//!         walk_expr(self, expr);
//!
//!         if matches!(expr.node, Expr::Call { .. } | Expr::MethodCall { .. }) {
//!             self.call_depth -= 1;
//!         }
//!     }
//! }
//! ```
//!
//! ## Pattern 4: Non-standard traversal (pruning)
//!
//! Some passes need to skip certain subtrees. Simply don't call the walk function:
//!
//! ```rust
//! impl Visitor for SpawnOpaqueAnalysis {
//!     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
//!         match &expr.node {
//!             Expr::Spawn { call } => {
//!                 // Visit spawn args but NOT the closure body
//!                 if let Expr::Closure { params, .. } = &call.node {
//!                     // Only visit param types, skip body
//!                     for p in params {
//!                         self.visit_type_expr(&p.ty);
//!                     }
//!                 }
//!                 // Don't call walk_expr — pruned!
//!                 return;
//!             }
//!             _ => {}
//!         }
//!         walk_expr(self, expr);
//!     }
//! }
//! ```
//!
//! ## Pattern 5: Two-pass analysis
//!
//! Collect information in one pass, then transform based on it:
//!
//! ```rust
//! // Pass 1: Collect
//! let mut collector = InfoCollector::new();
//! collector.visit_program(&program);
//!
//! // Pass 2: Transform using collected info
//! let mut rewriter = Rewriter::new(collector.info);
//! rewriter.visit_program_mut(&mut program);
//! ```
//!
//! # Special Cases
//!
//! ## Spawn closures
//!
//! Spawn closures have special semantics in several passes:
//! - Error analysis: errors don't propagate across spawn boundaries
//! - Concurrency analysis: spawn closure bodies are opaque to caller effects
//! - Closure lifting: spawn closures are already desugared
//!
//! If your pass needs spawn-aware traversal, override `visit_expr` and handle
//! `Expr::Spawn` specially (see Pattern 4 above).
//!
//! ## String interpolation
//!
//! `Expr::StringInterp` contains `StringInterpPart` (not `Expr`), so it needs
//! manual iteration:
//!
//! ```rust
//! Expr::StringInterp { parts } => {
//!     for part in parts {
//!         if let StringInterpPart::Expr(e) = part {
//!             self.visit_expr(e);
//!         }
//!     }
//! }
//! ```
//!
//! The default `walk_expr` handles this, but if you override `visit_expr` and
//! don't call `walk_expr`, you'll need to handle it manually.
//!
//! ## QualifiedAccess
//!
//! `Expr::QualifiedAccess` should never appear after module flattening. If you
//! encounter it, panic:
//!
//! ```rust
//! Expr::QualifiedAccess { segments } => {
//!     panic!(
//!         "QualifiedAccess should be resolved by module flattening. Segments: {:?}",
//!         segments
//!     )
//! }
//! ```
//!
//! # Performance
//!
//! The visitor pattern uses trait method calls, but Rust's monomorphization
//! optimizes these away at compile time. There is **no runtime overhead**
//! compared to hand-written match blocks — the generated code is identical.
//!
//! # See Also
//!
//! - `docs/design/rfc-visitor-pattern.md` — Full RFC and rationale
//! - `docs/design/visitor-phase4-assessment.md` — Which walkers use which pattern
//! - `CLAUDE.md` (AST Walker Policy section) — Quick reference for contributors
```

#### 4.2 Inline documentation on walk functions

```rust
/// Recursively visit all child expressions in an expression.
///
/// This is the default implementation of [`Visitor::visit_expr`]. Call this
/// function inside your custom `visit_expr` override to continue the default
/// traversal after your custom logic.
///
/// # Example
///
/// ```rust
/// impl Visitor for MyVisitor {
///     fn visit_expr(&mut self, expr: &Spanned<Expr>) {
///         // Custom logic first
///         if let Expr::Call { name, .. } = &expr.node {
///             println!("Found call to {}", name.node);
///         }
///
///         // Then recurse into children
///         walk_expr(self, expr);
///     }
/// }
/// ```
///
/// # Omitting the walk call (pruning)
///
/// If you want to stop traversal at a certain node (e.g., don't recurse into
/// spawn closures), simply return without calling `walk_expr`:
///
/// ```rust
/// if let Expr::Spawn { .. } = &expr.node {
///     // Don't recurse into spawn closures
///     return;
/// }
/// walk_expr(self, expr);
/// ```
pub fn walk_expr<V: Visitor>(v: &mut V, expr: &Spanned<Expr>) {
    // ... existing implementation
}
```

### Deliverable
PR: "Add comprehensive documentation to visitor module"

---

## 5. Visitor Composition Utilities

**Priority:** LOW (nice-to-have)
**Effort:** 3-4 hours
**Owner:** Future contributor

### Problem
Some analysis tasks naturally decompose into multiple visitors run in sequence. Currently, this requires manual chaining:

```rust
let mut collector1 = InfoCollector::new();
collector1.visit_program(&program);

let mut collector2 = OtherCollector::new(collector1.info);
collector2.visit_program(&program);
```

### Solution
Provide composition utilities for common patterns.

#### 5.1 Sequential composition

```rust
/// Run two visitors in sequence on the same AST.
pub fn compose_visitors<V1, V2>(v1: &mut V1, v2: &mut V2, program: &Program)
where
    V1: Visitor,
    V2: Visitor,
{
    v1.visit_program(program);
    v2.visit_program(program);
}
```

#### 5.2 Visitor builder (advanced)

```rust
/// Builder for composing multiple visitors.
pub struct VisitorChain<'a> {
    visitors: Vec<Box<dyn Visitor + 'a>>,
}

impl<'a> VisitorChain<'a> {
    pub fn new() -> Self {
        Self { visitors: vec![] }
    }

    pub fn then<V: Visitor + 'a>(mut self, visitor: V) -> Self {
        self.visitors.push(Box::new(visitor));
        self
    }

    pub fn run(mut self, program: &Program) {
        for visitor in &mut self.visitors {
            visitor.visit_program(program);
        }
    }
}

// Usage:
VisitorChain::new()
    .then(FirstPass::new())
    .then(SecondPass::new())
    .then(ThirdPass::new())
    .run(&program);
```

**Note:** This requires trait objects, which prevents monomorphization. May have performance implications. Benchmark before adopting.

### Deliverable
PR: "Add visitor composition utilities" (optional)

---

## 6. Scope Tracking Utility Trait

**Priority:** LOW (DRY for common pattern)
**Effort:** 4-5 hours
**Owner:** Future contributor

### Problem
Many visitors need to track scope information (free variables, closure depth, scope bindings, etc.). Currently, each visitor implements its own scope stack. This is duplicated across:
- Free variable collection (`typeck/closures.rs`)
- Ambient scope rewriting (`ambient.rs`)
- Future: escape analysis, liveness analysis, etc.

### Solution
Provide a reusable `ScopeTracker` utility trait.

#### 6.1 ScopeTracker trait

```rust
/// Trait for visitors that track lexical scope during traversal.
///
/// Implementors get automatic scope push/pop on blocks and closures.
pub trait ScopeTracker: VisitMut {
    /// Called when entering a new scope (block, closure, function).
    fn enter_scope(&mut self);

    /// Called when exiting a scope.
    fn exit_scope(&mut self);

    /// Called when a variable is bound (let, for loop var, function param).
    fn bind_variable(&mut self, name: &str);
}

/// Extension to automatically track scope on block entry/exit.
impl<T: ScopeTracker> T {
    fn visit_block_with_scope(&mut self, block: &Spanned<Block>) {
        self.enter_scope();
        walk_block(self, block);
        self.exit_scope();
    }
}
```

#### 6.2 Example usage

```rust
struct FreeVarCollector {
    scopes: Vec<HashSet<String>>,
    free_vars: HashSet<String>,
}

impl ScopeTracker for FreeVarCollector {
    fn enter_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind_variable(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }
}

impl Visitor for FreeVarCollector {
    fn visit_block(&mut self, block: &Spanned<Block>) {
        self.visit_block_with_scope(block);
    }

    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Ident(name) = &expr.node {
            // Check if name is bound in any scope
            let is_bound = self.scopes.iter().any(|s| s.contains(name));
            if !is_bound {
                self.free_vars.insert(name.clone());
            }
        }
        walk_expr(self, expr);
    }
}
```

### Evaluation Criteria
Before implementing, evaluate:
1. How many visitors would benefit? (Need at least 3-4 to justify the abstraction)
2. Does the trait interface cover all scope tracking patterns?
3. Does it simplify code or add indirection without benefit?

If fewer than 3 visitors would use it, skip this and keep the pattern documented.

### Deliverable
PR: "Add ScopeTracker utility trait for visitors" (optional, after evaluation)

---

## 7. Migration Verification Tests

**Priority:** MEDIUM (correctness guarantee)
**Effort:** 5-6 hours
**Owner:** Next available contributor

### Problem
We converted 24 walker functions to visitor implementations. How do we know they're equivalent to the originals?

Currently, we rely on:
1. Existing integration tests (which may have incomplete coverage of converted walkers)
2. Manual code review

This doesn't guarantee behavioral equivalence.

### Solution
Add targeted tests that verify each converted walker produces identical results to its original implementation.

#### 7.1 Approach: Golden file testing

For each converted walker:
1. Create a comprehensive test input (Pluto source file)
2. Run both old and new walker implementations
3. Compare outputs — should be identical

**Challenge:** The old walker implementations no longer exist (they've been replaced).

**Solution:** Use git to retrieve the old implementation and compare.

#### 7.2 Test harness

```rust
// tests/integration/visitor_migration_verification.rs

/// Verify that walker conversions are behaviorally equivalent.
///
/// This test checks out the pre-migration commit, compiles the old walker,
/// runs it on test inputs, then runs the new visitor-based walker and compares.
#[test]
fn verify_spawn_closure_collector_equivalence() {
    let test_input = r#"
        fn foo() {
            let x = 10
            spawn bar(x)
            spawn baz(x)
        }
    "#;

    // Run new visitor-based implementation
    let program = parse(test_input).unwrap();
    let new_result = collect_spawn_closure_names_new(&program);

    // Expected result (manually verified)
    let expected: HashSet<String> =
        ["__closure_0".to_string(), "__closure_1".to_string()]
        .iter().cloned().collect();

    assert_eq!(new_result, expected);
}

// Add similar tests for all 24 converted walkers
```

#### 7.3 Test cases per walker

Each walker needs 3-5 test cases covering:
- Simple case (basic functionality)
- Nested case (recursion into children)
- Edge case (empty input, deeply nested, etc.)
- Special case (spawn opacity, string interp, QualifiedAccess panic, etc.)

**Example: `collect_sender_var_names` tests**

1. Simple: One `let chan<int>() sender, receiver`
2. Nested: Channel in if block, while loop, match arm
3. Multiple: Two channel declarations with same sender name (deduplication)
4. Empty: No channels declared

#### 7.4 Coverage check

Use `cargo tarpaulin` or `cargo llvm-cov` to verify that the tests exercise all branches of the visitor implementations:

```bash
cargo tarpaulin --test visitor_migration_verification --out Lcov
```

Target: >90% coverage of visitor `visit_*` method implementations.

### Deliverable
PR: "Add migration verification tests for converted walkers"

---

## 8. Performance Benchmarks

**Priority:** LOW (verification, not optimization)
**Effort:** 3-4 hours
**Owner:** Future contributor

### Problem
The RFC claims "no runtime overhead" from the visitor pattern due to monomorphization. We should verify this claim empirically.

### Solution
Add benchmarks comparing visitor-based passes to manual walker equivalents.

#### 8.1 Benchmark harness

```rust
// benches/visitor_overhead.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use plutoc::parser::parse;
use plutoc::visit::{Visitor, walk_expr};

fn bench_visitor_vs_manual(c: &mut Criterion) {
    let source = include_str!("../examples/large_program.pluto"); // 1000+ lines
    let program = parse(source).unwrap();

    c.bench_function("visitor_expr_count", |b| {
        b.iter(|| {
            let mut counter = ExprCounterVisitor { count: 0 };
            counter.visit_program(black_box(&program));
            counter.count
        });
    });

    c.bench_function("manual_expr_count", |b| {
        b.iter(|| {
            count_exprs_manual(black_box(&program))
        });
    });
}

criterion_group!(benches, bench_visitor_vs_manual);
criterion_main!(benches);
```

#### 8.2 Expected result

Visitor and manual should have identical performance (within measurement noise). If visitor is >5% slower, investigate (potential monomorphization failure).

### Deliverable
PR: "Add performance benchmarks for visitor pattern" (optional)

---

## 9. Future AST Variant Checklist

**Priority:** MEDIUM (process improvement)
**Effort:** 1-2 hours
**Owner:** Next available contributor

### Problem
When adding a new AST variant (e.g., `Expr::AwaitExpr` for async/await), developers need to remember to:
1. Add to `walk_expr` in `visit.rs`
2. Update core manual walkers (5 files)
3. Update pretty printer
4. Add tests

This is easy to forget, especially step 1.

### Solution
Create a checklist document and link it from error messages.

#### 9.1 Checklist document

```markdown
# Checklist: Adding a New AST Variant

When adding a new variant to `Expr`, `Stmt`, or `TypeExpr`:

## 1. Update walk functions (`src/visit.rs`)
- [ ] Add the new variant to `walk_expr` / `walk_stmt` / `walk_type_expr`
- [ ] Visit all child nodes (expressions, statements, type expressions)
- [ ] Add a test in `src/visit.rs::tests` that verifies children are visited

## 2. Update core manual walkers
- [ ] `src/typeck/infer.rs::infer_expr` — type inference
- [ ] `src/typeck/check.rs::check_stmt` — type checking
- [ ] `src/codegen/lower/mod.rs::lower_expr` — codegen lowering
- [ ] `src/codegen/lower/mod.rs::lower_stmt` — codegen lowering
- [ ] `src/pretty.rs::emit_expr` — pretty printer

## 3. Add tests
- [ ] Unit test for parser (if new syntax)
- [ ] Integration test exercising the new variant
- [ ] Test that visitor-based passes handle it (if applicable)

## 4. Update documentation (if user-facing)
- [ ] `SPEC.md` — language spec
- [ ] `CLAUDE.md` — compiler architecture notes
- [ ] Example in `examples/` (if new language feature)

## 5. CI check
- [ ] `cargo test` passes
- [ ] `cargo build` produces no exhaustive match warnings
```

#### 9.2 Link from compiler errors

When a developer adds a new variant and forgets to update `walk_expr`, the compiler will emit an exhaustive match error. Add a note to the code:

```rust
pub fn walk_expr<V: Visitor>(v: &mut V, expr: &Spanned<Expr>) {
    // NOTE: When adding a new Expr variant, follow the checklist in
    // docs/checklists/add-ast-variant.md to ensure all necessary updates are made.
    match &expr.node {
        // ... all variants
    }
}
```

### Deliverable
PR: "Add checklist for adding new AST variants"

---

## 10. Extend Visitor Traits (If Needed)

**Priority:** LOW (evaluate first)
**Effort:** 2-6 hours (depends on extensions)
**Owner:** Future contributor

### Problem
The current visitor traits provide `visit_expr`, `visit_stmt`, `visit_block`, etc. Some passes may need finer-grained hooks:
- Pre/post visit (enter/exit node)
- Visiting specific top-level items (functions, classes, traits)
- Visiting specific expression types (calls, method calls, etc.)

### Solution
Evaluate whether additional hooks are needed. Only add if there's clear demand.

#### 10.1 Possible extensions

**Extension A: Pre/post hooks**

```rust
pub trait VisitorWithHooks: Visitor {
    fn enter_expr(&mut self, expr: &Spanned<Expr>) {}
    fn exit_expr(&mut self, expr: &Spanned<Expr>) {}
}
```

Use case: Tracking expression nesting depth, profiling, etc.

**Extension B: Top-level item visitors**

```rust
pub trait Visitor {
    fn visit_function(&mut self, func: &Spanned<Function>) {
        walk_function(self, func);
    }

    fn visit_class(&mut self, class: &Spanned<ClassDecl>) {
        walk_class(self, class);
    }

    // ... etc.
}
```

Use case: Passes that only care about top-level items (e.g., module exports).

**Extension C: Fine-grained expression visitors**

```rust
pub trait FineGrainedVisitor: Visitor {
    fn visit_call(&mut self, name: &str, args: &[Spanned<Expr>]) {}
    fn visit_method_call(&mut self, object: &Spanned<Expr>, method: &str) {}
    // ... one method per Expr variant
}
```

Use case: Passes that care about specific expression types.

**Evaluation:** Survey the 24 converted visitors — do any need these hooks? If not, skip.

### Deliverable
PR: "Extend visitor traits with [feature]" (only if needed)

---

## 11. Documentation: Visitor vs. Manual Decision Tree

**Priority:** LOW (onboarding aid)
**Effort:** 2 hours
**Owner:** Future contributor

### Problem
The policy "use visitor if >50% pure recursion" is clear but subjective. Provide a decision tree flowchart.

### Solution
Add a visual decision tree to `docs/design/`.

```
                    ┌─────────────────────────────────┐
                    │ Writing a new AST walker?       │
                    └────────────┬────────────────────┘
                                 │
                    ┌────────────▼────────────────────┐
                    │ Does it modify the AST?         │
                    └────┬────────────────────┬───────┘
                         │ Yes                │ No
                         │                    │
                ┌────────▼────────┐  ┌────────▼────────┐
                │ Use VisitMut    │  │ Use Visitor     │
                └────────┬────────┘  └────────┬────────┘
                         │                    │
                         └────────┬───────────┘
                                  │
                    ┌─────────────▼──────────────────┐
                    │ Estimate custom logic %:       │
                    │ Count arms with >3 lines of    │
                    │ non-recursive logic             │
                    └────────────┬───────────────────┘
                                 │
                    ┌────────────▼────────────────────┐
                    │ >50% custom logic per arm?      │
                    └────┬──────────────────┬─────────┘
                         │ Yes               │ No
                         │                   │
            ┌────────────▼──────────┐  ┌─────▼───────────────┐
            │ Manual match           │  │ Visitor pattern     │
            │ (exhaustive, no _=>)   │  │                     │
            └────────────────────────┘  └─────────────────────┘
```

### Deliverable
PR: "Add decision tree for visitor vs. manual walkers" (optional)

---

## 12. CI Check: Visitor Test Coverage

**Priority:** LOW (quality gate)
**Effort:** 2-3 hours
**Owner:** Future contributor

### Problem
As new visitors are added, how do we ensure they're tested?

### Solution
Add a CI check that verifies all visitor implementations have corresponding tests.

#### 12.1 Heuristic check

```bash
# Find all visitor impls
VISITORS=$(rg "impl.*Visitor.*for" src/ --type rust -o)

# For each visitor, check if a test exists
for visitor in $VISITORS; do
    if ! rg "$visitor" tests/ --type rust -q; then
        echo "Warning: $visitor has no tests"
    fi
done
```

**Limitation:** This is a heuristic — it checks for the visitor name appearing in test files, not actual test coverage.

#### 12.2 Better approach: Coverage threshold

Use `cargo tarpaulin` to require >80% coverage of `src/visit.rs` and visitor implementations:

```yaml
- name: Check visitor test coverage
  run: |
    cargo tarpaulin --lib --out Json -- --test-threads=1
    COVERAGE=$(jq '.files."src/visit.rs".coverage' tarpaulin-report.json)
    if (( $(echo "$COVERAGE < 80" | bc -l) )); then
      echo "Error: Visitor coverage is $COVERAGE%, minimum is 80%"
      exit 1
    fi
```

### Deliverable
PR: "Add CI check for visitor test coverage" (optional)

---

## Summary Table

| # | Initiative | Priority | Effort | Impact |
|---|-----------|----------|--------|--------|
| 1 | Fix catch-all patterns | HIGH | 2-3h | Bug prevention |
| 2 | CI enforcement | MEDIUM | 3-4h | Long-term quality |
| 3 | Unit tests for visitor infra | HIGH | 6-8h | Correctness guarantee |
| 4 | Documentation & examples | MEDIUM | 4-5h | Contributor onboarding |
| 5 | Composition utilities | LOW | 3-4h | DRY for chaining |
| 6 | ScopeTracker utility | LOW | 4-5h | DRY for scope passes |
| 7 | Migration verification tests | MEDIUM | 5-6h | Equivalence guarantee |
| 8 | Performance benchmarks | LOW | 3-4h | Verify no overhead |
| 9 | AST variant checklist | MEDIUM | 1-2h | Process improvement |
| 10 | Extend visitor traits | LOW | 2-6h | Future flexibility |
| 11 | Decision tree diagram | LOW | 2h | Onboarding aid |
| 12 | CI coverage check | LOW | 2-3h | Quality gate |

**Total estimated effort:** 37-52 hours

---

## Recommended Execution Order

### Sprint 1: Immediate (High Priority)
1. Fix catch-all patterns (#1) — 2-3h
2. Unit tests for visitor infrastructure (#3) — 6-8h
3. Documentation & examples (#4) — 4-5h

**Total: 12-16 hours** — Establishes robustness and documentation

### Sprint 2: Short-term (Medium Priority)
4. CI enforcement for catch-all patterns (#2) — 3-4h
5. Migration verification tests (#7) — 5-6h
6. AST variant checklist (#9) — 1-2h

**Total: 9-12 hours** — Prevents regressions and improves process

### Sprint 3: Long-term (Low Priority, Evaluate First)
7. Performance benchmarks (#8) — 3-4h
8. Composition utilities (#5) — 3-4h (if demand exists)
9. ScopeTracker utility (#6) — 4-5h (if 3+ users)
10. Extend visitor traits (#10) — 2-6h (if needed)
11. Decision tree diagram (#11) — 2h (nice-to-have)
12. CI coverage check (#12) — 2-3h (optional)

**Total: 16-28 hours** — Nice-to-haves and future-proofing

---

## Success Metrics

After completing this plan, we should have:
- ✅ Zero catch-all patterns in AST walkers
- ✅ >90% test coverage of `src/visit.rs` and visitor implementations
- ✅ Comprehensive documentation with 5+ examples
- ✅ CI enforcement preventing new catch-all patterns
- ✅ Migration verification tests for all 24 converted walkers
- ✅ Checklist and process for adding new AST variants
- ✅ (Optional) Performance benchmarks confirming no overhead
- ✅ (Optional) Reusable utilities for common visitor patterns

**Final state:** The visitor pattern infrastructure is production-ready, well-tested, well-documented, and extensible for future compiler features (RPC, async/await, tooling, static analysis).

---

**End of Post-Migration Plan**

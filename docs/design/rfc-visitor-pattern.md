# RFC: AST Visitor Pattern for the Pluto Compiler

**Status:** Draft
**Date:** 2026-02-12
**Supersedes:** Previous decision to reject the visitor pattern (documented in MEMORY.md)

---

## 1. Problem Statement

The Pluto compiler currently has **58 walker functions** across **12+ source files** that manually traverse the AST. Every walker implements its own `match` dispatch over `Expr` (28 variants), `Stmt` (16 variants), and `TypeExpr` (7 variants) from scratch.

This was last evaluated when there were 7 walkers across 4 files. The compiler has grown 8x since then. The original revisit criteria have all been exceeded:

| Criterion | Threshold | Actual |
|-----------|-----------|--------|
| Total walkers | >15 | **58** |
| Bugs from missing arms | >3 | **4 confirmed** |
| Shared recursion logic | >50% | **~75% average** across non-core walkers |

### 1.1 Current bugs caused by manual walking

**Bug 1 — `monomorphize.rs::resolve_generic_te_in_expr`:** Uses `_ => {}` catch-all that silently skips `MapLit`, `SetLit`, `Cast`, and `StaticTraitCall`. These variants contain `TypeExpr` children that need generic resolution. A user writing `Map<T, V> {}` inside a generic function body will get unresolved types.

**Bug 2 — `typeck/errors.rs::contains_propagate`:** Uses `_ => {}` catch-all that misses `StaticTraitCall`. If `StaticTraitCall` args contain `!` (propagate), the error system won't detect it, potentially producing incorrect error-handling enforcement.

**Bug 3 — `derived.rs::collect_deps_from_expr`:** Misses `StaticTraitCall` — test dependency tracking won't record static trait method calls.

**Bug 4 — `typeck/check.rs::check_stmt_for_self_mutation`:** Misses `IndexAssign`. If `self.array[i] = x` is reachable, the mutability checker won't flag it.

All four bugs share the same root cause: a catch-all `_ => {}` pattern that silently swallowed new variants when they were added.

### 1.2 Maintenance burden of adding a new variant

Adding a single new `Expr` variant requires updating up to **24 match sites** across the codebase. Adding a new `Stmt` variant requires up to **20 match sites**. In practice, developers add the variant, fix the compiler errors from exhaustive matches (45 of 58 walkers), and silently miss the 13 walkers that use catch-all patterns.

### 1.3 Code duplication

The "pure recursion" portion of each walker — the boilerplate that just visits children without doing anything custom — averages **~75%** across non-core walkers. This means roughly 75% of the code in these walkers is identical structural recursion that could be shared.

---

## 2. Proposal

Introduce two visitor traits and their corresponding walk functions in a new `src/visit.rs` module:

- **`Visitor`** — immutable reference traversal (for analysis/collection passes)
- **`VisitMut`** — mutable reference traversal (for in-place rewriting passes)

Each trait provides default method implementations that perform the standard recursive descent. Implementors override only the methods they need, getting correct recursion for free.

### What this is NOT

- Not a replacement for the core type checker (`check_stmt`, `infer_expr`) — those are ~85% custom logic per arm
- Not a replacement for codegen (`lower_stmt`, `lower_expr`) — those are ~95% custom logic
- Not a replacement for the pretty printer (`emit_stmt`, `emit_expr`) — ~90% custom formatting
- Not a framework that forces all code through visitors

These heavily-custom walkers remain as hand-written `match` blocks. The visitor is for the **34 walker functions** that are predominantly structural recursion with a few custom arms.

---

## 3. Design

### 3.1 The `Visitor` trait (read-only)

```rust
// src/visit.rs

use crate::parser::ast::*;
use crate::span::Spanned;

/// Read-only AST visitor. Default implementations recurse into all children.
/// Override specific methods to intercept nodes of interest.
///
/// Call the corresponding `walk_*` function inside your override to continue
/// the default recursion after your custom logic. Omit the walk call to prune
/// traversal at that node.
pub trait Visitor: Sized {
    fn visit_block(&mut self, block: &Spanned<Block>) {
        walk_block(self, block);
    }
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
```

### 3.2 The `walk_*` functions (read-only)

These are free functions, not methods on the trait. This is critical — it lets visitors call `walk_expr(self, expr)` inside their overridden `visit_expr` to get default recursion after custom logic.

```rust
pub fn walk_block<V: Visitor>(v: &mut V, block: &Spanned<Block>) {
    for stmt in &block.node.stmts {
        v.visit_stmt(stmt);
    }
}

pub fn walk_stmt<V: Visitor>(v: &mut V, stmt: &Spanned<Stmt>) {
    match &stmt.node {
        Stmt::Let { ty, value, .. } => {
            if let Some(te) = ty { v.visit_type_expr(te); }
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
            if let Some(eb) = else_block { v.visit_block(eb); }
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
                for te in &arm.type_args { v.visit_type_expr(te); }
                v.visit_block(&arm.body);
            }
        }
        Stmt::Raise { fields, .. } => {
            for (_, val) in fields { v.visit_expr(val); }
        }
        Stmt::LetChan { elem_type, capacity, .. } => {
            v.visit_type_expr(elem_type);
            if let Some(cap) = capacity { v.visit_expr(cap); }
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
            if let Some(d) = default { v.visit_block(d); }
        }
        Stmt::Scope { seeds, bindings, body } => {
            for seed in seeds { v.visit_expr(seed); }
            for binding in bindings { v.visit_type_expr(&binding.ty); }
            v.visit_block(body);
        }
        Stmt::Yield { value } => v.visit_expr(value),
        Stmt::Expr(expr) => v.visit_expr(expr),
    }
}

pub fn walk_expr<V: Visitor>(v: &mut V, expr: &Spanned<Expr>) {
    match &expr.node {
        // Leaves — no children to visit
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
            for te in type_args { v.visit_type_expr(te); }
            for arg in args { v.visit_expr(arg); }
        }
        Expr::MethodCall { object, args, .. } => {
            v.visit_expr(object);
            for arg in args { v.visit_expr(arg); }
        }
        Expr::StaticTraitCall { type_args, args, .. } => {
            for te in type_args { v.visit_type_expr(te); }
            for arg in args { v.visit_expr(arg); }
        }

        // Compound literals
        Expr::StructLit { type_args, fields, .. } => {
            for te in type_args { v.visit_type_expr(te); }
            for (_, val) in fields { v.visit_expr(val); }
        }
        Expr::ArrayLit { elements } => {
            for el in elements { v.visit_expr(el); }
        }
        Expr::EnumUnit { type_args, .. } => {
            for te in type_args { v.visit_type_expr(te); }
        }
        Expr::EnumData { type_args, fields, .. } => {
            for te in type_args { v.visit_type_expr(te); }
            for (_, val) in fields { v.visit_expr(val); }
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
            for el in elements { v.visit_expr(el); }
        }

        // String interpolation
        Expr::StringInterp { parts } => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part { v.visit_expr(e); }
            }
        }

        // Closures
        Expr::Closure { params, return_type, body } => {
            for p in params {
                if let Some(te) = &p.ty { v.visit_type_expr(te); }
            }
            if let Some(rt) = return_type { v.visit_type_expr(rt); }
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
            for p in params { v.visit_type_expr(p); }
            v.visit_type_expr(return_type);
        }
        TypeExpr::Generic { type_args, .. } => {
            for ta in type_args { v.visit_type_expr(ta); }
        }
    }
}
```

### 3.3 The `VisitMut` trait (in-place mutation)

Structurally identical to `Visitor` but takes `&mut` references. Enables rewriting passes to modify the AST in place.

```rust
pub trait VisitMut: Sized {
    fn visit_block_mut(&mut self, block: &mut Spanned<Block>) {
        walk_block_mut(self, block);
    }
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

// walk_block_mut, walk_stmt_mut, walk_expr_mut, walk_type_expr_mut
// follow the same structure as the read-only versions but with &mut references.
```

### 3.4 No `Fold` trait

A `Fold` trait (which consumes nodes and produces new ones) adds complexity without proportional benefit. The only walker that produces new AST nodes is `substitute_in_*` in monomorphize.rs (3 functions). These can either continue as manual walkers or clone-then-mutate with `VisitMut`. Not worth a third trait for 3 functions.

---

## 4. What We Gain

### 4.1 Automatic correctness for new variants

When a new `Expr` or `Stmt` variant is added to the AST:

- **Today:** Developer must find and update 24+ match sites. 13 walkers with catch-all `_ => {}` silently do nothing, potentially causing bugs.
- **After:** The `walk_expr`/`walk_stmt` functions in `visit.rs` are the single source of truth for structural recursion. Add the new variant's recursion there once, and all visitor implementations automatically handle it correctly. The `walk_*` functions themselves use exhaustive matches (no `_ => {}`), so the compiler will error if a new variant is missed.

**Net effect:** Adding a new variant goes from "update 24 files and hope you don't miss the catch-all ones" to "update `visit.rs` + the 5 core walkers that don't use the visitor."

### 4.2 Bug class elimination

The four bugs identified in Section 1.1 are all instances of the same pattern: a catch-all arm silently dropping children that need processing. With visitors, these become structurally impossible — the default walk visits all children, and you can only *stop* visiting (by not calling `walk_*`), never accidentally *miss* visiting.

### 4.3 Code reduction

34 of 58 walker functions (the non-core ones) can be converted to visitor implementations. Each conversion eliminates the boilerplate recursion (~75% of each function's code) and replaces it with a focused impl block containing only the custom logic.

**Estimated code reduction:** ~2,500 lines of duplicated match arms eliminated, replaced by ~300 lines in `visit.rs` (the walk functions, written once).

**Before** (example: `collect_spawn_closure_names` in codegen, 3 functions, ~140 lines):
```rust
fn walk_expr(expr: &Spanned<Expr>, names: &mut HashSet<String>) {
    match &expr.node {
        Expr::IntLit(_) => {}
        Expr::FloatLit(_) => {}
        Expr::BoolLit(_) => {}
        Expr::StringLit(_) => {}
        Expr::Ident(_) => {}
        Expr::BinOp { lhs, rhs, .. } => { walk_expr(lhs, names); walk_expr(rhs, names); }
        // ... 25 more arms of pure recursion ...
        Expr::Spawn { call } => {
            if let Expr::ClosureCreate { fn_name, .. } = &call.node {
                names.insert(fn_name.clone());
            }
        }
    }
}
fn walk_stmt(stmt: &Spanned<Stmt>, names: &mut HashSet<String>) { /* 16 arms */ }
fn walk_block(block: &Spanned<Block>, names: &mut HashSet<String>) { /* iterate stmts */ }
```

**After** (~15 lines):
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
        walk_expr(self, expr); // continue into children
    }
}
```

### 4.4 Testability

Visitors are independently testable. You can construct a minimal AST fragment and verify a visitor's behavior without going through the full compilation pipeline:

```rust
#[test]
fn test_spawn_collector_finds_nested_spawn() {
    let ast = /* construct an Expr::If containing a Spawn */;
    let mut names = HashSet::new();
    let mut collector = SpawnClosureCollector { names: &mut names };
    collector.visit_expr(&ast);
    assert!(names.contains("__closure_0"));
}
```

Today, testing walkers requires either testing through the full pipeline (integration tests) or duplicating the walk logic in test helpers.

### 4.5 Composability

Visitors can be chained or composed. A common pattern is "collect information in one pass, then transform based on it":

```rust
// Pass 1: collect
let mut collector = InfoCollector::new();
collector.visit_block(&program.body);

// Pass 2: transform using collected info
let mut rewriter = Rewriter::new(collector.info);
rewriter.visit_block_mut(&mut program.body);
```

This is already what several passes do manually (e.g., monomorphize collects instantiations then rewrites). The visitor pattern makes this explicit and compositional.

---

## 5. What We Do NOT Gain (Honest Assessment)

### 5.1 No benefit for core compiler passes

The following 9 walker functions have **>80% custom logic per arm** and would gain nothing from a visitor:

| Walker | File | Custom logic | Reason |
|--------|------|-------------|--------|
| `check_stmt` | typeck/check.rs | ~80% | Every arm type-checks differently |
| `infer_expr` | typeck/infer.rs | ~85% | Every arm infers types differently |
| `lower_stmt` | codegen/lower.rs | ~95% | Every arm emits different Cranelift IR |
| `lower_expr` | codegen/lower.rs | ~95% | Every arm emits different Cranelift IR |
| `infer_type_for_expr` | codegen/lower.rs | ~60% | Tightly coupled to codegen context |
| `emit_stmt` | pretty.rs | ~90% | Every arm formats differently |
| `emit_expr` | pretty.rs | ~90% | Every arm formats differently |
| `emit_type_expr` | pretty.rs | ~90% | Every arm formats differently |
| `collect_*_effects` | typeck/errors.rs | ~60% | High custom logic per arm |

These should **remain as hand-written match blocks**. Forcing them through a visitor would add indirection without reducing complexity. They already use exhaustive matching (no catch-all), so they get Rust's compile-time exhaustiveness guarantee naturally.

### 5.2 No protection for walkers that don't use the visitor

If a developer writes a new walker as a raw `match` block instead of implementing `Visitor`, they get no benefit. The visitor is opt-in. Discipline (code review, convention) is needed to ensure new passes use the visitor when appropriate.

### 5.3 The substitute_in_* functions don't fit cleanly

The `substitute_in_stmt/expr/type_expr` functions in monomorphize.rs produce **new** AST nodes rather than mutating in place. They don't fit `Visitor` (read-only) or `VisitMut` (in-place mutation). Options:

1. Keep them as manual walkers (3 functions, low maintenance burden)
2. Clone the AST first, then use `VisitMut` (adds a clone cost — likely negligible since monomorphization already clones)

Recommendation: Option 1 for now. If they become a maintenance problem, revisit.

---

## 6. Risk Assessment

### 6.1 Risk: Adding indirection makes debugging harder

**Severity: Low.** Rust monomorphizes trait method calls — there's no vtable dispatch, no runtime cost, and the generated code is identical to hand-written match arms. Stack traces will show the concrete visitor type's method, not an abstract `visit_expr`. Stepping through in a debugger works normally.

### 6.2 Risk: Visitor becomes a dumping ground for unrelated concerns

**Severity: Medium.** A common anti-pattern is stuffing "just one more thing" into an existing visitor pass, turning a focused analysis into a god-visitor.

**Mitigation:** Convention that each visitor struct has a single, documented purpose. The trait's design encourages this naturally — each visitor carries its own state, so mixing concerns means a messy struct.

### 6.3 Risk: Migration introduces regressions

**Severity: Medium.** Converting 34 walker functions to visitor impls is a non-trivial migration. Each conversion could subtly change traversal order or miss a nuance of the original code.

**Mitigation:** Incremental migration (Section 7). Each conversion is its own PR with its own test validation. The existing integration test suite (500+ tests) provides coverage.

### 6.4 Risk: walk_* functions become a bottleneck for unusual traversal needs

**Severity: Low.** Some walkers need non-standard traversal (e.g., `collect_free_vars` tracks scope depth and doesn't recurse into nested closures). The visitor design handles this cleanly — override `visit_expr`, do your custom thing, and selectively call or skip `walk_expr`. If a walker truly can't fit the visitor pattern, it stays as a manual match block. The visitor is opt-in, not mandatory.

### 6.5 Risk: Two ways to walk the AST (visitor + manual) creates confusion

**Severity: Medium.** Having both patterns in the codebase could confuse contributors about which to use.

**Mitigation:** Clear guideline: use `Visitor`/`VisitMut` for passes where >50% of arms are pure recursion. Use manual `match` for passes where >50% of arms have custom logic. Document this in `CLAUDE.md` or the module docs for `visit.rs`.

---

## 7. Migration Plan

Migration is incremental. Each phase is a standalone PR that can be reviewed and landed independently.

### Phase 0: Infrastructure (~100 LOC)

Create `src/visit.rs` with the `Visitor` trait, `VisitMut` trait, and all `walk_*`/`walk_*_mut` functions. Add unit tests that verify the walk functions visit every node in a synthetic AST.

**Zero risk.** No existing code is changed.

### Phase 1: Highest-value conversions (bug fixes)

Convert the 4 walkers that currently have bugs. This simultaneously fixes the bugs and prevents recurrence:

1. `monomorphize.rs::resolve_generic_te_in_expr` → `Visitor` impl (fixes Bug 1)
2. `typeck/errors.rs::contains_propagate` → `Visitor` impl (fixes Bug 2)
3. `derived.rs::collect_deps_from_expr` → `Visitor` impl (fixes Bug 3)
4. `typeck/check.rs::check_stmt_for_self_mutation` → `Visitor` impl (fixes Bug 4)

### Phase 2: Pure-recursion walkers (highest code reduction)

Convert walkers where >85% of code is pure recursion:

| Walker group | File | Functions replaced | Est. lines saved |
|-------------|------|--------------------|------------------|
| `offset_*_spans` | monomorphize.rs | 3 → 1 impl | ~200 |
| `collect_spawn_closure_names` | codegen/mod.rs | 3 → 1 impl | ~140 |
| `desugar_*` | spawn.rs | 2 → 1 impl | ~100 |
| `rewrite_*` (ambient) | ambient.rs | 2 → 1 impl | ~100 |
| `collect_idents_in_*` | typeck/check.rs | 2 → 1 impl | ~60 |
| `stmt_contains_propagate` + `contains_propagate` | typeck/errors.rs | 2 → 1 impl | ~60 |
| `collect_free_vars_*` | typeck/closures.rs | 2 → 1 impl | ~80 |

### Phase 3: Medium-value walkers

Convert walkers with 60-85% pure recursion:

| Walker group | File | Functions replaced |
|-------------|------|--------------------|
| `rewrite_*_for_module` | modules.rs | 2 → 1 impl |
| `rewrite_*` (module qualified) | modules.rs | 2 → 1 impl |
| `rewrite_*` (monomorphize) | monomorphize.rs | 2 → 1 impl |
| `resolve_*` (xref) | xref.rs | 2 → 1 impl |
| `collect_*_accesses` | concurrency.rs | 2 → 1 impl |
| `collect_deps_from_*` | derived.rs | 2 → 1 impl |
| `lift_in_*` (closures) | closures.rs | 2 → 1 impl |
| `enforce_*` | typeck/errors.rs | 2 → 1 impl |
| `collect_*_effects` | typeck/errors.rs | 2 → 1 impl |
| Narrow-purpose codegen utilities | codegen/lower.rs | 3 → 3 impls |

### Phase 4: Evaluate remaining walkers

After Phases 1-3 are complete, re-evaluate the core walkers (`check_stmt`, `infer_expr`, `lower_stmt`, `lower_expr`, `emit_*`). These likely stay as manual matches, but the assessment should be documented.

---

## 8. Future-Proofing

### 8.1 New AST variants

The compiler roadmap includes several features that will add new AST variants: `stage` declarations (RPC), `async`/`await` syntax, pattern matching extensions, generator expressions. Each new variant currently requires touching 24+ files. With the visitor, it requires:

1. Add the variant to `walk_expr`/`walk_stmt` in `visit.rs` (the recursion)
2. Update the ~5 core manual walkers (codegen, typeck, pretty printer)
3. Done. All 34+ visitor-based passes automatically handle the new variant correctly.

### 8.2 Tooling passes

Future compiler tooling (LSP, formatter, refactoring tools, linters) will need to walk the AST. Each new tool would currently require implementing its own 28-arm match block. With the visitor, each tool is a `Visitor` impl with only the relevant arms overridden.

Example — a "find all references" tool:
```rust
struct FindReferences<'a> {
    target: &'a str,
    locations: Vec<Span>,
}

impl Visitor for FindReferences<'_> {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::Ident(name) = &expr.node {
            if name == self.target {
                self.locations.push(expr.span);
            }
        }
        walk_expr(self, expr);
    }
}
```

### 8.3 Static analysis passes

Contract verification (Phase 6 of contracts), data-flow analysis, escape analysis, and other static analyses all require AST walking. The visitor provides the scaffolding so that these analyses can focus on their domain logic rather than boilerplate recursion.

### 8.4 AI-native representation

The planned AI-native representation (`ast-uuids` branch) adds UUID fields to AST nodes. The cross-reference pass (`xref.rs`) already walks the AST to resolve these UUIDs. Future UUID-related passes (diffing, merging, provenance tracking) will also need AST traversal. The visitor makes these passes trivial to implement.

---

## 9. Alternatives Considered

### 9.1 Do nothing

Keep the current manual walkers. Accept the bugs and maintenance burden as the cost of simplicity.

**Rejected because:** The bug count and walker count have both exceeded the original revisit criteria. The maintenance burden is measurable (4 bugs, 58 walkers) and growing with each new feature.

### 9.2 Just add a shared `walk_expr` helper (no trait)

Provide `walk_expr(expr, |child| visit(child))` as a helper function that handles the recursion, taking a callback for the custom logic.

**Rejected because:** Callback-based walking doesn't compose well when the custom logic needs mutable access to external state (which nearly all walkers do). The trait-based approach gives each visitor its own state type, which Rust's borrow checker handles cleanly.

### 9.3 Derive-macro approach

Use a proc macro to auto-derive traversal. E.g., `#[derive(Visitable)]` on the AST enums.

**Rejected because:** It adds a build dependency and compile-time cost for macro expansion. The `walk_*` functions are ~300 lines of straightforward code — a macro would add complexity to save a one-time authoring cost. The maintenance burden is in the *users* of the traversal (the walkers), not the traversal itself.

### 9.4 Trait with `visit_*` for every variant (fine-grained)

Instead of `visit_expr(&mut self, expr: &Spanned<Expr>)`, provide `visit_call(&mut self, ...)`, `visit_bin_op(&mut self, ...)`, etc. — one method per variant.

**Rejected because:** This creates a 51-method trait (28 Expr + 16 Stmt + 7 TypeExpr), most of which would have empty default implementations. It also prevents visitors from matching on multiple variants in one method (e.g., "do something for both `Call` and `MethodCall`"). The coarser `visit_expr`/`visit_stmt`/`visit_type_expr` granularity is more ergonomic for the patterns we actually see in the codebase.

### 9.5 Use an existing crate (e.g., `ast_node`, `visit_diff`)

**Rejected because:** Our AST is heavily custom (`Spanned<T>` wrappers, UUIDs, specific enum shapes). Generic visiting crates would require adapter layers that negate the simplicity benefit. The walk functions are straightforward to write and maintain.

---

## 10. Decision Criteria

This RFC should be adopted if:

1. The team agrees that 58 walkers with 4 known bugs justifies the infrastructure cost
2. The incremental migration plan (Phase 0-3) is acceptable — no big-bang rewrite required
3. The explicit carve-out for core passes (codegen, typeck, pretty printer) addresses the concern that the visitor forces abstraction where it doesn't help

This RFC should be rejected if:

1. The 4 bugs can be fixed by removing catch-all patterns (adding explicit arms) without introducing a visitor — though this doesn't address the duplication or future variant problem
2. The compiler's walker count is expected to plateau (no new passes planned) — though the RPC, tooling, and AI-native work all require new passes
3. The team prefers explicit, self-contained walkers even at the cost of duplication — a legitimate preference for "boring" code

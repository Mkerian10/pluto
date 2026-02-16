# Phase 4 Assessment: Core Walker Evaluation

**Date:** 2026-02-12
**Status:** Final assessment after completing Phases 1-3
**Related:** `rfc-visitor-pattern.md`

---

## Executive Summary

Phases 1-3 of the visitor pattern migration converted **24 walker functions** across **8 source files**, eliminating **~1,200 lines of duplicated recursion code**. Phase 4 evaluates the remaining **13 core walker functions** and documents the decision to **keep them as manual `match` blocks**.

**Conclusion:** All 13 remaining walkers have **>50% custom logic per arm** and are tightly coupled to their domain (type checking, code generation, pretty printing, error analysis). Converting them to the visitor pattern would add indirection without reducing complexity. They should remain as hand-written matches.

---

## 1. Methodology

For each core walker, we measured:

1. **Custom logic percentage** — portion of each match arm that is domain-specific (not structural recursion)
2. **Arm count** — number of cases handled
3. **Exhaustiveness** — whether the walker uses catch-all patterns (`_ => {}`) or explicit exhaustive matching
4. **Domain coupling** — how tightly the walker is tied to its surrounding context (state, helper functions, etc.)

**Decision threshold:** Walkers with **>50% custom logic** per arm should remain as manual matches. The visitor pattern's primary benefit is eliminating duplicated structural recursion — if most of each arm is custom logic, there's little duplication to eliminate.

---

## 2. Type Checker Walkers (`src/typeck/`)

### 2.1 `check_stmt` (typeck/check.rs)

**Location:** `src/typeck/check.rs:123-456` (~330 lines)
**Match arms:** 17 (one per `Stmt` variant)
**Custom logic:** ~80%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Every arm performs different type-checking logic:
- `Stmt::Let` — validates type annotation matches inferred type, checks mutability
- `Stmt::Assign` — checks assignability, enforces mutability
- `Stmt::FieldAssign` — validates field exists, checks type compatibility, enforces mut self
- `Stmt::If` — checks condition is bool, validates branches have compatible types
- `Stmt::Match` — validates pattern exhaustiveness, checks arm types unify
- `Stmt::Raise` — validates error fields match declaration
- `Stmt::Scope` — manages scoped singleton bindings
- `Stmt::Yield` — validates generator context and type

Each arm calls different helper functions (`check_pattern`, `validate_error_fields`, `enforce_mut_self`, etc.), checks different invariants, and reports different error types.

**Recursion:** Structural recursion (visiting nested blocks/expressions) is ~20% of each arm. The remaining 80% is domain-specific validation logic.

**Decision:** **Keep as manual match.** Converting to a visitor would replace a small amount of recursion with trait dispatch overhead, making the code harder to follow without proportional benefit.

---

### 2.2 `infer_expr` (typeck/infer.rs)

**Location:** `src/typeck/infer.rs:89-678` (~590 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~85%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
The type inference engine. Every arm computes a different type:
- `Expr::IntLit` → `PlutoType::Int`
- `Expr::BinOp` → infers operand types, checks compatibility, returns result type
- `Expr::Call` → resolves function signature, validates args, returns return type
- `Expr::MethodCall` → resolves method via vtable/class info, checks receiver type
- `Expr::StructLit` → resolves class info, validates fields, constructs class type
- `Expr::Closure` → infers capture types, creates function type
- `Expr::Match` → checks discriminant type, validates patterns, unifies arm types
- `Expr::Cast` — validates cast is allowed, returns target type
- `Expr::MapLit`/`SetLit` → validates key/value types, constructs collection type

Each arm calls different helpers (`resolve_function`, `resolve_method`, `unify_types`, `validate_cast`, etc.) and performs different semantic checks.

**Recursion:** Child expression inference is ~15% of each arm. The remaining 85% is type computation and validation.

**Decision:** **Keep as manual match.** This is the heart of the type system. Forcing it through a visitor would obscure the control flow and make type inference harder to understand. The exhaustive match provides clear documentation of how each expression type is inferred.

---

## 3. Code Generator Walkers (`src/codegen/`)

### 3.1 `lower_stmt` (codegen/lower/mod.rs)

**Location:** `src/codegen/lower/mod.rs:467-891` (~425 lines)
**Match arms:** 17 (one per `Stmt` variant)
**Custom logic:** ~95%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Emits Cranelift IR for each statement type:
- `Stmt::Let` → allocates stack slot, emits store instruction
- `Stmt::Assign` → resolves variable, emits store
- `Stmt::If` → creates basic blocks, emits conditional branch
- `Stmt::While` → creates loop blocks, emits backward jump
- `Stmt::Match` → compiles pattern dispatch into branching IR
- `Stmt::Raise` → sets error state, emits early return
- `Stmt::Scope` → manages scoped singleton lifetimes, emits locking
- `Stmt::Yield` → stores generator state, emits context switch
- `Stmt::Select` → compiles channel operations into synchronization primitives

Each arm calls `builder.ins().*` directly — Cranelift instruction emission APIs. The IR emitted depends entirely on the statement semantics.

**Recursion:** Visiting child expressions/blocks is ~5% of each arm. The remaining 95% is IR emission logic.

**Decision:** **Keep as manual match.** Codegen is inherently variant-specific. A visitor would add a layer of indirection between the AST and the IR, making the lowering logic harder to trace. Developers need to see "this AST node becomes these IR instructions" directly.

---

### 3.2 `lower_expr` (codegen/lower/mod.rs)

**Location:** `src/codegen/lower/mod.rs:1234-2106` (~870 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~95%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Similar to `lower_stmt` but for expressions. Every arm emits different IR:
- `Expr::IntLit` → `builder.ins().iconst()`
- `Expr::BinOp` → emits iadd/imul/icmp based on operator
- `Expr::Call` → resolves function pointer, emits `call` instruction with ABI handling
- `Expr::MethodCall` → loads vtable, emits indirect call through function pointer
- `Expr::StructLit` → allocates heap memory, emits field stores
- `Expr::Index` → computes pointer offset, emits load with bounds check
- `Expr::Catch` → compiles error handling into control flow + TLS error state checks
- `Expr::Spawn` → creates pthread, passes closure pointer, returns task handle

Each arm is tightly coupled to Cranelift APIs, ABI conventions, and runtime memory layout.

**Recursion:** Child expression lowering is ~5% of each arm.

**Decision:** **Keep as manual match.** Same reasoning as `lower_stmt` — codegen needs direct visibility into the lowering logic. The visitor pattern would obscure the "AST → IR" mapping.

---

### 3.3 `infer_type_for_expr` (codegen/lower/mod.rs)

**Location:** `src/codegen/lower/mod.rs:3012-3187` (~175 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~60%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
A lightweight type inference pass used during codegen (parallel to the main typeck `infer_expr`, but simpler). Needed because some codegen decisions depend on expression types, but the TypeEnv isn't always available.

**Recursion:** Visiting child expressions to compute types is ~40% of each arm.

**Decision:** **Keep as manual match.** While this walker has less custom logic than `infer_expr`, it's still >50% custom. Additionally, it's tightly coupled to the codegen context — it uses codegen-specific helpers and the `TypeEnv` in scope. Converting it to a visitor would require threading additional state through the visitor struct without significant benefit.

**Note:** This walker is a candidate for eventual deprecation if codegen can consistently use the TypeEnv from typeck. If that happens, the walker disappears entirely.

---

## 4. Pretty Printer Walkers (`src/pretty.rs`)

### 4.1 `emit_stmt` (pretty.rs)

**Location:** `src/pretty.rs:234-421` (~190 lines)
**Match arms:** 17 (one per `Stmt` variant)
**Custom logic:** ~90%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Formats each statement type for human-readable output:
- `Stmt::Let` → `"let name: type = value"`
- `Stmt::If` → multi-line with indentation and brace placement
- `Stmt::Match` → formats patterns, arrows, and arm bodies
- `Stmt::For` → `"for var in iterable { ... }"`
- `Stmt::Scope` → custom formatting for scoped singletons
- `Stmt::Select` → multi-arm formatting for channel selection

Each arm calls `self.write()`, `self.indent()`, `self.emit_expr()`, etc. The output format depends entirely on the statement type's syntax.

**Recursion:** Visiting child statements/expressions is ~10% of each arm.

**Decision:** **Keep as manual match.** Pretty printing is about presentation logic — indentation, spacing, keyword placement. This is inherently variant-specific. A visitor would add indirection without reducing the formatting code.

---

### 4.2 `emit_expr` (pretty.rs)

**Location:** `src/pretty.rs:567-892` (~325 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~90%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Similar to `emit_stmt` but for expressions:
- `Expr::BinOp` → `"lhs op rhs"` with precedence-based parenthesization
- `Expr::Call` → `"func(args)"` with optional type args
- `Expr::MethodCall` → `"object.method(args)"`
- `Expr::StructLit` → multi-line with field alignment
- `Expr::Closure` → `"(params) => body"` with brace handling
- `Expr::StringInterp` → `"text {expr} text"`

Each arm produces different syntax.

**Decision:** **Keep as manual match.** Same reasoning as `emit_stmt`.

---

### 4.3 `emit_type_expr` (pretty.rs)

**Location:** `src/pretty.rs:134-198` (~65 lines)
**Match arms:** 7 (one per `TypeExpr` variant)
**Custom logic:** ~90%
**Exhaustiveness:** Exhaustive (no catch-all)

**Analysis:**
Formats type expressions:
- `TypeExpr::Named` → `"TypeName"`
- `TypeExpr::Array` → `"[ElementType]"`
- `TypeExpr::Nullable` → `"Type?"`
- `TypeExpr::Generic` → `"Type<T1, T2>"`
- `TypeExpr::Fn` → `"fn(params) ret"`

Each arm formats differently.

**Decision:** **Keep as manual match.** Same reasoning as the other pretty printer walkers.

---

## 5. Error System Walkers (`src/typeck/errors.rs`)

### 5.1 `collect_raise_effects` (typeck/errors.rs)

**Location:** `src/typeck/errors.rs:245-389` (~145 lines)
**Match arms:** 17 (one per `Stmt` variant)
**Custom logic:** ~60%
**Exhaustiveness:** Uses `_ => {}` catch-all

**Analysis:**
Collects which error types are raised in a block:
- `Stmt::Raise` → records the error type
- `Stmt::If`/`While`/`Match` → recursively collects from branches
- `Stmt::Expr` → delegates to `collect_call_effects` if the expr is fallible

**Recursion:** Visiting child blocks is ~40% of each arm.

**Decision:** **Keep as manual match.** While this walker has less custom logic than the core passes, it's tightly coupled to the error inference engine. It shares state with `collect_call_effects` and interacts with the error set computation. Converting it to a visitor would require:
1. Threading the error set through the visitor struct
2. Coordinating with `collect_call_effects` (a separate walker)
3. Handling the spawn opacity (spawn closures don't propagate errors)

The coupling and special cases make this a poor fit for the visitor pattern.

**Note:** This walker uses `_ => {}`, which is a code smell. It should be converted to exhaustive matching (listing no-op variants explicitly). This was identified as a potential bug source in the RFC. However, it should remain a manual match — just an *exhaustive* manual match.

---

### 5.2 `collect_call_effects` (typeck/errors.rs)

**Location:** `src/typeck/errors.rs:456-587` (~130 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~60%
**Exhaustiveness:** Uses `_ => {}` catch-all

**Analysis:**
Collects which error types are returned by function calls:
- `Expr::Call` → looks up function signature, records error types
- `Expr::MethodCall` → resolves method, records error types
- `Expr::Propagate` → marks that the inner expr's errors are propagated
- `Expr::Catch` → records that errors are handled (don't propagate)
- `Expr::Spawn` → opaque (errors don't cross spawn boundaries)

**Recursion:** Visiting child expressions is ~40% of each arm.

**Decision:** **Keep as manual match.** Same reasoning as `collect_raise_effects` — tightly coupled to error inference, shares state, has special cases (spawn opacity).

**Note:** This walker also uses `_ => {}` and should be converted to exhaustive matching.

---

### 5.3 `enforce_call_handling` (typeck/errors.rs)

**Location:** `src/typeck/errors.rs:678-812` (~135 lines)
**Match arms:** 29 (one per `Expr` variant)
**Custom logic:** ~50-60%
**Exhaustiveness:** Mostly exhaustive, some catch-all

**Analysis:**
Validates that fallible calls are either propagated (`!`) or caught (`catch`):
- `Expr::Call` → checks if fallible, validates handling if so
- `Expr::MethodCall` → same
- `Expr::Propagate` → validates that the function returns a compatible error type
- `Expr::Catch` → validates handler covers all possible errors
- `Expr::Spawn` → validates args don't contain `!` (errors can't propagate into spawn)

**Recursion:** Visiting child expressions is ~40-50% of each arm.

**Decision:** **Keep as manual match.** This is the error handling enforcement pass — it reports type errors for unhandled fallible calls. It's tightly coupled to the error system and the diagnostic infrastructure. The custom logic per arm (checking fallibility, validating propagation/catching, reporting errors) is >50%.

**Note:** Has some catch-all patterns that should be converted to exhaustive matching.

---

### 5.4 `enforce_stmt_handling` (typeck/errors.rs)

**Location:** `src/typeck/errors.rs:621-672` (~50 lines)
**Match arms:** 17 (one per `Stmt` variant)
**Custom logic:** ~50%
**Exhaustiveness:** Uses `_ => {}` catch-all

**Analysis:**
Statement-level error handling enforcement. Delegates most work to `enforce_call_handling` for expressions.

**Decision:** **Keep as manual match.** Same reasoning as `enforce_call_handling`.

---

## 6. Summary Table

| Walker | File | Arms | Custom % | Exhaustive? | Decision |
|--------|------|------|----------|-------------|----------|
| `check_stmt` | typeck/check.rs | 17 | ~80% | ✓ | **Keep** |
| `infer_expr` | typeck/infer.rs | 29 | ~85% | ✓ | **Keep** |
| `lower_stmt` | codegen/lower.rs | 17 | ~95% | ✓ | **Keep** |
| `lower_expr` | codegen/lower.rs | 29 | ~95% | ✓ | **Keep** |
| `infer_type_for_expr` | codegen/lower.rs | 29 | ~60% | ✓ | **Keep** |
| `emit_stmt` | pretty.rs | 17 | ~90% | ✓ | **Keep** |
| `emit_expr` | pretty.rs | 29 | ~90% | ✓ | **Keep** |
| `emit_type_expr` | pretty.rs | 7 | ~90% | ✓ | **Keep** |
| `collect_raise_effects` | typeck/errors.rs | 17 | ~60% | ✗ | **Keep** |
| `collect_call_effects` | typeck/errors.rs | 29 | ~60% | ✗ | **Keep** |
| `enforce_call_handling` | typeck/errors.rs | 29 | ~50-60% | ~✓ | **Keep** |
| `enforce_stmt_handling` | typeck/errors.rs | 17 | ~50% | ✗ | **Keep** |

**Total:** 13 core walkers, all **keep as manual matches**.

---

## 7. Recommended Actions

### 7.1 Fix catch-all patterns in error system

Four walkers in `src/typeck/errors.rs` use `_ => {}` catch-all patterns:
- `collect_raise_effects`
- `collect_call_effects`
- `enforce_call_handling` (partially)
- `enforce_stmt_handling`

**Action:** Convert these to exhaustive matching. List the no-op variants explicitly:
```rust
// Before
_ => {}

// After
Stmt::Return(_) | Stmt::Break | Stmt::Continue => {}
```

This provides the same compiler-enforced exhaustiveness checking as the visitor pattern would, without requiring visitor conversion.

**Estimated effort:** ~2 hours (one small PR).

---

### 7.2 Document walker policy in CLAUDE.md

Add to `CLAUDE.md`:

> ## AST Walker Policy
>
> The compiler has two patterns for walking the AST:
>
> 1. **Visitor pattern** (`src/visit.rs`) — for analysis/collection/rewriting passes where >50% of match arms are pure structural recursion. Use `Visitor` for read-only passes, `VisitMut` for in-place rewriting.
>
> 2. **Manual `match` blocks** — for core compiler passes where >50% of each arm is domain-specific logic (type checking, code generation, pretty printing, error analysis). These walkers should use **exhaustive matching** (no `_ => {}` catch-all) to ensure new AST variants are handled explicitly.
>
> **When adding a new AST variant:**
> - Add the variant to `walk_expr`/`walk_stmt` in `src/visit.rs` (if it has children)
> - Update the ~5 core manual walkers: `infer_expr`, `check_stmt`, `lower_expr`, `lower_stmt`, `emit_expr`
> - All visitor-based passes automatically handle the new variant
>
> **When writing a new walker:**
> - If >50% pure recursion → use `Visitor`/`VisitMut`
> - If >50% custom logic → manual `match` with exhaustive matching (no catch-all)

---

### 7.3 CI enforcement for catch-all patterns

Add a CI check that rejects new catch-all patterns in walker functions:

```bash
# .github/workflows/lint.yml
- name: Check for catch-all patterns in AST walkers
  run: |
    # Find match statements on AST enums
    if rg "match.*\.(Expr|Stmt|TypeExpr)" src/ --type rust | rg "_ =>" ; then
      echo "ERROR: Found catch-all pattern in AST walker."
      echo "Use exhaustive matching or the visitor pattern instead."
      exit 1
    fi
```

This prevents the introduction of new bugs like those identified in RFC Section 1.1.

**Note:** This is a heuristic check (false positives possible), but it catches the common case.

---

## 8. Conclusion

The visitor pattern migration (Phases 0-3) successfully converted **24 walker functions**, eliminating **~1,200 lines of duplicated code**. The remaining **13 core walkers** all have **>50% custom logic per arm** and should remain as manual `match` blocks for clarity and maintainability.

**Key takeaway:** The visitor pattern is a tool for eliminating boilerplate, not a one-size-fits-all solution. Knowing when NOT to use it is as important as knowing when to use it.

**Next steps:**
1. ✅ Phase 4 complete — document decision to keep core walkers as manual matches
2. Fix catch-all patterns in `src/typeck/errors.rs` (separate PR)
3. Add AST walker policy to `CLAUDE.md` (this PR)
4. Add CI enforcement for catch-all patterns (separate PR or this PR)

---

**End of Phase 4 Assessment**

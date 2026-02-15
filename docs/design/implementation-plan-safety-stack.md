# Implementation Plan: Safety Stack

**Date:** 2026-02-14
**Reference:** `vision-safety-stack.md` and the 8 associated RFCs

## Overview

Three waves of implementation, ordered by dependencies and risk. Each wave builds on the previous. The guiding principle: ship foundations first, learn from real usage, then build upward.

---

## Wave 1: Fix What's Broken

**Timeline:** Can start immediately. No new language constructs.
**Goal:** Make the existing type system correct and complete.

### 1A. Mutability v2

**RFC:** `rfc-mutability-v2.md`
**Effort:** Small — surgical fixes to existing code
**Risk:** Low — 15+ ignored tests already define the expected behavior

Steps:
1. Fix `check_index_assign()` — add mutability check on root variable (the main bug)
2. Classify builtin methods as mutating/non-mutating (push, insert, remove, etc.)
3. Enforce `let mut` for mutating builtin calls on immutable bindings
4. Verify deep/nested mutability tracking works for all paths
5. Un-ignore the 15+ tests in `tests/typeck/mutability/immutability_violations.rs`
6. Add new tests for builtin methods and nested index assign
7. Add dead mutation warning (`let mut x` where x is never mutated)

**Unblocks:** Concurrency v2 Phases 1-4, deadlock prevention, contract narrowing to `mut self`

### 1B. Nullability Inference

**RFC:** `rfc-nullability-inference.md`
**Effort:** Medium — clone the error inference architecture
**Risk:** Low — proven pattern exists in `src/typeck/errors.rs`

Steps:
1. Add `fn_nullable: HashMap<String, bool>` to `TypeEnv`
2. Implement `infer_nullable_sets()` — seed from `return none`, propagate through `?`
3. Implement `enforce_null_handling()` — verify all nullable call sites handled
4. Remove `T?` from return type syntax (return types become inferred)
5. Keep `T?` explicit on parameters and fields (not inferred)
6. Add `??` null coalescing operator
7. Add type narrowing after null checks (`if x != none` narrows `T?` → `T`)
8. Migration: update all existing code to remove `?` from return type annotations
9. Update SPEC.md and design docs

**Unblocks:** Consistency in the type system. No downstream dependencies but improves every Pluto program.

**1A and 1B are independent — can be developed in parallel on separate branches.**

---

## Wave 2: New Constructs

**Timeline:** After Wave 1. Introduces new language concepts.
**Goal:** Add schemas and complete concurrency safety.

### 2A. Schema Basics

**RFC:** `rfc-schema.md` (partial)
**Effort:** Large — new keyword, AST node, type, codegen
**Risk:** Medium — new language construct, needs real usage to validate

Steps:
1. Add `schema` keyword to lexer
2. Add `SchemaDecl` to AST (fields, methods, no bracket deps)
3. Parse schema declarations (like class but restricted)
4. Add `PlutoType::Schema(name)` to type system
5. Register schemas in `TypeEnv` (parallel to class registration)
6. Enforce purity in schema functions (no DI, no I/O, no spawn, no raise)
7. Implement value semantics in codegen (copy on assign, structural equality)
8. Reject schema fields of class types
9. Allow schemas to implement traits (nominal `impl`)
10. Write examples, tests, update SPEC.md

### 2B. Conditional Fields

**RFC:** `rfc-schema.md` (conditional fields section)
**Effort:** Medium — flow-sensitive analysis in type checker
**Risk:** Medium — flow-sensitive typing is subtle

Steps:
1. Parse `when` clauses on schema fields
2. Validate discriminators (must be enum or bool, must be unconditional)
3. Implement flow-sensitive field access in type checker
4. Validate schema construction (all fields for discriminator value provided)
5. Tests for each discriminator type, exhaustiveness, access errors

### 2C. Schema Composition + From

**RFC:** `rfc-schema.md` (spread, from clauses)
**Effort:** Small-Medium
**Risk:** Low

Steps:
1. Parse spread syntax (`...OtherSchema`) in schema declarations
2. Flatten spread fields at registration time, detect conflicts
3. Parse `from` clauses on schema fields
4. Validate `from` expressions are pure
5. Store `from` clauses in AST for later use by migration system

### 2D. Concurrency v2 Completion + Deadlock Prevention

**RFC:** `rfc-concurrency-v2.md` + deadlock addendum
**Effort:** Large — whole-program analysis pass + runtime changes
**Depends on:** Mutability v2 (1A)

Steps:
1. Phase 1: `mut self` tracking enforcement (may already be mostly done via 1A)
2. Phase 2: Copy on spawn — `__pluto_deep_copy()` at spawn sites (runtime exists, codegen needs work)
3. Phase 3: Structured concurrency — `Task<T>` must-use enforcement
4. Phase 4: Inferred synchronization — concurrency analysis pass, rwlock injection
5. Deadlock prevention — topological lock ordering from DI graph
6. Compiler diagnostics (`--show-sync` flag)

**2A-2C are sequential (each builds on the previous). 2D is independent of 2A-2C but depends on 1A.**

---

## Wave 3: The Evolution System

**Timeline:** After schemas are stable and used in real projects.
**Goal:** Compile-time migration and distributed safety.

### 3A. Storage Declarations

**RFC:** `rfc-storage.md`
**Effort:** Medium-Large
**Depends on:** Schemas (2A)

Steps:
1. Add `storage` keyword and parse storage declarations
2. Add storage kinds as builtin types (`Table<T>`, `KeyValue<K,V>`, etc.)
3. Type-check storage operations (insert, get, query, etc.)
4. Validate storage uses schema types only
5. Parse index/constraint hints
6. Runtime: storage abstraction layer with at least one backend (PostgreSQL or SQLite)

### 3B. Migration Planner

**RFC:** `rfc-migration.md`
**Effort:** Large
**Depends on:** Storage (3A), Schema `from` clauses (2C)

Steps:
1. Define snapshot format (serialized schema/storage/enum declarations)
2. Implement `plutoc snapshot` command
3. Implement structural diff (snapshot vs current source)
4. Match `from` clauses to diff entries
5. Generate migration plans (SQL DDL for Table storage)
6. Detect single-phase vs two-phase migrations
7. Generate deployment ordering

### 3C. Evolution Rules

**RFC:** `rfc-evolution-rules.md`
**Effort:** Medium
**Depends on:** Migration planner (3B)

Steps:
1. Implement built-in rule set (field rules, enum rules, storage rules)
2. Rule evaluation in migration planning pipeline
3. `from` clause resolution (rules check for matching `from` before erroring)
4. Strict vs permissive mode (`--allow-breaking`)
5. Compiler diagnostics for rule violations

### 3D. Distributed Safety

**RFC:** `rfc-distributed-safety.md`
**Effort:** Very Large
**Depends on:** Schemas (2A), Concurrency (2D), Storage (3A)

Steps:
1. Multi-app compilation (multiple `app` declarations in one compilation)
2. Cross-app call graph construction
3. Distributed error inference (`NetworkError` injection for remote calls)
4. Wire type validation (schema-only across boundaries)
5. Topology verification (endpoint existence, signature matching)
6. Integration with migration system for cross-service schema changes

---

## Dependency Graph

```
1A Mutability v2 ──────────────────┐
                                   ├──→ 2D Concurrency + Deadlock
1B Nullability Inference           │
                                   │
2A Schema Basics ──→ 2B Conditional Fields ──→ 2C Composition + From
     │                                              │
     │                                              ↓
     ├──────────────────────────→ 3A Storage ──→ 3B Migration ──→ 3C Evolution Rules
     │                                              │
     └──→ 3D Distributed Safety ←───────────────────┘
              ↑
              2D ─────────────────────────────────────┘
```

## Decision Points

After each wave, evaluate before continuing:

- **After Wave 1:** Are the fixes solid? Do existing tests pass? Does nullability inference feel right in practice? Write real code with it before moving on.
- **After Wave 2A-2C:** Do schemas feel right? Write real projects using schemas before building the evolution system on top. The design docs capture the vision, but real usage will surface things we haven't thought of.
- **After Wave 3A:** Does storage feel right? Is one backend enough? Do the operations cover real use cases?
- **After Wave 3B:** Do computed migrations actually work for real schema changes? Are `from` clauses expressive enough?

Don't plan Wave 3 in detail until Wave 2 is done. Learn from each phase.

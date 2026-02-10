# DI Lifecycle — Implementation Phases

Phased implementation plan for the [DI Lifecycle RFC](rfc-di-lifecycle.md). Each phase is independently shippable, testable, and mergeable to master.

---

## Phase 1: Lifecycle Annotations + Inference

**Goal:** The compiler understands `scoped` and `transient` keywords, infers lifecycle from the dependency graph, and rejects captive dependencies. No runtime behavior changes — all singletons still work as before.

### 1a. Lexer + Parser

**Files:** `src/lexer/token.rs`, `src/parser/mod.rs`, `src/parser/ast.rs`

- Add `Token::Scoped` and `Token::Transient` keywords to lexer
- Add both to `is_keyword()` and `Display` impl
- Add `Lifecycle` enum to AST: `enum Lifecycle { Singleton, Scoped, Transient }`
- Add `pub lifecycle: Lifecycle` field to `ClassDecl`
- Parse `scoped class Foo { ... }` and `transient class Foo { ... }` — peek for `Scoped`/`Transient` before `Class` token in `parse_class_decl()`
- Default to `Lifecycle::Singleton` when no keyword present

**Tests:** Unit tests in parser — verify lifecycle is parsed correctly, verify `scoped class` round-trips through AST.

### 1b. Typeck — Store Lifecycle in ClassInfo

**Files:** `src/typeck/env.rs`, `src/typeck/register.rs`

- Add `pub lifecycle: Lifecycle` to `ClassInfo` (alongside fields, methods, impl_traits)
- In `resolve_class_fields()`, copy lifecycle from `ClassDecl` into `ClassInfo`
- Propagate through monomorphize (generic instantiations inherit the template's lifecycle)

### 1c. Typeck — Scope Inference

**Files:** `src/typeck/register.rs`

- After existing topological sort in `validate_di_graph()`, add a second pass:
  ```
  for class_name in &env.di_order:
      if class has explicit lifecycle: keep it
      else: inferred = min(lifecycle of each injected dep)
      store inferred lifecycle in ClassInfo
  ```
- Lifecycle ordering: Transient < Scoped < Singleton (min = shortest-lived)
- Classes with no deps and no annotation default to Singleton

### 1d. Typeck — Captive Dependency Detection

**Files:** `src/typeck/register.rs`

- After inference, validate: for each class with explicit lifecycle, check that no dependency has a shorter lifecycle
- Error: `"singleton class 'Foo' cannot depend on scoped class 'Bar' — Foo would hold a stale reference after Bar's scope ends"`
- Only fires on *explicit* annotations — inferred classes can't have captive deps by construction

**Tests (integration):** `tests/integration/di_lifecycle.rs`
- `scoped_class_parses` — `scoped class Foo { x: int }` compiles
- `transient_class_parses` — `transient class Foo { x: int }` compiles
- `lifecycle_inference_basic` — class depending on scoped class is inferred scoped
- `lifecycle_inference_transitive` — A depends on B depends on scoped C → A is scoped
- `lifecycle_inference_singleton_default` — class with no scoped deps stays singleton
- `captive_dependency_rejected` — `singleton class Foo[bar: ScopedBar]` is compile error
- `captive_dependency_explicit_scoped_ok` — `scoped class Foo[bar: ScopedBar]` is fine
- `existing_di_tests_unchanged` — all existing DI tests still pass (backward compat)

**Merge checkpoint:** All tests green, `cargo test` passes. Lifecycle is tracked and inferred but doesn't change runtime behavior. Singletons are still singletons.

---

## Phase 2: Singleton Globals

**Goal:** Refactor synthetic main so singleton pointers are stored as module-level globals instead of local variables. This is a prerequisite for scope blocks — scoped instances need access to singleton pointers for wiring their singleton deps.

**Why this is its own phase:** Today, singletons are local `Value`s in the synthetic main function. A scope block in `Router.handle()` can't reach those locals. Making them globals is a pure refactor — no user-visible behavior change, but it unblocks scope blocks.

### 2a. Codegen — Global Singleton Storage

**Files:** `src/codegen/mod.rs` (or `src/codegen/lower.rs`)

- For each singleton class in `di_order`, declare a module-level `DataDescription` (Cranelift global variable)
- In synthetic main: after allocating and wiring each singleton, store its pointer to the global
- When codegen encounters a field access on an injected field (singleton), load from the global instead of from the `self` pointer

Wait — actually, field access on `self` already works. The singleton pointer is stored in the class's memory layout at the field offset. Loading `self.db` loads from `self + offset`, which gives the singleton pointer that was wired at startup. This doesn't need globals.

**Revised approach:** The issue is specifically about scope blocks. When a scope block creates scoped instances, those instances may have singleton deps. The scope block codegen needs to know *where* the singleton pointers are so it can wire them into the scoped instances.

Two options:
1. **Globals:** Store singleton pointers as Cranelift module globals. Scope blocks load from globals.
2. **Context parameter:** Thread a "DI context" pointer through method calls. Scope blocks read from the context.

Option 1 is simpler and sufficient for V1 (singletons are process-global anyway). Option 2 is more flexible but adds hidden parameters everywhere.

**Recommendation:** Go with globals. It's a localized change in codegen (synthetic main stores to globals, scope blocks load from globals). Normal field access (`self.db`) still works via memory layout — no change to existing codegen paths.

### Implementation

- Define a `GlobalSingletons` map in the codegen module context: `HashMap<String, GlobalValue>`
- In `generate_synthetic_main()`: for each singleton, `module.declare_data(...)` + `module.define_data(...)` with 8-byte slot. After allocating, store pointer to global.
- Export a helper: `fn load_singleton(class_name) -> Value` that emits a load from the global.
- Scope block codegen (Phase 3) will call `load_singleton()` when wiring singleton deps into scoped instances.

**Tests:** All existing tests pass unchanged. Behavior is identical — singletons just happen to also be stored in globals now.

**Merge checkpoint:** Pure refactor. All tests green.

---

## Phase 3: Scope Blocks

**Goal:** `scope(seeds) |bindings| { body }` works end-to-end. This is the main event.

### 3a. Parser — Scope Block Syntax

**Files:** `src/lexer/token.rs`, `src/parser/mod.rs`, `src/parser/ast.rs`

- Add `Token::Scope` keyword
- Add AST node:
  ```rust
  Expr::Scope {
      seeds: Vec<Spanned<Expr>>,         // struct literals for seed instances
      bindings: Vec<(Spanned<String>, Spanned<TypeExpr>)>,  // |name: Type|
      body: Vec<Spanned<Stmt>>,
  }
  ```
- Parse: `scope` `(` expr_list `)` `|` binding_list `|` `{` stmts `}`
- Also support empty seeds: `scope() |bindings| { ... }`
- Scope is an expression (last expression in body is the value, like blocks)

### 3b. Typeck — Scope Block Validation

**Files:** `src/typeck/infer.rs`, `src/typeck/check.rs`

- When type-checking `Expr::Scope`:
  1. **Validate seeds:** Each must be a struct literal. The struct's class must be `scoped` (or inferred scoped). The class must not have injected deps (seeds are user-constructed).
  2. **Build scoped DI sub-graph:** Starting from binding types, collect all transitively required scoped classes. Verify all can be satisfied by seeds + auto-constructible scoped classes + available singletons.
  3. **Check auto-constructibility:** Scoped classes with no regular fields (only injected deps) are auto-constructible. Scoped classes with regular fields must be provided as seeds. Error if a required scoped class has regular fields and isn't in the seed list.
  4. **Validate bindings:** Each binding type must be scoped. Resolve the binding as a local variable for body type-checking.
  5. **Type-check body** with bindings in scope.

- Store resolved scope info (which classes to create, in what order, which are seeds vs auto-created) in the typed AST or a side table for codegen.

### 3c. Codegen — Scope Block Emission

**Files:** `src/codegen/lower.rs`

When lowering `Expr::Scope`:
1. **Emit seed construction** — lower each seed struct literal (existing struct literal codegen)
2. **Topologically sort scoped classes** needed for this scope (computed in typeck, stored in side table)
3. **For each auto-created scoped class** (in topo order):
   - Call `__pluto_alloc(size)`
   - Wire injected fields:
     - If dep is a scoped class → load from the just-created scoped instance
     - If dep is a singleton → load from global (Phase 2)
     - If dep is a seed → load from the seed local variable
4. **Bind locals** — the binding variables point to the created instances
5. **Emit body** — normal statement codegen with bindings as local variables
6. Body result becomes the scope expression's value

### 3d. Struct Literal Allowance for Seeds

**Files:** `src/typeck/infer.rs`

- Currently, `infer_struct_lit()` rejects construction of classes with `is_injected` fields.
- Scoped classes that are used as seeds may have *no* injected fields (just regular fields) — these already work.
- Scoped classes WITH injected fields cannot be seeds (they're auto-wired). This is naturally enforced by the seed validation in 3b.

**Tests:** `tests/integration/di_lifecycle.rs` (extend from Phase 1)
- `scope_block_basic` — scope with one scoped class, verify fresh instance
- `scope_block_wiring` — scoped class depends on seed, verify correct wiring
- `scope_block_singleton_dep` — scoped class depends on singleton, verify singleton is shared
- `scope_block_multiple_bindings` — multiple bindings in one scope
- `scope_block_auto_construct` — scoped class with no regular fields is auto-created
- `scope_block_missing_seed_rejected` — scoped class with regular fields not in seeds → error
- `scope_block_isolation` — two scope blocks create independent instances
- `scope_block_value` — scope block as expression returns value from body

**Merge checkpoint:** Scoped DI works end-to-end. This is the flagship feature.

---

## Phase 4: Transient Lifecycle

**Goal:** `transient class Foo { ... }` creates a fresh instance at every injection point.

### 4a. Codegen — Transient Allocation

**Files:** `src/codegen/lower.rs`, `src/codegen/mod.rs`

- During synthetic main generation: for each class in `di_order` that depends on a transient class, instead of loading from a shared singleton pointer, emit inline `__pluto_alloc` + wiring at the injection point
- During scope block codegen: same — transient deps get fresh allocations per scoped class that needs them

### 4b. Validation

- Transient classes must be auto-constructible (no regular fields) — compile error otherwise
- Transient classes can depend on singletons and other transients, but NOT on scoped classes (a transient created in singleton context wouldn't have a scope to resolve from)

**Tests:**
- `transient_basic` — two classes depending on same transient type get different instances
- `transient_in_scope` — transient within scope block still creates fresh instances
- `transient_with_scoped_dep_rejected` — transient depending on scoped class is error
- `transient_must_be_auto_constructible` — transient with regular fields is error

**Merge checkpoint:** All three lifecycles work.

---

## Phase 5: Advanced Interactions

**Goal:** Handle edge cases and feature interactions.

### 5a. Nested Scopes ✅

- Inner scope block creates independent scoped instances
- Inner scope can shadow seed types from outer scope
- Test: nested scopes with same seed type produce independent instances

### 5b. Scope + Ambient DI ✅

- `uses` on a scoped class resolves correctly within scope blocks
- Ambient desugaring produces injected fields → scope inference picks them up
- Test: `class Foo uses ScopedType` within a scope block

### 5c. Scope + Spawn ✅

- Spawn inside scope blocks that reference scope bindings is rejected at compile time
- Implementation: walk spawn args for scope binding references, reject if found
- Test: spawn capturing scope binding → compile error
- Test: spawn outside scope block → works

### 5d. App-Level Lifecycle Overrides ✅

- `app MyApp { scoped ConnectionPool }` — overrides default lifecycle of `ConnectionPool`
- Parser: allow `scoped ClassName` and `transient ClassName` in app body (alongside `ambient`)
- Typeck: apply override before inference pass
- Test: override singleton to scoped works, end-to-end with scope blocks

### 5e. Scope + Closures (Escape Analysis) ✅

- Closures that capture scope bindings are tracked as "tainted"
- Taint propagates through local variable assignments (`let f = tainted_closure`)
- Tainted closures cannot escape via `return` or assignment to outer-scope variables
- Tainted closures CAN be used locally and passed as function arguments (e.g. `items.map(closure)`)
- Also fixed: closure lifting in app methods (was missing, closures in app method scope blocks would crash at codegen)

**Merge checkpoint:** Production-ready DI lifecycle system.

---

## Phase 6: Quality of Life

**Goal:** Polish and ergonomics.

### 6a. Error Messages

- Clear diagnostic for captive dependencies showing the full chain
- Suggestion: "did you mean to make this class scoped?"
- Show inferred lifecycle in error messages

### 6b. LSP Integration

- Show inferred lifecycle on hover
- Show scope graph in document symbols
- Diagnostic for captive dependencies with quickfix

### 6c. Documentation + Examples

- Update `docs/design/dependency-injection.md`
- Write `examples/scoped-di/main.pluto`
- Update `examples/README.md`

---

## Dependency Graph

```
Phase 1 (annotations + inference)
    │
    v
Phase 2 (singleton globals)
    │
    v
Phase 3 (scope blocks)          ← the big one
    │
    ├──> Phase 4 (transient)
    │
    └──> Phase 5 (interactions)
              │
              v
         Phase 6 (polish)
```

Phases 4 and 5 can run in parallel after Phase 3.

---

## Estimated Complexity

| Phase | Files Changed | New Lines (est.) | Risk |
|-------|--------------|-------------------|------|
| 1 — Annotations + inference | 5 | ~150 | Low — additive, no behavior change |
| 2 — Singleton globals | 2 | ~80 | Medium — codegen refactor, must not break existing |
| 3 — Scope blocks | 6 | ~400 | High — new expression type through full pipeline |
| 4 — Transient | 3 | ~100 | Low — variation on existing allocation patterns |
| 5 — Interactions | 4 | ~200 | Medium — edge cases, spawn analysis |
| 6 — Polish | 5 | ~150 | Low — non-functional |

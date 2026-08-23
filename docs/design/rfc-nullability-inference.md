# RFC: Nullability Inference

**Status:** Rejected
**Author:** Matt Kerian
**Date:** 2026-02-14
**Depends on:** None (Layer 1)

## Summary

Infer nullability the same way we infer errors. Functions should not annotate return types as `T?` — the compiler should infer which functions can return `none` via fixed-point analysis of the call graph, then enforce that every nullable result is handled with `?` (propagate) or a null check.

## Outcome

This proposal was rejected. Pluto keeps explicit nullability (`T?`) in signatures instead of inferring nullable return types.

Reasons for rejection:
- explicit signatures are clearer at API boundaries
- return-type stability is easier to reason about during refactors
- explicit nullability already composes with existing type checking and `?` propagation

This document is retained as design history only.

**Update (2026-08-22):** while inference stayed rejected, the two ergonomics
features specced below shipped independently: the `??` null-coalescing
operator and flow narrowing after null checks (`if x != none { ... }`,
including the `if x == none { return }` guard idiom). See
`tests/integration/nullable_ergonomics.rs` for the implemented semantics.

## Motivation

Pluto already infers error-ability. A function doesn't declare `fn foo() int ! SomeError` — the compiler walks the call graph, finds all `raise` statements, propagates error sets through callers, and enforces that every fallible call site uses `!` or `catch`. This is one of Pluto's best features.

Nullability is structurally identical, but currently explicit. You must write `fn find(id: string) User?` to declare that a function might return `none`. This creates an inconsistency:

| Feature | Declaration | Inference | Enforcement |
|---------|------------|-----------|-------------|
| Errors | Implicit (no annotation) | Inferred from `raise` statements | `!` or `catch` at call sites |
| Nullability | **Explicit** (`T?` in signatures) | **None** | `?` or null check at call sites |

The question: should nullability follow the same pattern?

**Arguments for inference:**
1. **Consistency.** The two systems are structurally identical. Having one inferred and one explicit is a surprising asymmetry.
2. **Less annotation burden.** Developers don't need to track which functions might return `none` through deep call chains.
3. **Refactoring safety.** Adding a `return none` path deep in a call chain automatically updates the inferred nullability of all callers.
4. **The same mental model.** "If it compiles, all nullable returns are handled" — just like errors.

**Arguments against inference:**
1. **Readability at the declaration site.** `fn find() User?` tells you at a glance it might return `none`. With inference, you'd need to look at the implementation.
2. **API stability.** Inferred nullability could change when internal implementation changes, breaking downstream callers.
3. **Null is more common than errors.** The inference set might be noisier.

The arguments for inference are stronger. Consistency is a first-class design principle in Pluto, and the readability concern is addressed by tooling (LSP can show inferred nullability). API stability is managed by the same mechanism as error inference — if your internal change makes a previously non-nullable function nullable, callers get a compile error, which is the correct behavior.

## Design

### Null Sources

The inference starts from **null source expressions** — places where `none` enters the type system:

| Source | Example |
|--------|---------|
| `none` literal | `return none` |
| Nullable variable | `let x: T? = ...` (from external input) |
| Map index (miss) | `map[key]` returns `T?` |
| Array `find` | `arr.find(pred)` returns `T?` |
| String `to_int` / `to_float` | Returns `int?` / `float?` |
| `?` propagation | `let val = expr?` — if expr is nullable, the enclosing function is nullable |

### Inference Algorithm

The algorithm mirrors error inference exactly:

**Phase 1: Seed.** Walk every function body. If it contains a `return none` statement or returns a null source expression directly, mark the function as **directly nullable**.

**Phase 2: Propagate.** Build the call graph. For each function that calls a directly-nullable function and propagates the null with `?`, mark the caller as nullable. Repeat (fixed-point iteration) until no new functions are marked.

**Phase 3: Enforce.** Walk every call site. If the callee is nullable, the caller must either:
- Propagate with `?` (making the caller nullable too), or
- Handle with a null check (`if result != none { ... }`, `match`, or `let val = expr ? default`)

```
// Phase 1: seed — find() directly returns none
fn find(users: [User], id: string) User {
    for user in users {
        if user.id == id {
            return user
        }
    }
    return none    // ← null source, find() is nullable
}

// Phase 2: propagate — get_name() calls find() with ?
fn get_name(users: [User], id: string) string {
    let user = find(users, id)?    // ← propagates, get_name() is nullable
    return user.name
}

// Phase 3: enforce — main() must handle nullability
fn main() {
    let users = load_users()
    let name = get_name(users, "123")    // COMPILE ERROR: nullable result not handled

    // Fix 1: propagate (if main can be nullable)
    let name = get_name(users, "123")?

    // Fix 2: handle with default
    let name = get_name(users, "123") ?? "unknown"

    // Fix 3: handle with null check
    let name_result = get_name(users, "123")
    if name_result != none {
        print(name_result)    // type narrowed to string (non-nullable) here
    }
}
```

### Signature Changes

With inference, function signatures drop the `?` annotation on return types:

```
// Before (explicit nullability)
fn find(users: [User], id: string) User? {
    ...
}

// After (inferred nullability)
fn find(users: [User], id: string) User {
    ...
    return none    // compiler infers this function is nullable
}
```

The declared return type is `User`, not `User?`. The compiler infers that the actual return type is `User?` based on the function body.

### Parameters

Nullability on **parameters** remains explicit. If a function accepts a possibly-null argument, it must declare `T?`:

```
fn greet(name: string?) {
    if name != none {
        print("hello {name}")
    } else {
        print("hello stranger")
    }
}
```

This is the same as error inference — error types are inferred on return types but explicit on parameters (parameters can't "propagate" errors, they receive them).

### Fields

Schema and class fields with nullable types remain explicit:

```
schema User {
    name: string
    nickname: string?    // explicitly nullable
}
```

Fields are data declarations, not computed values. There's nothing to infer — the field either allows `none` or it doesn't.

### Interaction with Error Inference

Nullability and error inference are independent systems that compose:

```
fn find_or_fail(db: Database, id: string) User {
    let user = db.query(id)!          // errors propagated with !
    let result = validate(user)?      // nullability propagated with ?
    return result
}
// Compiler infers: find_or_fail is both fallible AND nullable
```

At call sites, callers must handle both:
```
let user = find_or_fail(db, id)!?    // handle errors first, then null
```

### The `??` Operator

Introduce a null coalescing operator for providing defaults:

```
let name = get_name(users, id) ?? "unknown"
let count = parse_count(input) ?? 0
```

`a ?? b` evaluates to `a` if non-null, otherwise `b`. The result type is non-nullable (`T`, not `T?`).

### Type Narrowing

After a null check, the compiler narrows the type:

```
let user = find(users, id)
// user is User? here

if user != none {
    // user is User (non-nullable) here
    print(user.name)
}

// match also narrows
match find(users, id) {
    none { print("not found") }
    user { print(user.name) }    // user is User here
}
```

## Implementation

### Phase 1: Inference Pass

Add a new pass in `src/typeck/` (parallel to `infer_error_sets()` in `errors.rs`):

```
fn infer_nullable_sets(env: &mut TypeEnv, program: &Program) {
    // 1. Seed: walk all function bodies, find direct null sources
    // 2. Build call graph edges for ? propagation
    // 3. Fixed-point: propagate nullability through ? chains
    // 4. Store result in TypeEnv (fn_nullable: HashMap<String, bool>)
}
```

### Phase 2: Enforcement Pass

Add enforcement after inference (parallel to `enforce_error_handling()`):

```
fn enforce_null_handling(env: &mut TypeEnv, program: &Program) {
    // For each call site where callee is nullable:
    //   - Check that result is handled with ?, ??, null check, or match
    //   - If not, emit compile error
}
```

### Phase 3: Type Narrowing

Extend the type checker's flow-sensitive analysis to narrow `T?` to `T` after null checks:

- After `if x != none` — x is `T` in the then-branch
- After `match x { none { ... } val { ... } }` — val is `T`
- After `let val = x?` — val is `T` (none was propagated)
- After `let val = x ?? default` — val is `T`

### Migration Path

This is a **breaking change** for existing Pluto code. Migration:

1. Remove `?` from return type annotations (they become inferred)
2. Existing `?` propagation at call sites continues to work identically
3. Code that was correct before remains correct — the inference just removes the need for return type annotations

The compiler can provide a migration tool: `plutoc migrate-nullable` that removes `?` from return types and verifies the inferred types match.

## Resolved Questions

1. **Explicit `T?` on return types.** Should we allow explicit `T?` return annotations as documentation, even though they're inferred? **No.** Same policy as errors — inferred means inferred. Adding optional annotations creates two ways to do the same thing and they can get out of sync. Tooling (LSP hover, `plutoc inspect`) shows inferred nullability.

2. **Interaction with generics.** Generic functions infer nullability per monomorphization. `fn first<T>(items: [T]) T` is nullable (might be empty array) regardless of `T`. The inference works on the monomorphized copies.

3. **Extern functions.** `extern fn` declarations must explicitly declare `T?` return types since the compiler can't see the implementation. Same as extern functions explicitly declaring error types.

4. **Trait methods.** Trait method nullability is inferred from each implementation. If any impl is nullable, calls through the trait are treated as nullable. Same pattern as error inference for trait methods.

## Open Questions

- [ ] **Recursive nullability.** How does inference handle mutually recursive functions where one might return `none`? Same as error inference — fixed-point iteration handles cycles.
- [ ] **Null in collections.** Should `[T?]` (array of nullable) be different from a nullable array? Yes — `[T?]` has nullable elements, the array itself is non-null. These are independent.
- [ ] **Performance.** Does adding a second fixed-point pass (nullable + errors) meaningfully impact compile times? Likely negligible — the call graph is already built for error inference, and the nullable pass reuses it.

# Bug Report: Generic Error Inference Skips Generic Functions

**Date Reported:** 2026-02-13
**Reporter:** Codex
**Priority:** P1
**Component:** Type Checker
**Status:** Fixed

---

## Summary

Error inference does not mark generic functions as fallible, so unhandled errors in generic call chains can compile.

## Impact

Generic APIs that raise or propagate errors can silently bypass compile-time error-handling enforcement.

- **Severity:** High
- **Frequency:** Often
- **Workaround available:** Partial
- **Blocks real projects:** Yes (generic utility APIs and wrappers)

## Reproduction

### Minimal Example

```pluto
error E{}

fn id<T>(x:T)T{
    raise E{}
    return x
}

fn main(){
    let y = id(1)
    print(y)
}
```

### Steps to Reproduce

1. Run `cargo test --test typeck_generic_error_sets generic_fn_raises_no_handler -- --ignored`
2. Observe test failure indicating compilation succeeded when it should fail.

### Expected Behavior

Compilation should fail because `id(1)` is fallible and is called without `!` or `catch`.

### Actual Behavior

Compilation succeeds (false negative); the ignored regression test fails with "Compilation should have failed".

## Environment

- **Pluto Commit:** `2ec0425`
- **OS:** macOS (Darwin)
- **Architecture:** arm64
- **Rust Version:** `rustc 1.93.0 (254b59607 2026-01-19)`

## Error Messages

```
thread 'generic_fn_raises_no_handler' ... panicked ...
Compilation should have failed
```

## Analysis (Optional)

- **Suspected root cause:** Generic declarations are skipped during error-set collection.
- **Related code:**
  - `src/typeck/errors.rs:15` (skip generic top-level functions)
  - `src/typeck/errors.rs:24` (skip generic class methods)
  - `src/lib.rs:60` and `src/lib.rs:62` (error inference runs before monomorphization)
- **Related test:** `tests/typeck/errors/generic_error_sets.rs:10`

## Workaround (If Available)

```pluto
// Avoid unhandled error flow through generic call boundaries.
// Move fallible behavior to non-generic paths, or handle with catch inside generic bodies.
```

## Additional Context

This is a semantic soundness gap: compile-time enforcement says call is infallible when instantiated behavior is fallible.

---

## Investigation Notes

Root cause confirmed as reported: `infer_error_sets` and
`enforce_error_handling` skipped all generic declarations.

Fix: effect collection now includes generic templates under their template
names (direct raises and named-call edges are type-independent); template
error sets are copied onto instance-mangled names after the fixed point and
during monomorphization; template bodies are enforced in lenient mode
(unhandled fallible named calls rejected; unresolved method calls skipped).
Remaining gap: fallibility does not flow through fn-typed values (effect
typing) — tests for that stay ignored.

---

## Fix Status

*(This section will be filled when bug is fixed)*

- **Fixed date:** 2026-08-21
- **Branch:** generic-error-inference
- **Pull request:** #265
- **Status:** Fixed

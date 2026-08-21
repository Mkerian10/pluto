# Bug Report: Closure Call Fallibility Not Inferred

**Date Reported:** 2026-02-13
**Reporter:** Codex
**Priority:** P1
**Component:** Type Checker
**Status:** Fixed

---

## Summary

Calls through closure variables are treated as infallible even when the closure body raises errors.

## Impact

Error handling for closure-based APIs is rejected or misclassified, blocking normal `catch`/`!` usage on closure calls.

- **Severity:** High
- **Frequency:** Often
- **Workaround available:** Yes
- **Blocks real projects:** Yes (callbacks, higher-order utilities, closure-heavy code)

## Reproduction

### Minimal Example

```pluto
error ClosureError { value: int }

fn main() {
    let threshold = 10
    let check = (x: int) => {
        if x > threshold {
            raise ClosureError { value: x }
        }
        return x * 2
    }

    let a = check(5) catch 0
    print(a)
}
```

### Steps to Reproduce

1. Run `cargo test --test codegen_tests _06_error_handling::test_raise_error_in_closure -- --ignored`
2. Observe compile failure:
   `Type error: catch applied to infallible function 'check'`

### Expected Behavior

`check(...) catch ...` should compile because `check` can raise `ClosureError`.

### Actual Behavior

Compiler reports the closure call as infallible and rejects `catch`.

## Environment

- **Pluto Commit:** `2ec0425`
- **OS:** macOS (Darwin)
- **Architecture:** arm64
- **Rust Version:** `rustc 1.93.0 (254b59607 2026-01-19)`

## Error Messages

```
Compilation failed: Type error: catch applied to infallible function 'check'
```

## Analysis (Optional)

- **Suspected root cause:**
  - Call-site enforcement for `Expr::Call` checks named function fallibility only.
  - Closure variables are not represented in `fn_errors` fallibility checks.
  - Error inference runs before closure lifting, so lifted closure functions are not part of the inferred graph.
- **Related code:**
  - `src/typeck/errors.rs:607` (`env.is_fn_fallible(&name.node)` for calls)
  - `src/typeck/errors.rs:385` (closure body effects collected, but not closure-call binding)
  - `src/lib.rs:60` and `src/lib.rs:66` (typecheck/error inference before closure lifting)
- **Related test:** `tests/codegen/_06_error_handling.rs:84`

## Workaround (If Available)

```pluto
// Replace fallible closures with named functions/methods and call those directly.
```

## Additional Context

This is currently acknowledged in ignored tests as a pipeline/timing issue around closure handling.

---

## Investigation Notes

Root cause confirmed as reported: call-site enforcement only consulted
`env.is_fn_fallible(name)` (named functions), and `Expr::Closure` bodies were
absorbed into the enclosing function's direct error set — so defining a
fallible closure marked the definer fallible while calling it was treated as
infallible.

Fix: closures bound to variables get their own node in the error-inference
graph (`<closure@span>` keys in `fn_errors`), tracked lexically during effect
collection. Call sites through closure variables are recorded in
`env.closure_call_sites` and enforced like named calls; `!` adds an edge to the
closure node; typed-catch coverage reads the closure's error set. A closure
that escapes (referenced outside call position, passed inline, or returned)
conservatively absorbs into the enclosing function, preserving the old
behavior for pass-away closures. Reassignments union into the variable's node.
No codegen changes were needed — catch/propagate lowering is expression-
agnostic.

---

## Fix Status

*(This section will be filled when bug is fixed)*

- **Fixed date:** 2026-08-21
- **Branch:** closure-error-inference
- **Status:** Fixed

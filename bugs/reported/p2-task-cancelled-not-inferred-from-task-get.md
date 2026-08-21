# Feature Request: Infer TaskCancelled From Task.get() After cancel()

**Date Reported:** 2026-02-13
**Reporter:** Codex
**Priority:** P2
**Component:** Type Checker
**Status:** Reported

---

## Summary

`TaskCancelled` exists in the error catalog, but error inference does not currently model cancellation as a fallible outcome of `task.get()`.

## Impact

Code that cancels tasks cannot cleanly model/handle cancellation unless the spawned function is made artificially fallible.

- **Severity:** Medium
- **Frequency:** Occasionally
- **Workaround available:** Yes
- **Blocks real projects:** No (but causes awkward patterns in cancellation paths)

## Reproduction

### Minimal Example

```pluto
fn work() int {
    return 42
}

fn main() {
    let t = spawn work()
    t.cancel()
    let result = t.get() catch -1
    print(result)
}
```

### Steps to Reproduce

1. Compile the program.
2. Observe that `t.get()` is treated as infallible when spawn origin is infallible.
3. Attempting `catch`/`!` on `t.get()` is rejected unless `work()` is made syntactically fallible.

### Expected Behavior

After `cancel()`, `get()` should be inferable as fallible with `TaskCancelled`, or the language should provide an explicit cancellation-aware API.

### Actual Behavior

`Task.get()` fallibility is based only on spawned function error set, not cancellation state.

## Environment

- **Pluto Commit:** `2ec0425`
- **OS:** macOS (Darwin)
- **Architecture:** arm64
- **Rust Version:** `rustc 1.93.0 (254b59607 2026-01-19)`

## Error Messages

```
Typical rejection when adding handler on infallible origin:
Type error: catch applied to infallible method 'get'
```

## Analysis (Optional)

- **Current behavior in code:**
  - `TaskCancelled` is seeded in environment: `src/typeck/mod.rs:75`
  - `Task.cancel()` is modeled infallible: `src/typeck/env.rs:353`
  - `Task.get()` fallibility is only `spawned_fn`-based: `src/typeck/env.rs:342-346`
  - Error inference does not add `TaskCancelled` for cancel/get paths: `src/typeck/errors.rs:301`
- **Related test context:** `tests/integration/concurrency.rs:860-868` documents workaround by making `work()` artificially fallible so `.get() catch ...` is allowed.

## Workaround (If Available)

```pluto
error WorkError { message: string }

fn work() int {
    // make function syntactically fallible so get() supports catch
    if false {
        raise WorkError { message: "never" }
    }
    return 42
}
```

## Additional Context

This may be either a missing inference rule or a language-design gap; report is filed as a feature request to clarify intended cancellation semantics.

---

## Investigation Notes

*(This section will be filled during investigation phase)*

---

## Fix Status

*(This section will be filled when bug is fixed)*

- **Fixed in commit:** [commit hash]
- **Fixed date:** YYYY-MM-DD
- **Branch:** [branch name]
- **Pull request:** [if applicable]

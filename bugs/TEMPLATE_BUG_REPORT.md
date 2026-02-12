# Bug Report: [Short Descriptive Title]

**Date Reported:** YYYY-MM-DD
**Reporter:** [Your name or "User"]
**Priority:** P0 | P1 | P2 | P3
**Component:** Lexer | Parser | Type Checker | Codegen | Runtime | Stdlib | CLI
**Status:** Reported

---

## Summary

One-sentence description of the bug.

## Impact

Who is affected and how? What use cases does this block?

- **Severity:** Critical | High | Medium | Low
- **Frequency:** Always | Often | Occasionally | Rare
- **Workaround available:** Yes | No
- **Blocks real projects:** Yes | No (if yes, list which projects)

## Reproduction

### Minimal Example

```pluto
// Smallest possible code that reproduces the bug
fn main() {
    // ...
}
```

### Steps to Reproduce

1. Create file `test.pluto` with the code above
2. Run `cargo run -- compile test.pluto`
3. Observe error: [paste exact error message]

### Expected Behavior

What should happen?

### Actual Behavior

What actually happens?

## Environment

- **Pluto Commit:** [git commit hash]
- **OS:** macOS | Linux | Windows
- **Architecture:** aarch64 | x86_64
- **Rust Version:** [rustc --version]

## Error Messages

```
[Paste complete error output, including stack traces if applicable]
```

## Analysis (Optional)

If you've done any preliminary investigation:

- **Suspected root cause:** [File, function, line if known]
- **Related code:** [Links to relevant source files]
- **Similar bugs:** [Links to related issues]

## Workaround (If Available)

```pluto
// Alternative code that works around the bug
```

## Additional Context

Any other information that might be helpful:
- When did this start happening?
- Does this work in an older commit?
- Are there any related bugs or features?
- Screenshots, logs, or other artifacts

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

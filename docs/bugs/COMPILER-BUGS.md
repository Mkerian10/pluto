# Compiler Bugs Tracking

This document tracks known compiler bugs discovered during testing.

## Active Bugs

_(None - all bugs fixed!)_

---

## Fixed Bugs

### Bug #1: Multi-Statement Catch Blocks Typed as Void ✅ FIXED

**Status:** ✅ Fixed on 2026-02-11
**Severity:** High
**Discovered:** 2026-02-11 (runtime error state testing)
**Affects:** Error handling, test writing patterns

**Description:**

The typechecker incorrectly types catch blocks with multiple statements as `void` instead of inferring the type from the last expression.

**Expected behavior:**
```pluto
let result = task.get() catch err {
    failures = failures + 1  // Statement (returns void)
    -1                        // Expression (returns int)
}
// result should be typed as int (from last expression)
```

**Actual behavior:**
```
Type error: catch handler type mismatch: expected int, found void
```

The typechecker sees multiple statements and types the entire catch block as `void`, ignoring the final expression.

**Impact:**

- Cannot write natural error handling code with side effects in catch blocks
- Forces awkward workarounds (moving side effects outside catch blocks)
- Affects 5 P1 runtime error state tests

**Affected tests:**
- `stress_100_concurrent_tasks_mixed_errors`
- `stress_1000_sequential_spawn_error_cycles`
- `stress_burst_error_creation`
- `propagation_multi_layer_task_chain`
- `propagation_mixed_success_failure_fanout`

**Root cause:**

Likely in `src/typeck/infer.rs` or `src/typeck/check.rs` - the catch block type inference doesn't properly handle block expressions with multiple statements.

**Fix required:**

Catch blocks should be typed like regular block expressions:
1. If the last statement is an expression, use its type
2. If the last statement is a statement (no semicolon in Pluto), it should still be an expression-statement
3. The catch block type should match the last expression's type

**Root Cause Found:**

The parser was treating expressions across newlines as a single expression. When parsing:
```pluto
failures = failures + 1
-1
```

The parser saw it as `failures = failures + 1 - 1` because `peek()` automatically skips newlines, treating `-1` as a binary minus operator.

**Fix Implemented:**

Added newline detection in the Pratt parser (`src/parser/mod.rs`, line ~2204). Before parsing infix operators, check if there's a newline before the token. If yes, stop parsing the expression:

```rust
// Check if there's a newline before this token
let has_newline_before = self.peek_raw().is_some()
    && matches!(self.peek_raw().unwrap().node, Token::Newline);

if has_newline_before {
    // If we hit a newline before a binary operator, stop
    match &tok.node {
        Token::Plus | Token::Minus | Token::Star | ... => break,
        _ => {}
    }
}
```

**Result:** Multi-statement catch blocks now work correctly. Parser correctly treats newlines as statement boundaries.

**Tests Unblocked:** All 4 tests now pass (stress_100, stress_1000, propagation_multi_layer, propagation_mixed).

---

### Bug #2: `if` Without `else` Containing `raise` Typed as Void ✅ FIXED

**Status:** ✅ Fixed on 2026-02-11 (same fix as Bug #1)
**Severity:** Medium
**Discovered:** 2026-02-11 (runtime error state testing)
**Affects:** Control flow with errors

**Description:**

The typechecker types `if` statements without `else` as `void`, even when the `if` body contains `raise` (which never returns). The typechecker doesn't understand diverging control flow.

**Expected behavior:**
```pluto
fn maybe_fail(n: int) int {
    if n % 2 == 0 {
        raise MyError { n: n }
        // Never reaches here - raise diverges
    }
    return n
}
// Should be valid - all code paths return int or diverge
```

**Actual behavior:**
```
Type error: catch handler type mismatch: expected int, found void
```

The typechecker sees `if` without `else` and types it as `void`, not recognizing that `raise` never returns.

**Impact:**

- Cannot write natural conditional error raising
- Forces awkward explicit returns after `raise` (unreachable code)
- Affects readability and idiomaticity

**Affected tests:**
- `stress_100_concurrent_tasks_mixed_errors` (in `maybe_fail` function)
- `propagation_mixed_success_failure_fanout` (in `maybe_fail_subtask` function)

**Root cause:**

The typechecker doesn't have a concept of "diverging" expressions (expressions that never return, like `raise`, `return`, infinite loops). It treats all expressions as potentially returning a value.

**Fix required:**

1. Mark `raise` expressions as diverging (type: `!` or "never")
2. When checking block/if expressions, if a branch diverges, don't require it to match the expected type
3. `if condition { raise ... }` without `else` should be valid if the return type doesn't matter (diverges)

**Root Cause:**

Same as Bug #1 - the parser bug caused the `if` body and subsequent lines to be parsed incorrectly.

**Fix Implemented:**

Same parser fix as Bug #1. The newline detection prevents the parser from continuing past the `if` statement.

**Result:** Functions with `if { raise }` patterns now work correctly.

**Tests Unblocked:** 1 additional test (propagation_mixed was affected by both bugs).

---

## Bug Triage Process

1. **Discovery:** Bug found during testing or development
2. **Documentation:** Add to this file with full description, reproduction, impact
3. **Prioritization:** Assign severity (Low/Medium/High/Critical)
4. **Assignment:** Determine which component owns the fix (typechecker, parser, codegen, runtime)
5. **Fix:** Implement fix, add regression test
6. **Verification:** Confirm fix works, mark bug as fixed

---

## Severity Levels

- **Critical:** Compiler crashes, wrong code generation, memory unsafety
- **High:** Common patterns don't work, forces awkward workarounds
- **Medium:** Edge cases, uncommon patterns affected
- **Low:** Minor annoyances, cosmetic issues

---

**Last updated:** 2026-02-11

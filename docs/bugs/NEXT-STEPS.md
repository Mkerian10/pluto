# Next Steps: Runtime Error Testing & Compiler Bugs

**Date:** 2026-02-11
**Status:** 4 tests blocked by 2 compiler bugs

## Current State

### Tests Implemented: 21 total
- ✅ **17 passing** - Runtime is solid, zero bugs found
- 🔴 **4 blocked** - Compiler bugs prevent these from running

### Passing Tests (17)
**P0 - Core Validation (13 tests)**
- TLS isolation: 5 tests ✅
- Error GC: 3 tests ✅
- Error lifecycle: 3 tests ✅
- Edge cases: 2 tests ✅

**P1 - Stress & Propagation (4 tests)**
- `stress_rapid_spawn_under_error_load` ✅
- `stress_error_object_field_diversity` ✅
- `stress_burst_error_creation` ✅
- `propagation_task_fanout_all_fail` ✅

### Blocked Tests (4)
**All blocked by Compiler Bug #1 (multi-statement catch blocks)**
1. `stress_100_concurrent_tasks_mixed_errors` - 100 tasks, mixed errors
2. `stress_1000_sequential_spawn_error_cycles` - 1000 sequential cycles
3. `propagation_multi_layer_task_chain` - 3-layer task chain
4. `propagation_mixed_success_failure_fanout` - Also affected by Bug #2

## Compiler Bugs to Fix

### Bug #1: Multi-Statement Catch Blocks Typed as Void (HIGH PRIORITY)

**File:** `docs/bugs/COMPILER-BUGS.md` Bug #1
**Blocks:** 4 tests
**Severity:** HIGH

**Problem:**
```pluto
let result = task.get() catch err {
    failures = failures + 1  // Assignment (void)
    -1                        // Expression (int)
}
// Should be typed as int, but typechecker sees void
```

**Error:** `Type error: catch handler type mismatch: expected int, found void`

**Fix needed:**
- Location: `src/typeck/infer.rs` or `src/typeck/check.rs`
- Catch blocks are block expressions, should use last expression's type
- Similar to how function bodies work

**Test after fix:**
```bash
cargo test --test runtime_error_state stress_100_concurrent_tasks_mixed_errors
cargo test --test runtime_error_state stress_1000_sequential_spawn_error_cycles
cargo test --test runtime_error_state propagation_multi_layer_task_chain
cargo test --test runtime_error_state propagation_mixed_success_failure_fanout
```

### Bug #2: `if` Without `else` Containing `raise` Typed as Void (MEDIUM PRIORITY)

**File:** `docs/bugs/COMPILER-BUGS.md` Bug #2
**Blocks:** 1 test (also blocked by Bug #1)
**Severity:** MEDIUM

**Problem:**
```pluto
fn maybe_fail(n: int) int {
    if n % 2 == 0 { raise MyError { n: n } }
    // raise never returns, but typechecker sees if as void
    return n
}
```

**Fix needed:**
- Location: `src/typeck/infer.rs`
- Implement "never" type (`!`) for diverging expressions
- Mark `raise`, `return` as diverging
- If branch diverges, don't require it to match expected type

**Test after fix:**
```bash
cargo test --test runtime_error_state propagation_mixed_success_failure_fanout
```

## Action Plan

### Phase 1: Fix Bug #1 (Multi-Statement Catch Blocks)
**Priority:** CRITICAL
**Estimated effort:** 4-8 hours
**Expected result:** 3/4 blocked tests will pass

**Steps:**
1. Read `src/typeck/infer.rs` - find catch block type inference
2. Locate where catch handler blocks are typed
3. Change to use last expression's type (like function bodies)
4. Run blocked tests to verify fix
5. Run full test suite to ensure no regressions

### Phase 2: Fix Bug #2 (Diverging Control Flow)
**Priority:** HIGH
**Estimated effort:** 8-16 hours
**Expected result:** 4/4 blocked tests will pass

**Steps:**
1. Add `PlutoType::Never` to type system
2. Mark `raise` and `return` as returning `Never`
3. Update type checking for `if` expressions:
   - If condition branch is `Never`, don't require type match
   - If both branches are `Never`, result is `Never`
   - If one branch is `Never`, use other branch's type
4. Run blocked tests to verify fix
5. Run full test suite to ensure no regressions

### Phase 3: Validation
**After both bugs fixed**

1. All 21 runtime error state tests should pass:
   ```bash
   cargo test --test runtime_error_state
   # Expected: 21 passed; 0 failed
   ```

2. Update documentation:
   - Mark bugs as fixed in `docs/bugs/COMPILER-BUGS.md`
   - Update `docs/testing/runtime-error-testing-results.md`
   - Celebrate! 🎉

3. Continue with P2 tests (feature interactions):
   - Error + channel blocking
   - Error + map concurrent iteration
   - Error + contract invariant
   - Error + nullable interaction

## Success Criteria

✅ **Phase 1 Complete When:**
- Bug #1 fixed
- 20/21 tests passing (only propagation_mixed blocked by Bug #2)
- No regressions in other tests

✅ **Phase 2 Complete When:**
- Bug #2 fixed
- 21/21 tests passing
- No regressions in other tests

✅ **Phase 3 Complete When:**
- All 21 tests passing
- Documentation updated
- Ready for P2 tests

## Files to Read

**Typechecker:**
- `src/typeck/infer.rs` - Type inference, likely where catch blocks are typed
- `src/typeck/check.rs` - Type checking, may have catch block logic
- `src/typeck/env.rs` - Type environment
- `src/typeck/types.rs` - PlutoType definitions

**Error handling:**
- `src/parser/ast.rs` - AST definitions for catch blocks
- `src/codegen/lower.rs` - How catch blocks are lowered

**Tests:**
- `tests/integration/runtime_error_state.rs` - The 21 tests (lines 550-960 are P1 tests)

## Questions for Investigation

1. **Where are catch blocks typed?**
   - Search for "catch" in `src/typeck/infer.rs`
   - Look for `Expr::Catch` or similar

2. **How are block expressions typed?**
   - Find how function bodies determine their return type
   - Apply same logic to catch blocks

3. **Does Pluto have a `Never` type?**
   - Search for "never" or "!" in `src/typeck/types.rs`
   - If not, need to add it

4. **How are `raise` expressions currently typed?**
   - Search for "raise" in `src/typeck/infer.rs`
   - Probably typed as `void` or the error type

---

**Bottom line:** We have excellent tests (21 total), a solid runtime (zero bugs), but 2 compiler bugs blocking 4 tests. Fix the bugs, celebrate, continue with P2 tests.

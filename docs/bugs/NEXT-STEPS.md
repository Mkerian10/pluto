# Next Steps: Runtime Error Testing & Compiler Bugs

**Date:** 2026-02-11
**Status:** ✅ ALL BUGS FIXED - All 21 tests passing!

## Current State

### Tests Implemented: 21 total
- ✅ **21 passing** - Runtime is solid, zero bugs found
- ✅ **All compiler bugs fixed** - Parser now respects newlines as statement boundaries

### All Tests Passing (21)
**P0 - Core Validation (13 tests)**
- TLS isolation: 5 tests ✅
- Error GC: 3 tests ✅
- Error lifecycle: 3 tests ✅
- Edge cases: 2 tests ✅

**P1 - Stress & Propagation (8 tests)**
- `stress_rapid_spawn_under_error_load` ✅
- `stress_error_object_field_diversity` ✅
- `stress_burst_error_creation` ✅
- `propagation_task_fanout_all_fail` ✅
- `stress_100_concurrent_tasks_mixed_errors` ✅ (unblocked!)
- `stress_1000_sequential_spawn_error_cycles` ✅ (unblocked!)
- `propagation_multi_layer_task_chain` ✅ (unblocked!)
- `propagation_mixed_success_failure_fanout` ✅ (unblocked!)

## Compiler Bugs - ✅ ALL FIXED!

### Bug #1: Multi-Statement Catch Blocks Typed as Void ✅ FIXED

**File:** `docs/bugs/COMPILER-BUGS.md` Bug #1
**Status:** ✅ Fixed in commit 384ea61
**Unblocked:** 4 tests

**Root Cause:**
Parser was treating expressions across newlines as a single expression because `peek()` skips newlines.

**Fix Applied:**
- Location: `src/parser/mod.rs` (Pratt parser)
- Added newline detection before parsing infix operators
- Parser now checks `peek_raw()` for newlines and stops expression parsing when newline precedes binary operator

**Verification:**
```bash
cargo test --test runtime_error_state
# Result: 21 passed; 0 failed ✅
```

### Bug #2: `if` Without `else` Containing `raise` Typed as Void ✅ FIXED

**File:** `docs/bugs/COMPILER-BUGS.md` Bug #2
**Status:** ✅ Fixed by same parser fix (commit 384ea61)
**Unblocked:** 1 test (propagation_mixed_success_failure_fanout)

**Root Cause:**
Same as Bug #1 - parser bug, not a diverging control flow issue.

**Fix Applied:**
Same parser fix in `src/parser/mod.rs` resolved both bugs.

## ✅ Completed Phases

### Phase 1: Fix Bug #1 (Multi-Statement Catch Blocks) ✅ COMPLETE
**Status:** ✅ Fixed in commit 384ea61
**Actual approach:** Parser fix, not typechecker change
**Result:** 20/21 tests passing

**What was done:**
1. ✅ Investigated root cause using debug output
2. ✅ Discovered parser was treating newlines as whitespace in expressions
3. ✅ Added newline detection in `src/parser/mod.rs`
4. ✅ Verified fix with minimal test cases
5. ✅ Ran full test suite - no regressions

### Phase 2: Fix Bug #2 (Diverging Control Flow) ✅ COMPLETE
**Status:** ✅ Fixed by same parser fix
**Result:** 21/21 tests passing

**What was done:**
Bug #2 was actually the same root cause as Bug #1. The parser fix resolved both bugs simultaneously.

### Phase 3: Validation ✅ COMPLETE

1. ✅ All 21 runtime error state tests passing:
   ```bash
   cargo test --test runtime_error_state
   # Result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   ```

2. ✅ Documentation updated:
   - `docs/bugs/COMPILER-BUGS.md` - Both bugs marked as fixed
   - `docs/testing/runtime-error-testing-results.md` - Updated to 21/21 passing
   - `docs/bugs/NEXT-STEPS.md` - This file

3. ✅ PR created and merged:
   - PR #34: Fix parser bug: respect newlines as statement boundaries
   - Merged to master

## Next: P2 Tests (Feature Interactions)

Ready to implement P2 tests for feature interactions:
- Error + channel blocking
- Error + map concurrent iteration
- Error + contract invariant
- Error + nullable interaction

## Success Criteria - ✅ ALL MET

✅ **Phase 1 Complete:**
- ✅ Bug #1 fixed
- ✅ 20/21 tests passing
- ✅ No regressions in other tests

✅ **Phase 2 Complete:**
- ✅ Bug #2 fixed (same fix as Bug #1)
- ✅ 21/21 tests passing
- ✅ No regressions in other tests

✅ **Phase 3 Complete:**
- ✅ All 21 tests passing
- ✅ Documentation updated
- ✅ PR merged to master
- ✅ Ready for P2 tests

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

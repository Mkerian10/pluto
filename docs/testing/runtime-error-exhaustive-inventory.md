# Exhaustive Test Inventory: Runtime Error State

**Date:** 2026-02-11
**Analysis Type:** Comprehensive audit of all error-related tests

## Summary

**Total test files analyzed:** 57
**Test files with error handling:** 38
**Concurrent error tests found:** 10
**GC + error tests found:** 0 ❌
**TLS verification tests found:** 0 ❌
**Multiple concurrent error types:** 1 (in traits.rs, but not concurrent)

## Runtime Error Handling Implementation

From `runtime/builtins.c`:

```c
// Line 92: Thread-local error state
__thread void *__pluto_current_error = NULL;

// Line 1601-1614: Core error API
void __pluto_raise_error(void *error_obj) {
    __pluto_current_error = error_obj;
}

long __pluto_has_error() {
    return __pluto_current_error != NULL ? 1 : 0;
}

void *__pluto_get_error() {
    return __pluto_current_error;
}

void __pluto_clear_error() {
    __pluto_current_error = NULL;
}

// Line 3224: New thread initialization
__pluto_current_error = NULL;  // clean TLS for new thread

// Line 623-624: GC integration
if (__pluto_current_error) {
    gc_mark_candidate(__pluto_current_error);
}
```

**Key insight:** Error state is TLS (`__thread`), cleaned on thread spawn (line 3224), and GC-tracked (line 623).

## Existing Test Breakdown

### Category 1: Basic Error Tests (errors.rs - 48 tests)

| Test | What it tests | Concurrency? |
|------|---------------|--------------|
| `error_catch_shorthand_on_error` | Catch with default value | No |
| `error_catch_shorthand_no_error` | Catch when no error | No |
| `error_catch_wildcard` | Catch with error binding | No |
| `error_propagation_then_catch` | `!` propagation | No |
| `error_transitive_propagation` | Multi-layer `!` | No |
| `error_code_after_propagation_skipped` | Early return on `!` | No |
| `error_conditional_raise` | If/else raise | No |
| `error_no_fields` | Empty error struct | No |
| `error_with_string_field` | Error with heap field | No |
| `error_multiple_types` | 2+ error types in one program | No |
| `error_propagation_in_main` | Error in main | No |
| `error_catch_both_paths` | Success and failure paths | No |
| `error_multiple_catches_in_sequence` | Sequential catches | No |
| `error_in_while_loop` | Error in loop | No |
| `error_with_class_return` | Error from method | No |
| ... (33 more basic tests) | Various error scenarios | No |

**Gaps in basic tests:**
- ❌ No test for error object GC (10K errors without OOM)
- ❌ No test for error with very large string fields (memory pressure)
- ❌ No test for error field memory layout validation

### Category 2: Concurrent Error Tests (concurrency.rs - 7 tests)

| Test Name | Lines | What it Tests | # Tasks | # Error Types | Verified |
|-----------|-------|---------------|---------|---------------|----------|
| `spawn_error_propagation` | 110-135 | Task error → main via `!` | 1 | 1 (MathError) | ✅ |
| `spawn_error_catch` | 138-158 | Catch error from task | 1 | 1 (MathError) | ✅ |
| `stress_tasks_with_errors` | 716-758 | 10 tasks, mixed success/fail | 10 | 1 (ComputeError) | ✅ |
| `detach_with_error_does_not_crash` | ~800s | Detached task error | 1 | 1 | ✅ |
| `spawn_method_call_error_propagation` | 1062-1093 | Method error in task | 1 | 1 | ✅ |
| `spawn_method_call_error_caught` | 1093-1120 | Catch method error | 1 | 1 | ✅ |
| `sync_error_in_synchronized_method` | sync.rs | Error in sync method | N/A | 1 | ✅ |

**Analysis:**
- ✅ Basic error propagation from tasks
- ✅ Basic error catching
- ⚠️ Stress test only uses 10 tasks (industry standard: 100-1000)
- ❌ All tests use **single error type** - no TLS isolation validation
- ❌ No test verifies error state cleared after catch
- ❌ No test verifies no cross-contamination between tasks

### Category 3: Deterministic/Scheduler Tests (deterministic.rs)

| Test Name | What it Tests | Scheduler | # Tasks |
|-----------|---------------|-----------|---------|
| `sequential_spawn_error_propagation` | Error in sequential scheduler | Sequential | 1 |
| `round_robin_spawn_with_error` | Error in RR scheduler | Round-robin | 3 |
| `rr_task_error_propagation` | RR error with `!` | Round-robin | 1 |
| `rr_task_error_catch` | RR error catch | Round-robin | 1 |
| `seq_task_error_catch_inline` | Sequential inline catch | Sequential | 1 |
| `rand_error_propagation` | Random scheduler error | Random | 1 |
| `pattern_task_error_recovery` | Pattern-based error | Pattern | 1 |

**Analysis:**
- ✅ Error handling tested across all scheduler types
- ❌ All use **single error type**
- ❌ No concurrent different errors in deterministic mode

### Category 4: Trait Error Tests (traits.rs)

| Test Name | What it Tests | Concurrent? |
|-----------|---------------|-------------|
| `trait_method_raises_multiple_error_types` | Trait method raises NotFound OR InvalidInput | No |

**Analysis:**
- ✅ Multiple error types in ONE test (NotFound, InvalidInput)
- ❌ But NOT concurrent - all errors in main thread
- **Gap:** This is the ONLY test that uses 2+ different error types

### Category 5: GC Tests (gc.rs - 14 tests)

| Test Name | What it Tests | Error Objects? |
|-----------|---------------|----------------|
| `gc_string_pressure` | String GC under loop | No |
| `gc_class_allocation_loop` | Class GC | No |
| `gc_array_of_classes` | Array + class GC | No |
| `gc_closure_captures_survive` | Closure GC | No |
| `gc_enum_allocation_pressure` | Enum GC | No |
| ... (9 more) | Various GC scenarios | No |

**Critical Gap:**
- ❌ **ZERO tests** verify error objects are GC'd correctly
- ❌ No test for 10K error allocations (memory leak detection)
- ❌ No test for errors with heap fields under GC pressure

### Category 6: Other Files with Error Mentions

| File | # Error Mentions | Notable Tests |
|------|------------------|---------------|
| channels.rs | 27 | Errors with channel operations (not isolated) |
| sync.rs | 23 | 1 test: `sync_error_in_synchronized_method` |
| nullable.rs | 4 | Nullable + error interactions (basic) |
| fs.rs | 7 | File system error handling |
| contracts.rs | 2 | Contract violations (not catchable errors) |

## Critical Gaps Summary

### Gap 1: TLS Isolation Verification ❌ CRITICAL

**What exists:** Nothing explicitly verifies TLS isolation
**What's needed:**
1. Test with 2+ concurrent tasks raising **different** error types
2. Verify error from Task A doesn't appear in Task B
3. Sequential `.get()` calls verify no cross-contamination

**Why critical:** If TLS is broken, errors leak between tasks (data corruption)

### Gap 2: High Concurrency Stress ⚠️ IMPORTANT

**What exists:** `stress_tasks_with_errors` with 10 tasks
**What's needed:**
1. 100+ concurrent tasks with mixed errors
2. 1000+ sequential spawn-error-catch cycles
3. Rapid task spawning under error load

**Why important:** Race conditions in TLS management only appear under high load

### Gap 3: Error Object Memory Safety ❌ CRITICAL

**What exists:** Zero GC tests for error objects
**What's needed:**
1. 10K error allocations without OOM
2. Errors with large heap fields (strings, arrays)
3. Valgrind/ASan runs specifically for error handling

**Why critical:** Memory leaks in error handling compound over time

### Gap 4: Error Lifecycle ❌ CRITICAL

**What exists:** No tests for error state cleanup
**What's needed:**
1. Verify `__pluto_clear_error()` called after catch
2. Verify clean state on task exit
3. Verify clean state on task cancel

**Why critical:** Stale error state causes false positives

### Gap 5: Multi-Layer Error Propagation ⚠️ IMPORTANT

**What exists:** Basic multi-layer in `errors.rs` (single-threaded)
**What's needed:**
1. Task A spawns Task B spawns Task C, error propagates through all
2. Task fan-out (1→10 subtasks, all fail)

**Why important:** Complex task hierarchies are real-world scenarios

### Gap 6: Error + Feature Interactions ⚠️ NICE-TO-HAVE

**What exists:** Scattered tests (channels, sync, nullable)
**What's needed:**
1. Error during channel blocking operation
2. Error during map/set concurrent iteration
3. Error in contract invariant check
4. Error + nullable (`T?` returning from erroring task)

**Why nice-to-have:** Edge cases, less likely to hit in practice

## Quantitative Gap Analysis

| Test Category | Current | Needed | % Complete |
|---------------|---------|--------|------------|
| TLS Isolation | 0 | 5 | 0% |
| High Concurrency (100+ tasks) | 0 | 3 | 0% |
| Error GC | 0 | 3 | 0% |
| Error Lifecycle | 0 | 3 | 0% |
| Multi-Layer Concurrent | 1 | 2 | 33% |
| Feature Interactions | 3 | 4 | 43% |
| **TOTAL** | **4** | **20** | **17%** |

**Current coverage:** 17% of runtime error testing
**Target coverage:** 100% (24 tests total)
**Gap:** 20 new tests needed

## Test Implementation Priority

### P0 (Must Have - Correctness)

1. ✅ **TLS Isolation Tests** (5 tests)
   - Concurrent different error types
   - No cross-contamination verification
   - Main thread error isolation

2. ✅ **Error GC Tests** (3 tests)
   - 10K error allocations
   - Large heap fields in errors
   - Valgrind clean run

3. ✅ **Error Lifecycle Tests** (3 tests)
   - Error cleared after catch
   - Clean state on task exit
   - Clean state on cancel

### P1 (Should Have - Robustness)

4. **High Concurrency Stress** (3 tests)
   - 100+ concurrent tasks
   - 1000+ sequential cycles
   - Rapid spawn under error load

5. **Multi-Layer Propagation** (2 tests)
   - A→B→C task chain with error
   - 1→10 task fan-out with errors

### P2 (Nice to Have - Edge Cases)

6. **Feature Interactions** (4 tests)
   - Error + channel blocking
   - Error + map iteration
   - Error + contracts
   - Error + nullable

## Next Steps

1. ✅ Complete this exhaustive inventory
2. ⏭️ Implement P0 tests (11 tests) - **START HERE**
3. ⏭️ Implement P1 tests (5 tests)
4. ⏭️ Implement P2 tests (4 tests)
5. ⏭️ Run all tests with valgrind/helgrind/tsan
6. ⏭️ Document any bugs found

---

**Key Finding:** Out of 2000+ total tests, only **10 tests** verify concurrent error handling, and **ZERO tests** verify TLS isolation or error object GC. This is a **critical gap** for production readiness.

# Runtime Error State Testing - Gap Analysis

**Phase 2, Part 5: Error State Testing**
**Date:** 2026-02-11
**Status:** Exploratory Analysis

## Executive Summary

This analysis identifies gaps in runtime error state testing for concurrent/multi-threaded scenarios. While Pluto has good basic error handling tests (48 tests in `errors.rs`), **concurrent error state isolation is undertested**.

**Critical Gap:** TLS error state (`__pluto_current_error`) thread-safety and isolation needs comprehensive testing.

## Current Test Coverage

### Existing Error Tests (48 tests in `tests/integration/errors.rs`)

**Basic functionality** ✅:
- Error catch shorthand
- Error propagation with `!`
- Multiple error types
- Conditional raises
- Error fields (empty, string, int)
- Transitive propagation
- Errors in control flow (loops)

**Missing from basic tests**:
- ❌ Error object GC under stress
- ❌ Error field memory layout validation
- ❌ Error type hierarchy (if we add it later)

### Existing Concurrent Error Tests (7 tests in `tests/integration/concurrency.rs`)

| Test Name | What it Tests | Coverage Gap |
|-----------|---------------|--------------|
| `spawn_error_propagation` | Basic error from task → main via `!` | ✅ Basic path only |
| `spawn_error_catch` | Catch error from task | ✅ Single task only |
| `stress_tasks_with_errors` | 10 tasks, some fail | ⚠️ Only 10 tasks, same error type |
| `detach_with_error_does_not_crash` | Detached task error handling | ✅ Crash prevention only |
| `spawn_method_call_error_propagation` | Method call errors in tasks | ✅ Basic coverage |
| `spawn_method_call_error_caught` | Catch method call errors | ✅ Basic coverage |
| `sync_error_in_synchronized_method` | Sync + errors | ✅ Basic coverage |

**Total concurrent error tests:** 7
**Total concurrency tests:** 63
**Percentage:** 11% of concurrency tests involve errors

## Critical Gaps Identified

### Gap 1: Thread-Local Storage (TLS) Isolation ❌

**What's missing:**
- No tests verify that `__pluto_current_error` is truly thread-local
- No tests for concurrent tasks raising **different** error types simultaneously
- No tests for error state leaking between threads

**Risk:** If TLS is broken, errors from one task could appear in another task

**Required tests:**
1. Concurrent tasks with different error types (ErrorA, ErrorB, ErrorC, etc.)
2. Rapid task spawning with mixed success/failure
3. Sequential `.get()` calls on different tasks verifying no cross-contamination

### Gap 2: Error State Lifecycle ❌

**What's missing:**
- No tests verify error state is cleared after `catch`
- No tests for error state when task exits
- No tests for error state when task is cancelled

**Risk:** Error state could persist incorrectly, causing false positives

**Required tests:**
1. Error raised → caught → verify cleared
2. Error in task → task exits → new task spawned → verify clean state
3. Task cancelled mid-execution → verify error state cleanup

### Gap 3: High Concurrency Stress ⚠️

**Existing:** `stress_tasks_with_errors` only tests 10 tasks
**Industry standard:** 100-1000 concurrent tasks

**What's missing:**
- No test with 100+ concurrent tasks with mixed errors
- No test with rapid spawn/get cycles (1000+ iterations)
- No test with error set/clear cycles in a single thread

**Risk:** Race conditions in TLS management may only appear under high load

**Required tests:**
1. 100 concurrent tasks, each raising different errors
2. 1000 sequential spawn-error-catch cycles
3. Thread pool exhaustion with errors

### Gap 4: Error Memory Safety ❌

**What's missing:**
- No tests verify error objects are GC'd correctly
- No tests for error objects with heap fields (strings, arrays)
- No valgrind tests specifically for error handling

**Risk:** Memory leaks in error handling under concurrency

**Required tests:**
1. 10K errors created and caught (should not OOM)
2. Errors with large string fields under concurrent allocation
3. Valgrind run specifically targeting error handling

### Gap 5: Error Propagation Chains ⚠️

**Existing:** Basic multi-layer propagation in `errors.rs`
**Missing:** Concurrent multi-layer propagation

**What's missing:**
- No tests for error propagation across multiple spawned task layers
- No tests for error propagation with task fan-out (one task spawns many)

**Required tests:**
1. Task A spawns Task B spawns Task C, error propagates back through all
2. One task spawns 10 subtasks, all fail, verify all errors caught correctly

### Gap 6: Error State + Other Features ❌

**What's missing:**
- No tests for errors + channels (error while sending/receiving)
- No tests for errors + maps/sets (error during iteration)
- No tests for errors + GC (error during GC sweep)
- No tests for errors + contracts (error inside `invariant` check)

**Risk:** Feature interactions may have unexpected behavior

**Required tests:**
1. Error raised while channel is blocking
2. Error during map iteration with concurrent modifications
3. Error during GC (if possible to trigger)

## Coverage Metrics

| Category | Current Tests | Needed Tests | Total Target | % Complete |
|----------|--------------|--------------|--------------|------------|
| TLS Isolation | 1 | 4 | 5 | 20% |
| Error Lifecycle | 0 | 3 | 3 | 0% |
| High Concurrency | 1 | 3 | 4 | 25% |
| Memory Safety | 0 | 3 | 3 | 0% |
| Propagation Chains | 1 | 2 | 3 | 33% |
| Feature Interactions | 0 | 4 | 4 | 0% |
| **TOTAL** | **3** | **19** | **22** | **14%** |

**Current:** 7 concurrent error tests
**Target:** 26 concurrent error tests (7 existing + 19 new)
**Gap:** 19 tests needed

## Priority Ranking

### P0 (Critical - Must Have Before 1.0)

1. **TLS Isolation Tests** - Core correctness guarantee
   - Different error types in concurrent tasks
   - No cross-contamination between threads

2. **High Concurrency Stress** - Find race conditions
   - 100+ concurrent tasks with errors
   - Rapid spawn/catch cycles

3. **Error Memory Safety** - Prevent leaks
   - GC correctness under error load
   - Valgrind validation

### P1 (Important - Should Have)

4. **Error Lifecycle** - State management correctness
   - Error cleared after catch
   - Clean state on task exit

5. **Propagation Chains** - Complex scenarios
   - Multi-layer task spawning with errors

### P2 (Nice to Have)

6. **Feature Interactions** - Edge cases
   - Errors + channels, maps, contracts

## Recommended Test Implementation Order

### Batch 1 (Week 1): TLS Isolation (5 tests)
1. `concurrent_different_error_types` - 10 tasks, 5 different error types
2. `concurrent_error_no_cross_contamination` - Sequential .get() calls
3. `rapid_concurrent_error_raising` - 100 tasks, mixed success/failure
4. `concurrent_error_set_clear_cycles` - 50 tasks doing error cycles
5. `main_thread_error_isolated_from_tasks` - Main raises, task succeeds

### Batch 2 (Week 2): High Concurrency + Memory (6 tests)
6. `stress_100_concurrent_tasks_mixed_errors` - 100 tasks, 5 error types
7. `stress_1000_sequential_error_cycles` - Spawn-error-catch 1000x
8. `error_objects_garbage_collected` - 10K errors without OOM
9. `error_with_large_string_fields_concurrent` - Heap pressure test
10. `valgrind_error_handling_no_leaks` - Valgrind clean run
11. `valgrind_error_handling_no_races` - Helgrind clean run

### Batch 3 (Week 3): Lifecycle + Chains (5 tests)
12. `error_cleared_after_catch_verified` - State verification
13. `error_state_clean_after_task_exit` - Task lifecycle
14. `error_state_clean_after_cancel` - Cancel + state
15. `multi_layer_task_error_propagation` - A→B→C error chain
16. `task_fanout_all_fail_error_propagation` - 1→10 error fan-out

### Batch 4 (Week 4): Feature Interactions (3 tests)
17. `error_during_channel_send_blocking` - Channel + error
18. `error_during_map_concurrent_iteration` - Map + error
19. `error_in_contract_invariant_check` - Contract + error

**Total:** 19 new tests across 4 weeks

## Test Execution Strategy

### Validation Tools

For all concurrent error tests, run with:
- **Valgrind (leak-check):** `valgrind --leak-check=full ./binary`
- **Valgrind (helgrind):** `valgrind --tool=helgrind ./binary`
- **AddressSanitizer:** Compile with `-fsanitize=address`
- **ThreadSanitizer:** Compile with `-fsanitize=thread`

### CI Integration

- Fast tests (< 1s): Run on every commit
- Stress tests (> 5s): Run on PR creation
- Valgrind tests (> 30s): Run nightly

### Acceptance Criteria

Each test must:
1. Pass reliably (no flakiness)
2. Run in < 10s (except valgrind)
3. Have clear assertion messages
4. Document what it's testing in comments

## Open Questions

1. **Should we test error handling with 1000+ tasks?**
   - Pro: More realistic stress test
   - Con: Slow tests, may hit OS limits
   - Decision: Start with 100, add 1000 if we find issues

2. **Should we add error state introspection for testing?**
   - Expose `__pluto_get_current_error_state()` for tests?
   - Pro: Can directly verify TLS state
   - Con: Test-only API pollution
   - Decision: TBD - try without first

3. **Should we test platform differences (macOS vs Linux)?**
   - TLS implementation differs between platforms
   - Pro: Catch platform-specific bugs
   - Con: Requires CI on both platforms
   - Decision: Yes - add CI for both

4. **How do we test error state during GC?**
   - Hard to trigger GC at specific moments
   - May need GC introspection API
   - Decision: Defer to Phase 3 (advanced testing)

## Next Steps

1. **Review this analysis** with team
2. **Approve priority ranking** (P0, P1, P2)
3. **Assign Batch 1** (5 TLS isolation tests)
4. **Set up valgrind CI job** for error tests
5. **Begin implementation** following the 4-week plan

---

**Conclusion:** We have **14% coverage** of runtime error state testing. To reach production-grade, we need **19 additional tests** focusing on TLS isolation, high concurrency stress, and memory safety. Estimated effort: **4 weeks** for full implementation.

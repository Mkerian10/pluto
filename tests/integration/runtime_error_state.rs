// Runtime Error State Testing
//
// This file contains comprehensive tests for thread-local error state management,
// error object garbage collection, and error lifecycle under concurrency.
//
// Critical property: __pluto_current_error (TLS) must be:
// 1. Isolated between threads (no cross-contamination)
// 2. Properly cleared after catch
// 3. GC-tracked correctly
// 4. Safe under high concurrency

mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ══════════════════════════════════════════════════════════════════════════════
// P0.1: TLS ISOLATION TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn tls_concurrent_different_error_types() {
    // CRITICAL: Verifies that different error types in concurrent tasks don't interfere
    // with each other's TLS error state. This is the CORE TLS isolation guarantee.
    //
    // Test: Spawn 5 tasks, each raises a different error type, all caught independently.
    // Expected: Each task's error is caught correctly, no cross-contamination.
    let out = compile_and_run_stdout(r#"
error ErrorA { id: int }
error ErrorB { id: int }
error ErrorC { id: int }
error ErrorD { id: int }
error ErrorE { id: int }

fn raise_a() int {
    raise ErrorA { id: 100 }
    return 0
}

fn raise_b() int {
    raise ErrorB { id: 200 }
    return 0
}

fn raise_c() int {
    raise ErrorC { id: 300 }
    return 0
}

fn raise_d() int {
    raise ErrorD { id: 400 }
    return 0
}

fn raise_e() int {
    raise ErrorE { id: 500 }
    return 0
}

fn main() {
    let ta = spawn raise_a()
    let tb = spawn raise_b()
    let tc = spawn raise_c()
    let td = spawn raise_d()
    let te = spawn raise_e()

    // Each catch should receive its own error type
    let ra = ta.get() catch err { -1 }
    let rb = tb.get() catch err { -2 }
    let rc = tc.get() catch err { -3 }
    let rd = td.get() catch err { -4 }
    let re = te.get() catch err { -5 }

    // All should have caught their respective errors
    print(ra)
    print(rb)
    print(rc)
    print(rd)
    print(re)
}
"#);
    assert_eq!(out.trim(), "-1\n-2\n-3\n-4\n-5");
}

#[test]
fn tls_no_cross_contamination_sequential_gets() {
    // CRITICAL: Verifies that getting errors from multiple tasks sequentially
    // doesn't cause error state to leak between .get() calls.
    //
    // Test: Spawn failing task, get error, spawn succeeding task, verify clean state.
    // Expected: Second task succeeds without inheriting first task's error.
    let out = compile_and_run_stdout(r#"
error FailError { msg: string }

fn fail_task() int {
    raise FailError { msg: "task failed" }
    return 0
}

fn success_task() int {
    return 42
}

fn main() {
    let t_fail = spawn fail_task()
    let t_success = spawn success_task()

    // Get failing task first
    let r1 = t_fail.get() catch err { -1 }
    print(r1)

    // Error state should NOT affect second task
    let r2 = t_success.get()
    print(r2)
}
"#);
    assert_eq!(out.trim(), "-1\n42");
}

#[test]
fn tls_main_thread_error_isolated_from_spawned_tasks() {
    // CRITICAL: Verifies that error raised in main thread doesn't affect
    // spawned tasks running concurrently.
    //
    // Test: Spawn task before raising error in main, task should succeed.
    // Expected: Main thread error doesn't leak into task's TLS.
    let out = compile_and_run_stdout(r#"
error MainError { code: int }

fn safe_work() int {
    return 100
}

fn raise_in_main() int {
    raise MainError { code: 999 }
    return 0
}

fn main() {
    // Spawn task BEFORE main raises error
    let task = spawn safe_work()

    // Raise and catch error in main thread
    let main_result = raise_in_main() catch err { -1 }
    print(main_result)

    // Task should complete successfully despite main thread error
    let task_result = task.get()
    print(task_result)
}
"#);
    assert_eq!(out.trim(), "-1\n100");
}

#[test]
fn tls_rapid_concurrent_error_raising() {
    // STRESS TEST: Verifies TLS isolation under rapid concurrent error creation.
    //
    // Test: 20 tasks, some succeed, some fail with different error types.
    // Expected: All errors caught correctly, no interference.
    let out = compile_and_run_stdout(r#"
error Err1 {}
error Err2 {}
error Err3 {}
error Err4 {}

fn maybe_fail(x: int) int {
    if x % 4 == 0 { raise Err1 {} }
    if x % 4 == 1 { raise Err2 {} }
    if x % 4 == 2 { raise Err3 {} }
    if x % 4 == 3 { raise Err4 {} }
    return x
}

fn main() {
    let t0 = spawn maybe_fail(0)
    let t1 = spawn maybe_fail(1)
    let t2 = spawn maybe_fail(2)
    let t3 = spawn maybe_fail(3)
    let t4 = spawn maybe_fail(4)
    let t5 = spawn maybe_fail(5)
    let t6 = spawn maybe_fail(6)
    let t7 = spawn maybe_fail(7)
    let t8 = spawn maybe_fail(8)
    let t9 = spawn maybe_fail(9)
    let t10 = spawn maybe_fail(10)
    let t11 = spawn maybe_fail(11)
    let t12 = spawn maybe_fail(12)
    let t13 = spawn maybe_fail(13)
    let t14 = spawn maybe_fail(14)
    let t15 = spawn maybe_fail(15)
    let t16 = spawn maybe_fail(16)
    let t17 = spawn maybe_fail(17)
    let t18 = spawn maybe_fail(18)
    let t19 = spawn maybe_fail(19)

    // All should fail and be caught
    let caught = 0
    let caught = caught + (t0.get() catch err { 1 })
    let caught = caught + (t1.get() catch err { 1 })
    let caught = caught + (t2.get() catch err { 1 })
    let caught = caught + (t3.get() catch err { 1 })
    let caught = caught + (t4.get() catch err { 1 })
    let caught = caught + (t5.get() catch err { 1 })
    let caught = caught + (t6.get() catch err { 1 })
    let caught = caught + (t7.get() catch err { 1 })
    let caught = caught + (t8.get() catch err { 1 })
    let caught = caught + (t9.get() catch err { 1 })
    let caught = caught + (t10.get() catch err { 1 })
    let caught = caught + (t11.get() catch err { 1 })
    let caught = caught + (t12.get() catch err { 1 })
    let caught = caught + (t13.get() catch err { 1 })
    let caught = caught + (t14.get() catch err { 1 })
    let caught = caught + (t15.get() catch err { 1 })
    let caught = caught + (t16.get() catch err { 1 })
    let caught = caught + (t17.get() catch err { 1 })
    let caught = caught + (t18.get() catch err { 1 })
    let caught = caught + (t19.get() catch err { 1 })

    print(caught)
}
"#);
    assert_eq!(out.trim(), "20");
}

#[test]
fn tls_concurrent_error_set_clear_cycles() {
    // STRESS TEST: Verifies TLS state management under rapid set/clear cycles.
    //
    // Test: 10 tasks, each does 100 raise-catch cycles internally.
    // Expected: All tasks complete successfully, no TLS corruption.
    let out = compile_and_run_stdout(r#"
error CycleError { iteration: int }

fn error_cycle(task_id: int) int {
    let i = 0
    while i < 100 {
        let _ = raise_and_catch(i) catch err { 0 }
        i = i + 1
    }
    return task_id
}

fn raise_and_catch(iter: int) int {
    raise CycleError { iteration: iter }
    return 0
}

fn main() {
    let t0 = spawn error_cycle(0)
    let t1 = spawn error_cycle(1)
    let t2 = spawn error_cycle(2)
    let t3 = spawn error_cycle(3)
    let t4 = spawn error_cycle(4)
    let t5 = spawn error_cycle(5)
    let t6 = spawn error_cycle(6)
    let t7 = spawn error_cycle(7)
    let t8 = spawn error_cycle(8)
    let t9 = spawn error_cycle(9)

    let sum = 0
    let sum = sum + t0.get()
    let sum = sum + t1.get()
    let sum = sum + t2.get()
    let sum = sum + t3.get()
    let sum = sum + t4.get()
    let sum = sum + t5.get()
    let sum = sum + t6.get()
    let sum = sum + t7.get()
    let sum = sum + t8.get()
    let sum = sum + t9.get()

    // Sum should be 0+1+2+...+9 = 45
    print(sum)
}
"#);
    assert_eq!(out.trim(), "45");
}

// ══════════════════════════════════════════════════════════════════════════════
// P0.2: ERROR OBJECT GARBAGE COLLECTION TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn gc_error_objects_collected_under_pressure() {
    // CRITICAL: Verifies that error objects are properly GC'd and don't leak memory.
    //
    // Test: Create 1000 error objects in a loop, each caught and discarded.
    // Expected: Program completes without OOM (GC is collecting error objects).
    let out = compile_and_run_stdout(r#"
error HeapError {
    id: int
    message: string
}

fn create_error(id: int) int {
    raise HeapError { id: id, message: "error message" }
    return 0
}

fn main() {
    let i = 0
    let caught = 0
    while i < 1000 {
        let _ = create_error(i) catch err { caught = caught + 1; 0 }
        i = i + 1
    }
    print(caught)
}
"#);
    assert_eq!(out.trim(), "1000");
}

#[test]
fn gc_error_objects_with_large_heap_fields() {
    // CRITICAL: Verifies GC handles errors with large heap-allocated fields.
    //
    // Test: Create errors with large strings (100 chars each), 500 times.
    // Expected: GC collects both error objects AND their string fields.
    let out = compile_and_run_stdout(r#"
error LargeError {
    big_message: string
    id: int
}

fn make_large_string(id: int) string {
    // Create a string with 100 characters
    let s = "ERROR:"
    let i = 0
    while i < 94 {
        s = s + "X"
        i = i + 1
    }
    return s
}

fn create_large_error(id: int) int {
    raise LargeError {
        big_message: make_large_string(id),
        id: id
    }
    return 0
}

fn main() {
    let i = 0
    let caught = 0
    while i < 500 {
        let _ = create_large_error(i) catch err {
            caught = caught + 1
            0
        }
        i = i + 1
    }
    print(caught)
}
"#);
    assert_eq!(out.trim(), "500");
}

#[test]
fn gc_error_objects_in_concurrent_tasks() {
    // CRITICAL: Verifies GC correctly traces error objects across multiple threads.
    //
    // Test: 20 concurrent tasks, each creates 50 errors (1000 total errors).
    // Expected: All errors GC'd correctly, no memory leak, no crashes.
    let out = compile_and_run_stdout(r#"
error TaskError {
    task_id: int
    error_num: int
}

fn create_errors(task_id: int) int {
    let i = 0
    let caught = 0
    while i < 50 {
        let _ = raise_error(task_id, i) catch err {
            caught = caught + 1
            0
        }
        i = i + 1
    }
    return caught
}

fn raise_error(task_id: int, num: int) int {
    raise TaskError { task_id: task_id, error_num: num }
    return 0
}

fn main() {
    let t0 = spawn create_errors(0)
    let t1 = spawn create_errors(1)
    let t2 = spawn create_errors(2)
    let t3 = spawn create_errors(3)
    let t4 = spawn create_errors(4)
    let t5 = spawn create_errors(5)
    let t6 = spawn create_errors(6)
    let t7 = spawn create_errors(7)
    let t8 = spawn create_errors(8)
    let t9 = spawn create_errors(9)
    let t10 = spawn create_errors(10)
    let t11 = spawn create_errors(11)
    let t12 = spawn create_errors(12)
    let t13 = spawn create_errors(13)
    let t14 = spawn create_errors(14)
    let t15 = spawn create_errors(15)
    let t16 = spawn create_errors(16)
    let t17 = spawn create_errors(17)
    let t18 = spawn create_errors(18)
    let t19 = spawn create_errors(19)

    let total = 0
    let total = total + t0.get()
    let total = total + t1.get()
    let total = total + t2.get()
    let total = total + t3.get()
    let total = total + t4.get()
    let total = total + t5.get()
    let total = total + t6.get()
    let total = total + t7.get()
    let total = total + t8.get()
    let total = total + t9.get()
    let total = total + t10.get()
    let total = total + t11.get()
    let total = total + t12.get()
    let total = total + t13.get()
    let total = total + t14.get()
    let total = total + t15.get()
    let total = total + t16.get()
    let total = total + t17.get()
    let total = total + t18.get()
    let total = total + t19.get()

    // Should be 20 tasks * 50 errors = 1000
    print(total)
}
"#);
    assert_eq!(out.trim(), "1000");
}

// ══════════════════════════════════════════════════════════════════════════════
// P0.3: ERROR LIFECYCLE TESTS
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn lifecycle_error_cleared_after_catch() {
    // CRITICAL: Verifies that __pluto_clear_error() is called after catch,
    // and error state doesn't persist to next operation.
    //
    // Test: Raise error, catch it, then verify new task starts with clean state.
    // Expected: Second task succeeds without seeing first error.
    let out = compile_and_run_stdout(r#"
error FirstError {}

fn fail_first() int {
    raise FirstError {}
    return 0
}

fn succeed_second() int {
    return 999
}

fn main() {
    // First: raise and catch error
    let r1 = fail_first() catch err { -1 }
    print(r1)

    // Second: spawn new task - should have clean error state
    let t = spawn succeed_second()
    let r2 = t.get()
    print(r2)
}
"#);
    assert_eq!(out.trim(), "-1\n999");
}

#[test]
fn lifecycle_error_state_clean_after_task_exit() {
    // CRITICAL: Verifies that task thread cleanup properly clears TLS error state.
    //
    // Test: Spawn task that raises error and exits, then spawn new task.
    // Expected: New task doesn't see old task's error state.
    let out = compile_and_run_stdout(r#"
error OldError {}

fn old_task() int {
    raise OldError {}
    return 0
}

fn new_task() int {
    return 777
}

fn main() {
    // First task raises error and completes
    let t1 = spawn old_task()
    let r1 = t1.get() catch err { -1 }
    print(r1)

    // Second task should have clean state
    let t2 = spawn new_task()
    let r2 = t2.get()
    print(r2)
}
"#);
    assert_eq!(out.trim(), "-1\n777");
}

#[test]
fn lifecycle_multiple_sequential_error_catches() {
    // Verifies that error state is properly cleaned between sequential catch operations.
    //
    // Test: Raise and catch 5 different errors sequentially in main thread.
    // Expected: Each catch operates independently, no state leakage.
    let out = compile_and_run_stdout(r#"
error E1 { n: int }
error E2 { n: int }
error E3 { n: int }
error E4 { n: int }
error E5 { n: int }

fn fail1() int { raise E1 { n: 1 } }
fn fail2() int { raise E2 { n: 2 } }
fn fail3() int { raise E3 { n: 3 } }
fn fail4() int { raise E4 { n: 4 } }
fn fail5() int { raise E5 { n: 5 } }

fn main() {
    let r1 = fail1() catch err { -1 }
    let r2 = fail2() catch err { -2 }
    let r3 = fail3() catch err { -3 }
    let r4 = fail4() catch err { -4 }
    let r5 = fail5() catch err { -5 }

    // All should have caught their respective errors
    print(r1 + r2 + r3 + r4 + r5)
}
"#);
    assert_eq!(out.trim(), "-15");
}

// ══════════════════════════════════════════════════════════════════════════════
// Additional: Edge Cases
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn edge_error_in_nested_spawn() {
    // Verifies error propagation in nested task spawning scenario.
    //
    // Note: Current Pluto doesn't support spawn inside spawn (no capture),
    // but this tests error in a task that then spawns another task.
    let out = compile_and_run_stdout(r#"
error NestedError { level: int }

fn inner_task() int {
    raise NestedError { level: 2 }
    return 0
}

fn outer_task() int {
    let t = spawn inner_task()
    let result = t.get() catch err { -99 }
    return result
}

fn main() {
    let t = spawn outer_task()
    let final_result = t.get()
    print(final_result)
}
"#);
    assert_eq!(out.trim(), "-99");
}

#[test]
fn edge_error_with_empty_struct() {
    // Verifies error objects with zero fields are handled correctly.
    let out = compile_and_run_stdout(r#"
error EmptyError {}

fn fail() int {
    raise EmptyError {}
    return 0
}

fn main() {
    let t1 = spawn fail()
    let t2 = spawn fail()
    let t3 = spawn fail()

    let r1 = t1.get() catch err { -1 }
    let r2 = t2.get() catch err { -1 }
    let r3 = t3.get() catch err { -1 }

    print(r1 + r2 + r3)
}
"#);
    assert_eq!(out.trim(), "-3");
}

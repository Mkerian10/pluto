//! Task error tracking.
//!
//! `task.get()`'s fallibility tracks the spawned function: spawning an
//! infallible function yields a task whose `get()` needs no handling, while
//! spawning a fallible function makes every bare `get()` an error until
//! handled with `!` or `catch` — in every expression position.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

const MUST_HANDLE: &str = "must be handled";

const FWORK: &str = "error E {}\n\nfn work() int {\n    raise E {}\n}\n";

fn fail_with_fallible_work(body: &str) {
    compile_should_fail_with(&format!("{FWORK}\n{body}"), MUST_HANDLE);
}

// ── Fallible work: bare get() rejected in every position ─────────────────────

#[test]
fn task_get_no_handler() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    t.get()\n}");
}

#[test]
fn task_get_in_assignment() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    let x = t.get()\n}");
}

#[test]
fn task_get_in_binop() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    let x = t.get() + 1\n}");
}

#[test]
fn task_get_in_if() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    if true {\n        t.get()\n    }\n}");
}

#[test]
fn task_get_in_while() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    while false {\n        t.get()\n    }\n}");
}

#[test]
fn task_get_in_for() {
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    for i in 0..1 {\n        t.get()\n    }\n}");
}

#[test]
fn task_get_as_arg() {
    fail_with_fallible_work("fn consume(x: int) {\n    print(x)\n}\n\nfn main(){\n    let t = spawn work()\n    consume(t.get())\n}");
}

#[test]
fn task_get_in_struct_field() {
    fail_with_fallible_work("class C {\n    x: int\n}\n\nfn main(){\n    let t = spawn work()\n    let c = C { x: t.get() }\n}");
}

#[test]
fn task_stored_then_get() {
    // Fallibility follows the task through reassignment to another variable
    fail_with_fallible_work("fn main(){\n    let t = spawn work()\n    let x = t\n    let y = x.get()\n}");
}

#[test]
fn two_tasks_one_fallible() {
    // Only the fallible task's get needs handling; t1.get() alone is fine
    compile_should_fail_with(
        "error E {}\n\nfn work1() int {\n    return 42\n}\n\nfn work2() int {\n    raise E {}\n}\n\nfn main(){\n    let t1 = spawn work1()\n    let t2 = spawn work2()\n    print(t1.get())\n    t2.get()\n}",
        MUST_HANDLE,
    );
}

#[test]
fn task_get_needs_handling_outside_main_too() {
    compile_should_fail_with(
        "error E {}\n\nfn work() int {\n    raise E {}\n}\n\nfn f() int {\n    let t = spawn work()\n    return t.get()\n}\n\nfn main(){}",
        MUST_HANDLE,
    );
}

// ── Infallible work: bare get() is fine ──────────────────────────────────────

#[test]
fn infallible_task_get_no_handling_needed() {
    let out = compile_and_run_stdout(
        "fn work() int {\n    return 42\n}\n\nfn main(){\n    let t = spawn work()\n    print(t.get())\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn generic_task_get() {
    let out = compile_and_run_stdout(
        "fn work<T>(x: T) T {\n    return x\n}\n\nfn main(){\n    let t = spawn work(42)\n    print(t.get())\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn bang_on_infallible_task_get_rejected() {
    compile_should_fail_with(
        "fn work() int {\n    return 42\n}\n\nfn main(){\n    let t = spawn work()\n    let x = t.get()!\n}",
        "'!' applied to infallible method 'get'",
    );
}

// ── Handled fallible get() works end to end ──────────────────────────────────

#[test]
fn fallible_work_error_travels_through_get() {
    let out = compile_and_run_stdout(
        "error E {}\n\nfn work() int {\n    raise E {}\n}\n\nfn main(){\n    let t = spawn work()\n    print(t.get() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn task_get_with_propagate() {
    let out = compile_and_run_stdout(
        "error E {}\n\nfn work(fail: bool) int {\n    if fail {\n        raise E {}\n    }\n    return 42\n}\n\nfn f() int {\n    let t = spawn work(false)\n    return t.get()!\n}\n\nfn main(){\n    print(f() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "42");
}

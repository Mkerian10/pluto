//! Spawn validation.
//!
//! `spawn callee(args)` requires a named function, method, or closure-typed
//! variable as the callee; arguments are validated like a normal call and
//! must not contain unhandled fallible calls or `!` propagation.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Legal spawn forms ────────────────────────────────────────────────────────

#[test]
fn spawn_closure() {
    let out = compile_and_run_stdout(
        "fn main(){\n    let f = () => 7\n    let t = spawn f()\n    print(t.get())\n}",
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn spawn_method() {
    let out = compile_and_run_stdout(
        "class C {\n    x: int\n\n    fn foo(self) int {\n        return self.x\n    }\n}\n\nfn main(){\n    let c = C { x: 5 }\n    let t = spawn c.foo()\n    print(t.get())\n}",
    );
    assert_eq!(out.trim(), "5");
}

// ── Invalid callees ──────────────────────────────────────────────────────────

#[test]
fn spawn_non_function() {
    compile_should_fail_with(
        "fn main(){\n    let x = 42\n    spawn x()\n}",
        "undefined function 'x'",
    );
}

#[test]
fn spawn_undefined() {
    compile_should_fail_with(
        "fn main(){\n    spawn unknown()\n}",
        "undefined",
    );
}

#[test]
fn spawn_lambda_literal_rejected() {
    // The callee must be named; spawn an immediately-invoked lambda is a parse error
    compile_should_fail_with(
        "fn main(){\n    spawn ((x: int) => x + 1)(42)\n}",
        "expected identifier",
    );
}

#[test]
fn double_spawn_rejected() {
    compile_should_fail_with(
        "fn f() int {\n    return 1\n}\n\nfn main(){\n    spawn spawn f()\n}",
        "expected identifier",
    );
}

#[test]
fn spawn_unused_handle_rejected() {
    compile_should_fail_with(
        "fn main(){\n    spawn print(\"hi\")\n}",
        "Task handle must be used",
    );
}

// ── Argument validation ──────────────────────────────────────────────────────

#[test]
fn spawn_wrong_arg_count() {
    compile_should_fail_with(
        "fn f(x: int) int {\n    return x\n}\n\nfn main(){\n    let t = spawn f()\n}",
        "expects 1 arguments, got 0",
    );
}

#[test]
fn spawn_wrong_arg_type() {
    compile_should_fail_with(
        "fn f(x: int) int {\n    return x\n}\n\nfn main(){\n    let t = spawn f(\"hi\")\n}",
        "argument 1 of 'f': expected int, found string",
    );
}

#[test]
fn spawn_fallible_arg_unhandled() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn g(x: int) int {\n    return x\n}\n\nfn main(){\n    let t = spawn g(f())\n}",
        "must be handled with ! or catch",
    );
}

#[test]
fn spawn_propagate_in_arg_rejected() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn g(x: int) int {\n    return x\n}\n\nfn h() {\n    let t = spawn g(f()!)\n    t.detach()\n}\n\nfn main(){\n    h() catch e {}\n}",
        "error propagation (!) is not allowed in spawn arguments",
    );
}

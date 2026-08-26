//! `!` on infallible expressions.
//!
//! `!` propagates a callee's error to the caller, so it is only meaningful
//! on calls to fallible functions/methods. Applying it to an infallible
//! call is rejected with "'!' applied to infallible ...", and applying it
//! to a non-call expression is rejected outright.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// ── Infallible calls ─────────────────────────────────────────────────────────

#[test]
fn propagate_on_safe_call() {
    compile_should_fail_with(
        "fn f() int {\n    return 1\n}\n\nfn main(){\n    let x = f()!\n}",
        "'!' applied to infallible function 'f'",
    );
}

#[test]
fn propagate_on_safe_method() {
    compile_should_fail_with(
        "class C {\n    x: int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){\n    let c = C { x: 1 }\n    let x = c.foo()!\n}",
        "'!' applied to infallible method 'foo'",
    );
}

#[test]
fn propagate_on_builtin_method() {
    compile_should_fail_with(
        "fn main(){\n    let s = \"hi\"\n    let x = s.len()!\n}",
        "'!' applied to infallible method 'len'",
    );
}

#[test]
fn propagate_on_safe_closure() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => x + 1\n    let y = f(1)!\n}",
        "'!' applied to infallible",
    );
}

// ── Non-call expressions ─────────────────────────────────────────────────────

#[test]
fn propagate_on_literal() {
    compile_should_fail_with(
        "fn main(){\n    let x = 42!\n}",
        "! can only be applied to function calls",
    );
}

#[test]
fn propagate_on_binop() {
    compile_should_fail_with(
        "fn main(){\n    let x = (1 + 2)!\n}",
        "! can only be applied to function calls",
    );
}

#[test]
fn propagate_on_string() {
    compile_should_fail_with(
        "fn main(){\n    let s = \"hi\"!\n}",
        "! can only be applied to function calls",
    );
}

#[test]
fn propagate_on_array() {
    compile_should_fail_with(
        "fn main(){\n    let arr = [1, 2, 3]!\n}",
        "! can only be applied to function calls",
    );
}

#[test]
fn propagate_on_variable() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    let y = x!\n}",
        "! can only be applied to function calls",
    );
}

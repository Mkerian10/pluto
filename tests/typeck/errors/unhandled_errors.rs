//! Unhandled-error enforcement.
//!
//! Fallibility is inferred: a function that raises (or transitively calls a
//! fallible function without handling) is fallible, and every call to it
//! must be handled with `!` or `catch`. `main` is the exception — errors may
//! escape it at compile time; the runtime prints "unhandled error escaped
//! main" and exits nonzero (see the runtime contract from #274).
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

const MUST_HANDLE: &str = "must be handled with ! or catch";

// ── Raising inside main: compiles, fails at runtime ──────────────────────────

#[test]
fn raise_in_main_escapes_at_runtime() {
    assert_ne!(compile_and_run("error E {}\n\nfn main(){\n    raise E {}\n}"), 0);
}

#[test]
fn raise_in_if_branch_escapes_at_runtime() {
    assert_ne!(
        compile_and_run("error E {}\n\nfn main(){\n    if true {\n        raise E {}\n    }\n}"),
        0
    );
}

#[test]
fn raise_in_else_branch_escapes_at_runtime() {
    assert_ne!(
        compile_and_run("error E {}\n\nfn main(){\n    if false {\n        print(1)\n    } else {\n        raise E {}\n    }\n}"),
        0
    );
}

#[test]
fn raise_in_while_escapes_at_runtime() {
    assert_ne!(
        compile_and_run("error E {}\n\nfn main(){\n    while true {\n        raise E {}\n    }\n}"),
        0
    );
}

#[test]
fn raise_in_for_escapes_at_runtime() {
    assert_ne!(
        compile_and_run("error E {}\n\nfn main(){\n    for i in 0..10 {\n        raise E {}\n    }\n}"),
        0
    );
}

#[test]
fn raise_in_match_arm_escapes_at_runtime() {
    assert_ne!(
        compile_and_run("error E {}\n\nenum Opt {\n    Some { v: int }\n    None\n}\n\nfn main(){\n    let x = Opt.None\n    match x {\n        Opt.Some { v } {\n            print(v)\n        }\n        Opt.None {\n            raise E {}\n        }\n    }\n}"),
        0
    );
}

// ── Defining a fallible function is fine; calling it unhandled is not ────────

#[test]
fn uncalled_fallible_fn_compiles() {
    assert_eq!(
        compile_and_run("error E {}\n\nfn f() {\n    raise E {}\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn uncalled_fallible_method_compiles() {
    assert_eq!(
        compile_and_run("error E {}\n\nclass C {\n    x: int\n\n    fn foo(self) {\n        raise E {}\n    }\n}\n\nfn main(){}"),
        0
    );
}

// ── Unhandled fallible calls ─────────────────────────────────────────────────

#[test]
fn fallible_call_no_handler() {
    compile_should_fail_with(
        "error E {}\n\nfn f() {\n    raise E {}\n}\n\nfn main(){\n    f()\n}",
        MUST_HANDLE,
    );
}

#[test]
fn fallible_call_in_assignment() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let x = f()\n}",
        MUST_HANDLE,
    );
}

#[test]
fn fallible_call_in_binop() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let x = f() + 1\n}",
        MUST_HANDLE,
    );
}

#[test]
fn fallible_call_in_return() {
    // Inside a non-main function the call must be handled or propagated
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn g() int {\n    return f()\n}\n\nfn main(){}",
        MUST_HANDLE,
    );
}

#[test]
fn multiple_bare_calls() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let x = f()\n    let y = f()\n}",
        MUST_HANDLE,
    );
}

#[test]
fn transitive_fallible_without_handling() {
    // g becomes fallible only if it handles or propagates; a bare call is an error
    compile_should_fail_with(
        "error E {}\n\nfn f() {\n    raise E {}\n}\n\nfn g() {\n    f()\n}\n\nfn main(){}",
        MUST_HANDLE,
    );
}

#[test]
fn method_call_fallible_no_handler() {
    compile_should_fail_with(
        "error E {}\n\nclass C {\n    x: int\n\n    fn foo(self) {\n        raise E {}\n    }\n}\n\nfn main(){\n    let c = C { x: 1 }\n    c.foo()\n}",
        "must be handled",
    );
}

#[test]
fn method_calls_fallible_no_propagate() {
    compile_should_fail_with(
        "error E {}\n\nfn f() {\n    raise E {}\n}\n\nclass C {\n    x: int\n\n    fn foo(self) {\n        f()\n    }\n}\n\nfn main(){}",
        MUST_HANDLE,
    );
}

#[test]
fn match_arm_calls_fallible() {
    compile_should_fail_with(
        "error E {}\n\nfn f() {\n    raise E {}\n}\n\nenum Opt {\n    Some { v: int }\n    None\n}\n\nfn main(){\n    let x = Opt.None\n    match x {\n        Opt.Some { v } {\n            print(v)\n        }\n        Opt.None {\n            f()\n        }\n    }\n}",
        MUST_HANDLE,
    );
}

// ── Unhandled fallible calls in literal/expression positions ─────────────────

#[test]
fn array_element_fallible() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let arr = [f(), 2, 3]\n}",
        MUST_HANDLE,
    );
}

#[test]
fn struct_field_fallible() {
    compile_should_fail_with(
        "error E {}\n\nclass C {\n    x: int\n}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let c = C { x: f() }\n}",
        MUST_HANDLE,
    );
}

#[test]
fn enum_variant_field_fallible() {
    compile_should_fail_with(
        "error E {}\n\nenum Opt {\n    Some { v: int }\n}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let x = Opt.Some { v: f() }\n}",
        MUST_HANDLE,
    );
}

#[test]
fn interpolation_fallible() {
    compile_should_fail_with(
        "error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    let s = f\"{f()}\"\n    print(s)\n}",
        MUST_HANDLE,
    );
}

#[test]
fn union_errors_unhandled() {
    // Two different error types, both unhandled at the call sites
    compile_should_fail_with(
        "error E1 {}\n\nerror E2 {}\n\nfn f() {\n    raise E1 {}\n}\n\nfn g() {\n    raise E2 {}\n}\n\nfn main(){\n    f()\n    g()\n}",
        MUST_HANDLE,
    );
}

// Unhandled in closures. Defining a fallible closure is fine — its errors
// surface at its call sites (or absorb into the definer if it escapes).
#[test]
fn closure_raises_no_handler() {
    compile_should_fail_with(
        r#"error E{}
fn main() {
    let f = () => {
        raise E{}
    }
    f()
}"#,
        "call to fallible closure 'f' must be handled",
    );
}
#[test]
fn closure_calls_fallible_no_handler() {
    compile_should_fail_with(
        r#"error E{}
fn g() int {
    raise E{}
    return 0
}
fn main() {
    let f = () => {
        g()
        return 0
    }
    print(f())
}"#,
        "call to fallible function 'g' must be handled",
    );
}
#[test]
fn closure_propagating_makes_call_site_fallible() {
    compile_should_fail_with(
        r#"error E{}
fn g() int {
    raise E{}
    return 0
}
fn main() {
    let f = () => {
        return g()!
    }
    print(f())
}"#,
        "call to fallible closure 'f' must be handled",
    );
}


//! Error propagation chains.
//!
//! Fallibility is inferred: `a()!` inside `b` makes `b` fallible, and the
//! chain continues until someone catches (or it escapes main at runtime).
//! These tests exercise `!` in every expression position and verify the
//! error actually travels the chain; the rejected case is calling the top
//! of a chain without handling it.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

const PREAMBLE: &str = "error E {\n    code: int\n}\n\nfn a() int {\n    raise E { code: 7 }\n}\n";

fn run_chain(body: &str) -> String {
    compile_and_run_stdout(&format!("{PREAMBLE}\n{body}"))
}

// ── Chains propagate ─────────────────────────────────────────────────────────

#[test]
fn two_level_propagation() {
    let out = run_chain(
        "fn b() int {\n    return a()!\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn three_level_propagation() {
    let out = run_chain(
        "fn b() int {\n    return a()! + 1\n}\n\nfn c() int {\n    return b()!\n}\n\nfn main(){\n    print(c() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_through_assignment() {
    let out = run_chain(
        "fn b() int {\n    let x = a()!\n    return x\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_binop() {
    let out = run_chain(
        "fn b() int {\n    return a()! + a()!\n}\n\nfn main(){\n    print(b() catch e { -2 })\n}",
    );
    assert_eq!(out.trim(), "-2");
}

#[test]
fn propagate_in_if_branch() {
    let out = run_chain(
        "fn b(flag: bool) int {\n    if flag {\n        return a()!\n    }\n    return 0\n}\n\nfn main(){\n    print(b(false) catch e { -1 })\n    print(b(true) catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "0\n-1");
}

#[test]
fn propagate_in_while_body() {
    let out = run_chain(
        "fn b() int {\n    while true {\n        a()!\n    }\n    return 0\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_for_body() {
    let out = run_chain(
        "fn b() int {\n    for i in 0..10 {\n        a()!\n    }\n    return 0\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_array_element() {
    let out = run_chain(
        "fn b() int {\n    let arr = [a()!, 2, 3]\n    return arr[0]\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_struct_field() {
    let out = run_chain(
        "class C {\n    x: int\n}\n\nfn b() int {\n    let c = C { x: a()! }\n    return c.x\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_enum_variant() {
    let out = run_chain(
        "enum Opt {\n    Some { v: int }\n}\n\nfn b() int {\n    let x = Opt.Some { v: a()! }\n    return 0\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_function_arg() {
    let out = run_chain(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn b() int {\n    return double(a()!)\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_method_arg() {
    let out = run_chain(
        "class C {\n    fn foo(self, x: int) int {\n        return x\n    }\n}\n\nfn b() int {\n    let c = C {}\n    return c.foo(a()!)\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_match_arm() {
    let out = run_chain(
        "enum Opt {\n    Some { v: int }\n    None\n}\n\nfn b() int {\n    let x = Opt.None\n    match x {\n        Opt.Some { v } {\n            print(v)\n        }\n        Opt.None {\n            a()!\n        }\n    }\n    return 0\n}\n\nfn main(){\n    print(b() catch e { -1 })\n}",
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn propagate_in_string_interpolation() {
    let out = run_chain(
        "fn b() string {\n    return f\"got {a()!}\"\n}\n\nfn main(){\n    print(b() catch e { \"failed\" })\n}",
    );
    assert_eq!(out.trim(), "failed");
}

#[test]
fn union_errors_in_chain() {
    // Two error types travel the same chain; typed catch distinguishes them
    let out = compile_and_run_stdout(
        "error E1 {}\n\nerror E2 {}\n\nfn a() {\n    raise E1 {}\n}\n\nfn b() {\n    raise E2 {}\n}\n\nfn c(which: bool) {\n    if which {\n        a()!\n    } else {\n        b()!\n    }\n}\n\nfn main(){\n    c(true) catch e1: E1 {\n        print(\"one\")\n    } catch e2: E2 {\n        print(\"two\")\n    }\n    c(false) catch e1: E1 {\n        print(\"one\")\n    } catch e2: E2 {\n        print(\"two\")\n    }\n}",
    );
    assert_eq!(out.trim(), "one\ntwo");
}

// ── The chain must end in handling ───────────────────────────────────────────

#[test]
fn chain_top_unhandled_rejected() {
    compile_should_fail_with(
        "error E {}\n\nfn a() {\n    raise E {}\n}\n\nfn b() {\n    a()!\n}\n\nfn c() {\n    b()\n}\n\nfn main(){}",
        "must be handled with ! or catch",
    );
}

#[test]
fn nested_call_result_unhandled_rejected() {
    compile_should_fail_with(
        "error E {}\n\nfn a() int {\n    raise E {}\n}\n\nfn double(x: int) int {\n    return x * 2\n}\n\nfn b() int {\n    return double(a())\n}\n\nfn main(){}",
        "must be handled with ! or catch",
    );
}

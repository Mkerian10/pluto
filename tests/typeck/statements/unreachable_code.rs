//! Unreachable-code detection (#163, shipped as warnings in #274).
//!
//! Code after a point where every path has terminated (return, raise,
//! break, continue, or an if/match whose branches all terminate) produces
//! an "unreachable code" *warning*, not an error — compilation always
//! proceeds. `raise` alone satisfies a fallible function's return
//! requirement, so a trailing `return x` after raise is dead code and the
//! warning nudges toward removing it.
#[path = "../common.rs"]
mod common;

/// Compile and return the number of "unreachable code" warnings.
fn unreachable_warnings(source: &str) -> usize {
    match pluto::compile_to_object_with_warnings(source) {
        Ok((_obj, warnings)) => warnings
            .iter()
            .filter(|w| w.msg.contains("unreachable"))
            .count(),
        Err(e) => panic!("Compilation failed unexpectedly: {e}"),
    }
}

// ── Warns: code after a terminator ───────────────────────────────────────────

#[test]
fn code_after_return() {
    assert_eq!(unreachable_warnings("fn main(){\n    return\n    let x = 1\n}"), 1);
}

#[test]
fn multiple_stmts_after_return() {
    // One warning per block, not one per statement
    assert_eq!(
        unreachable_warnings("fn main(){\n    return\n    let x = 1\n    let y = 2\n    print(\"hi\")\n}"),
        1
    );
}

#[test]
fn code_after_break() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    while true {\n        break\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn code_after_continue() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    for i in 0..10 {\n        continue\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn if_else_both_return() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    if true {\n        return\n    } else {\n        return\n    }\n    let x = 1\n}"),
        1
    );
}

#[test]
fn if_else_both_break() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    while true {\n        if true {\n            break\n        } else {\n            break\n        }\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn match_all_return() {
    assert_eq!(
        unreachable_warnings("enum E {\n    A\n    B\n}\n\nfn main(){\n    match E.A {\n        E.A {\n            return\n        }\n        E.B {\n            return\n        }\n    }\n    let x = 1\n}"),
        1
    );
}

#[test]
fn nested_return() {
    // Only the second return terminates the block; the if is conditional
    assert_eq!(
        unreachable_warnings("fn main(){\n    if true {\n        return\n    }\n    return\n    let x = 1\n}"),
        1
    );
}

#[test]
fn call_then_unreachable() {
    assert_eq!(
        unreachable_warnings("fn f() {}\n\nfn main(){\n    f()\n    return\n    let x = 1\n}"),
        1
    );
}

#[test]
fn unreachable_in_if_branch() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    if true {\n        return\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn unreachable_in_else_branch() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    if true {\n        print(1)\n    } else {\n        return\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn unreachable_in_match_arm() {
    assert_eq!(
        unreachable_warnings("enum E {\n    A\n    B\n}\n\nfn main(){\n    match E.A {\n        E.A {\n            return\n            let x = 1\n        }\n        E.B {\n            print(1)\n        }\n    }\n}"),
        1
    );
}

#[test]
fn for_body_unreachable() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    for i in 0..10 {\n        return\n        let x = 1\n    }\n}"),
        1
    );
}

#[test]
fn method_body_unreachable() {
    assert_eq!(
        unreachable_warnings("class C {\n    fn go(self) int {\n        return 1\n        let x = 2\n    }\n}\n\nfn main(){\n    let c = C {}\n    print(c.go())\n}"),
        1
    );
}

#[test]
fn closure_body_unreachable() {
    assert_eq!(
        unreachable_warnings("fn main(){\n    let f = (x: int) => {\n        return x\n        let y = 2\n    }\n    print(f(1))\n}"),
        1
    );
}

// ── Does not warn ────────────────────────────────────────────────────────────

#[test]
fn code_after_raise_warns() {
    // `raise` terminates the block; a trailing `return x` is dead code
    // (raise alone satisfies the return requirement — see below)
    assert_eq!(
        unreachable_warnings("error E {}\n\nfn f() int {\n    raise E {}\n    return 0\n}\n\nfn main(){\n    print(f() catch e { -1 })\n}"),
        1
    );
}

#[test]
fn raise_as_final_statement_no_warning() {
    // A fallible fn may end on raise with no return; nothing to warn about
    assert_eq!(
        unreachable_warnings("error E {}\n\nfn f() int {\n    raise E {}\n}\n\nfn main(){\n    print(f() catch e { -1 })\n}"),
        0
    );
}

#[test]
fn while_with_break_reachable() {
    // Code after a breakable loop is reachable
    assert_eq!(
        unreachable_warnings("fn main(){\n    while true {\n        break\n    }\n    let x = 1\n    print(x)\n}"),
        0
    );
}

#[test]
fn infinite_loop_no_warning() {
    // The analysis does not treat `while true` without break as terminating
    assert_eq!(
        unreachable_warnings("fn f() {\n    while true {\n        let x = 1\n    }\n    let y = 2\n    print(y)\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn conditional_return_no_warning() {
    assert_eq!(
        unreachable_warnings("fn f(n: int) int {\n    if n > 0 {\n        return 1\n    }\n    return 0\n}\n\nfn main(){\n    print(f(1))\n}"),
        0
    );
}

#[test]
fn match_partial_return_no_warning() {
    assert_eq!(
        unreachable_warnings("enum E {\n    A\n    B\n}\n\nfn main(){\n    match E.A {\n        E.A {\n            return\n        }\n        E.B {\n            print(1)\n        }\n    }\n    let x = 1\n    print(x)\n}"),
        0
    );
}

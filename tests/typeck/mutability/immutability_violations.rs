//! Immutability violation tests.
//!
//! Variables declared with plain `let` cannot be reassigned, and fields
//! cannot be assigned through an immutable binding. Known enforcement gaps
//! (container element assignment, loop variables, match bindings, parameter
//! reassignment) are kept as ignored tests with accurate reasons.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

const IMMUT: &str = "cannot assign to immutable variable";

// Reassign immutable variable
#[test]
fn reassign_immutable() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    x = 2\n}",
        IMMUT,
    );
}

// Reassign parameter
#[test]
#[ignore] // #278 enforcement gap: parameter reassignment (fn f(x: int) { x = 2 }) is not rejected
fn reassign_param() {
    compile_should_fail_with(
        "fn f(x: int) {\n    x = 2\n}\n\nfn main(){}",
        IMMUT,
    );
}

// Reassign in loop body
#[test]
fn reassign_in_loop() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    for i in 0..10 {\n        x = i\n    }\n}",
        IMMUT,
    );
}

// Reassign in if branch
#[test]
fn reassign_in_if() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    if true {\n        x = 2\n    }\n}",
        IMMUT,
    );
}

// Reassign captured variable inside a closure
#[test]
fn reassign_captured() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    let f = () => {\n        x = 2\n    }\n    f()\n}",
        IMMUT,
    );
}

// Reassign field of immutable instance
#[test]
fn reassign_immut_field() {
    compile_should_fail_with(
        "class C {\n    x: int\n}\n\nfn main(){\n    let c = C { x: 1 }\n    c.x = 2\n}",
        "cannot assign to field of immutable variable",
    );
}

// Reassign array element through immutable binding
#[test]
#[ignore] // #278 enforcement gap: index assignment (arr[0] = 5) through an immutable binding is not rejected
fn reassign_array_elem() {
    compile_should_fail_with(
        "fn main(){\n    let arr = [1, 2, 3]\n    arr[0] = 5\n}",
        "",
    );
}

// Reassign map value through immutable binding
#[test]
#[ignore] // #278 enforcement gap: map insertion (m[\"a\"] = 2) through an immutable binding is not rejected
fn reassign_map_value() {
    compile_should_fail_with(
        "fn main(){\n    let m = Map<string, int> { \"a\": 1 }\n    m[\"a\"] = 2\n}",
        "",
    );
}

// Mutate outer variable inside a match arm
#[test]
fn mutate_in_match() {
    compile_should_fail_with(
        "enum E {\n    A\n    B\n}\n\nfn main(){\n    let x = 1\n    match E.A {\n        E.A {\n            x = 2\n        }\n        E.B {\n            x = 3\n        }\n    }\n}",
        IMMUT,
    );
}

// Mutate loop variable
#[test]
#[ignore] // #278 enforcement gap: loop variables (for i in .. { i = i + 1 }) are reassignable
fn mutate_loop_var() {
    compile_should_fail_with(
        "fn main(){\n    for i in 0..10 {\n        i = i + 1\n    }\n}",
        IMMUT,
    );
}

// Mutate match binding
#[test]
#[ignore] // #278 enforcement gap: match bindings (E.A { x } { x = 2 }) are reassignable
fn mutate_match_binding() {
    compile_should_fail_with(
        "enum E {\n    A { x: int }\n}\n\nfn main(){\n    match (E.A { x: 1 }) {\n        E.A { x } {\n            x = 2\n        }\n    }\n}",
        IMMUT,
    );
}

// Reassign after the variable was captured by a closure
#[test]
fn mutate_through_closure() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    let f = () => x\n    let y = f()\n    x = 2\n}",
        IMMUT,
    );
}

// Reassign after passing to spawn
#[test]
fn mutate_spawn_arg() {
    compile_should_fail_with(
        "fn task(x: int) int {\n    return x\n}\n\nfn main(){\n    let x = 1\n    let t = spawn task(x)\n    x = 2\n}",
        IMMUT,
    );
}

// Reassign after a catch expression
#[test]
fn reassign_after_catch() {
    compile_should_fail_with(
        "error E {\n    msg: string\n}\n\nfn f() int {\n    raise E { msg: \"x\" }\n}\n\nfn main(){\n    let x = 1\n    let y = f() catch e { 0 }\n    x = 2\n}",
        IMMUT,
    );
}

// Mutate a field through a narrowed nullable binding
#[test]
fn mutate_through_nullable() {
    compile_should_fail_with(
        "class C {\n    x: int\n}\n\nfn main(){\n    let c: C? = C { x: 1 }\n    if c != none {\n        c.x = 2\n    }\n}",
        "cannot assign to field of immutable variable",
    );
}

// Reassign in while body
#[test]
fn reassign_in_while() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    while x < 10 {\n        x = x + 1\n    }\n}",
        IMMUT,
    );
}

// Reassign with self-referencing value
#[test]
fn reassign_const() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    x = x * 2\n}",
        IMMUT,
    );
}

// Mutate iteration variable over an array
#[test]
#[ignore] // #278 enforcement gap: loop variables (for x in arr { x = x + 1 }) are reassignable
fn mutate_array_iter() {
    compile_should_fail_with(
        "fn main(){\n    let arr = [1, 2, 3]\n    for x in arr {\n        x = x + 1\n    }\n}",
        IMMUT,
    );
}

// Reassign in nested scope
#[test]
fn reassign_nested() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    if true {\n        if true {\n            x = 2\n        }\n    }\n}",
        IMMUT,
    );
}

// Mutate self field in a non-mut method
#[test]
fn mutate_self_non_mut() {
    compile_should_fail_with(
        "class C {\n    x: int\n\n    fn update(self) {\n        self.x = self.x + 1\n    }\n}\n\nfn main(){}",
        "non-mut method",
    );
}

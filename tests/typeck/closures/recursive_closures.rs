//! Recursive closures are not supported.
//!
//! A closure's own name is not in scope inside its body (`let f = ... f(x)`),
//! and forward references to later closures are equally undefined — recursion
//! belongs to named functions. The if-expression arrow-body corner is
//! inconsistent today and stays ignored (see the tracking issue).
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn self_capture() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => f(x - 1)\n}",
        "undefined function 'f'",
    );
}

#[test]
fn block_body_recursive() {
    compile_should_fail_with(
        "fn main(){\n    let fac = (n: int) => {\n        if n <= 1 {\n            return 1\n        }\n        return n * fac(n - 1)\n    }\n    print(fac(5))\n}",
        "undefined function 'fac'",
    );
}

#[test]
fn mutual_recursion() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => g(x)\n    let g = (y: int) => f(y)\n}",
        "undefined function 'g'",
    );
}

#[test]
fn nested_recursive() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => {\n        let g = (y: int) => g(y - 1)\n        return g(x)\n    }\n}",
        "undefined function 'g'",
    );
}

#[test]
fn multi_param_recursive() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int, y: int) => f(x - 1, y - 1)\n}",
        "undefined function 'f'",
    );
}

#[test]
fn indirect_recursion() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => g(x)\n    let g = (y: int) => f(y)\n    print(f(1))\n}",
        "undefined function 'g'",
    );
}

#[test]
fn struct_recursive_closure() {
    // Referencing the struct binding from its own initializer
    compile_should_fail_with(
        "class C {\n    f: fn(int) int\n}\n\nfn main(){\n    let c = C { f: (x: int) => c.f(x - 1) }\n}",
        "undefined",
    );
}

#[test]
fn array_recursive() {
    // Calling an indexed expression (arr[0](x)) is not parseable either
    compile_should_fail_with(
        "fn main(){\n    let arr = [(x: int) => arr[0](x - 1)]\n}",
        "expected ,",
    );
}

#[test]
#[ignore] // Inconsistent: if-expression arrow bodies with self-reference (see tracking issue)
fn if_expr_body_recursive() {
    compile_should_fail_with(
        "fn main(){\n    let f = (x: int) => if x > 0 { return f(x - 1) } else { return 0 }\n}",
        "undefined function 'f'",
    );
}

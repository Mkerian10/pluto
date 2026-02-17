mod common;
use common::compile_and_run_stdout;

#[test]
fn closure_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = (x: int) => x + 1\n    print(f(5))\n}",
    );
    assert_eq!(out.trim(), "6");
}

#[test]
fn closure_no_params() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = () => 42\n    print(f())\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn closure_multi_params() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = (x: int, y: int) => x + y\n    print(f(3, 7))\n}",
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn closure_block_body() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = (x: int) => {\n        let y = x + 1\n        return y * 2\n    }\n    print(f(5))\n}",
    );
    assert_eq!(out.trim(), "12");
}

#[test]
fn closure_capture() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 10\n    let f = (x: int) => x + a\n    print(f(5))\n}",
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn closure_capture_by_value() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 10\n    let f = (x: int) => x + a\n    let a = 999\n    print(f(5))\n}",
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn closure_multiple_captures() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 10\n    let b = 20\n    let f = (x: int) => x + a + b\n    print(f(5))\n}",
    );
    assert_eq!(out.trim(), "35");
}

#[test]
fn closure_higher_order() {
    let out = compile_and_run_stdout(
        "fn apply(f: fn(int) int, x: int) int {\n    return f(x)\n}\n\nfn main() {\n    let f = (x: int) => x * 3\n    print(apply(f, 7))\n}",
    );
    assert_eq!(out.trim(), "21");
}

#[test]
fn closure_return_from_fn() {
    let out = compile_and_run_stdout(
        "fn make_adder(n: int) fn(int) int {\n    let f = (x: int) => x + n\n    return f\n}\n\nfn main() {\n    let add5 = make_adder(5)\n    print(add5(10))\n}",
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn closure_returning_closure() {
    let out = compile_and_run_stdout(
        "fn make_multiplier(factor: int) fn(int) int {\n    let f = (x: int) => x * factor\n    return f\n}\n\nfn main() {\n    let double = make_multiplier(2)\n    let triple = make_multiplier(3)\n    print(double(5))\n    print(triple(5))\n}",
    );
    assert_eq!(out, "10\n15\n");
}

#[test]
fn closure_capture_loop_variable() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut sum = 0\n    for i in 0..5 {\n        let captured = i\n        let f = () => captured\n        sum = sum + f()\n    }\n    print(sum)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn closure_capture_mixed_types() {
    let out = compile_and_run_stdout(
        "class Point {\n    x: int\n    y: int\n}\n\nfn main() {\n    let n = 42\n    let s = \"hello\"\n    let p = Point { x: 1, y: 2 }\n    let f = () => {\n        print(n)\n        print(s)\n        print(p.x + p.y)\n    }\n    f()\n}",
    );
    assert_eq!(out, "42\nhello\n3\n");
}

// ============================================================
// If-Expression Integration Tests
// ============================================================

#[test]
fn if_expr_in_closure_body() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let f = (x: int) => if x > 0 { x } else { -x }
            print(f(10))
            print(f(-5))
        }
        "#,
    );
    assert_eq!(out.trim(), "10\n5");
}

#[test]
fn closure_in_if_expr_branch() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let f = if true {
                (x: int) => x * 2
            } else {
                (x: int) => x * 3
            }
            print(f(10))
        }
        "#,
    );
    assert_eq!(out.trim(), "20");
}

#[test]
fn closure_capturing_if_expr_value() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let x = if true { 10 } else { 20 }
            let f = () => x + 5
            print(f())
        }
        "#,
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn nested_closures_with_if_expr() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let outer = (a: int) => (b: int) => if a > b { a } else { b }
            let inner = outer(10)
            print(inner(5))
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

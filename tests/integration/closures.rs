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

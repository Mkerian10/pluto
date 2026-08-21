mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

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
        "fn main() {\n    let mut a = 10\n    let f = (x: int) => x + a\n    a = 999\n    print(f(5))\n}",
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

// ── Closure error inference ──────────────────────────────────────────────────
// Closures bound to variables get their own node in the error graph: calls
// through the variable are enforced and can be handled with catch/!.

#[test]
fn fallible_closure_call_with_catch() {
    let out = compile_and_run_stdout(
        r#"
error ClosureError { value: int }

fn main() {
    let threshold = 10
    let check = (x: int) => {
        if x > threshold {
            raise ClosureError { value: x }
        }
        return x * 2
    }

    let a = check(5) catch 0
    print(a)
    let b = check(15) catch 0
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "10\n0");
}

#[test]
fn unhandled_fallible_closure_call_rejected() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn main() {
    let f = (x: int) => {
        if x > 3 {
            raise E { code: x }
        }
        return x
    }
    let a = f(5)
    print(a)
}
"#,
        "call to fallible closure 'f' must be handled with ! or catch",
    );
}

#[test]
fn catch_on_infallible_closure_rejected() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = (x: int) => x + 1
    let a = f(5) catch 0
    print(a)
}
"#,
        "catch applied to infallible function 'f'",
    );
}

#[test]
fn closure_error_propagates_through_function() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn use_it(x: int) int {
    let f = (n: int) => {
        if n > 3 {
            raise E { code: n }
        }
        return n * 2
    }
    return f(x)!
}

fn main() {
    let ok = use_it(2) catch -1
    print(ok)
    let bad = use_it(9) catch err: E { err.code }
    print(bad)
}
"#,
    );
    assert_eq!(out.trim(), "4\n9");
}

#[test]
fn closure_calling_fallible_function_is_fallible() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn raiser() int {
    raise E { code: 3 }
    return 0
}

fn main() {
    let f = (x: int) => {
        return raiser()! + x
    }
    let a = f(10) catch err: E { err.code }
    print(a)
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn typed_catch_must_cover_closure_error_set() {
    compile_should_fail_with(
        r#"
error A { x: int }
error B { y: int }

fn main() {
    let f = (n: int) => {
        if n == 1 {
            raise A { x: 1 }
        }
        raise B { y: 2 }
        return n
    }
    let a = f(1) catch err: A { 0 }
    print(a)
}
"#,
        "no catch handler covers",
    );
}

#[test]
fn function_handling_closure_error_internally_is_infallible() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn handles_internally(x: int) int {
    let f = (n: int) => {
        if n > 3 {
            raise E { code: n }
        }
        return n
    }
    return f(x) catch -1
}

fn main() {
    let a = handles_internally(9)
    print(a)
    let b = handles_internally(2)
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "-1\n2");
}

#[test]
fn fallible_closure_into_infallible_contract_rejected() {
    // fn(int) int is an infallible contract: a fallible closure cannot cross
    // it. Declare the parameter fallible (fn(int) int!) or handle inside.
    compile_should_fail_with(
        r#"
error E { code: int }

fn apply(g: fn(int) int, x: int) int {
    return g(x)
}

fn definer(x: int) int {
    let f = (n: int) => {
        if n > 3 {
            raise E { code: n }
        }
        return n
    }
    return apply(f, x)
}

fn main() {
    print(definer(1))
}
"#,
        "cannot pass fallible 'f' where an infallible function type is expected",
    );
}

#[test]
fn fallible_closure_into_fallible_contract_works() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn apply(g: fn(int) int!, x: int) int {
    return g(x)!
}

fn definer(x: int) int {
    let f = (n: int) => {
        if n > 3 {
            raise E { code: n }
        }
        return n
    }
    return apply(f, x) catch -1
}

fn main() {
    print(definer(1))
    print(definer(9))
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

#[test]
fn reassigned_closure_unions_error_sets() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn main() {
    let mut f = (n: int) => n + 1
    f = (n: int) => {
        raise E { code: n }
        return n
    }
    let a = f(5) catch -1
    print(a)
}
"#,
    );
    assert_eq!(out.trim(), "-1");
}

#[test]
fn nested_closure_error_propagation() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn main() {
    let counter = (n: int) => {
        let inner = (m: int) => {
            raise E { code: m }
            return m
        }
        return inner(n)!
    }
    let a = counter(4) catch err: E { err.code + 100 }
    print(a)
}
"#,
    );
    assert_eq!(out.trim(), "104");
}

#[test]
fn nested_closure_calls_captured_fn_typed_var() {
    // A call's callee is a name, not an Ident expression — the capture
    // collector must still capture an outer fn-typed variable that is only
    // ever *called* inside the nested closure.
    let out = compile_and_run_stdout(
        r#"
fn compose(f: fn(int) int, g: fn(int) int) fn(int) int {
    return (x: int) => g(f(x))
}

fn main() {
    let d = (x: int) => x * 2
    let s = (x: int) => x * x
    let c = compose(d, s)
    print(c(3))
}
"#,
    );
    assert_eq!(out.trim(), "36");
}

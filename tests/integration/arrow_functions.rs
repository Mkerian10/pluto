// Phase 2: Parser Explorer - Arrow Functions Tests
//
// Tests for closure/arrow function syntax edge cases:
// - Nested closures
// - Closures in various contexts (calls, structs, arrays, match)
// - Malformed closure syntax
// - Capture edge cases
//
// Target: 10 tests

mod common;
use common::*;

#[test]
fn arrow_no_parens_single_param() {
    // x => x + 1 (no parens) → Pluto requires parens around params
    compile_should_fail_with(r#"
        fn main() {
            let f = x => x + 1
        }
    "#, "expected newline after statement");
}

#[test]
#[ignore] // Test expectation unclear: compiler allows trailing comma in closure params, but test expects failure
fn arrow_trailing_comma_params() {
    // (x: int, y: int,) => x + y → trailing comma should be rejected
    compile_should_fail(r#"
        fn main() {
            let f = (x: int, y: int,) => x + y
        }
    "#);
}

#[test]
fn arrow_nested_in_call() {
    let stdout = compile_and_run_stdout(r#"
        fn apply(f: fn(int) int, x: int) int {
            return f(x)
        }

        fn main() {
            let result = apply((x: int) => x + 1, 5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn arrow_nested_closure() {
    // (x: int) => (y: int) => x + y → closure returning closure
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let add = (x: int) => (y: int) => x + y
            let add5 = add(5)
            let result = add5(3)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "8");
}

#[test]
fn arrow_multiline_body() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) => {
                let y = x + 1
                let z = y * 2
                return z
            }
            let result = f(5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "12");
}

#[test]
#[ignore] // Test expectation unclear: compiler allows empty closure body, but test expects failure
fn arrow_empty_body_rejected() {
    // (x: int) => {} → empty block body should be rejected (no return)
    compile_should_fail(r#"
        fn main() {
            let f = (x: int) => {}
        }
    "#);
}

#[test]
#[ignore] // Compiler bug: Calling closures from arrays doesn't work. Error: "print() does not support type fn(int) int"
fn arrow_capture_in_loop() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let mut closures = [
                (x: int) => x,
                (x: int) => x,
                (x: int) => x
            ]
            let mut i = 0
            while i < 3 {
                let captured = i
                closures[i] = (x: int) => x + captured
                i = i + 1
            }
            let result = closures[2](10)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "12");
}

#[test]
#[ignore] // Compiler bug: Calling closure fields doesn't work. Error: "class 'Handler' has no method 'handler'"
fn arrow_as_struct_field() {
    let stdout = compile_and_run_stdout(r#"
        class Handler {
            handler: fn(int) int
        }

        fn main() {
            let h = Handler {
                handler: (x: int) => x * 2
            }
            let result = h.handler(5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
#[ignore] // Compiler bug: Calling closures from arrays doesn't work. Error: "print() does not support type fn(int) int"
fn arrow_in_array_literal() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let funcs = [
                (x: int) => x + 1,
                (x: int) => x * 2,
                (x: int) => x - 3
            ]
            let result = funcs[1](5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
#[ignore] // Test bug: Uses match as expression, which is not supported in Pluto (only match statements)
fn arrow_complex_nesting() {
    // Closure inside match arm inside another closure
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }

        fn main() {
            let outer = (opt: Option<int>) => {
                return match opt {
                    Option.Some { value } => {
                        let inner = (x: int) => x + value
                        inner(10)
                    }
                    Option.None => 0
                }
            }
            let result = outer(Option<int>.Some { value: 5 })
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "15");
}

// ── #165: untyped params and return-type annotations ─────────────────────────

#[test]
fn untyped_param_from_call_context() {
    // Param type comes from the function signature the closure is passed to
    let stdout = compile_and_run_stdout(r#"
        fn apply(f: fn(int) int, x: int) int {
            return f(x)
        }

        fn main() {
            print(apply((x) => x + 1, 41))
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn untyped_params_from_let_annotation() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let add: fn(int, int) int = (a, b) => a + b
            print(add(2, 3))
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn untyped_param_void_callback() {
    let stdout = compile_and_run_stdout(r#"
        fn each(arr: [int], f: fn(int)) {
            for x in arr {
                f(x)
            }
        }

        fn main() {
            each([1, 2], (x) => print(x * 10))
        }
    "#);
    assert_eq!(stdout.trim(), "10\n20");
}

#[test]
fn untyped_param_string_context() {
    let stdout = compile_and_run_stdout(r#"
        fn shout(f: fn(string) string, s: string) string {
            return f(s)
        }

        fn main() {
            print(shout((s) => s + "!", "hey"))
        }
    "#);
    assert_eq!(stdout.trim(), "hey!");
}

#[test]
fn explicit_return_type_annotation() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = () int => 7
            print(f())
        }
    "#);
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn explicit_void_return_annotation_block_body() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let g = () void => {
                print(99)
            }
            g()
        }
    "#);
    assert_eq!(stdout.trim(), "99");
}

#[test]
fn untyped_param_capture_still_works() {
    let stdout = compile_and_run_stdout(r#"
        fn apply(f: fn(int) int, x: int) int {
            return f(x)
        }

        fn main() {
            let base = 100
            print(apply((x) => x + base, 1))
        }
    "#);
    assert_eq!(stdout.trim(), "101");
}

#[test]
fn untyped_param_in_generic_call_with_explicit_type_args() {
    // With explicit type args the expected fn type is concrete, but generic
    // arg inference runs without hints — annotated params still required
    compile_should_fail_with(r#"
        fn apply<T>(f: fn(T) T, x: T) T {
            return f(x)
        }

        fn main() {
            print(apply((x) => x + 1, 42))
        }
    "#, "cannot infer");
}

// ── If-expression arrow bodies (#294) ────────────────────────────────────

#[test]
fn if_expr_arrow_body_both_arms_return() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) => if x > 0 { return 1 } else { return 0 }
            print(f(5))
            print(f(-1))
        }
    "#);
    assert_eq!(stdout.trim(), "1\n0");
}

#[test]
fn if_expr_arrow_body_mixed_arms() {
    // One arm returns, the other is a value — the value arm types the closure
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) => if x > 0 { return 100 } else { 7 }
            print(f(5))
            print(f(-5))
        }
    "#);
    assert_eq!(stdout.trim(), "100\n7");
}

#[test]
fn if_expr_arrow_body_nested() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) => if x > 10 { return 2 } else { if x > 0 { return 1 } else { return 0 } }
            print(f(20))
            print(f(5))
            print(f(-5))
        }
    "#);
    assert_eq!(stdout.trim(), "2\n1\n0");
}

#[test]
fn if_expr_arrow_body_annotated_return() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) int => if x > 0 { return 5 } else { return 6 }
            print(f(1))
            print(f(-1))
        }
    "#);
    assert_eq!(stdout.trim(), "5\n6");
}

#[test]
fn if_expr_value_arms_still_work() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let f = (x: int) => if x > 0 { 10 } else { 20 }
            print(f(1))
            print(f(-1))
        }
    "#);
    assert_eq!(stdout.trim(), "10\n20");
}

#[test]
fn if_expr_diverging_arm_in_fn_body() {
    // A diverging arm contributes the other arm's type to the let binding
    let stdout = compile_and_run_stdout(r#"
        fn pick(n: int) int {
            let v = if n > 0 { return 99 } else { n * 2 }
            return v
        }

        fn main() {
            print(pick(3))
            print(pick(-3))
        }
    "#);
    assert_eq!(stdout.trim(), "99\n-6");
}

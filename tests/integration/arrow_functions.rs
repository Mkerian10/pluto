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
    compile_should_fail(r#"
        fn main() {
            let f = x => x + 1
        }
    "#);
}

#[test]
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
fn arrow_empty_body_rejected() {
    // (x: int) => {} → empty block body should be rejected (no return)
    compile_should_fail(r#"
        fn main() {
            let f = (x: int) => {}
        }
    "#);
}

#[test]
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

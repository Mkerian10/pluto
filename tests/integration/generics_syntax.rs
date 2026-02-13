// Phase 2: Parser Explorer - Generics Syntax Tests
//
// Tests for generic type syntax edge cases, including:
// - Deep nesting
// - >> ambiguity with shift operators
// - Comparison operator conflicts
// - Malformed generic syntax
//
// Target: 10 tests

mod common;
use common::*;

#[test]
#[ignore] // Compiler bug: Generic nullable field doesn't accept T values (expected string?, found string)
fn generic_nested_three_levels() {
    let stdout = compile_and_run_stdout(r#"
        class Box<T> {
            value: T
        }
        class Pair<A, B> {
            first: A
            second: B
        }
        class Option<T> {
            value: T?
        }

        fn main() {
            let x = Box<Pair<int, Option<string>>> {
                value: Pair<int, Option<string>> {
                    first: 42,
                    second: Option<string> { value: "hello" }
                }
            }
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
#[ignore] // Compiler bug: Generic TypeExpr should not reach codegen (monomorphization issue)
fn generic_map_with_nested_value() {
    let stdout = compile_and_run_stdout(r#"
        class Pair<A, B> {
            first: A
            second: B
        }

        fn main() {
            let m = Map<string, Pair<int, int>> {}
            m["key"] = Pair<int, int> { first: 1, second: 2 }
            let p = m["key"]
            print(p.first)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn generic_array_of_generics() {
    let stdout = compile_and_run_stdout(r#"
        class Box<T> { value: T }

        fn main() {
            let arr = [
                Box<int> { value: 1 },
                Box<int> { value: 2 },
                Box<int> { value: 3 }
            ]
            print(arr[1].value)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
#[ignore] // Compiler bug: Generic nullable field doesn't accept T values (expected [int]?, found [int])
fn generic_fn_return_nested() {
    let stdout = compile_and_run_stdout(r#"
        class Option<T> {
            value: T?
        }

        fn get_optional_array() Option<[int]> {
            return Option<[int]> { value: [1, 2, 3] }
        }

        fn main() int? {
            let opt = get_optional_array()
            let arr = opt.value?
            print(arr[0])
            return none
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn generic_explicit_call_with_expr() {
    let stdout = compile_and_run_stdout(r#"
        fn identity<T>(x: T) T {
            return x
        }

        fn main() {
            let result = identity<int>(2 + 2)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn generic_comparison_ambiguity() {
    // x < y > z should parse as (x < y) > z (comparison), NOT as generic type args
    // This should fail because you can't compare bool > int
    compile_should_fail(r#"
        fn main() {
            let x = 1
            let y = 2
            let z = 3
            let result = x < y > z
        }
    "#);
}

#[test]
fn generic_shift_right_in_nested() {
    // Pair<int, Pair<int, int>> → the >> should NOT be parsed as shift operator
    let stdout = compile_and_run_stdout(r#"
        class Pair<A, B> {
            first: A
            second: B
        }

        fn main() {
            let x = Pair<int, Pair<int, int>> {
                first: 1,
                second: Pair<int, int> { first: 2, second: 3 }
            }
            print(x.second.second)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
#[ignore] // Parser currently accepts trailing commas in generic type args (design decision needed)
fn generic_trailing_comma_rejected() {
    // Box<int,> → trailing comma should be rejected
    compile_should_fail(r#"
        class Box<T> {
            value: T
        }

        fn main() {
            let x = Box<int,> { value: 42 }
        }
    "#);
}

#[test]
fn generic_empty_type_args_rejected() {
    // Box<> → empty type args should be rejected
    compile_should_fail(r#"
        class Box<T> { value: T }

        fn main() {
            let x = Box<> { value: 42 }
        }
    "#);
}

#[test]
#[ignore] // Parser currently accepts space before < in generic syntax (design decision needed)
fn generic_space_before_bracket() {
    // Box <int> → space before < should either fail or parse as comparison
    // This tests that the parser doesn't accidentally accept this as generic syntax
    compile_should_fail(r#"
        class Box<T> {
            value: T
        }

        fn main() {
            let x = Box <int> { value: 42 }
        }
    "#);
}

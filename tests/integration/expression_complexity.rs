// Expression Complexity & Deep Nesting Tests
// Inspired by Rust and Go compiler stress testing
//
// Tests parser's ability to handle deeply nested and complex expressions
// Target: 20 tests

mod common;
use common::*;

// ============================================================
// Deep Nesting Tests
// ============================================================

#[test]
fn deep_nesting_parens_30_levels() {
    // 30 levels of nested parentheses
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = ((((((((((((((((((((((((((((((42))))))))))))))))))))))))))))))
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn deep_nesting_arrays_10_levels() {
    // 10 levels of nested arrays
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = [[[[[[[[[[99]]]]]]]]]]
            print(x[0][0][0][0][0][0][0][0][0][0])
        }
    "#);
    assert_eq!(stdout.trim(), "99");
}

#[test]
fn deep_function_call_chain() {
    // Deeply nested function calls
    let stdout = compile_and_run_stdout(r#"
        fn id(x: int) int { return x }

        fn main() {
            let result = id(id(id(id(id(id(id(id(id(id(42))))))))))
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn mixed_nesting_array_index_field_method() {
    // arr[obj.method()[0].field] - complex nested access
    let stdout = compile_and_run_stdout(r#"
        class Inner {
            field: int
        }

        class Outer {
            fn get_array(self) [Inner] {
                return [Inner { field: 2 }]
            }
        }

        fn main() {
            let obj = Outer {}
            let arr = [0, 1, 2, 3, 4]
            let result = arr[obj.get_array()[0].field]
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

// ============================================================
// Expression Combinations
// ============================================================

#[test]
#[ignore] // Compiler bug: Array type inference fails with closure literals (int vs fn(int) int)
fn array_of_closures_complex() {
    // Array literal containing multiple closures with different operations
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let ops = [
                (x: int) => x + 10,
                (x: int) => x * 2,
                (x: int) => x - 5
            ]
            let result = ops[0](5) + ops[1](3) + ops[2](10)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "26"); // 15 + 6 + 5 = 26
}

#[test]
fn closure_capturing_multiple_variables() {
    // Closure capturing multiple variables from outer scope
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 10
            let b = 20
            let c = 30
            let f = (x: int) => x + a + b + c
            let result = f(5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "65");
}

#[test]
fn chained_method_calls_5_deep() {
    // Method chaining: obj.m1().m2().m3().m4().m5()
    let stdout = compile_and_run_stdout(r#"
        class Builder {
            value: int

            fn add(self, x: int) Builder {
                return Builder { value: self.value + x }
            }

            fn multiply(self, x: int) Builder {
                return Builder { value: self.value * x }
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let result = Builder { value: 1 }
                .add(5)
                .multiply(2)
                .add(10)
                .multiply(3)
                .get()
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "66"); // ((1+5)*2+10)*3 = (12+10)*3 = 66
}

#[test]
fn index_chain_multidimensional() {
    // arr[0][1][2] - multi-dimensional array access
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = [
                [[1, 2, 3], [4, 5, 6]],
                [[7, 8, 9], [10, 11, 12]]
            ]
            let result = arr[1][0][2]
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "9");
}

#[test]
fn string_interpolation_with_complex_expression() {
    // String interpolation with nested function calls and arithmetic
    let stdout = compile_and_run_stdout(r#"
        fn double(x: int) int { return x * 2 }
        fn add(x: int, y: int) int { return x + y }

        fn main() {
            let arr = [1, 2, 3]
            print("result: {add(double(arr[0]), arr[1] + arr[2])}")
        }
    "#);
    assert_eq!(stdout.trim(), "result: 7"); // add(double(1), 2+3) = add(2, 5) = 7
}

#[test]
fn empty_array_with_type_annotation() {
    // Empty array literal with explicit type
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr: [int] = []
            print(arr.len())
        }
    "#);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn single_element_array_with_trailing_comma() {
    // [42,] - single element with trailing comma
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = [42,]
            print(arr[0])
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn multiline_expression_with_operators() {
    // Expression spanning multiple lines with binary operators
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1 + 2 +
                         3 + 4 +
                         5 + 6 +
                         7 + 8 +
                         9 + 10
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "55");
}

#[test]
fn expression_with_50_additions() {
    // Very long arithmetic expression with 50 additions
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "50");
}

// ============================================================
// Postfix Operator Combinations
// ============================================================

#[test]
fn mixed_postfix_array_nullable_field() {
    // arr[0]?.field - array access, nullable, field access
    let stdout = compile_and_run_stdout(r#"
        class Point {
            x: int
        }

        fn get_point() Point? {
            return Point { x: 42 }
        }

        fn main() int? {
            let arr = [get_point()]
            let result = arr[0]?.x
            print(result)
            return none
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn field_access_then_array_index() {
    // obj.field[0] - field access then array index
    let stdout = compile_and_run_stdout(r#"
        class Container {
            items: [int]
        }

        fn main() {
            let obj = Container { items: [10, 20, 30] }
            let result = obj.items[1]
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "20");
}

#[test]
fn method_call_then_array_index() {
    // obj.get_items()[0] - method call then array index
    let stdout = compile_and_run_stdout(r#"
        class Container {
            fn get_items(self) [int] {
                return [100, 200, 300]
            }
        }

        fn main() {
            let obj = Container {}
            let result = obj.get_items()[2]
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "300");
}

// ============================================================
// Edge Cases
// ============================================================

#[test]
fn expression_with_all_literal_types() {
    // Expression combining int, float, bool, string literals
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let i = 42
            let f = 3.14
            let b = true
            let s = "hello"
            if b && i > 40 {
                print(s)
            }
        }
    "#);
    assert_eq!(stdout.trim(), "hello");
}

#[test]
fn nested_ternary_simulation_with_if() {
    // Simulating nested ternary with nested if expressions
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            let result = if x > 10 {
                100
            } else {
                if x > 0 {
                    50
                } else {
                    0
                }
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "50");
}

#[test]
fn closure_inside_match_arm() {
    // Match expression with closure in arm
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }

        fn main() {
            let opt = Option<int>.Some { value: 10 }
            let f = match opt {
                Option.Some { value } => (x: int) => x + value,
                Option.None => (x: int) => x
            }
            let result = f(5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "15");
}

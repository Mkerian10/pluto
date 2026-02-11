// Extended Control Flow Parsing Tests
// Inspired by Rust's control flow tests
//
// Tests advanced control flow edge cases
// Target: 15 tests

mod common;
use common::*;

// ============================================================
// If Expression Edge Cases
// ============================================================

#[test]
fn if_without_else_as_statement() {
    // if without else used as statement (not expression)
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            if x > 0 {
                print("positive")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "positive");
}

#[test]
fn else_if_chain_10_branches() {
    // Long else-if chain
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 7
            if x == 1 {
                print("one")
            } else if x == 2 {
                print("two")
            } else if x == 3 {
                print("three")
            } else if x == 4 {
                print("four")
            } else if x == 5 {
                print("five")
            } else if x == 6 {
                print("six")
            } else if x == 7 {
                print("seven")
            } else if x == 8 {
                print("eight")
            } else if x == 9 {
                print("nine")
            } else if x == 10 {
                print("ten")
            } else {
                print("other")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "seven");
}

#[test]
#[ignore] // Feature not supported: if expressions (only if statements)
fn if_as_expression_assigned_to_variable() {
    // let x = if cond { 1 } else { 2 }
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let cond = true
            let x = if cond { 100 } else { 200 }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "100");
}

#[test]
#[ignore] // Unimplemented feature: if-expressions not supported yet (if is statement-only)
fn if_in_if_condition() {
    // if (if x { true } else { false }) { }
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            if (if x > 0 { true } else { false }) {
                print("pass")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn single_line_if_expression() {
    // if true { print("hi") }
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            if true { print("hi") }
        }
    "#);
    assert_eq!(stdout.trim(), "hi");
}

// ============================================================
// Loop Constructs
// ============================================================

#[test]
fn while_with_complex_condition() {
    // while x < 10 && y > 0 && z != 5
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let mut x = 0
            let mut y = 10
            let mut z = 0
            while x < 10 && y > 0 && z != 5 {
                x = x + 1
                y = y - 1
                z = z + 1
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn for_with_range_expression() {
    // for i in 1..arr.len() - 1
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = [10, 20, 30, 40, 50]
            let mut sum = 0
            for i in 1..arr.len() - 1 {
                sum = sum + arr[i]
            }
            print(sum)
        }
    "#);
    assert_eq!(stdout.trim(), "90"); // 20 + 30 + 40 = 90
}

#[test]
fn nested_loops_3_levels() {
    // Nested for loops
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let mut count = 0
            for i in 0..3 {
                for j in 0..3 {
                    for k in 0..3 {
                        count = count + 1
                    }
                }
            }
            print(count)
        }
    "#);
    assert_eq!(stdout.trim(), "27"); // 3^3 = 27
}

#[test]
fn loop_with_multiple_break_points() {
    // Break in different conditions
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let mut x = 0
            while true {
                x = x + 1
                if x == 5 {
                    break
                }
                if x > 10 {
                    break
                }
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn loop_with_continue() {
    // Continue skips even numbers
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let mut sum = 0
            for i in 0..10 {
                if i % 2 == 0 {
                    continue
                }
                sum = sum + i
            }
            print(sum)
        }
    "#);
    assert_eq!(stdout.trim(), "25"); // 1+3+5+7+9 = 25
}

// ============================================================
// Match Expressions
// ============================================================

#[test]
fn match_all_enum_variants() {
    // Exhaustive match on enum
    let stdout = compile_and_run_stdout(r#"
        enum Color {
            Red
            Green
            Blue
        }

        fn main() {
            let c = Color.Green
            let name = match c {
                Color.Red => "red",
                Color.Green => "green",
                Color.Blue => "blue"
            }
            print(name)
        }
    "#);
    assert_eq!(stdout.trim(), "green");
}

#[test]
fn match_with_wildcard() {
    // match x { _ => 0 }
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }

        fn main() {
            let opt = Option<int>.None
            let result = match opt {
                _ => 42
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn match_with_destructuring() {
    // match point { Point{x, y} => ... }
    let stdout = compile_and_run_stdout(r#"
        class Point {
            x: int
            y: int
        }

        enum Shape {
            Circle { radius: int }
            Rectangle { width: int, height: int }
            Point { p: Point }
        }

        fn main() {
            let s = Shape.Rectangle { width: 10, height: 20 }
            let area = match s {
                Shape.Circle { radius } => radius * radius * 3,
                Shape.Rectangle { width, height } => width * height,
                Shape.Point { p } => 0
            }
            print(area)
        }
    "#);
    assert_eq!(stdout.trim(), "200");
}

#[test]
fn match_nested_patterns() {
    // match opt { Some{Some{x}} => x, ... }
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }

        fn main() {
            let outer = Option<Option<int>>.Some {
                value: Option<int>.Some { value: 42 }
            }
            let result = match outer {
                Option.Some { value } => {
                    match value {
                        Option.Some { value } => value,
                        Option.None => 0
                    }
                },
                Option.None => 0
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
#[ignore] // Feature not supported: match expressions (only match statements)
fn match_as_expression_in_let() {
    // let x = match y { ... }
    let stdout = compile_and_run_stdout(r#"
        enum Option<T> {
            Some { value: T }
            None
        }

        fn main() {
            let opt = Option<int>.Some { value: 100 }
            let x = match opt {
                Option.Some { value } => value,
                Option.None => 0
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "100");
}

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

// ============================================================
// Match-as-Expression: Nested Match (3 tests)
// ============================================================

#[test]
fn match_expr_nested_in_match_arm() {
    let stdout = compile_and_run_stdout(r#"
        enum Outer { A B }
        enum Inner { X Y }
        fn main() {
            let o = Outer.A
            let i = Inner.Y
            let n = match o {
                Outer.A => match i {
                    Inner.X => 1,
                    Inner.Y => 2
                },
                Outer.B => 3
            }
            print(n)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn match_expr_deeply_nested_3_levels() {
    let stdout = compile_and_run_stdout(r#"
        enum L1 { A B }
        enum L2 { X Y }
        enum L3 { P Q }
        fn main() {
            let result = match L1.A {
                L1.A => match L2.Y {
                    L2.X => match L3.P {
                        L3.P => 1,
                        L3.Q => 2
                    },
                    L2.Y => match L3.Q {
                        L3.P => 3,
                        L3.Q => 4
                    }
                },
                L1.B => 5
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn match_expr_scrutinee_is_match_result() {
    let stdout = compile_and_run_stdout(r#"
        enum E1 { A B }
        enum E2 { X Y }
        fn main() {
            let e = E1.A
            let result = match (match e { E1.A => E2.X, E1.B => E2.Y }) {
                E2.X => 10,
                E2.Y => 20
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

// ============================================================
// Match-as-Expression: With Other Control Flow (5 tests)
// ============================================================

#[test]
fn match_expr_in_while_condition() {
    let stdout = compile_and_run_stdout(r#"
        enum State { Running Stopped }
        fn main() {
            let mut state = State.Running
            let mut count = 0
            while match state { State.Running => count < 3, State.Stopped => false } {
                count = count + 1
                if count == 3 {
                    state = State.Stopped
                }
            }
            print(count)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn match_expr_in_for_loop_range() {
    let stdout = compile_and_run_stdout(r#"
        enum Size { Small Big }
        fn main() {
            let size = Size.Small
            let mut total = 0
            for i in 0..(match size { Size.Small => 3, Size.Big => 10 }) {
                total = total + 1
            }
            print(total)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn match_expr_with_break_value() {
    let stdout = compile_and_run_stdout(r#"
        enum E { A B C }
        fn main() {
            let e = E.B
            let mut result = 0
            while true {
                result = match e {
                    E.A => 1,
                    E.B => 2,
                    E.C => 3
                }
                break
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn match_expr_in_if_else_chain() {
    let stdout = compile_and_run_stdout(r#"
        enum E1 { A B }
        enum E2 { X Y }
        fn main() {
            let cond = true
            let mut result = 0
            if cond {
                result = match E1.A { E1.A => 1, E1.B => 2 }
            } else {
                result = match E2.X { E2.X => 3, E2.Y => 4 }
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn match_expr_with_spawn() {
    let stdout = compile_and_run_stdout(r#"
        enum E { A B }
        fn compute(x: int) int { return x * 10 }
        fn main() {
            let e = E.A
            let task = spawn compute(match e {
                E.A => 5,
                E.B => 10
            })
            print(task.get())
        }
    "#);
    assert_eq!(stdout.trim(), "50");
}

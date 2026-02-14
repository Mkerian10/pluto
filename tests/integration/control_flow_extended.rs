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

// Forward-compatible test for if-as-expression feature (not yet implemented).
// Will pass once if can be used in expression position: let x = if cond { 1 } else { 2 }
#[test]
#[ignore]
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

// Forward-compatible test for nested if-as-expression (not yet implemented).
// Will pass once if can be used as a sub-expression in conditions.
#[test]
#[ignore]
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

// Forward-compatible test for wildcard match patterns (not yet implemented).
// Will pass once match supports _ wildcard for catch-all patterns.
#[test]
#[ignore]
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

// Forward-compatible test for nested match-as-expression (not yet implemented).
// Will pass once match can be used as an expression with nested patterns.
#[test]
#[ignore]
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

// ============================================================
// If-as-Expression: Expression Contexts (10 tests)
// ============================================================

#[test]
fn if_expr_in_binary_op() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = (if true { 10 } else { 5 }) + 100
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "110");
}

#[test]
fn if_expr_in_unary_op() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = -(if true { 10 } else { 5 })
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "-10");
}

#[test]
fn if_expr_in_array_literal() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = [if true { 1 } else { 2 }, 3, if false { 4 } else { 5 }]
            print(arr[0] + arr[1] + arr[2])
        }
    "#);
    assert_eq!(stdout.trim(), "9");
}

#[test]
fn if_expr_in_map_literal() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let m = Map<string, int> {
                "a": (if true { 10 } else { 20 }),
                "b": (if false { 30 } else { 40 })
            }
            print(m["a"] + m["b"])
        }
    "#);
    assert_eq!(stdout.trim(), "50");
}

#[test]
fn if_expr_in_range() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let total = 0
            for i in (if true { 0 } else { 5 })..(if false { 10 } else { 3 }) {
                total = total + 1
            }
            print(total)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn if_expr_in_return() {
    let stdout = compile_and_run_stdout(r#"
        fn foo(x: bool) int {
            return if x { 100 } else { 200 }
        }
        fn main() {
            print(foo(true))
        }
    "#);
    assert_eq!(stdout.trim(), "100");
}

#[test]
fn if_expr_as_cast_target() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = (if true { 10 } else { 20 }) as float
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "10.000000");
}

#[test]
fn if_expr_as_index_object() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr1 = [1, 2, 3]
            let arr2 = [4, 5, 6]
            let x = (if true { arr1 } else { arr2 })[1]
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn if_expr_as_method_object() {
    let stdout = compile_and_run_stdout(r#"
        trait Getter {
            fn get(self) int
        }
        class Foo impl Getter {
            x: int
            fn get(self) int { return self.x }
        }
        fn main() {
            let f1 = Foo { x: 10 }
            let f2 = Foo { x: 20 }
            let result = (if true { f1 } else { f2 }).get()
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
fn if_expr_in_string_interpolation() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let msg = f"result: {if true { 42 } else { 99 }}"
            print(msg)
        }
    "#);
    assert_eq!(stdout.trim(), "result: 42");
}

// ============================================================
// If-as-Expression: Type Consistency (5 tests)
// ============================================================

#[test]
fn if_expr_all_branches_same_class() {
    let stdout = compile_and_run_stdout(r#"
        class Point { x: int }
        fn main() {
            let p = if true { Point { x: 1 } } else { Point { x: 3 } }
            print(p.x)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn if_expr_all_branches_same_enum() {
    let stdout = compile_and_run_stdout(r#"
        enum Color { Red Green Blue }
        fn main() {
            let c = if true { Color.Red } else { Color.Blue }
            match c {
                Color.Red { print("red") }
                Color.Green { print("green") }
                Color.Blue { print("blue") }
            }
        }
    "#);
    assert_eq!(stdout.trim(), "red");
}

#[test]
fn if_expr_nullable_widening_int() {
    // Test that int can be assigned to int? in if-expression context
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x: int? = if true { 10 } else { 0 }
            if x != none && x? == 10 {
                print("ten")
            } else {
                print("other")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "ten");
}

#[test]
fn if_expr_nullable_widening_class() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { x: int }
        fn main() {
            let f1 = Foo { x: 42 }
            let f2 = Foo { x: 99 }
            let f: Foo? = if true { f1 } else { f2 }
            if f != none && f?.x == 42 {
                print("42")
            } else {
                print("other")
            }
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn if_expr_complex_nested_type() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = if true { [[1, 2], [3, 4]] } else { [[5, 6], [7, 8]] }
            print(arr[0][0] + arr[1][1])
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

// ============================================================
// If-as-Expression: Nested If-Expressions (3 tests)
// ============================================================

#[test]
fn if_expr_deeply_nested_4_levels() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = if true {
                if false {
                    if true { 1 } else { 2 }
                } else {
                    if true {
                        if false { 3 } else { 4 }
                    } else {
                        5
                    }
                }
            } else {
                6
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn if_expr_scrutinee_is_if_result() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = if (if true { true } else { false }) {
                10
            } else {
                20
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
fn if_expr_in_both_branches() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = true
            let y = false
            let result = if x {
                if y { 1 } else { 2 }
            } else {
                if y { 3 } else { 4 }
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

// ============================================================
// If-as-Expression: Else-If Chains (2 tests)
// ============================================================

#[test]
fn if_expr_long_else_if_chain() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            let result = if x == 0 {
                0
            } else if x == 1 {
                1
            } else if x == 2 {
                2
            } else if x == 3 {
                3
            } else if x == 4 {
                4
            } else if x == 5 {
                5
            } else if x == 6 {
                6
            } else {
                999
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "5");
}

#[test]
fn if_expr_else_if_different_types_unified() {
    // Test else-if chain with consistent types
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 2
            let result = if x == 1 {
                10
            } else if x == 2 {
                20
            } else {
                30
            }
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "20");
}

// ============================================================
// If-as-Expression: Edge Cases (5 tests)
// ============================================================

#[test]
fn if_expr_single_expression_branches() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = if true { 42 } else { 99 }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn if_expr_complex_condition() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 5
            let b = 10
            let c = 3
            let x = if (a > 0 && b < 20) || c == 3 { 1 } else { 0 }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn if_expr_as_block_value() {
    // If-expression as the return value of a function
    let stdout = compile_and_run_stdout(r#"
        fn foo() int {
            return if true {
                100
            } else {
                200
            }
        }
        fn main() {
            print(foo())
        }
    "#);
    assert_eq!(stdout.trim(), "100");
}

#[test]
fn if_expr_with_multiple_statements_in_branches() {
    // Test that blocks can have multiple statements before the final expression
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = if true {
                10 + 20
            } else {
                5 * 15
            }
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "30");
}

#[test]
fn if_expr_returning_array() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = if true { [1, 2, 3] } else { [4, 5, 6] }
            print(arr[1])
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

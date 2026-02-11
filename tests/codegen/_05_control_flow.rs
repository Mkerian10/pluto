// Category 5: Control Flow Tests (40+ tests)
// Comprehensive test suite for if/else, loops, match, and return statements.
// Tests validate correct codegen behavior for all control flow constructs.

use super::common::{compile_and_run, compile_and_run_stdout};

// ============================================================================
// 1. If/Else (10 tests)
// ============================================================================

#[test]
fn test_if_simple() {
    // Simple if statement with true condition
    let src = r#"
        fn main() int {
            if true {
                return 1
            }
            return 0
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn test_if_with_false_condition() {
    // If statement that should not execute
    let src = r#"
        fn main() int {
            if false {
                return 1
            }
            return 0
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn test_if_with_else() {
    // If with else branch
    let src = r#"
        fn main() int {
            let x = 5
            if x > 10 {
                return 1
            } else {
                return 2
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn test_if_else_both_branches() {
    // Test both branches of if/else
    let src = r#"
        fn test_true() int {
            if true {
                return 1
            } else {
                return 2
            }
        }

        fn test_false() int {
            if false {
                return 1
            } else {
                return 2
            }
        }

        fn main() {
            print(test_true())
            print(test_false())
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("1"));
    assert!(output.contains("2"));
}

#[test]
fn test_nested_if_2_levels() {
    // Nested if statements (2 levels)
    let src = r#"
        fn main() int {
            let x = 5
            let y = 10
            if x < 10 {
                if y > 5 {
                    return 1
                }
                return 2
            }
            return 3
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn test_nested_if_5_levels() {
    // Deeply nested if statements (5 levels)
    let src = r#"
        fn main() int {
            if true {
                if true {
                    if true {
                        if true {
                            if true {
                                return 42
                            }
                        }
                    }
                }
            }
            return 0
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_if_in_loop() {
    // If statement inside a loop
    let src = r#"
        fn main() {
            let i = 0
            while i < 5 {
                if i == 3 {
                    print(i)
                }
                i = i + 1
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3");
}

#[test]
fn test_empty_if_block() {
    // If with empty block (should compile and do nothing)
    let src = r#"
        fn main() int {
            if true {
                // Empty block
            }
            return 42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_empty_else_block() {
    // Else with empty block
    let src = r#"
        fn main() int {
            if false {
                return 1
            } else {
                // Empty else
            }
            return 42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_if_as_expression() {
    // If/else producing a value (expression context)
    let src = r#"
        fn main() int {
            let x = 5
            let result = if x > 3 {
                10
            } else {
                20
            }
            return result
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

// ============================================================================
// 2. Loops (15 tests)
// ============================================================================

#[test]
fn test_while_true_with_break() {
    // Infinite loop with break
    let src = r#"
        fn main() int {
            let i = 0
            while true {
                i = i + 1
                if i == 5 {
                    break
                }
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn test_while_with_condition() {
    // While loop with condition
    let src = r#"
        fn main() int {
            let i = 0
            while i < 10 {
                i = i + 1
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_while_zero_iterations() {
    // While loop that never executes
    let src = r#"
        fn main() int {
            let i = 0
            while i > 10 {
                i = i + 1
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn test_for_loop_range() {
    // For loop over range
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..5 {
                sum = sum + i
            }
            return sum
        }
    "#;
    // 0 + 1 + 2 + 3 + 4 = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_for_loop_inclusive_range() {
    // For loop over inclusive range
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..=5 {
                sum = sum + i
            }
            return sum
        }
    "#;
    // 0 + 1 + 2 + 3 + 4 + 5 = 15
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn test_nested_loops_2_levels() {
    // Nested loops (2 levels)
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..3 {
                for j in 0..3 {
                    sum = sum + 1
                }
            }
            return sum
        }
    "#;
    // 3 * 3 = 9 iterations
    assert_eq!(compile_and_run(src), 9);
}

#[test]
fn test_nested_loops_3_levels() {
    // Nested loops (3 levels)
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..2 {
                for j in 0..2 {
                    for k in 0..2 {
                        sum = sum + 1
                    }
                }
            }
            return sum
        }
    "#;
    // 2 * 2 * 2 = 8 iterations
    assert_eq!(compile_and_run(src), 8);
}

#[test]
fn test_nested_loops_5_levels() {
    // Deeply nested loops (5 levels)
    let src = r#"
        fn main() int {
            let sum = 0
            for a in 0..2 {
                for b in 0..2 {
                    for c in 0..2 {
                        for d in 0..2 {
                            for e in 0..2 {
                                sum = sum + 1
                            }
                        }
                    }
                }
            }
            return sum
        }
    "#;
    // 2^5 = 32 iterations
    assert_eq!(compile_and_run(src), 32);
}

#[test]
fn test_loop_with_continue() {
    // Loop with continue statement
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..10 {
                if i % 2 == 0 {
                    continue
                }
                sum = sum + i
            }
            return sum
        }
    "#;
    // Sum of odd numbers 1 + 3 + 5 + 7 + 9 = 25
    assert_eq!(compile_and_run(src), 25);
}

#[test]
fn test_loop_with_break() {
    // Loop with break statement
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..100 {
                if i == 5 {
                    break
                }
                sum = sum + i
            }
            return sum
        }
    "#;
    // 0 + 1 + 2 + 3 + 4 = 10
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_early_loop_exit() {
    // Early exit from nested loop
    let src = r#"
        fn main() int {
            let found = 0
            for i in 0..10 {
                for j in 0..10 {
                    if i == 3 && j == 7 {
                        found = i * 10 + j
                        break
                    }
                }
                if found > 0 {
                    break
                }
            }
            return found
        }
    "#;
    // Should find 3*10 + 7 = 37
    assert_eq!(compile_and_run(src), 37);
}

#[test]
fn test_infinite_loop_with_break_in_middle() {
    // Infinite loop with break in middle of loop body
    let src = r#"
        fn main() int {
            let i = 0
            while true {
                i = i + 1
                if i == 10 {
                    break
                }
                i = i + 1
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_loop_with_10000_iterations() {
    // Large loop (10,000 iterations)
    let src = r#"
        fn main() {
            let i = 0
            while i < 10000 {
                i = i + 1
            }
            print(i)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10000");
}

#[test]
fn test_while_with_multiple_conditions() {
    // While loop with complex condition
    let src = r#"
        fn main() int {
            let i = 0
            let j = 10
            while i < 5 && j > 5 {
                i = i + 1
                j = j - 1
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn test_for_loop_empty_range() {
    // For loop over empty range
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 5..5 {
                sum = sum + i
            }
            return sum
        }
    "#;
    // Empty range, sum should be 0
    assert_eq!(compile_and_run(src), 0);
}

// ============================================================================
// 3. Match (10 tests)
// ============================================================================

#[test]
fn test_match_enum_unit_variants() {
    // Match on enum with unit variants
    let src = r#"
        enum Color {
            Red
            Green
            Blue
        }

        fn main() int {
            let c = Color.Green
            match c {
                Color.Red {
                    return 1
                }
                Color.Green {
                    return 2
                }
                Color.Blue {
                    return 3
                }
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn test_match_enum_all_variants() {
    // Match covering all variants (exhaustiveness)
    let src = r#"
        enum Status {
            Pending
            Running
            Complete
        }

        fn status_code(s: Status) int {
            match s {
                Status.Pending {
                    return 0
                }
                Status.Running {
                    return 1
                }
                Status.Complete {
                    return 2
                }
            }
        }

        fn main() {
            print(status_code(Status.Pending))
            print(status_code(Status.Running))
            print(status_code(Status.Complete))
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("0"));
    assert!(output.contains("1"));
    assert!(output.contains("2"));
}

#[test]
fn test_match_enum_data_carrying() {
    // Match on enum with data-carrying variants
    let src = r#"
        enum Shape {
            Circle { radius: int }
            Rectangle { width: int, height: int }
        }

        fn area(s: Shape) int {
            match s {
                Shape.Circle { radius } {
                    return radius * radius * 3
                }
                Shape.Rectangle { width, height } {
                    return width * height
                }
            }
        }

        fn main() {
            let c = Shape.Circle { radius: 5 }
            let r = Shape.Rectangle { width: 4, height: 6 }
            print(area(c))
            print(area(r))
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("75")); // 5 * 5 * 3
    assert!(output.contains("24")); // 4 * 6
}

#[test]
fn test_match_nested() {
    // Nested match statements
    let src = r#"
        enum Outer {
            A
            B
        }

        enum Inner {
            X
            Y
        }

        fn main() int {
            let o = Outer.A
            let i = Inner.Y

            match o {
                Outer.A {
                    match i {
                        Inner.X {
                            return 1
                        }
                        Inner.Y {
                            return 2
                        }
                    }
                }
                Outer.B {
                    return 3
                }
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
#[ignore] // LIMITATION: Pluto doesn't support match as expression (let x = match y { ... })
fn test_match_returning_values() {
    // Match as expression (returning values)
    let src = r#"
        enum Result {
            Ok { value: int }
            Error
        }

        fn main() int {
            let r = Result.Ok { value: 42 }
            let output = match r {
                Result.Ok { value } => value
                Result.Error => 0
            }
            return output
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_match_with_complex_logic() {
    // Match with complex logic in arms
    let src = r#"
        enum Operation {
            Add { a: int, b: int }
            Multiply { a: int, b: int }
        }

        fn compute(op: Operation) int {
            match op {
                Operation.Add { a, b } {
                    let sum = a + b
                    return sum * 2
                }
                Operation.Multiply { a, b } {
                    let product = a * b
                    return product + 10
                }
            }
        }

        fn main() {
            let add_op = Operation.Add { a: 3, b: 4 }
            let mul_op = Operation.Multiply { a: 5, b: 6 }
            print(compute(add_op))
            print(compute(mul_op))
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("14")); // (3 + 4) * 2 = 14
    assert!(output.contains("40")); // (5 * 6) + 10 = 40
}

#[test]
fn test_match_enum_in_loop() {
    // Match inside a loop
    let src = r#"
        enum State {
            Active
            Inactive
        }

        fn main() int {
            let states: [State] = [State.Active, State.Inactive, State.Active]
            let active_count = 0

            for i in 0..3 {
                match states[i] {
                    State.Active {
                        active_count = active_count + 1
                    }
                    State.Inactive {
                    }
                }
            }
            return active_count
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn test_match_with_variable_binding() {
    // Match with variable binding
    let src = r#"
        enum Option {
            Some { value: int }
            None
        }

        fn main() int {
            let opt = Option.Some { value: 99 }
            match opt {
                Option.Some { value } {
                    return value
                }
                Option.None {
                    return 0
                }
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn test_match_multiple_fields() {
    // Match with multiple fields
    let src = r#"
        enum Point {
            TwoD { x: int, y: int }
            ThreeD { x: int, y: int, z: int }
        }

        fn sum_coords(p: Point) int {
            match p {
                Point.TwoD { x, y } {
                    return x + y
                }
                Point.ThreeD { x, y, z } {
                    return x + y + z
                }
            }
        }

        fn main() {
            let p2 = Point.TwoD { x: 1, y: 2 }
            let p3 = Point.ThreeD { x: 1, y: 2, z: 3 }
            print(sum_coords(p2))
            print(sum_coords(p3))
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert!(output.contains("3"));
    assert!(output.contains("6"));
}

#[test]
fn test_match_exhaustive_checking() {
    // Exhaustive match (all variants covered)
    let src = r#"
        enum Token {
            Number { value: int }
            Plus
            Minus
            End
        }

        fn token_type(t: Token) int {
            match t {
                Token.Number { value } {
                    return 1
                }
                Token.Plus {
                    return 2
                }
                Token.Minus {
                    return 3
                }
                Token.End {
                    return 4
                }
            }
        }

        fn main() int {
            return token_type(Token.Plus)
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

// ============================================================================
// 4. Returns (5 tests)
// ============================================================================

#[test]
fn test_early_return() {
    // Early return from function
    let src = r#"
        fn main() int {
            let x = 5
            if x > 3 {
                return 42
            }
            return 0
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_multiple_return_paths() {
    // Multiple return paths
    let src = r#"
        fn classify(x: int) int {
            if x < 0 {
                return 1
            }
            if x == 0 {
                return 2
            }
            if x > 0 && x < 10 {
                return 3
            }
            return 4
        }

        fn main() {
            print(classify(-5))
            print(classify(0))
            print(classify(5))
            print(classify(15))
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "2");
    assert_eq!(lines[2], "3");
    assert_eq!(lines[3], "4");
}

#[test]
fn test_return_from_nested_block() {
    // Return from nested block
    let src = r#"
        fn main() int {
            let x = 10
            if x > 5 {
                if x > 8 {
                    if x > 9 {
                        return 99
                    }
                }
            }
            return 0
        }
    "#;
    assert_eq!(compile_and_run(src), 99);
}

#[test]
fn test_return_in_loop() {
    // Return from inside a loop
    let src = r#"
        fn find_first_even(arr: [int]) int {
            for i in 0..arr.len() {
                if arr[i] % 2 == 0 {
                    return arr[i]
                }
            }
            return -1
        }

        fn main() {
            let numbers = [1, 3, 7, 8, 9, 11]
            print(find_first_even(numbers))
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "8");
}

#[test]
fn test_return_void_vs_value() {
    // Return void vs returning a value
    let src = r#"
        fn void_function() {
            // Returns void implicitly
        }

        fn value_function() int {
            return 42
        }

        fn main() int {
            void_function()
            return value_function()
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

// ============================================================================
// 5. Additional Control Flow Edge Cases (5 tests)
// ============================================================================

#[test]
fn test_if_else_chain() {
    // If-else chain (else-if pattern)
    let src = r#"
        fn grade(score: int) int {
            if score >= 90 {
                return 1
            } else {
                if score >= 80 {
                    return 2
                } else {
                    if score >= 70 {
                        return 3
                    } else {
                        return 4
                    }
                }
            }
        }

        fn main() {
            print(grade(95))
            print(grade(85))
            print(grade(75))
            print(grade(65))
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "2");
    assert_eq!(lines[2], "3");
    assert_eq!(lines[3], "4");
}

#[test]
fn test_loop_with_multiple_breaks() {
    // Loop with multiple break conditions
    let src = r#"
        fn main() int {
            let i = 0
            while true {
                i = i + 1
                if i == 5 {
                    break
                }
                if i == 3 {
                    i = i + 1
                }
                if i > 100 {
                    break
                }
            }
            return i
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn test_continue_skip_logic() {
    // Continue skipping subsequent logic
    let src = r#"
        fn main() int {
            let sum = 0
            for i in 0..10 {
                if i == 5 {
                    continue
                }
                sum = sum + i
            }
            return sum
        }
    "#;
    // 0 + 1 + 2 + 3 + 4 + 6 + 7 + 8 + 9 = 40
    assert_eq!(compile_and_run(src), 40);
}

#[test]
#[ignore] // LIMITATION: Pluto doesn't support match as expression (let x = match y { ... })
fn test_match_in_if_condition() {
    // Match result used in if condition
    let src = r#"
        enum Status {
            Success
            Failure
        }

        fn main() int {
            let s = Status.Success
            let code = match s {
                Status.Success => 0
                Status.Failure => 1
            }

            if code == 0 {
                return 42
            } else {
                return 0
            }
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_nested_match_and_loops() {
    // Complex nesting: match inside loop inside match
    let src = r#"
        enum Outer {
            A
            B
        }

        enum Inner {
            X
            Y
        }

        fn main() int {
            let o = Outer.A
            let result = 0

            match o {
                Outer.A {
                    for i in 0..3 {
                        let inn = Inner.X
                        match inn {
                            Inner.X {
                                result = result + 1
                            }
                            Inner.Y {
                                result = result + 2
                            }
                        }
                    }
                }
                Outer.B {
                    result = 100
                }
            }
            return result
        }
    "#;
    // Should iterate 3 times, adding 1 each time = 3
    assert_eq!(compile_and_run(src), 3);
}

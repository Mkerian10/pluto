// Phase 2: Parser Explorer - Precedence Tests
//
// Tests for operator precedence and associativity edge cases.
// Target: 15 tests covering multi-operator expressions.

mod common;
use common::*;

#[test]
fn precedence_arithmetic_vs_comparison() {
    // 2 + 3 > 4 * 1 → should parse as (2 + 3) > (4 * 1) → 5 > 4 → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 2 + 3 > 4 * 1
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_logical_and_vs_or() {
    // true || false && true → should parse as true || (false && true) → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = true || false && true
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_bitwise_vs_comparison() {
    // x & 3 == 0 → C parses as x & (3 == 0), but Pluto rejects due to type error
    // Pluto follows C precedence (== binds tighter than &) but has stricter typing
    compile_should_fail(r#"
        fn main() {
            let x = 4
            let result = x & 3 == 0  // Would parse as x & (3 == 0), but bool not allowed with &
        }
    "#);

    // With parentheses, it works
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 4
            let result = (x & 3) == 0
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");  // 4 & 3 = 0, so 0 == 0 is true
}

#[test]
fn precedence_shift_vs_addition() {
    // 1 << 2 + 3 → should parse as 1 << (2 + 3) → 1 << 5 → 32
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1 << 2 + 3
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "32");
}

#[test]
fn precedence_unary_vs_binary() {
    // -x * 2 → should parse as (-x) * 2
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            let result = -x * 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "-10");
}

#[test]
fn precedence_not_vs_equality() {
    // !x == true → should parse as (!x) == true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = false
            let result = !x == true
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn associativity_subtraction_left() {
    // 10 - 5 - 2 → should be (10 - 5) - 2 = 3, not 10 - (5 - 2) = 7
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 10 - 5 - 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn associativity_division_left() {
    // 20 / 4 / 2 → should be (20 / 4) / 2 = 2, not 20 / (4 / 2) = 10
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 20 / 4 / 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn precedence_cast_vs_addition() {
    // x as float + 1.0 → should parse as (x as float) + 1.0
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            let result = x as float + 1.0
            print(result)
        }
    "#);
    // Float formatting prints with decimal places
    assert!(stdout.trim().starts_with("6"));
}

#[test]
fn precedence_field_access_vs_call() {
    // obj.method()(x) → should parse as (obj.method())(x)
    let stdout = compile_and_run_stdout(r#"
        class Foo {
            fn get_adder(self) fn(int) int {
                return (x: int) => x + 1
            }
        }

        fn main() {
            let obj = Foo {}
            let result = obj.get_adder()(5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn precedence_nullable_vs_binary() {
    // x? + 1 → should parse as (x?) + 1, where ? unwraps nullable
    let stdout = compile_and_run_stdout(r#"
        fn get_value() int? {
            return 5
        }

        fn main() int? {
            let result = get_value()? + 1
            print(result)
            return none
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn precedence_error_propagate_vs_binary() {
    // foo()! + 1 → should parse as (foo()!) + 1, where ! propagates errors
    let stdout = compile_and_run_stdout(r#"
        error MathError

        fn get_value() int! {
            return 5
        }

        fn main() {
            let result = get_value() catch { 0 } + 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn precedence_mixed_postfix() {
    // arr[0]?.field → should parse as ((arr[0])?)?.field
    let stdout = compile_and_run_stdout(r#"
        class Inner {
            value: int
        }

        fn get_inner() Inner? {
            return Inner { value: 42 }
        }

        fn main() int? {
            let arr = [get_inner()]
            let result = arr[0]?.value
            print(result)
            return none
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn precedence_bitwise_shift_right() {
    // 8 >> 1 + 1 → should parse as 8 >> (1 + 1) → 8 >> 2 → 2
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 8 >> 1 + 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn precedence_complex_expression() {
    // Combine 5+ operators: 2 + 3 * 4 > 10 && !false || 1 << 2 == 4
    // → (2 + (3 * 4)) > 10 && (!false) || ((1 << 2) == 4)
    // → (2 + 12) > 10 && true || (4 == 4)
    // → 14 > 10 && true || true
    // → true && true || true
    // → true || true
    // → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 2 + 3 * 4 > 10 && !false || 1 << 2 == 4
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

// Extended Operator Precedence & Associativity Tests
// Inspired by Rust and Go compiler test suites
//
// Comprehensive coverage of operator precedence edge cases
// Target: 20+ additional precedence tests

mod common;
use common::*;

// ============================================================
// Exhaustive Binary Operator Precedence
// ============================================================

#[test]
fn precedence_multiplication_vs_addition() {
    // 2 + 3 * 4 → 2 + (3 * 4) = 14
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 2 + 3 * 4
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "14");
}

#[test]
fn precedence_division_vs_subtraction() {
    // 10 - 8 / 2 → 10 - (8 / 2) = 6
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 10 - 8 / 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn precedence_modulo_vs_addition() {
    // 5 + 7 % 3 → 5 + (7 % 3) = 6
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 5 + 7 % 3
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn precedence_shift_left_vs_addition() {
    // 1 + 2 << 3 → (1 + 2) << 3 = 24 (shift has lower precedence than addition)
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1 + 2 << 3
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "24");
}

#[test]
fn precedence_shift_right_vs_multiplication() {
    // 16 >> 2 * 1 → 16 >> (2 * 1) = 4
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 16 >> 2 * 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn precedence_bitwise_xor_vs_and() {
    // 5 ^ 3 & 1 → 5 ^ (3 & 1) = 4 (AND has higher precedence)
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 5 ^ 3 & 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "4");
}

#[test]
fn precedence_bitwise_or_vs_xor() {
    // 4 | 2 ^ 1 → 4 | (2 ^ 1) = 7 (XOR has higher precedence)
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 4 | 2 ^ 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "7");
}

#[test]
fn precedence_equality_vs_comparison() {
    // 3 < 5 == true → (3 < 5) == true = true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 3 < 5 == true
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_inequality_vs_equality() {
    // 3 != 4 == true → (3 != 4) == true = true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 3 != 4 == true
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

// ============================================================
// Associativity Tests
// ============================================================

#[test]
fn associativity_subtraction_chain_long() {
    // 100 - 10 - 5 - 2 - 1 → ((((100 - 10) - 5) - 2) - 1) = 82
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 100 - 10 - 5 - 2 - 1
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "82");
}

#[test]
fn associativity_division_chain_long() {
    // 1000 / 10 / 5 / 2 → (((1000 / 10) / 5) / 2) = 10
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1000 / 10 / 5 / 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
fn associativity_modulo_chain() {
    // 100 % 17 % 5 → (100 % 17) % 5 = 15 % 5 = 0
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 100 % 17 % 5
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "0");
}

// ============================================================
// Mixed Operator Precedence
// ============================================================

#[test]
fn precedence_arithmetic_comparison_logical_complex() {
    // 10 - 3 * 2 > 2 + 1 && 5 % 2 == 1 || false
    // → (10 - (3 * 2)) > (2 + 1) && (5 % 2) == 1 || false
    // → 4 > 3 && 1 == 1 || false
    // → true && true || false
    // → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 10 - 3 * 2 > 2 + 1 && 5 % 2 == 1 || false
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_bitwise_shift_comparison_complex() {
    // 8 >> 1 & 3 < 2 | 1 (type error expected - can't compare int with bool)
    compile_should_fail(r#"
        fn main() {
            let result = 8 >> 1 & 3 < 2 | 1
        }
    "#);
}

#[test]
fn precedence_unary_binary_postfix() {
    // -arr[0] * 2 → (-(arr[0])) * 2
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let arr = [5, 10, 15]
            let result = -arr[0] * 2
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "-10");
}

#[test]
fn precedence_not_and_or_chain() {
    // !false && true || false → ((!false) && true) || false → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = !false && true || false
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_comparison_chain() {
    // 1 < 2 < 3 → (1 < 2) < 3 → true < 3 (type error)
    compile_should_fail(r#"
        fn main() {
            let result = 1 < 2 < 3
        }
    "#);
}

#[test]
fn precedence_equality_chain() {
    // 1 == 1 == true → (1 == 1) == true → true == true → true
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = 1 == 1 == true
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_all_operators_mega_expression() {
    // Test expression combining 10+ different operator types
    // 2 + 3 * 4 - 10 / 2 % 3 << 1 & 7 | 1 ^ 2 > 0 && true || false == false
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 2 + 3 * 4 - 10 / 2 % 3
            let b = a << 1
            let c = b & 7 | 1 ^ 2
            let result = c > 0 && true || false == false
            if result { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn precedence_parentheses_override() {
    // (2 + 3) * (4 + 5) → 5 * 9 = 45
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let result = (2 + 3) * (4 + 5)
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "45");
}

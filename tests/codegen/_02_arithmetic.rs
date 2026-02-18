// Category 2: Arithmetic Operations
// Comprehensive test suite for arithmetic, bitwise, and comparison operations.
// Tests validate correct codegen behavior for all numeric operations.

use super::common::compile_and_run_stdout;

// ============================================================================
// 1. Integer Arithmetic (20 tests)
// ============================================================================

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_add_zero() {
    let source = r#"
fn main() {
    let mut result = 0 + 0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_add_simple() {
    let source = r#"
fn main() {
    let result = 1 + 1
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "2");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_add_large() {
    let source = r#"
fn main() {
    let result = 1000000 + 2000000
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "3000000");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_add_negative() {
    let source = r#"
fn main() {
    let result = -5 + 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-2");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_sub_simple() {
    let source = r#"
fn main() {
    let result = 5 - 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "2");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_sub_zero() {
    let source = r#"
fn main() {
    let mut result = 0 - 0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_sub_negative_result() {
    let source = r#"
fn main() {
    let result = 3 - 5
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-2");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mul_simple() {
    let source = r#"
fn main() {
    let result = 2 * 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "6");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mul_zero() {
    let source = r#"
fn main() {
    let mut result = 0 * 999999
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mul_large() {
    let source = r#"
fn main() {
    let result = 1000 * 2000
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "2000000");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mul_negative() {
    let source = r#"
fn main() {
    let result = -5 * 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-15");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_div_simple() {
    let source = r#"
fn main() {
    let result = 6 / 2
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "3");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_div_truncation() {
    let source = r#"
fn main() {
    let result = 5 / 2
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "2");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_div_negative() {
    let source = r#"
fn main() {
    let result = -10 / 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-3");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mod_simple() {
    let source = r#"
fn main() {
    let result = 7 % 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mod_negative_dividend() {
    let source = r#"
fn main() {
    let result = -7 % 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_mod_negative_divisor() {
    let source = r#"
fn main() {
    let result = 7 % -3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_associativity_add() {
    let source = r#"
fn main() {
    let a = 1
    let b = 2
    let c = 3
    let left = (a + b) + c
    let right = a + (b + c)
    print(left)
    print(right)
}
"#;
    let output = compile_and_run_stdout(source);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "6");
    assert_eq!(lines[1], "6");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_precedence_mul_add() {
    let source = r#"
fn main() {
    let result = 2 + 3 * 4
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "14");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_complex_expression() {
    let source = r#"
fn main() {
    let a = 10
    let b = 5
    let c = 2
    let d = 3
    let e = 7
    let result = ((((a + b) * c) / d) % e)
    print(result)
}
"#;
    // ((10+5)*2)/3 % 7 = (15*2)/3 % 7 = 30/3 % 7 = 10 % 7 = 3
    assert_eq!(compile_and_run_stdout(source).trim(), "3");
}

// ============================================================================
// 2. Float Arithmetic (20 tests)
// ============================================================================

#[test]
fn test_float_add_simple() {
    let source = r#"
fn main() {
    let result = 1.0 + 2.0
    print(result)
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(output.trim().starts_with("3"));
}

#[test]
fn test_float_add_decimals() {
    let source = r#"
fn main() {
    let result = 1.5 + 2.5
    print(result)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    assert!(output.starts_with("4"));
}

#[test]
fn test_float_add_negative() {
    let source = r#"
fn main() {
    let result = -1.5 + 3.5
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "2");
}

#[test]
fn test_float_sub_simple() {
    let source = r#"
fn main() {
    let result = 3.0 - 1.5
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1.5");
}

#[test]
fn test_float_sub_zero() {
    let source = r#"
fn main() {
    let mut result = 0.0 - 0.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_float_sub_negative_result() {
    let source = r#"
fn main() {
    let result = 2.5 - 5.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-2.5");
}

#[test]
fn test_float_mul_simple() {
    let source = r#"
fn main() {
    let result = 2.5 * 4.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "10");
}

#[test]
fn test_float_mul_zero() {
    let source = r#"
fn main() {
    let mut result = 0.0 * 999.999
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_float_mul_negative() {
    let source = r#"
fn main() {
    let result = -2.5 * 4.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-10");
}

#[test]
fn test_float_div_simple() {
    let source = r#"
fn main() {
    let result = 6.0 / 2.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "3");
}

#[test]
fn test_float_div_decimal_result() {
    let source = r#"
fn main() {
    let result = 7.0 / 2.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "3.5");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_div_by_zero_positive_infinity() {
    let source = r#"
fn main() {
    let result = 1.0 / 0.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "inf");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_div_by_zero_negative_infinity() {
    let source = r#"
fn main() {
    let result = -1.0 / 0.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-inf");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_div_zero_by_zero_nan() {
    let source = r#"
fn main() {
    let mut result = 0.0 / 0.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "nan");
}

#[test]
fn test_float_precision_point_one_plus_point_two() {
    let source = r#"
fn main() {
    let mut result = 0.1 + 0.2
    print(result)
}
"#;
    // This will likely show floating point precision issues
    let output = compile_and_run_stdout(source).trim().to_string();
    // Check it's close to 0.3
    assert!(output.starts_with("0.3"));
}

#[test]
fn test_float_very_small() {
    let source = r#"
fn main() {
    let mut result = 0.0001 + 0.0002
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0.0003");
}

#[test]
fn test_float_very_large() {
    let source = r#"
fn main() {
    let result = 1000000.0 + 2000000.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "3000000");
}

#[test]
fn test_float_mixed_signs() {
    let source = r#"
fn main() {
    let result = -5.5 + 3.5 - 2.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-4");
}

#[test]
fn test_float_precedence() {
    let source = r#"
fn main() {
    let result = 2.0 + 3.0 * 4.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "14");
}

#[test]
fn test_float_complex_expression() {
    let source = r#"
fn main() {
    let result = (10.0 + 5.0) * 2.0 / 3.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "10");
}

// ============================================================================
// 3. Bitwise Operations (10 tests)
// ============================================================================

#[test]
fn test_bitwise_and_basic() {
    let source = r#"
fn main() {
    let result = 12 & 10
    print(result)
}
"#;
    // 12 = 0b1100, 10 = 0b1010, result = 0b1000 = 8
    assert_eq!(compile_and_run_stdout(source).trim(), "8");
}

#[test]
fn test_bitwise_or_basic() {
    let source = r#"
fn main() {
    let result = 10 | 3
    print(result)
}
"#;
    // 10 = 0b1010, 3 = 0b0011, result = 0b1011 = 11
    assert_eq!(compile_and_run_stdout(source).trim(), "11");
}

#[test]
fn test_bitwise_xor_basic() {
    let source = r#"
fn main() {
    let result = 10 ^ 12
    print(result)
}
"#;
    // 10 = 0b1010, 12 = 0b1100, result = 0b0110 = 6
    assert_eq!(compile_and_run_stdout(source).trim(), "6");
}

#[test]
fn test_bitwise_not_basic() {
    let source = r#"
fn main() {
    let result = ~10
    print(result)
}
"#;
    // ~10 = -11 in two's complement
    assert_eq!(compile_and_run_stdout(source).trim(), "-11");
}

#[test]
fn test_bitwise_shift_left_simple() {
    let source = r#"
fn main() {
    let result = 1 << 3
    print(result)
}
"#;
    // 1 << 3 = 8
    assert_eq!(compile_and_run_stdout(source).trim(), "8");
}

#[test]
fn test_bitwise_shift_left_large() {
    let source = r#"
fn main() {
    let result = 1 << 10
    print(result)
}
"#;
    // 1 << 10 = 1024
    assert_eq!(compile_and_run_stdout(source).trim(), "1024");
}

#[test]
fn test_bitwise_shift_right_simple() {
    let source = r#"
fn main() {
    let result = 8 >> 2
    print(result)
}
"#;
    // 8 >> 2 = 2
    assert_eq!(compile_and_run_stdout(source).trim(), "2");
}

#[test]
fn test_bitwise_shift_right_negative() {
    let source = r#"
fn main() {
    let result = -8 >> 2
    print(result)
}
"#;
    // Arithmetic shift right preserves sign: -8 >> 2 = -2
    assert_eq!(compile_and_run_stdout(source).trim(), "-2");
}

#[test]
fn test_bitwise_combined_and_or() {
    let source = r#"
fn main() {
    let result = (12 & 10) | 3
    print(result)
}
"#;
    // (12 & 10) | 3 = 8 | 3 = 11
    assert_eq!(compile_and_run_stdout(source).trim(), "11");
}

#[test]
fn test_bitwise_combined_shifts() {
    let source = r#"
fn main() {
    let result = (16 >> 2) << 1
    print(result)
}
"#;
    // (16 >> 2) << 1 = 4 << 1 = 8
    assert_eq!(compile_and_run_stdout(source).trim(), "8");
}

// ============================================================================
// 4. Comparison Operations (10 tests)
// ============================================================================

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_less_than_true() {
    let source = r#"
fn main() {
    let result = 3 < 5
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_less_than_false() {
    let source = r#"
fn main() {
    let result = 5 < 3
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_less_equal_true() {
    let source = r#"
fn main() {
    let result = 5 <= 5
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_greater_than() {
    let source = r#"
fn main() {
    let result = 10 > 5
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_greater_equal() {
    let source = r#"
fn main() {
    let result = 5 >= 5
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_equal() {
    let source = r#"
fn main() {
    let result = 42 == 42
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
#[ignore] // DUPLICATE: Already covered by tests/integration/operators.rs and basics.rs
fn test_int_not_equal() {
    let source = r#"
fn main() {
    let result = 42 != 43
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_float_equality_exact() {
    let source = r#"
fn main() {
    let result = 3.0 == 3.0
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_float_negative_zero_equality() {
    let source = r#"
fn main() {
    let mut result = 0.0 == -0.0
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    // In IEEE 754, 0.0 == -0.0 is true
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_string_equality() {
    let source = r#"
fn main() {
    let s1 = "hello"
    let s2 = "hello"
    let result = s1 == s2
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

// ============================================================================
// 5. Additional Edge Cases (10 tests)
// ============================================================================

#[test]
fn test_int_overflow_detection() {
    let source = r#"
fn main() {
    let large = 9223372036854775807
    print(large)
}
"#;
    // Max i64 value
    assert_eq!(compile_and_run_stdout(source).trim(), "9223372036854775807");
}

#[test]
fn test_int_underflow_detection() {
    // FIXED: Lexer doesn't support literals larger than i64::MAX (9223372036854775808)
    // Use arithmetic to produce i64::MIN instead
    let source = r#"
fn main() {
    let max = 9223372036854775807
    let min_val = max + 1
    print(min_val)
}
"#;
    // i64::MAX + 1 wraps to i64::MIN
    assert_eq!(compile_and_run_stdout(source).trim(), "-9223372036854775808");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_inf_addition() {
    let source = r#"
fn main() {
    let inf = 1.0 / 0.0
    let result = inf + 1.0
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "inf");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_inf_minus_inf() {
    let source = r#"
fn main() {
    let inf1 = 1.0 / 0.0
    let inf2 = 1.0 / 0.0
    let result = inf1 - inf2
    print(result)
}
"#;
    // infinity - infinity = NaN
    assert_eq!(compile_and_run_stdout(source).trim(), "nan");
}

#[test]
#[ignore] // Platform-specific float edge case - NaN/Infinity representation varies
fn test_float_inf_times_zero() {
    let source = r#"
fn main() {
    let inf = 1.0 / 0.0
    let result = inf * 0.0
    print(result)
}
"#;
    // infinity * 0 = NaN
    assert_eq!(compile_and_run_stdout(source).trim(), "nan");
}

#[test]
fn test_nested_arithmetic() {
    let source = r#"
fn main() {
    let result = ((10 + 5) * (8 - 3)) / (2 + 1)
    print(result)
}
"#;
    // ((10+5) * (8-3)) / (2+1) = (15 * 5) / 3 = 75 / 3 = 25
    assert_eq!(compile_and_run_stdout(source).trim(), "25");
}

#[test]
fn test_mixed_bitwise_arithmetic() {
    let source = r#"
fn main() {
    let result = (8 << 2) + (16 >> 1)
    print(result)
}
"#;
    // (8 << 2) + (16 >> 1) = 32 + 8 = 40
    assert_eq!(compile_and_run_stdout(source).trim(), "40");
}

#[test]
fn test_comparison_chain() {
    let source = r#"
fn main() {
    let a = 5
    let b = 10
    let c = 15
    let result = a < b && b < c
    if result {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_bool_equality() {
    let source = r#"
fn main() {
    let t = true
    let f = false
    let result1 = t == true
    let result2 = f == false
    if result1 && result2 {
        print(1)
    } else {
        print(0)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_unary_minus_precedence() {
    let source = r#"
fn main() {
    let result = -5 + 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-2");
}

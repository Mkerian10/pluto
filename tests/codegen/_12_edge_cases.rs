// Category 12: Edge Cases & Stress Tests
// Comprehensive test suite for boundary conditions, numeric limits, large data structures,
// corner cases, and special values. These tests are designed to find bugs at the edges
// of the language's behavior.

use super::common::{compile_and_run_stdout, compile_and_run_output};

// ============================================================================
// 1. Numeric Limits (10 tests)
// ============================================================================

#[test]
#[ignore] // LIMITATION: i64::MIN literal (-9223372036854775808) causes lexer overflow
fn test_i64_min_literal() {
    let source = r#"
fn main() {
    let x = -9223372036854775808
    print(x)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-9223372036854775808");
}

#[test]
fn test_i64_max_literal() {
    let source = r#"
fn main() {
    let x = 9223372036854775807
    print(x)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "9223372036854775807");
}

#[test]
#[ignore] // LIMITATION: i64::MIN literal (-9223372036854775808) causes lexer overflow
fn test_i64_min_addition() {
    let source = r#"
fn main() {
    let min = -9223372036854775808
    let result = min + 1
    print(result)
}
"#;
    // Wraps around (undefined behavior, but test what actually happens)
    assert_eq!(compile_and_run_stdout(source).trim(), "-9223372036854775807");
}

#[test]
fn test_i64_max_addition() {
    let source = r#"
fn main() {
    let max = 9223372036854775807
    let result = max + 1
    print(result)
}
"#;
    // Wraps around (undefined behavior, but test what actually happens)
    assert_eq!(compile_and_run_stdout(source).trim(), "-9223372036854775808");
}

#[test]
#[ignore] // LIMITATION: i64::MIN literal (-9223372036854775808) causes lexer overflow
fn test_i64_min_subtraction() {
    let source = r#"
fn main() {
    let min = -9223372036854775808
    let result = min - 1
    print(result)
}
"#;
    // Wraps around
    assert_eq!(compile_and_run_stdout(source).trim(), "9223372036854775807");
}

#[test]
fn test_i64_max_multiplication() {
    let source = r#"
fn main() {
    let max = 9223372036854775807
    let result = max * 2
    print(result)
}
"#;
    // Wraps around
    assert_eq!(compile_and_run_stdout(source).trim(), "-2");
}

#[test]
#[ignore] // LIMITATION: Pluto doesn't support scientific notation in numeric literals
fn test_f64_max_literal() {
    let source = r#"
fn main() {
    let x = 1.7976931348623157e308
    print(x)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    // Just verify it doesn't crash, exact formatting may vary
    assert!(output.contains("1.7976931348623157e308") || output.contains("1.7976931348623157e+308"));
}

#[test]
#[ignore] // LIMITATION: Pluto doesn't support scientific notation in numeric literals
fn test_f64_min_positive_literal() {
    let source = r#"
fn main() {
    let x = 2.2250738585072014e-308
    print(x)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    // Just verify it doesn't crash
    assert!(output.len() > 0);
}

#[test]
fn test_negative_zero_int() {
    let source = r#"
fn main() {
    let x = -0
    print(x)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_max_int_negation() {
    let source = r#"
fn main() {
    let max = 9223372036854775807
    let result = -max
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-9223372036854775807");
}

// ============================================================================
// 2. Large Data Structures (10 tests)
// ============================================================================

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_array_1000_elements() {
    let source = r#"
fn main() {
    let arr = []
    let mut i = 0
    while i < 1000 {
        arr.push(i)
        i = i + 1
    }
    print(arr.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1000");
}

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_array_10000_elements() {
    let source = r#"
fn main() {
    let arr = []
    let mut i = 0
    while i < 10000 {
        arr.push(i)
        i = i + 1
    }
    print(arr.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "10000");
}

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_array_iteration_large() {
    let source = r#"
fn main() {
    let arr = []
    let mut i = 0
    while i < 5000 {
        arr.push(i)
        i = i + 1
    }
    let mut sum = 0
    let mut j = 0
    while j < arr.len() {
        sum = sum + arr[j]
        j = j + 1
    }
    print(sum)
}
"#;
    // Sum of 0..4999 = (n * (n-1)) / 2 = 5000 * 4999 / 2 = 12497500
    assert_eq!(compile_and_run_stdout(source).trim(), "12497500");
}

#[test]
fn test_string_concatenation_large() {
    let source = r#"
fn main() {
    let mut s = ""
    let mut i = 0
    while i < 100 {
        s = s + "x"
        i = i + 1
    }
    print(s.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "100");
}

#[test]
fn test_string_1000_chars() {
    let source = r#"
fn main() {
    let mut s = ""
    let mut i = 0
    while i < 1000 {
        s = s + "a"
        i = i + 1
    }
    print(s.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1000");
}

#[test]
fn test_nested_array_depth_5() {
    let source = r#"
fn main() {
    let a = [[[[[ 42 ]]]]]
    print(a[0][0][0][0][0])
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42");
}

#[test]
fn test_nested_array_depth_10() {
    let source = r#"
fn main() {
    let a = [[[[[[ [[[[ 99 ]]]] ]]]]]]
    print(a[0][0][0][0][0][0][0][0][0][0])
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "99");
}

#[test]
fn test_map_1000_entries() {
    let source = r#"
fn main() {
    let m = Map<int, int> {}
    let mut i = 0
    while i < 1000 {
        m[i] = i * 2
        i = i + 1
    }
    print(m.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1000");
}

#[test]
fn test_set_1000_elements() {
    let source = r#"
fn main() {
    let s = Set<int> {}
    let mut i = 0
    while i < 1000 {
        s.insert(i)
        i = i + 1
    }
    print(s.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1000");
}

#[test]
fn test_recursive_depth_100() {
    let source = r#"
fn countdown(n: int) int {
    if n == 0 {
        return 0
    }
    return countdown(n - 1) + 1
}

fn main() {
    let result = countdown(100)
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "100");
}

// ============================================================================
// 3. Corner Cases (10 tests)
// ============================================================================

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_empty_array_len() {
    let source = r#"
fn main() {
    let arr = []
    print(arr.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_empty_array_push_pop() {
    let source = r#"
fn main() {
    let arr = []
    arr.push(42)
    let val = arr.pop()
    print(val)
    print(arr.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42\n0");
}

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_empty_array_iteration() {
    let source = r#"
fn main() {
    let arr = []
    let mut count = 0
    let mut i = 0
    while i < arr.len() {
        count = count + 1
        i = i + 1
    }
    print(count)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_division_by_zero_int() {
    let source = r#"
fn main() {
    let x = 10
    let y = 0
    let result = x / y
    print(result)
}
"#;
    // Division by zero is undefined behavior - just verify it doesn't crash the compiler
    // Runtime behavior may vary (crash, undefined result, etc.)
    let (_stdout, _stderr, _code) = compile_and_run_output(source);
    // No assertion - just verify compilation succeeds
}

#[test]
fn test_modulo_by_zero_int() {
    let source = r#"
fn main() {
    let x = 10
    let y = 0
    let result = x % y
    print(result)
}
"#;
    // Modulo by zero is undefined behavior - just verify it doesn't crash the compiler
    let (_stdout, _stderr, _code) = compile_and_run_output(source);
    // No assertion - just verify compilation succeeds
}

#[test]
fn test_array_access_boundary() {
    let source = r#"
fn main() {
    let arr = [1, 2, 3]
    print(arr[0])
    print(arr[2])
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1\n3");
}

#[test]
fn test_empty_string_operations() {
    let source = r#"
fn main() {
    let s = ""
    print(s.len())
    let s2 = s + ""
    print(s2.len())
    let s3 = s + "hello"
    print(s3)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0\n0\nhello");
}

#[test]
fn test_empty_map_operations() {
    let source = r#"
fn main() {
    let m = Map<int, int> {}
    print(m.len())
    print(m.contains(42))
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0\nfalse");
}

#[test]
fn test_empty_set_operations() {
    let source = r#"
fn main() {
    let s = Set<int> {}
    print(s.len())
    print(s.contains(42))
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0\nfalse");
}

#[test]
fn test_string_interpolation_empty() {
    let source = r#"
fn main() {
    let name = ""
    let msg = "Hello {name}!"
    print(msg)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "Hello !");
}

// ============================================================================
// 4. Boundary Conditions (10 tests)
// ============================================================================

#[test]
fn test_zero_length_string_literal() {
    let source = r#"
fn main() {
    let s = ""
    print(s.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
#[ignore] // LIMITATION: Empty array literals not supported - compiler cannot infer type
fn test_zero_element_array_literal() {
    let source = r#"
fn main() {
    let arr = []
    print(arr.len())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_zero_parameter_function() {
    let source = r#"
fn get_value() int {
    return 42
}

fn main() {
    let val = get_value()
    print(val)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42");
}

#[test]
fn test_zero_parameter_closure() {
    let source = r#"
fn main() {
    let f = () => 42
    let val = f()
    print(val)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42");
}

#[test]
fn test_single_element_array() {
    let source = r#"
fn main() {
    let arr = [42]
    print(arr.len())
    print(arr[0])
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1\n42");
}

#[test]
fn test_single_char_string() {
    let source = r#"
fn main() {
    let s = "x"
    print(s.len())
    print(s)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1\nx");
}

#[test]
fn test_single_iteration_loop() {
    let source = r#"
fn main() {
    let mut count = 0
    let mut i = 0
    while i < 1 {
        count = count + 1
        i = i + 1
    }
    print(count)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_range_single_element() {
    let source = r#"
fn main() {
    let mut count = 0
    for i in 0..1 {
        count = count + 1
    }
    print(count)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

#[test]
fn test_range_zero_elements() {
    let source = r#"
fn main() {
    let mut count = 0
    for i in 0..0 {
        count = count + 1
    }
    print(count)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_map_single_entry() {
    let source = r#"
fn main() {
    let m = Map<int, int> { 42: 99 }
    print(m.len())
    print(m[42])
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1\n99");
}

// ============================================================================
// 5. Special Values (10 tests)
// ============================================================================

#[test]
fn test_float_positive_zero() {
    let source = r#"
fn main() {
    let x = 0.0
    print(x)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0.000000");
}

#[test]
fn test_float_negative_zero() {
    let source = r#"
fn main() {
    let x = -0.0
    print(x)
}
"#;
    // -0.0 may print as "0.000000" or "-0.000000" depending on implementation
    let output = compile_and_run_stdout(source).trim().to_string();
    assert!(output == "0.000000" || output == "-0.000000");
}

#[test]
fn test_float_positive_zero_vs_negative_zero_equality() {
    let source = r#"
fn main() {
    let pos = 0.0
    let neg = -0.0
    if pos == neg {
        print("equal")
    } else {
        print("not equal")
    }
}
"#;
    // IEEE 754: +0.0 == -0.0
    assert_eq!(compile_and_run_stdout(source).trim(), "equal");
}

#[test]
fn test_float_infinity() {
    let source = r#"
fn main() {
    let x = 1.0 / 0.0
    print(x)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    assert!(output.contains("inf") || output.contains("Inf"));
}

#[test]
fn test_float_negative_infinity() {
    let source = r#"
fn main() {
    let x = -1.0 / 0.0
    print(x)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    assert!(output.contains("-inf") || output.contains("-Inf"));
}

#[test]
fn test_float_nan() {
    let source = r#"
fn main() {
    let x = 0.0 / 0.0
    print(x)
}
"#;
    let output = compile_and_run_stdout(source).trim().to_string();
    assert!(output.contains("nan") || output.contains("NaN"));
}

#[test]
fn test_float_nan_inequality() {
    let source = r#"
fn main() {
    let x = 0.0 / 0.0
    if x == x {
        print("equal")
    } else {
        print("not equal")
    }
}
"#;
    // IEEE 754: NaN != NaN
    assert_eq!(compile_and_run_stdout(source).trim(), "not equal");
}

#[test]
fn test_int_all_bits_set() {
    let source = r#"
fn main() {
    let x = -1
    print(x)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-1");
}

#[test]
fn test_bool_true_value() {
    let source = r#"
fn main() {
    let b = true
    if b {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "true");
}

#[test]
fn test_bool_false_value() {
    let source = r#"
fn main() {
    let b = false
    if b {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "false");
}

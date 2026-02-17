// Category 13: Codegen Correctness
// Comprehensive test suite for IR generation correctness.
// Tests validate type conversions, constant folding, dead code elimination, and register allocation.

use super::common::{compile_and_run, compile_and_run_stdout};

// ============================================================================
// 1. Type Conversions (10 tests)
// ============================================================================

#[test]
fn test_int_to_float_simple() {
    let source = r#"
fn main() {
    let i = 42
    let f = i as float
    print(f)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42.000000");
}

#[test]
fn test_int_to_float_zero() {
    let source = r#"
fn main() {
    let mut i = 0
    let f = i as float
    print(f)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0.000000");
}

#[test]
fn test_int_to_float_negative() {
    let source = r#"
fn main() {
    let i = -123
    let f = i as float
    print(f)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-123.000000");
}

#[test]
fn test_float_to_int_simple() {
    let source = r#"
fn main() {
    let f = 42.7
    let i = f as int
    print(i)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42");
}

#[test]
fn test_float_to_int_truncation() {
    let source = r#"
fn main() {
    let f = 99.999
    let i = f as int
    print(i)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "99");
}

#[test]
fn test_float_to_int_negative_truncation() {
    let source = r#"
fn main() {
    let f = -7.8
    let i = f as int
    print(i)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-7");
}

#[test]
fn test_int_to_bool_zero_is_false() {
    let source = r#"
fn main() {
    let mut i = 0
    let b = i as bool
    if b {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "false");
}

#[test]
fn test_int_to_bool_nonzero_is_true() {
    let source = r#"
fn main() {
    let i = 42
    let b = i as bool
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
fn test_bool_to_int_false() {
    let source = r#"
fn main() {
    let b = false
    let i = b as int
    print(i)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "0");
}

#[test]
fn test_bool_to_int_true() {
    let source = r#"
fn main() {
    let b = true
    let i = b as int
    print(i)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

// ============================================================================
// 2. Constant Folding (10 tests)
// ============================================================================

#[test]
fn test_const_fold_int_add() {
    // Compiler should evaluate 2+3 at compile time
    let source = r#"
fn main() {
    let result = 2 + 3
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "5");
}

#[test]
fn test_const_fold_int_mul() {
    // Compiler should evaluate 7*6 at compile time
    let source = r#"
fn main() {
    let result = 7 * 6
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "42");
}

#[test]
fn test_const_fold_int_complex() {
    // Compiler should evaluate (2+3)*4-5 at compile time
    let source = r#"
fn main() {
    let result = (2 + 3) * 4 - 5
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "15");
}

#[test]
fn test_const_fold_bool_and() {
    // Compiler should evaluate true && false at compile time
    let source = r#"
fn main() {
    let result = true && false
    if result {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "false");
}

#[test]
fn test_const_fold_bool_or() {
    // Compiler should evaluate true || false at compile time
    let source = r#"
fn main() {
    let result = true || false
    if result {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "true");
}

#[test]
fn test_const_fold_comparison() {
    // Compiler should evaluate 10 > 5 at compile time
    let source = r#"
fn main() {
    let result = 10 > 5
    if result {
        print("true")
    } else {
        print("false")
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "true");
}

#[test]
fn test_const_fold_in_array() {
    // Constant expressions in array should be folded
    let source = r#"
fn main() {
    let arr = [1+1, 2*3, 10-4]
    print(arr[0])
    print(arr[1])
    print(arr[2])
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(output.contains("2"));
    assert!(output.contains("6"));
}

#[test]
fn test_const_fold_nested_arithmetic() {
    // Deeply nested constant arithmetic
    let source = r#"
fn main() {
    let result = ((5 + 3) * (4 - 2)) / 2
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "8");
}

#[test]
#[ignore] // LIMITATION: Binary literal syntax (0b1010) not supported in Pluto
fn test_const_fold_bitwise() {
    // Bitwise operations on constants
    let source = r#"
fn main() {
    let mut result = 0b1010 & 0b1100
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "8");
}

#[test]
fn test_const_fold_mixed_types() {
    // Multiple constant folding in one expression
    let source = r#"
fn main() {
    let a = 2 + 3
    let b = 4 * 5
    let c = a + b
    print(c)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "25");
}

// ============================================================================
// 3. Dead Code Elimination (5 tests)
// ============================================================================

#[test]
fn test_dead_code_if_false() {
    // Code in if (false) branch should never execute
    let source = r#"
fn main() {
    if false {
        print("SHOULD NOT PRINT")
    }
    print("success")
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(!output.contains("SHOULD NOT PRINT"));
    assert!(output.contains("success"));
}

#[test]
fn test_dead_code_after_return() {
    // Code after return should be unreachable
    let source = r#"
fn helper() int {
    return 42
    print("UNREACHABLE")
    return 99
}

fn main() {
    let result = helper()
    print(result)
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(!output.contains("UNREACHABLE"));
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_dead_code_if_true_else() {
    // Else branch should never execute when condition is always true
    let source = r#"
fn main() {
    if true {
        print("correct")
    } else {
        print("SHOULD NOT PRINT")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(output.contains("correct"));
    assert!(!output.contains("SHOULD NOT PRINT"));
}

#[test]
fn test_dead_code_unreachable_after_loop_break() {
    // Code after unconditional break is unreachable
    let source = r#"
fn main() {
    let mut count = 0
    while true {
        count = count + 1
        break
        print("UNREACHABLE")
    }
    print(count)
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(!output.contains("UNREACHABLE"));
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_dead_code_multiple_returns() {
    // Only first return should execute
    let source = r#"
fn compute() int {
    let x = 10
    if x > 5 {
        return 1
        return 2
    }
    return 3
}

fn main() {
    print(compute())
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "1");
}

// ============================================================================
// 4. Register Allocation (5 tests)
// ============================================================================

#[test]
fn test_register_pressure_many_locals() {
    // Create heavy register pressure with 20+ live variables
    let source = r#"
fn main() {
    let v0 = 0
    let v1 = 1
    let v2 = 2
    let v3 = 3
    let v4 = 4
    let v5 = 5
    let v6 = 6
    let v7 = 7
    let v8 = 8
    let v9 = 9
    let v10 = 10
    let v11 = 11
    let v12 = 12
    let v13 = 13
    let v14 = 14
    let v15 = 15
    let v16 = 16
    let v17 = 17
    let v18 = 18
    let v19 = 19
    let v20 = 20
    let sum = v0 + v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9 +
              v10 + v11 + v12 + v13 + v14 + v15 + v16 + v17 + v18 + v19 + v20
    print(sum)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "210");
}

#[test]
fn test_register_pressure_complex_expression() {
    // Single complex expression requiring many temporaries
    let source = r#"
fn main() {
    let a = 1
    let b = 2
    let c = 3
    let d = 4
    let e = 5
    let result = (a + b) * (c + d) + (a * b) + (c * d) + (a + e) * (b + c)
    print(result)
}
"#;
    // (1+2)*(3+4) + (1*2) + (3*4) + (1+5)*(2+3) = 3*7 + 2 + 12 + 6*5 = 21+2+12+30 = 65
    assert_eq!(compile_and_run_stdout(source).trim(), "65");
}

#[test]
fn test_register_pressure_nested_calls() {
    // Nested function calls requiring register preservation
    let source = r#"
fn add(a: int, b: int) int {
    return a + b
}

fn mul(a: int, b: int) int {
    return a * b
}

fn main() {
    let result = add(mul(2, 3), add(mul(4, 5), add(6, 7)))
    print(result)
}
"#;
    // mul(2,3) + add(mul(4,5), add(6,7)) = 6 + add(20, 13) = 6 + 33 = 39
    assert_eq!(compile_and_run_stdout(source).trim(), "39");
}

#[test]
fn test_register_spill_loop_with_accumulator() {
    // Loop with many accumulators to force stack spilling
    let source = r#"
fn main() {
    let mut sum1 = 0
    let mut sum2 = 0
    let mut sum3 = 0
    let mut sum4 = 0
    let mut sum5 = 0
    let mut i = 0
    while i < 10 {
        sum1 = sum1 + i
        sum2 = sum2 + i * 2
        sum3 = sum3 + i * 3
        sum4 = sum4 + i * 4
        sum5 = sum5 + i * 5
        i = i + 1
    }
    print(sum1 + sum2 + sum3 + sum4 + sum5)
}
"#;
    // sum1 = 0+1+2+...+9 = 45
    // sum2 = 0+2+4+...+18 = 90
    // sum3 = 0+3+6+...+27 = 135
    // sum4 = 0+4+8+...+36 = 180
    // sum5 = 0+5+10+...+45 = 225
    // total = 45+90+135+180+225 = 675
    assert_eq!(compile_and_run_stdout(source).trim(), "675");
}

#[test]
#[ignore] // LIMITATION: If-as-expression not supported (let x = if cond { ... })
fn test_register_allocation_with_conditionals() {
    // Mix of live variables and conditionals
    let source = r#"
fn main() {
    let a = 10
    let b = 20
    let c = 30
    let d = 40
    let e = 50
    let result = if a > 5 {
        if b > 15 {
            a + b + c
        } else {
            b + c + d
        }
    } else {
        c + d + e
    }
    print(result)
}
"#;
    // a=10 > 5, so outer if true
    // b=20 > 15, so inner if true
    // result = 10+20+30 = 60
    assert_eq!(compile_and_run_stdout(source).trim(), "60");
}

// ============================================================================
// Additional Edge Cases for Codegen Correctness
// ============================================================================

#[test]
fn test_type_conversion_chain() {
    // Chain multiple type conversions
    let source = r#"
fn main() {
    let i = 42
    let f = i as float
    let b = i as bool
    let i2 = b as int
    print(f)
    print(i2)
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(output.contains("42.0"));
    assert!(output.contains("1"));
}

#[test]
fn test_constant_folding_with_variables() {
    // Mix of constants and variables
    let source = r#"
fn main() {
    let x = 10
    let result = 5 + 3 + x
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "18");
}

#[test]
fn test_dead_code_in_nested_blocks() {
    // Dead code in nested scope
    let source = r#"
fn main() {
    let x = 10
    if x > 5 {
        let y = 20
        if false {
            print("DEAD")
        }
        print(y)
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert!(!output.contains("DEAD"));
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_register_allocation_array_operations() {
    // Register pressure from array operations
    let source = r#"
fn main() {
    let arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let sum = arr[0] + arr[1] + arr[2] + arr[3] + arr[4] +
              arr[5] + arr[6] + arr[7] + arr[8] + arr[9]
    print(sum)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "55");
}

#[test]
fn test_constant_propagation_across_blocks() {
    // Constant should propagate across scopes
    let source = r#"
fn main() {
    let x = 42
    if true {
        let y = x + 8
        print(y)
    }
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "50");
}

#[test]
fn test_type_conversion_in_expression() {
    // Type conversion within larger expression
    let source = r#"
fn main() {
    let i = 10
    let f = 5.5
    let result = (i as float) + f
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "15.500000");
}

#[test]
fn test_dead_code_elimination_with_return_value() {
    // Dead code after return in value-producing function
    let source = r#"
fn compute(x: int) int {
    if x > 0 {
        return x * 2
        return x * 3
    }
    return 0
}

fn main() {
    print(compute(5))
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "10");
}

#[test]
fn test_register_pressure_float_operations() {
    // Heavy register pressure with float operations
    let source = r#"
fn main() {
    let f0 = 0.0
    let f1 = 1.1
    let f2 = 2.2
    let f3 = 3.3
    let f4 = 4.4
    let f5 = 5.5
    let f6 = 6.6
    let f7 = 7.7
    let f8 = 8.8
    let f9 = 9.9
    let result = f0 + f1 + f2 + f3 + f4 + f5 + f6 + f7 + f8 + f9
    print(result)
}
"#;
    // Sum should be approximately 49.5
    let output_full = compile_and_run_stdout(source);
    let output = output_full.trim();
    let value: f64 = output.parse().expect("Should parse as float");
    assert!((value - 49.5).abs() < 0.1);
}

#[test]
fn test_constant_folding_with_negation() {
    // Constant folding with unary negation
    let source = r#"
fn main() {
    let result = -(5 + 3)
    print(result)
}
"#;
    assert_eq!(compile_and_run_stdout(source).trim(), "-8");
}

#[test]
fn test_exit_code_optimization() {
    // Return value should be correctly passed as exit code
    let source = r#"
fn main() int {
    return 42
}
"#;
    assert_eq!(compile_and_run(source), 42);
}

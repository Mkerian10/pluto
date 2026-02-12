// Category 15: Platform-Specific Tests (10+ tests)
// Validates platform-specific code generation for AArch64 and x86_64

use super::common::{compile_and_run, compile_and_run_stdout, compile_and_run_output};

// ============================================================================
// AArch64 Tests (5 tests)
// ============================================================================

#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_basic_function_call() {
    // Verify that basic function calls work correctly on AArch64
    // This validates calling convention (arguments in x0-x7, return in x0)
    let src = r#"
        fn add(a: int, b: int) int {
            return a + b
        }

        fn main() {
            let result = add(42, 58)
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "100");
}

#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_register_pressure() {
    // Test code with high register pressure to verify spilling/reload
    // AArch64 has 31 general-purpose registers (x0-x30), test heavy usage
    let src = r#"
        fn main() {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
            let e = 5
            let f = 6
            let g = 7
            let h = 8
            let i = 9
            let j = 10
            let k = 11
            let l = 12
            let m = 13
            let n = 14
            let o = 15
            let p = 16
            // Compute something with all variables to force them live
            let result = a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "136");
}

#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_stack_alignment() {
    // Verify stack is properly aligned (16-byte on AArch64)
    // Test with multiple function calls and local variables
    let src = r#"
        fn level3(x: int) int {
            let a = x + 1
            let b = x + 2
            let c = x + 3
            return a + b + c
        }

        fn level2(x: int) int {
            let y = level3(x)
            return y * 2
        }

        fn level1(x: int) int {
            let z = level2(x)
            return z + 10
        }

        fn main() {
            print(level1(5))
        }
    "#;
    // level3(5) = (6 + 7 + 8) = 21
    // level2(5) = 21 * 2 = 42
    // level1(5) = 42 + 10 = 52
    assert_eq!(compile_and_run_stdout(src).trim(), "52");
}

#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_float_operations() {
    // Verify floating-point operations use correct AArch64 FP instructions
    // AArch64 has dedicated FP registers (v0-v31)
    let src = r#"
        fn main() {
            let a = 3.5
            let b = 2.0
            let sum = a + b
            let product = a * b
            let quotient = a / b
            print(sum)
            print(product)
            print(quotient)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "5.500000");
    assert_eq!(lines[1], "7.000000");
    assert_eq!(lines[2], "1.750000");
}

#[test]
#[cfg(target_arch = "aarch64")]
fn test_aarch64_struct_field_access() {
    // Test struct field access patterns (offset calculations)
    // Verify correct load/store instructions with offset addressing
    let src = r#"
        class Point {
            x: int
            y: int
            z: int
        }

        fn main() {
            let p = Point { x: 10, y: 20, z: 30 }
            print(p.x)
            print(p.y)
            print(p.z)
            let sum = p.x + p.y + p.z
            print(sum)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
    assert_eq!(lines[2], "30");
    assert_eq!(lines[3], "60");
}

// ============================================================================
// x86_64 Tests (5 tests)
// ============================================================================

#[test]
#[cfg(target_arch = "x86_64")]
fn test_x86_64_basic_function_call() {
    // Verify that basic function calls work correctly on x86_64
    // This validates System V AMD64 ABI (arguments in rdi, rsi, rdx, rcx, r8, r9)
    let src = r#"
        fn add(a: int, b: int) int {
            return a + b
        }

        fn main() {
            let result = add(42, 58)
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "100");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_x86_64_register_pressure() {
    // Test code with high register pressure to verify spilling/reload
    // x86_64 has 16 general-purpose registers (rax, rbx, rcx, rdx, rsi, rdi, rbp, rsp, r8-r15)
    let src = r#"
        fn main() {
            let a = 1
            let b = 2
            let c = 3
            let d = 4
            let e = 5
            let f = 6
            let g = 7
            let h = 8
            let i = 9
            let j = 10
            let k = 11
            let l = 12
            let m = 13
            let n = 14
            let o = 15
            let p = 16
            // Compute something with all variables to force them live
            let result = a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "136");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_x86_64_stack_alignment() {
    // Verify stack is properly aligned (16-byte on x86_64)
    // Test with multiple function calls and local variables
    let src = r#"
        fn level3(x: int) int {
            let a = x + 1
            let b = x + 2
            let c = x + 3
            return a + b + c
        }

        fn level2(x: int) int {
            let y = level3(x)
            return y * 2
        }

        fn level1(x: int) int {
            let z = level2(x)
            return z + 10
        }

        fn main() {
            print(level1(5))
        }
    "#;
    // level3(5) = (6 + 7 + 8) = 21
    // level2(5) = 21 * 2 = 42
    // level1(5) = 42 + 10 = 52
    assert_eq!(compile_and_run_stdout(src).trim(), "52");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_x86_64_float_operations() {
    // Verify floating-point operations use correct x86_64 SSE/AVX instructions
    // x86_64 uses XMM registers for floating-point
    let src = r#"
        fn main() {
            let a = 3.5
            let b = 2.0
            let sum = a + b
            let product = a * b
            let quotient = a / b
            print(sum)
            print(product)
            print(quotient)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "5.5");
    assert_eq!(lines[1], "7");
    assert_eq!(lines[2], "1.75");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_x86_64_struct_field_access() {
    // Test struct field access patterns (offset calculations)
    // Verify correct load/store instructions with offset addressing
    let src = r#"
        class Point {
            x: int
            y: int
            z: int
        }

        fn main() {
            let p = Point { x: 10, y: 20, z: 30 }
            print(p.x)
            print(p.y)
            print(p.z)
            let sum = p.x + p.y + p.z
            print(sum)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
    assert_eq!(lines[2], "30");
    assert_eq!(lines[3], "60");
}

// ============================================================================
// Cross-platform Tests (tests that should work on all platforms)
// ============================================================================

#[test]
fn test_target_triple_detection() {
    // Verify that the compiler successfully detects and uses the correct target triple
    // This test runs on any platform and validates basic compilation works
    let src = r#"
        fn main() {
            print("Target detection works")
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "Target detection works");
}

#[test]
fn test_cross_platform_arithmetic() {
    // Verify that basic arithmetic works identically on all platforms
    let src = r#"
        fn main() {
            let a = 100
            let b = 50
            print(a + b)
            print(a - b)
            print(a * b)
            print(a / b)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "150");
    assert_eq!(lines[1], "50");
    assert_eq!(lines[2], "5000");
    assert_eq!(lines[3], "2");
}

#[test]
fn test_cross_platform_function_parameters() {
    // Test function with many parameters (exercises parameter passing conventions)
    // Should work identically regardless of calling convention differences
    let src = r#"
        fn sum8(a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int) int {
            return a + b + c + d + e + f + g + h
        }

        fn main() {
            print(sum8(1, 2, 3, 4, 5, 6, 7, 8))
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "36");
}

#[test]
fn test_cross_platform_recursion() {
    // Test recursive function (validates stack frame setup/teardown)
    let src = r#"
        fn factorial(n: int) int {
            if n <= 1 {
                return 1
            }
            return n * factorial(n - 1)
        }

        fn main() {
            print(factorial(5))
            print(factorial(10))
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "120");
    assert_eq!(lines[1], "3628800");
}

#[test]
fn test_cross_platform_array_operations() {
    // Test array creation and indexing (validates memory layout consistency)
    let src = r#"
        fn main() {
            let arr = [10, 20, 30, 40, 50]
            print(arr[0])
            print(arr[2])
            print(arr[4])
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "30");
    assert_eq!(lines[2], "50");
}

#[test]
fn test_cross_platform_closure_creation() {
    // Test closure creation and calling (validates closure calling convention)
    let src = r#"
        fn main() {
            let x = 10
            let f = (y: int) => x + y
            print(f(5))
            print(f(15))
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "15");
    assert_eq!(lines[1], "25");
}

#[test]
#[ignore] // LIMITATION: Binary literal syntax (0b1010) not supported in Pluto
fn test_cross_platform_bitwise_operations() {
    // Test bitwise operations (platform-independent but good to verify)
    let src = r#"
        fn main() {
            let a = 0b1010
            let b = 0b1100
            print(a & b)
            print(a | b)
            print(a ^ b)
            print(~a)
            print(1 << 4)
            print(16 >> 2)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "8");
    assert_eq!(lines[1], "14");
    assert_eq!(lines[2], "6");
    assert_eq!(lines[3], "-11");
    assert_eq!(lines[4], "16");
    assert_eq!(lines[5], "4");
}

#[test]
fn test_cross_platform_string_operations() {
    // Test string operations (heap allocation and pointer handling)
    let src = r#"
        fn main() {
            let s1 = "Hello"
            let s2 = " World"
            let s3 = s1 + s2
            print(s3)
            print(s3.len())
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "Hello World");
    assert_eq!(lines[1], "11");
}

#[test]
fn test_cross_platform_class_methods() {
    // Test class method calls (validates method calling convention)
    let src = r#"
        class Counter {
            value: int
        }

        fn Counter.increment(mut self) {
            self.value = self.value + 1
        }

        fn Counter.get(self) int {
            return self.value
        }

        fn main() {
            let c = Counter { value: 0 }
            c.increment()
            c.increment()
            c.increment()
            print(c.get())
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3");
}

#[test]
#[ignore] // LIMITATION: Field binding syntax in match arms ({ value: v }) not supported
fn test_cross_platform_enum_match() {
    // Test enum matching (validates discriminant handling)
    let src = r#"
        enum Result {
            Ok { value: int }
            Err { msg: string }
        }

        fn main() {
            let r1 = Result.Ok { value: 42 }
            let r2 = Result.Err { msg: "failed" }

            match r1 {
                Result.Ok { value: v } => print(v)
                Result.Err { msg: m } => print(m)
            }

            match r2 {
                Result.Ok { value: v } => print(v)
                Result.Err { msg: m } => print(m)
            }
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "42");
    assert_eq!(lines[1], "failed");
}

#[test]
fn test_cross_platform_nested_calls() {
    // Test deeply nested function calls (validates stack management)
    let src = r#"
        fn f1(x: int) int { return x + 1 }
        fn f2(x: int) int { return f1(x) + 1 }
        fn f3(x: int) int { return f2(x) + 1 }
        fn f4(x: int) int { return f3(x) + 1 }
        fn f5(x: int) int { return f4(x) + 1 }
        fn f6(x: int) int { return f5(x) + 1 }
        fn f7(x: int) int { return f6(x) + 1 }
        fn f8(x: int) int { return f7(x) + 1 }
        fn f9(x: int) int { return f8(x) + 1 }
        fn f10(x: int) int { return f9(x) + 1 }

        fn main() {
            print(f10(0))
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10");
}

#[test]
fn test_cross_platform_mixed_types() {
    // Test functions with mixed parameter types (validates type size handling)
    let src = r#"
        fn process(i: int, f: float, b: bool, s: string) {
            print(i)
            print(f)
            print(b)
            print(s)
        }

        fn main() {
            process(42, 3.14, true, "test")
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "42");
    assert_eq!(lines[1], "3.140000");
    assert_eq!(lines[2], "true");
    assert_eq!(lines[3], "test");
}

#[test]
fn test_cross_platform_large_stack_frame() {
    // Test function with many local variables (validates stack frame allocation)
    let src = r#"
        fn main() {
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

            let sum = v1 + v2 + v3 + v4 + v5 + v6 + v7 + v8 + v9 + v10 +
                     v11 + v12 + v13 + v14 + v15 + v16 + v17 + v18 + v19 + v20
            print(sum)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "210");
}

#[test]
fn test_cross_platform_return_large_struct() {
    // Test returning a struct by value (validates struct return convention)
    let src = r#"
        class Data {
            a: int
            b: int
            c: int
            d: int
        }

        fn create_data() Data {
            return Data { a: 10, b: 20, c: 30, d: 40 }
        }

        fn main() {
            let data = create_data()
            print(data.a)
            print(data.b)
            print(data.c)
            print(data.d)
        }
    "#;
    let output = compile_and_run_stdout(src);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
    assert_eq!(lines[2], "30");
    assert_eq!(lines[3], "40");
}

// Category 6: Error Handling Tests (30+ tests)
// Validates error handling codegen: raise, propagate (!), catch, TLS error state

use super::common::{compile_and_run, compile_and_run_stdout, compile_and_run_output, compile_should_fail};

// ============================================================================
// Raise (5 tests)
// ============================================================================

#[test]
fn test_raise_custom_error_in_function() {
    let src = r#"
        error MyError {
            code: int
        }

        fn fail() int {
            raise MyError { code: 42 }
            return 0
        }

        fn main() {
            let result = fail() catch 99
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "99");
}

#[test]
fn test_raise_error_with_string_field() {
    let src = r#"
        error ValidationError {
            message: string
        }

        fn validate(input: string) string {
            if input == "" {
                raise ValidationError { message: "empty input not allowed" }
            }
            return input
        }

        fn main() {
            let a = validate("hello") catch "error"
            print(a)
            let b = validate("") catch "error"
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "hello\nerror");
}

#[test]
fn test_raise_error_in_method() {
    let src = r#"
        error OutOfBounds {
            index: int
        }

        class Container {
            size: int

            fn check(self, index: int) int {
                if index >= self.size {
                    raise OutOfBounds { index: index }
                }
                return index
            }
        }

        fn main() {
            let c = Container { size: 5 }
            let a = c.check(3) catch -1
            print(a)
            let b = c.check(10) catch -1
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3\n-1");
}

#[test]
#[ignore] // BUG: Errors in closures not supported - pipeline timing bug. See issue #137
fn test_raise_error_in_closure() {
    let src = r#"
        error ClosureError {
            value: int
        }

        fn main() {
            let threshold = 10
            let check = (x: int) => {
                if x > threshold {
                    raise ClosureError { value: x }
                }
                return x * 2
            }

            let a = check(5) catch 0
            print(a)
            let b = check(15) catch 0
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10\n0");
}

#[test]
fn test_raise_error_no_fields() {
    let src = r#"
        error Empty {}

        fn always_fail() int {
            raise Empty {}
            return 100
        }

        fn main() {
            let result = always_fail() catch 0
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

// ============================================================================
// Propagate (!) (10 tests)
// ============================================================================

#[test]
fn test_propagate_from_function_call() {
    let src = r#"
        error FailError {}

        fn inner() int {
            raise FailError {}
            return 1
        }

        fn outer() int {
            let x = inner()!
            return x + 1
        }

        fn main() {
            let result = outer() catch 999
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "999");
}

#[test]
fn test_propagate_chain_multiple_calls() {
    // FIXED: a() and b() were infallible, can't use `!` on them
    // Made all functions fallible to test propagation in expression chains
    let src = r#"
        error E {}

        fn a() int {
            if false { raise E {} }
            return 10
        }

        fn b() int {
            if false { raise E {} }
            return 20
        }

        fn c() int {
            raise E {}
            return 30
        }

        fn process() int {
            let sum = a()! + b()! + c()!
            return sum
        }

        fn main() {
            let result = process() catch 0
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_propagate_in_arithmetic_expression() {
    let src = r#"
        error MathError {}

        fn safe_divide(a: int, b: int) int {
            if b == 0 {
                raise MathError {}
            }
            return a / b
        }

        fn compute(x: int, y: int) int {
            let result = safe_divide(x, y)! * 2 + 5
            return result
        }

        fn main() {
            let a = compute(10, 2) catch -1
            print(a)
            let b = compute(10, 0) catch -1
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15\n-1");
}

#[test]
fn test_propagate_nested_call() {
    let src = r#"
        error DeepError {}

        fn level1() int {
            raise DeepError {}
            return 1
        }

        fn level2() int {
            return level1()! + 10
        }

        fn level3() int {
            return level2()! + 100
        }

        fn main() {
            let result = level3() catch 777
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "777");
}

#[test]
fn test_propagate_with_value_unwrap() {
    let src = r#"
        error ParseError {}

        fn parse_int(s: string) int {
            if s == "" {
                raise ParseError {}
            }
            return 42
        }

        fn process(input: string) int {
            let value = parse_int(input)!
            print(value)
            return value * 2
        }

        fn main() {
            let a = process("valid") catch 0
            print(a)
            let b = process("") catch 0
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42\n84\n0");
}

#[test]
fn test_propagate_skips_subsequent_code() {
    let src = r#"
        error E {}

        fn might_fail(x: int) int {
            if x == 0 {
                raise E {}
            }
            return x
        }

        fn wrapper(x: int) int {
            let a = might_fail(x)!
            print(888)  // Should not execute when error propagates
            return a
        }

        fn main() {
            let result = wrapper(0) catch 0
            print(result)
        }
    "#;
    // Only "0" should be printed, NOT "888"
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_propagate_transitive_chain() {
    let src = r#"
        error ChainError {}

        fn step1() int {
            raise ChainError {}
            return 0
        }

        fn step2() int {
            let x = step1()!
            return x + 1
        }

        fn step3() int {
            let x = step2()!
            return x + 1
        }

        fn step4() int {
            let x = step3()!
            return x + 1
        }

        fn main() {
            let result = step4() catch -1
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1");
}

#[test]
fn test_propagate_in_loop() {
    let src = r#"
        error LoopError {}

        fn check(x: int) int {
            if x > 5 {
                raise LoopError {}
            }
            return x
        }

        fn sum_until_error() int {
            let total = 0
            let i = 0
            while i < 10 {
                total = total + check(i)!
                i = i + 1
            }
            return total
        }

        fn main() {
            let result = sum_until_error() catch 999
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "999");
}

#[test]
fn test_propagate_multiple_in_sequence() {
    // FIXED: first() and second() were infallible, can't use `!` on them
    // Made all functions fallible to test sequential propagation
    let src = r#"
        error E {}

        fn first() int {
            if false { raise E {} }
            return 1
        }

        fn second() int {
            if false { raise E {} }
            return 2
        }

        fn third() int {
            raise E {}
            return 3
        }

        fn caller() int {
            let a = first()!
            let b = second()!
            let c = third()!
            return a + b + c
        }

        fn main() {
            let result = caller() catch 0
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_propagate_from_method_call() {
    let src = r#"
        error MethodError {}

        class Calculator {
            base: int

            fn compute(self, x: int) int {
                if x < 0 {
                    raise MethodError {}
                }
                return self.base + x
            }
        }

        fn process(calc: Calculator, val: int) int {
            return calc.compute(val)! * 2
        }

        fn main() {
            let calc = Calculator { base: 10 }
            let a = process(calc, 5) catch 0
            print(a)
            let b = process(calc, -3) catch 0
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "30\n0");
}

// ============================================================================
// Catch (10 tests)
// ============================================================================

#[test]
fn test_catch_specific_error_type() {
    let src = r#"
        error SpecificError {
            code: int
        }

        fn fail_with_code(code: int) int {
            raise SpecificError { code: code }
            return 0
        }

        fn main() {
            let result = fail_with_code(404) catch 0
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_catch_multiple_error_types() {
    let src = r#"
        error NotFound {
            id: int
        }

        error Forbidden {
            reason: string
        }

        fn find(id: int) int {
            if id < 0 {
                raise NotFound { id: id }
            }
            return id
        }

        fn check_access(level: int) int {
            if level < 5 {
                raise Forbidden { reason: "insufficient level" }
            }
            return level
        }

        fn main() {
            let a = find(-1) catch 0
            print(a)
            let b = check_access(3) catch 0
            print(b)
            let c = find(10) catch 0
            print(c)
            let d = check_access(7) catch 0
            print(d)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0\n0\n10\n7");
}

#[test]
fn test_catch_with_fallback_value() {
    let src = r#"
        error DivisionError {}

        fn safe_divide(a: int, b: int) int {
            if b == 0 {
                raise DivisionError {}
            }
            return a / b
        }

        fn main() {
            let a = safe_divide(10, 2) catch 0
            print(a)
            let b = safe_divide(10, 0) catch 999
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "5\n999");
}

#[test]
fn test_catch_wildcard_with_variable() {
    let src = r#"
        error BadInput {
            code: int
        }

        fn validate(x: int) int {
            if x < 0 {
                raise BadInput { code: x }
            }
            return x
        }

        fn main() {
            let fallback = 42
            let result = validate(-5) catch err { fallback }
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_nested_catch() {
    let src = r#"
        error OuterError {}
        error InnerError {}

        fn inner() int {
            raise InnerError {}
            return 1
        }

        fn outer() int {
            let x = inner() catch 5
            if x == 5 {
                raise OuterError {}
            }
            return x
        }

        fn main() {
            let result = outer() catch 99
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "99");
}

#[test]
fn test_catch_both_success_and_error_paths() {
    let src = r#"
        error Maybe {}

        fn sometimes(x: int) int {
            if x == 0 {
                raise Maybe {}
            }
            return x * 3
        }

        fn main() {
            let a = sometimes(0) catch -1
            let b = sometimes(4) catch -1
            print(a)
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1\n12");
}

#[test]
fn test_catch_in_variable_assignment() {
    let src = r#"
        error E {}

        fn fail() int {
            raise E {}
            return 0
        }

        fn main() {
            let x = fail() catch 100
            let y = fail() catch 200
            let z = fail() catch 300
            print(x)
            print(y)
            print(z)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "100\n200\n300");
}

#[test]
fn test_catch_with_class_fallback() {
    let src = r#"
        error NotFound {}

        class Point {
            x: int
            y: int
        }

        fn find_point(id: int) Point {
            if id < 0 {
                raise NotFound {}
            }
            return Point { x: id, y: id * 2 }
        }

        fn default_point() Point {
            return Point { x: 0, y: 0 }
        }

        fn main() {
            let p1 = find_point(5) catch default_point()
            print(p1.x)
            print(p1.y)

            let p2 = find_point(-1) catch default_point()
            print(p2.x)
            print(p2.y)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "5\n10\n0\n0");
}

#[test]
fn test_catch_in_conditional() {
    let src = r#"
        error E {}

        fn might_fail(x: int) int {
            if x < 0 {
                raise E {}
            }
            return x
        }

        fn main() {
            let input = -5
            if input < 0 {
                let result = might_fail(input) catch 0
                print(result)
            } else {
                print(999)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0");
}

#[test]
fn test_catch_multiple_in_sequence() {
    let src = r#"
        error E {}

        fn fail() int {
            raise E {}
            return 0
        }

        fn main() {
            let a = fail() catch 1
            let b = fail() catch 2
            let c = fail() catch 3
            let d = fail() catch 4
            print(a)
            print(b)
            print(c)
            print(d)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1\n2\n3\n4");
}

// ============================================================================
// Error State Management (5+ tests)
// ============================================================================

#[test]
fn test_error_state_cleared_after_catch() {
    let src = r#"
        error E1 {}
        error E2 {}

        fn fail1() int {
            raise E1 {}
            return 1
        }

        fn fail2() int {
            raise E2 {}
            return 2
        }

        fn main() {
            let a = fail1() catch 10
            let b = fail2() catch 20
            let c = fail1() catch 30
            print(a)
            print(b)
            print(c)
        }
    "#;
    // Each catch should clear error state, allowing next call to work
    assert_eq!(compile_and_run_stdout(src).trim(), "10\n20\n30");
}

#[test]
fn test_error_state_across_function_calls() {
    let src = r#"
        error E {}

        fn first() int {
            raise E {}
            return 1
        }

        fn second() int {
            return 2
        }

        fn main() {
            let a = first() catch 0
            print(a)

            // Error state should be clear, second() should work normally
            let b = second()
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0\n2");
}

#[test]
fn test_error_not_propagated_after_catch() {
    let src = r#"
        error E {}

        fn inner() int {
            raise E {}
            return 1
        }

        fn middle() int {
            let x = inner() catch 5
            return x + 10
        }

        fn outer() int {
            return middle()  // No ! needed, middle() doesn't propagate
        }

        fn main() {
            let result = outer()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_error_state_isolation_in_sequence() {
    let src = r#"
        error E {}

        fn fail() int {
            raise E {}
            return 0
        }

        fn safe() int {
            return 100
        }

        fn main() {
            let a = fail() catch 1
            let b = safe()  // Should work fine, error state cleared
            let c = fail() catch 2
            let d = safe()  // Should work fine, error state cleared

            print(a)
            print(b)
            print(c)
            print(d)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1\n100\n2\n100");
}

#[test]
fn test_propagate_in_main_exits_silently() {
    let src = r#"
        error Fatal {}

        fn will_fail() {
            raise Fatal {}
        }

        fn main() {
            will_fail()!
            print(42)  // Should not execute
        }
    "#;
    // When error propagates in main, program exits without printing
    assert_eq!(compile_and_run_stdout(src).trim(), "");
}

// ============================================================================
// Additional Edge Cases (5+ tests)
// ============================================================================

#[test]
fn test_error_in_while_loop_with_catch() {
    let src = r#"
        error OutOfRange {}

        fn check(x: int) int {
            if x > 5 {
                raise OutOfRange {}
            }
            return x
        }

        fn main() {
            let i = 0
            let sum = 0
            while i < 10 {
                let val = check(i) catch 0
                sum = sum + val
                i = i + 1
            }
            print(sum)
        }
    "#;
    // Sum of 0+1+2+3+4+5 + (0 for 6,7,8,9) = 15
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_error_with_multiple_fields() {
    let src = r#"
        error ComplexError {
            code: int
            message: string
            level: int
        }

        fn fail_complex() int {
            raise ComplexError { code: 500, message: "internal error", level: 3 }
            return 0
        }

        fn main() {
            let result = fail_complex() catch 999
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "999");
}

#[test]
fn test_conditional_raise_in_branch() {
    let src = r#"
        error TooSmall {
            value: int
        }

        fn check_positive(x: int) int {
            if x <= 0 {
                raise TooSmall { value: x }
            }
            return x
        }

        fn main() {
            let a = check_positive(10) catch 0
            print(a)
            let b = check_positive(-5) catch 0
            print(b)
            let c = check_positive(7) catch 0
            print(c)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10\n0\n7");
}

#[test]
fn test_error_propagation_in_expression_context() {
    let src = r#"
        error E {}

        fn get_value(x: int) int {
            if x == 0 {
                raise E {}
            }
            return x
        }

        fn compute(a: int, b: int) int {
            return get_value(a)! + get_value(b)!
        }

        fn main() {
            let r1 = compute(5, 10) catch 0
            print(r1)
            let r2 = compute(0, 10) catch 0
            print(r2)
            let r3 = compute(5, 0) catch 0
            print(r3)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15\n0\n0");
}

#[test]
fn test_catch_with_string_return_type() {
    let src = r#"
        error E {
            msg: string
        }

        fn get_name(valid: bool) string {
            if valid {
                return "success"
            } else {
                raise E { msg: "failed" }
            }
        }

        fn main() {
            let a = get_name(true) catch "error"
            print(a)
            let b = get_name(false) catch "error"
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "success\nerror");
}

#[test]
fn test_error_raise_after_normal_computation() {
    let src = r#"
        error LateError {}

        fn process(x: int) int {
            let temp = x * 2
            print(temp)
            if temp > 20 {
                raise LateError {}
            }
            return temp
        }

        fn main() {
            let a = process(5) catch 0
            print(a)
            let b = process(15) catch 0
            print(b)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "10\n10\n30\n0");
}

#[test]
fn test_propagate_through_multiple_return_paths() {
    let src = r#"
        error E {}

        fn may_fail(x: int) int {
            if x < 0 {
                raise E {}
            }
            return x
        }

        fn wrapper(x: int) int {
            if x == 0 {
                return 0
            }
            if x > 100 {
                return 100
            }
            return may_fail(x)!
        }

        fn main() {
            let a = wrapper(0) catch -1
            print(a)
            let b = wrapper(50) catch -1
            print(b)
            let c = wrapper(-10) catch -1
            print(c)
            let d = wrapper(150) catch -1
            print(d)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "0\n50\n-1\n100");
}

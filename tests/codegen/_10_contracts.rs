// Category 10: Contracts Tests (20+ tests)
// Comprehensive test suite for runtime contract checking codegen.
// Tests validate correct invariant, requires, and assert enforcement.

use super::common::{compile_and_run, compile_and_run_output, compile_and_run_stdout};

// ============================================================================
// 1. Invariants - Construction (5 tests)
// ============================================================================

#[test]
fn test_invariant_checked_after_construction_simple() {
    // Verify invariant is checked immediately after struct literal construction
    let src = r#"
        class Positive {
            value: int

            invariant self.value > 0
        }

        fn main() int {
            let p = Positive { value: 5 }
            return p.value
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn test_invariant_checked_after_construction_multiple_fields() {
    // Multiple fields with multiple invariants
    let src = r#"
        class Rectangle {
            width: int
            height: int

            invariant self.width > 0
            invariant self.height > 0
        }

        fn main() int {
            let r = Rectangle { width: 10, height: 20 }
            return r.width + r.height
        }
    "#;
    assert_eq!(compile_and_run(src), 30);
}

#[test]
fn test_invariant_violation_at_construction_aborts() {
    // Invariant violation during construction should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        class Positive {
            value: int

            invariant self.value > 0
        }

        fn main() {
            let p = Positive { value: -1 }
            print(p.value)
        }
        "#,
    );
    assert_ne!(code, 0, "Should exit with non-zero for invariant violation");
    assert!(
        stderr.contains("invariant violation"),
        "stderr should contain 'invariant violation', got: {stderr}"
    );
    assert!(
        stderr.contains("Positive"),
        "stderr should mention class name, got: {stderr}"
    );
}

#[test]
fn test_invariant_construction_with_complex_expression() {
    // Invariant with arithmetic and logical operations
    let src = r#"
        class BoundedPair {
            x: int
            y: int

            invariant self.x + self.y >= 0 && self.x + self.y <= 100
        }

        fn main() int {
            let p = BoundedPair { x: 30, y: 40 }
            return p.x + p.y
        }
    "#;
    assert_eq!(compile_and_run(src), 70);
}

#[test]
fn test_invariant_construction_boundary_value() {
    // Test exact boundary of invariant condition
    let src = r#"
        class NonNegative {
            value: int

            invariant self.value >= 0
        }

        fn main() int {
            let n = NonNegative { value: 0 }
            return n.value
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

// ============================================================================
// 2. Invariants - Method Calls (5 tests)
// ============================================================================

#[test]
fn test_invariant_checked_after_mut_method() {
    // Invariant re-checked after mut method completes
    let src = r#"
        class Counter {
            value: int

            invariant self.value >= 0

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() int {
            let mut c = Counter { value: 0 }
            c.increment()
            c.increment()
            return c.get()
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn test_invariant_violation_after_mut_method_aborts() {
    // Invariant violation after mut method should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        class BoundedCounter {
            value: int

            invariant self.value >= 0
            invariant self.value <= 10

            fn set(mut self, v: int) {
                self.value = v
            }
        }

        fn main() {
            let mut c = BoundedCounter { value: 5 }
            c.set(100)
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"), "stderr: {stderr}");
}

#[test]
fn test_invariant_read_lock_for_non_mut_methods() {
    // Non-mut methods should not trigger invariant checks
    // (they acquire read locks, invariants checked only with write locks)
    let src = r#"
        class Data {
            value: int

            invariant self.value > 0

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let d = Data { value: 42 }
            print(d.get())
            print(d.get())
            print(d.get())
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert_eq!(output, "42\n42\n42\n");
}

#[test]
fn test_invariant_write_lock_for_mut_methods() {
    // Mut methods acquire write locks and re-check invariants
    let src = r#"
        class Stack {
            size: int

            invariant self.size >= 0

            fn push(mut self) {
                self.size = self.size + 1
            }

            fn pop(mut self) {
                self.size = self.size - 1
            }
        }

        fn main() int {
            let mut s = Stack { size: 0 }
            s.push()
            s.push()
            s.push()
            s.pop()
            return s.size
        }
    "#;
    assert_eq!(compile_and_run(src), 2);
}

#[test]
fn test_invariant_multiple_mut_methods_chained() {
    // Multiple mut method calls in sequence
    let src = r#"
        class Range {
            lo: int
            hi: int

            invariant self.hi > self.lo

            fn shift_up(mut self, amount: int) {
                self.lo = self.lo + amount
                self.hi = self.hi + amount
            }

            fn widen(mut self, amount: int) {
                self.hi = self.hi + amount
            }
        }

        fn main() int {
            let mut r = Range { lo: 0, hi: 10 }
            r.shift_up(5)
            r.widen(10)
            return r.hi - r.lo
        }
    "#;
    assert_eq!(compile_and_run(src), 20);
}

// ============================================================================
// 3. Requires - Entry Checks (5 tests)
// ============================================================================

#[test]
fn test_requires_checked_on_entry() {
    // Requires clause checked at function entry
    let src = r#"
        fn positive_double(x: int) int
            requires x > 0
        {
            return x * 2
        }

        fn main() int {
            return positive_double(5)
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_requires_violation_aborts() {
    // Requires violation should abort with diagnostic
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn positive_double(x: int) int
            requires x > 0
        {
            return x * 2
        }

        fn main() {
            print(positive_double(-1))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("positive_double"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn test_requires_multiple_clauses_all_satisfied() {
    // Multiple requires clauses, all satisfied
    let src = r#"
        fn bounded(x: int) int
            requires x > 0
            requires x < 100
        {
            return x
        }

        fn main() int {
            return bounded(50)
        }
    "#;
    assert_eq!(compile_and_run(src), 50);
}

#[test]
fn test_requires_multiple_clauses_one_violated() {
    // Multiple requires, second one violated
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn bounded(x: int) int
            requires x > 0
            requires x < 100
        {
            return x
        }

        fn main() {
            print(bounded(200))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x < 100"), "stderr: {stderr}");
}

#[test]
fn test_requires_on_method() {
    // Requires on class method
    let src = r#"
        class Account {
            balance: int

            fn deposit(mut self, amount: int)
                requires amount > 0
            {
                self.balance = self.balance + amount
            }
        }

        fn main() int {
            let mut a = Account { balance: 100 }
            a.deposit(50)
            return a.balance
        }
    "#;
    assert_eq!(compile_and_run(src), 150);
}

// ============================================================================
// 4. Assert Statement (5 tests)
// ============================================================================

#[test]
fn test_assert_true_succeeds() {
    // Assert with true condition should not abort
    let src = r#"
        fn main() int {
            assert 1 > 0
            return 42
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_assert_false_aborts() {
    // Assert with false condition should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn main() {
            assert 1 < 0
            print("unreachable")
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("assertion failed"), "stderr: {stderr}");
    assert!(stderr.contains("1 < 0"), "stderr: {stderr}");
}

#[test]
fn test_assert_with_variables() {
    // Assert can reference local variables
    let src = r#"
        fn main() int {
            let x = 10
            let y = 5
            assert x > y
            return x - y
        }
    "#;
    assert_eq!(compile_and_run(src), 5);
}

#[test]
fn test_assert_with_function_call() {
    // Assert accepts any boolean expression including function calls
    let src = r#"
        fn is_positive(x: int) bool {
            return x > 0
        }

        fn main() int {
            assert is_positive(42)
            return 1
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn test_assert_complex_expression() {
    // Assert with compound logical expression
    let src = r#"
        fn main() int {
            let x = 50
            let y = 20
            assert (x > 0) && (y < 100) && (x + y < 200)
            return x + y
        }
    "#;
    assert_eq!(compile_and_run(src), 70);
}

// ============================================================================
// 5. Assert in Methods and Functions (5 tests)
// ============================================================================

#[test]
fn test_assert_in_function() {
    // Assert inside a regular function
    let src = r#"
        fn check_and_double(x: int) int {
            assert x > 0
            return x * 2
        }

        fn main() int {
            return check_and_double(5)
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

#[test]
fn test_assert_in_function_failure() {
    // Assert failure inside a function should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn check_and_double(x: int) int {
            assert x > 0
            return x * 2
        }

        fn main() {
            print(check_and_double(-1))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("assertion failed"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn test_assert_in_method() {
    // Assert inside a class method
    let src = r#"
        class Validator {
            min_value: int

            fn validate(self, x: int) int {
                assert x >= self.min_value
                return x
            }
        }

        fn main() int {
            let v = Validator { min_value: 0 }
            return v.validate(42)
        }
    "#;
    assert_eq!(compile_and_run(src), 42);
}

#[test]
fn test_assert_with_field_access() {
    // Assert referencing object fields
    let src = r#"
        class Config {
            max_retries: int
        }

        fn main() int {
            let c = Config { max_retries: 3 }
            assert c.max_retries > 0
            return c.max_retries
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn test_assert_multiple_in_sequence() {
    // Multiple asserts in sequence
    let src = r#"
        fn main() int {
            let x = 10
            assert x > 0
            assert x < 100
            assert x != 5
            return x
        }
    "#;
    assert_eq!(compile_and_run(src), 10);
}

// ============================================================================
// 6. Combined Contracts with Assert (5 tests)
// ============================================================================

#[test]
fn test_requires_with_assert_in_body() {
    // Requires on entry, assert as additional check inside body
    let src = r#"
        fn safe_divide(a: int, b: int) int
            requires b != 0
        {
            let result = a / b
            assert result * b <= a
            return result
        }

        fn main() int {
            return safe_divide(10, 3)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn test_invariant_with_assert_in_method() {
    // Class with invariant, method uses assert for internal checks
    let src = r#"
        class PositiveCounter {
            value: int

            invariant self.value >= 0

            fn add(mut self, amount: int) int
                requires amount > 0
            {
                let old_value = self.value
                self.value = self.value + amount
                assert self.value == old_value + amount
                return self.value
            }
        }

        fn main() int {
            let mut c = PositiveCounter { value: 10 }
            return c.add(5)
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn test_all_contracts_with_assert() {
    // Invariant + requires + assert in body
    let src = r#"
        class Range {
            lo: int
            hi: int

            invariant self.lo >= 0
            invariant self.hi > self.lo

            fn expand(mut self, amount: int) {
                assert amount > 0
                self.hi = self.hi + amount
            }

            fn size(self) int {
                return self.hi - self.lo
            }
        }

        fn main() int {
            let mut r = Range { lo: 0, hi: 10 }
            r.expand(5)
            return r.size()
        }
    "#;
    assert_eq!(compile_and_run(src), 15);
}

#[test]
fn test_requires_violation_before_assert() {
    // When requires fails, assert in body should not be reached
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn foo(x: int) int
            requires x > 0
        {
            assert x < 100
            return x
        }

        fn main() {
            print(foo(-5))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(!stderr.contains("assertion failed"), "should not reach assert, stderr: {stderr}");
}

#[test]
fn test_invariant_violation_before_assert() {
    // If invariant is violated at construction, assert in method is not reached
    let (_, stderr, code) = compile_and_run_output(
        r#"
        class BoundedValue {
            value: int

            invariant self.value >= 0
            invariant self.value <= 100

            fn check(self) int {
                assert self.value > 50
                return self.value
            }
        }

        fn main() {
            let b = BoundedValue { value: 200 }
            print(b.check())
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"), "stderr: {stderr}");
}

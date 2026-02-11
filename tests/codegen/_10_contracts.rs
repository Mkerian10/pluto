// Category 10: Contracts Tests (20+ tests)
// Comprehensive test suite for runtime contract checking codegen.
// Tests validate correct invariant, requires, and ensures enforcement.

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
// 4. Ensures - Exit Checks (5 tests)
// ============================================================================

#[test]
fn test_ensures_checked_on_exit() {
    // Ensures clause checked before function returns
    let src = r#"
        fn always_positive(x: int) int
            ensures result > 0
        {
            return x * x + 1
        }

        fn main() int {
            return always_positive(-5)
        }
    "#;
    assert_eq!(compile_and_run(src), 26);
}

#[test]
fn test_ensures_violation_aborts() {
    // Ensures violation should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn always_positive(x: int) int
            ensures result > 0
        {
            return x
        }

        fn main() {
            print(always_positive(-5))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures violation"), "stderr: {stderr}");
    assert!(stderr.contains("always_positive"), "stderr: {stderr}");
    assert!(stderr.contains("result > 0"), "stderr: {stderr}");
}

#[test]
fn test_ensures_result_in_expression() {
    // Ensures can reference 'result' in complex expressions
    let src = r#"
        fn double(x: int) int
            ensures result == x * 2
        {
            return x * 2
        }

        fn main() int {
            return double(7)
        }
    "#;
    assert_eq!(compile_and_run(src), 14);
}

#[test]
fn test_ensures_on_void_function() {
    // Ensures on void function (no result binding)
    let src = r#"
        class Counter {
            count: int

            fn increment(mut self)
                ensures self.count > 0
            {
                self.count = self.count + 1
            }
        }

        fn main() int {
            let mut c = Counter { count: 0 }
            c.increment()
            return c.count
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn test_ensures_multiple_clauses() {
    // Multiple ensures clauses
    let src = r#"
        fn in_range(x: int) int
            ensures result >= 0
            ensures result <= 100
        {
            if x < 0 {
                return 0
            }
            if x > 100 {
                return 100
            }
            return x
        }

        fn main() int {
            let a = in_range(-10)
            let b = in_range(50)
            let c = in_range(200)
            return a + b + c
        }
    "#;
    assert_eq!(compile_and_run(src), 150); // 0 + 50 + 100
}

// ============================================================================
// 5. old() Snapshots (5 tests)
// ============================================================================

#[test]
fn test_old_snapshot_single_field() {
    // old() captures field value at function entry
    let src = r#"
        class Counter {
            count: int

            fn increment(mut self)
                ensures self.count == old(self.count) + 1
            {
                self.count = self.count + 1
            }
        }

        fn main() int {
            let mut c = Counter { count: 0 }
            c.increment()
            return c.count
        }
    "#;
    assert_eq!(compile_and_run(src), 1);
}

#[test]
fn test_old_snapshot_violation() {
    // old() snapshot mismatch should abort
    let (_, stderr, code) = compile_and_run_output(
        r#"
        class Counter {
            count: int

            fn increment(self)
                ensures self.count == old(self.count) + 1
            {
                // Bug: forgot to increment
            }
        }

        fn main() {
            let c = Counter { count: 0 }
            c.increment()
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures violation"), "stderr: {stderr}");
}

#[test]
fn test_old_snapshot_multiple_fields() {
    // old() can snapshot multiple fields
    let src = r#"
        class Pair {
            x: int
            y: int

            fn swap(mut self)
                ensures self.x == old(self.y)
                ensures self.y == old(self.x)
            {
                let temp = self.x
                self.x = self.y
                self.y = temp
            }
        }

        fn main() {
            let mut p = Pair { x: 10, y: 20 }
            p.swap()
            print(p.x)
            print(p.y)
        }
    "#;
    let output = compile_and_run_stdout(src);
    assert_eq!(output, "20\n10\n");
}

#[test]
fn test_old_in_arithmetic_expression() {
    // old() used in complex arithmetic
    let src = r#"
        class Account {
            balance: int

            fn deposit(mut self, amount: int)
                ensures self.balance == old(self.balance) + amount
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

#[test]
fn test_old_nested_in_logical_expression() {
    // old() inside logical operators
    let src = r#"
        class BoundedCounter {
            value: int

            fn safe_increment(mut self)
                ensures self.value >= old(self.value) && self.value <= 100
            {
                if self.value < 100 {
                    self.value = self.value + 1
                }
            }
        }

        fn main() int {
            let mut c = BoundedCounter { value: 99 }
            c.safe_increment()
            c.safe_increment()
            return c.value
        }
    "#;
    assert_eq!(compile_and_run(src), 100);
}

// ============================================================================
// 6. Combined Contracts (5 tests)
// ============================================================================

#[test]
fn test_requires_and_ensures_together() {
    // Function with both requires and ensures
    let src = r#"
        fn safe_divide(a: int, b: int) int
            requires b != 0
            ensures result * b <= a
        {
            return a / b
        }

        fn main() int {
            return safe_divide(10, 3)
        }
    "#;
    assert_eq!(compile_and_run(src), 3);
}

#[test]
fn test_invariant_requires_ensures_together() {
    // Class with invariant, method with requires and ensures
    let src = r#"
        class PositiveCounter {
            value: int

            invariant self.value >= 0

            fn add(mut self, amount: int) int
                requires amount > 0
                ensures result == old(self.value) + amount
            {
                self.value = self.value + amount
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
fn test_all_contracts_satisfied_complex() {
    // Complex scenario with all contract types
    let src = r#"
        class Range {
            lo: int
            hi: int

            invariant self.lo >= 0
            invariant self.hi > self.lo

            fn expand(mut self, amount: int)
                requires amount > 0
                ensures self.hi - self.lo == old(self.hi - self.lo) + amount
            {
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
fn test_requires_violation_prevents_ensures_check() {
    // When requires fails, ensures should not be checked
    let (_, stderr, code) = compile_and_run_output(
        r#"
        fn foo(x: int) int
            requires x > 0
            ensures result > 0
        {
            return x
        }

        fn main() {
            print(foo(-5))
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(!stderr.contains("ensures violation"), "stderr: {stderr}");
}

#[test]
fn test_invariant_violation_before_method_ensures() {
    // If a method's mut operations violate invariant, abort before ensures
    let (_, stderr, code) = compile_and_run_output(
        r#"
        class BoundedValue {
            value: int

            invariant self.value >= 0
            invariant self.value <= 100

            fn set(mut self, v: int)
                ensures self.value == v
            {
                self.value = v
            }
        }

        fn main() {
            let mut b = BoundedValue { value: 50 }
            b.set(200)
        }
        "#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"), "stderr: {stderr}");
}

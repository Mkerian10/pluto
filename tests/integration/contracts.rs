mod common;
use common::{
    compile_and_run_output, compile_and_run_stdout, compile_should_fail_with,
};

// ── Parsing success: class invariants compile and run ────────────────────────

#[test]
fn invariant_single_field_check() {
    let out = compile_and_run_stdout(
        r#"
class Positive {
    value: int

    invariant self.value > 0
}

fn main() {
    let p = Positive { value: 5 }
    print(p.value)
}
"#,
    );
    assert_eq!(out, "5\n");
}

#[test]
fn invariant_multiple_invariants() {
    let out = compile_and_run_stdout(
        r#"
class BoundedInt {
    value: int

    invariant self.value >= 0
    invariant self.value <= 100
}

fn main() {
    let b = BoundedInt { value: 50 }
    print(b.value)
}
"#,
    );
    assert_eq!(out, "50\n");
}

#[test]
fn invariant_with_arithmetic() {
    let out = compile_and_run_stdout(
        r#"
class Pair {
    x: int
    y: int

    invariant self.x + self.y > 0
}

fn main() {
    let p = Pair { x: 3, y: 4 }
    print(p.x + p.y)
}
"#,
    );
    assert_eq!(out, "7\n");
}

#[test]
fn invariant_with_logical_ops() {
    let out = compile_and_run_stdout(
        r#"
class Rect {
    width: int
    height: int

    invariant self.width > 0 && self.height > 0
}

fn main() {
    let r = Rect { width: 10, height: 20 }
    print(r.width)
    print(r.height)
}
"#,
    );
    assert_eq!(out, "10\n20\n");
}

#[test]
fn invariant_with_len() {
    let out = compile_and_run_stdout(
        r#"
class NonEmptyList {
    items: [int]

    invariant self.items.len() > 0
}

fn main() {
    let list = NonEmptyList { items: [1, 2, 3] }
    print(list.items.len())
}
"#,
    );
    assert_eq!(out, "3\n");
}

#[test]
fn invariant_preserved_by_method() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    value: int

    invariant self.value >= 0

    fn increment(self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

fn main() {
    let c = Counter { value: 0 }
    c.increment()
    c.increment()
    c.increment()
    print(c.get())
}
"#,
    );
    assert_eq!(out, "3\n");
}

#[test]
fn invariant_class_with_no_methods() {
    let out = compile_and_run_stdout(
        r#"
class Valid {
    x: int

    invariant self.x != 0
}

fn main() {
    let v = Valid { x: 42 }
    print(v.x)
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn invariant_float_comparison() {
    let out = compile_and_run_stdout(
        r#"
class Temperature {
    celsius: float

    invariant self.celsius >= -273.15
}

fn main() {
    let t = Temperature { celsius: 20.0 }
    print(t.celsius)
}
"#,
    );
    assert!(out.starts_with("20"), "expected output starting with 20, got: {out}");
}

// ── Requires/ensures parse (not enforced) ────────────────────────────────────

#[test]
fn requires_parses_without_error() {
    let out = compile_and_run_stdout(
        r#"
fn positive_add(a: int, b: int) int
    requires a > 0
    requires b > 0
{
    return a + b
}

fn main() {
    print(positive_add(3, 4))
}
"#,
    );
    assert_eq!(out, "7\n");
}

#[test]
fn ensures_parses_without_error() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int
    ensures x > 0
{
    return x * 2
}

fn main() {
    print(double(5))
}
"#,
    );
    assert_eq!(out, "10\n");
}

#[test]
fn method_requires_parses() {
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: float

    fn withdraw(self, amount: float) float
        requires amount > 0.0
    {
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let a = Account { balance: 100.0 }
    print(a.withdraw(30.0))
}
"#,
    );
    assert!(out.starts_with("70"), "expected output starting with 70, got: {out}");
}

// ── Decidable fragment rejection ─────────────────────────────────────────────

#[test]
fn invariant_rejects_function_call() {
    compile_should_fail_with(
        r#"
fn helper() bool {
    return true
}

class Bad {
    x: int

    invariant helper()
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "not allowed in contract expressions",
    );
}

#[test]
fn invariant_rejects_string_literal() {
    compile_should_fail_with(
        r#"
class Bad {
    x: int

    invariant "hello"
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "string literals are not allowed in contract expressions",
    );
}

#[test]
fn invariant_rejects_cast() {
    compile_should_fail_with(
        r#"
class Bad {
    x: int

    invariant self.x as bool
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "type casts are not allowed in contract expressions",
    );
}

#[test]
fn invariant_rejects_non_len_method_call() {
    compile_should_fail_with(
        r#"
class Bad {
    name: string

    invariant self.name.contains("x")
}

fn main() {
    let b = Bad { name: "hello" }
}
"#,
        "method call '.contains()' is not allowed in contract expressions",
    );
}

#[test]
fn invariant_rejects_array_literal() {
    compile_should_fail_with(
        r#"
class Bad {
    x: int

    invariant [1, 2, 3]
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "array literals are not allowed in contract expressions",
    );
}

#[test]
fn invariant_rejects_index_expression() {
    compile_should_fail_with(
        r#"
class Bad {
    items: [int]

    invariant self.items[0]
}

fn main() {
    let b = Bad { items: [1] }
}
"#,
        "index expressions are not allowed in contract expressions",
    );
}

#[test]
fn requires_rejects_function_call() {
    compile_should_fail_with(
        r#"
fn is_valid(x: int) bool {
    return x > 0
}

fn foo(x: int) int
    requires is_valid(x)
{
    return x
}

fn main() {
    print(foo(1))
}
"#,
        "not allowed in contract expressions",
    );
}

// ── Type validation ──────────────────────────────────────────────────────────

#[test]
fn invariant_non_bool_rejected() {
    compile_should_fail_with(
        r#"
class Bad {
    x: int

    invariant self.x + 1
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "invariant expression must be bool",
    );
}

#[test]
fn invariant_nonexistent_field_rejected() {
    compile_should_fail_with(
        r#"
class Bad {
    x: int

    invariant self.y > 0
}

fn main() {
    let b = Bad { x: 1 }
}
"#,
        "y",
    );
}

// ── Runtime enforcement ──────────────────────────────────────────────────────

#[test]
fn invariant_violation_at_construction() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class Positive {
    value: int

    invariant self.value > 0
}

fn main() {
    let p = Positive { value: 0 }
    print(p.value)
}
"#,
    );
    assert_ne!(code, 0, "Should have exited with non-zero for invariant violation");
    assert!(
        stderr.contains("invariant violation"),
        "stderr should contain 'invariant violation', got: {stderr}"
    );
    assert!(
        stderr.contains("Positive"),
        "stderr should mention the class name, got: {stderr}"
    );
}

#[test]
fn invariant_violation_at_construction_negative() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class BoundedInt {
    value: int

    invariant self.value >= 0
    invariant self.value <= 100
}

fn main() {
    let b = BoundedInt { value: 150 }
    print(b.value)
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"));
}

#[test]
fn invariant_violation_after_method_call() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class Counter {
    value: int

    invariant self.value >= 0

    fn decrement(self) {
        self.value = self.value - 1
    }
}

fn main() {
    let c = Counter { value: 0 }
    c.decrement()
    print(c.value)
}
"#,
    );
    assert_ne!(code, 0, "Should have exited with non-zero for invariant violation after method");
    assert!(
        stderr.contains("invariant violation"),
        "stderr should contain 'invariant violation', got: {stderr}"
    );
}

#[test]
fn invariant_multiple_one_violated() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class Range {
    lo: int
    hi: int

    invariant self.lo >= 0
    invariant self.hi > self.lo
}

fn main() {
    let r = Range { lo: 5, hi: 3 }
    print(r.lo)
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"));
}

#[test]
fn invariant_method_preserves_multiple() {
    let out = compile_and_run_stdout(
        r#"
class Range {
    lo: int
    hi: int

    invariant self.lo >= 0
    invariant self.hi > self.lo

    fn widen(self, amount: int) {
        self.hi = self.hi + amount
    }

    fn get_hi(self) int {
        return self.hi
    }
}

fn main() {
    let r = Range { lo: 0, hi: 10 }
    r.widen(5)
    print(r.get_hi())
}
"#,
    );
    assert_eq!(out, "15\n");
}

// ── Edge cases ───────────────────────────────────────────────────────────────

#[test]
fn invariant_with_bool_field() {
    let out = compile_and_run_stdout(
        r#"
class Active {
    enabled: bool

    invariant self.enabled
}

fn main() {
    let a = Active { enabled: true }
    print(a.enabled)
}
"#,
    );
    assert_eq!(out, "true\n");
}

#[test]
fn invariant_bool_field_violation() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class Active {
    enabled: bool

    invariant self.enabled
}

fn main() {
    let a = Active { enabled: false }
    print(a.enabled)
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("invariant violation"));
}

#[test]
fn invariant_with_negation() {
    let out = compile_and_run_stdout(
        r#"
class NonZero {
    value: int

    invariant !(self.value == 0)
}

fn main() {
    let n = NonZero { value: 42 }
    print(n.value)
}
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn invariant_or_condition() {
    let out = compile_and_run_stdout(
        r#"
class FlexRange {
    lo: int
    hi: int

    invariant self.lo == 0 || self.hi > 0
}

fn main() {
    let r = FlexRange { lo: 0, hi: -5 }
    print(r.lo)
}
"#,
    );
    assert_eq!(out, "0\n");
}

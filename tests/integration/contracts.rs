mod common;
use common::{
    compile_and_run_output, compile_and_run_stdout, compile_should_fail, compile_should_fail_with,
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

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

fn main() {
    let mut c = Counter { value: 0 }
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

    fn withdraw(mut self, amount: float) float
        requires amount > 0.0
    {
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut a = Account { balance: 100.0 }
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

    fn decrement(mut self) {
        self.value = self.value - 1
    }
}

fn main() {
    let mut c = Counter { value: 0 }
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

    fn widen(mut self, amount: int) {
        self.hi = self.hi + amount
    }

    fn get_hi(self) int {
        return self.hi
    }
}

fn main() {
    let mut r = Range { lo: 0, hi: 10 }
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

// ── Phase 2: requires runtime enforcement ──────────────────────────────────

#[test]
fn requires_satisfied_runs_ok() {
    let out = compile_and_run_stdout(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn main() {
    print(positive(5))
}
"#,
    );
    assert_eq!(out, "10\n");
}

#[test]
fn requires_violated_aborts() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn main() {
    print(positive(-1))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("positive"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn requires_multiple_one_violated() {
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
fn requires_on_class_method() {
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int

    fn deposit(mut self, amount: int)
        requires amount > 0
    {
        self.balance = self.balance + amount
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    a.deposit(50)
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "150\n");
}

#[test]
fn requires_on_method_violated() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
class Account {
    balance: int

    fn deposit(mut self, amount: int)
        requires amount > 0
    {
        self.balance = self.balance + amount
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    a.deposit(-10)
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("amount > 0"), "stderr: {stderr}");
}

#[test]
fn requires_with_arithmetic() {
    let out = compile_and_run_stdout(
        r#"
fn in_range(x: int) int
    requires x > 0 && x < 100
{
    return x
}

fn main() {
    print(in_range(50))
}
"#,
    );
    assert_eq!(out, "50\n");
}

// ── Phase 2: ensures runtime enforcement ──────────────────────────────────

#[test]
fn ensures_satisfied_runs_ok() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int
    ensures result > 0
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
fn ensures_violated_aborts() {
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
fn ensures_result_equals_expression() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int
    ensures result == x * 2
{
    return x * 2
}

fn main() {
    print(double(7))
}
"#,
    );
    assert_eq!(out, "14\n");
}

#[test]
fn ensures_result_violated() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn double(x: int) int
    ensures result == x * 2
{
    return x + 1
}

fn main() {
    print(double(5))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures violation"), "stderr: {stderr}");
}

#[test]
fn ensures_on_void_function() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn increment(mut self)
        ensures self.count > 0
    {
        self.count = self.count + 1
    }
}

fn main() {
    let mut c = Counter { count: 0 }
    c.increment()
    print(c.count)
}
"#,
    );
    assert_eq!(out, "1\n");
}

// ── Phase 2: old() ──────────────────────────────────────────────────────

#[test]
fn ensures_old_satisfied() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    count: int

    fn increment(mut self)
        ensures self.count == old(self.count) + 1
    {
        self.count = self.count + 1
    }
}

fn main() {
    let mut c = Counter { count: 0 }
    c.increment()
    print(c.count)
}
"#,
    );
    assert_eq!(out, "1\n");
}

#[test]
fn ensures_old_violated() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
class Counter {
    count: int

    fn increment(self)
        ensures self.count == old(self.count) + 1
    {
        // Bug: forgot to increment!
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
fn ensures_old_nested_field() {
    let out = compile_and_run_stdout(
        r#"
class Wallet {
    balance: int

    fn withdraw(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) - amount
    {
        self.balance = self.balance - amount
    }
}

fn main() {
    let mut w = Wallet { balance: 100 }
    w.withdraw(30)
    print(w.balance)
}
"#,
    );
    assert_eq!(out, "70\n");
}

#[test]
fn old_in_requires_rejected() {
    compile_should_fail_with(
        r#"
fn foo(x: int) int
    requires old(x) > 0
{
    return x
}

fn main() {
    print(foo(5))
}
"#,
        "old() is only allowed in ensures clauses",
    );
}

// ── Phase 2: type checking ──────────────────────────────────────────────

#[test]
fn requires_non_bool_rejected() {
    compile_should_fail_with(
        r#"
fn foo(x: int) int
    requires x + 1
{
    return x
}

fn main() {
    print(foo(5))
}
"#,
        "requires expression must be bool",
    );
}

#[test]
fn ensures_non_bool_rejected() {
    compile_should_fail_with(
        r#"
fn foo(x: int) int
    ensures result + 1
{
    return x
}

fn main() {
    print(foo(5))
}
"#,
        "ensures expression must be bool",
    );
}

#[test]
fn result_in_requires_rejected() {
    compile_should_fail(
        r#"
fn foo(x: int) int
    requires result > 0
{
    return x
}

fn main() {
    print(foo(5))
}
"#,
    );
}

#[test]
fn result_type_in_ensures() {
    let out = compile_and_run_stdout(
        r#"
fn negate(x: bool) bool
    ensures result != x
{
    if x {
        return false
    }
    return true
}

fn main() {
    print(negate(true))
    print(negate(false))
}
"#,
    );
    assert_eq!(out, "false\ntrue\n");
}

// ── Phase 2: edge cases ──────────────────────────────────────────────────

#[test]
fn requires_and_ensures_together() {
    let out = compile_and_run_stdout(
        r#"
fn safe_div(a: int, b: int) int
    requires b != 0
    ensures result * b == a
{
    return a / b
}

fn main() {
    print(safe_div(10, 2))
}
"#,
    );
    assert_eq!(out, "5\n");
}

#[test]
fn method_with_requires_ensures_and_invariant() {
    let out = compile_and_run_stdout(
        r#"
class BoundedCounter {
    count: int

    invariant self.count >= 0

    fn add(mut self, n: int)
        requires n > 0
        ensures self.count == old(self.count) + n
    {
        self.count = self.count + n
    }
}

fn main() {
    let mut c = BoundedCounter { count: 0 }
    c.add(5)
    c.add(3)
    print(c.count)
}
"#,
    );
    assert_eq!(out, "8\n");
}

#[test]
fn old_and_result_in_same_ensures() {
    let out = compile_and_run_stdout(
        r#"
class Stack {
    size: int

    fn push(mut self) int
        ensures result == old(self.size)
        ensures self.size == old(self.size) + 1
    {
        let old_size = self.size
        self.size = self.size + 1
        return old_size
    }
}

fn main() {
    let mut s = Stack { size: 0 }
    let idx = s.push()
    print(idx)
    print(s.size)
}
"#,
    );
    assert_eq!(out, "0\n1\n");
}

#[test]
fn requires_on_multiple_params() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn add_positive(a: int, b: int) int
    requires a > 0
    requires b > 0
{
    return a + b
}

fn main() {
    print(add_positive(5, -1))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("b > 0"), "stderr: {stderr}");
}

#[test]
fn ensures_multiple_satisfied() {
    let out = compile_and_run_stdout(
        r#"
fn clamp(x: int) int
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

fn main() {
    print(clamp(-5))
    print(clamp(50))
    print(clamp(200))
}
"#,
    );
    assert_eq!(out, "0\n50\n100\n");
}

#[test]
fn ensures_multiple_one_violated() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn clamp(x: int) int
    ensures result >= 0
    ensures result <= 100
{
    return x
}

fn main() {
    print(clamp(200))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures violation"), "stderr: {stderr}");
    assert!(stderr.contains("result <= 100"), "stderr: {stderr}");
}

// ── Phase 3: Interface Guarantees (Trait Method Contracts) ──────────────────

#[test]
fn trait_requires_satisfied_on_impl() {
    let out = compile_and_run_stdout(
        r#"
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
}

class PositiveValidator impl Validator {
    id: int

    fn validate(self, x: int) int {
        return x * 2
    }
}

fn main() {
    let v = PositiveValidator { id: 1 }
    print(v.validate(5))
}
"#,
    );
    assert_eq!(out, "10\n");
}

#[test]
fn trait_requires_violated_on_impl() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
}

class PositiveValidator impl Validator {
    id: int

    fn validate(self, x: int) int {
        return x * 2
    }
}

fn main() {
    let v = PositiveValidator { id: 1 }
    print(v.validate(-3))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn trait_ensures_satisfied_on_impl() {
    let out = compile_and_run_stdout(
        r#"
trait Doubler {
    fn double(self, x: int) int
        ensures result > 0
}

class MyDoubler impl Doubler {
    id: int

    fn double(self, x: int) int {
        return x * 2
    }
}

fn main() {
    let d = MyDoubler { id: 1 }
    print(d.double(5))
}
"#,
    );
    assert_eq!(out, "10\n");
}

#[test]
fn trait_ensures_violated_on_impl() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Doubler {
    fn double(self, x: int) int
        ensures result > 0
}

class MyDoubler impl Doubler {
    id: int

    fn double(self, x: int) int {
        return x * 2
    }
}

fn main() {
    let d = MyDoubler { id: 1 }
    print(d.double(-5))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("ensures violation"), "stderr: {stderr}");
    assert!(stderr.contains("result > 0"), "stderr: {stderr}");
}

#[test]
fn liskov_class_cannot_add_requires() {
    compile_should_fail_with(
        r#"
trait Processor {
    fn process(self, x: int) int
        requires x > 0
}

class MyProcessor impl Processor {
    id: int

    fn process(self, x: int) int
        requires x > 10
    {
        return x
    }
}

fn main() {
    let p = MyProcessor { id: 1 }
    print(p.process(5))
}
"#,
        "Liskov Substitution Principle",
    );
}

#[test]
fn liskov_class_cannot_add_requires_even_when_trait_has_no_contracts() {
    compile_should_fail_with(
        r#"
trait Processor {
    fn process(self, x: int) int
}

class MyProcessor impl Processor {
    id: int

    fn process(self, x: int) int
        requires x > 0
    {
        return x
    }
}

fn main() {
    let p = MyProcessor { id: 1 }
    print(p.process(5))
}
"#,
        "Liskov Substitution Principle",
    );
}

#[test]
fn liskov_class_can_add_ensures() {
    let out = compile_and_run_stdout(
        r#"
trait Processor {
    fn process(self, x: int) int
        requires x > 0
}

class MyProcessor impl Processor {
    id: int

    fn process(self, x: int) int
        ensures result > 0
    {
        return x * 2
    }
}

fn main() {
    let p = MyProcessor { id: 1 }
    print(p.process(5))
}
"#,
    );
    assert_eq!(out, "10\n");
}

#[test]
fn trait_contract_via_dynamic_dispatch() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
}

class SimpleValidator impl Validator {
    id: int

    fn validate(self, x: int) int {
        return x
    }
}

fn run_validation(v: Validator, x: int) int {
    return v.validate(x)
}

fn main() {
    let v = SimpleValidator { id: 1 }
    print(run_validation(v, -5))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn trait_contract_non_bool_rejected() {
    compile_should_fail_with(
        r#"
trait Bad {
    fn compute(self, x: int) int
        requires x + 1
}

class Impl impl Bad {
    id: int

    fn compute(self, x: int) int {
        return x
    }
}

fn main() {
    let b = Impl { id: 1 }
    print(b.compute(5))
}
"#,
        "requires expression must be bool",
    );
}

#[test]
fn trait_contract_self_field_rejected() {
    compile_should_fail(
        r#"
trait Bad {
    fn check(self) bool
        requires self.value > 0
}

class Impl impl Bad {
    value: int

    fn check(self) bool {
        return true
    }
}

fn main() {
    let b = Impl { value: 5 }
    print(b.check())
}
"#,
    );
}

#[test]
fn multi_trait_same_method_with_contracts_rejected() {
    compile_should_fail_with(
        r#"
trait A {
    fn do_thing(self, x: int) int
        requires x > 0
}

trait B {
    fn do_thing(self, x: int) int
        requires x > 10
}

class MyClass impl A, B {
    id: int

    fn do_thing(self, x: int) int {
        return x
    }
}

fn main() {
    let c = MyClass { id: 1 }
    print(c.do_thing(5))
}
"#,
        "both define method",
    );
}

#[test]
fn trait_default_method_contracts_inherited() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Clamper {
    fn clamp(self, x: int) int
        requires x >= 0
    {
        if x > 100 {
            return 100
        }
        return x
    }
}

class MyClamper impl Clamper {
    id: int
}

fn main() {
    let c = MyClamper { id: 1 }
    print(c.clamp(-1))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x >= 0"), "stderr: {stderr}");
}

#[test]
fn trait_overridden_default_method_still_has_trait_contracts() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Clamper {
    fn clamp(self, x: int) int
        requires x >= 0
    {
        if x > 100 {
            return 100
        }
        return x
    }
}

class MyClamper impl Clamper {
    id: int

    fn clamp(self, x: int) int {
        return x * 2
    }
}

fn main() {
    let c = MyClamper { id: 1 }
    print(c.clamp(-1))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x >= 0"), "stderr: {stderr}");
}

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
fn invariant_with_len_rejected() {
    // Collection facts are outside the provable fragment — rejected at
    // declaration under static discharge.
    compile_should_fail_with(
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
        "outside the provable fragment",
    );
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
fn invariant_float_comparison_rejected() {
    // Float comparisons are outside the provable fragment.
    compile_should_fail_with(
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
        "outside the provable fragment",
    );
}

// ── Requires parse ───────────────────────────────────────────────────────────

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

// ── Static discharge: violations are compile errors ─────────────────────────

#[test]
fn invariant_violation_at_construction() {
    // A construction that refutes the invariant never compiles.
    compile_should_fail_with(
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
        "construction of 'Positive' violates its invariant",
    );
}

#[test]
fn invariant_violation_at_construction_negative() {
    compile_should_fail_with(
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
        "construction of 'BoundedInt' violates its invariant",
    );
}

#[test]
fn invariant_unpreserved_method_rejected() {
    // A method that cannot be proven to preserve the invariant is a
    // compile error at the method — for every input, not just the one in
    // main (the old runtime test aborted on c.decrement() from 0).
    compile_should_fail_with(
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
        "cannot prove invariant 'self.value >= 0' of class 'Counter'",
    );
}

#[test]
fn invariant_multiple_one_violated() {
    compile_should_fail_with(
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
        "violates its invariant 'self.hi > self.lo'",
    );
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

    fn widen(mut self, amount: int)
        requires amount >= 0
    {
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
fn invariant_with_bool_field_rejected() {
    // Boolean invariants are outside the provable fragment.
    compile_should_fail_with(
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
        "outside the provable fragment",
    );
}

#[test]
fn invariant_on_generic_class_rejected() {
    // Generic bodies are checked against opaque type parameters and their
    // instantiations are never re-checked, so invariants on generic
    // classes are rejected for now.
    compile_should_fail_with(
        r#"
class Box<T> {
    value: T
    count: int

    invariant self.count >= 0
}

fn main() {
    let b = Box<int> { value: 1, count: 0 }
    print(b.count)
}
"#,
        "invariants on generic classes are not yet supported",
    );
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
fn result_in_requires_rejected() {
    compile_should_fail_with(
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
        "undefined variable 'result'",
    );
}

// ── Phase 2: edge cases ──────────────────────────────────────────────────

#[test]
fn requires_with_multiple_conditions() {
    let out = compile_and_run_stdout(
        r#"
fn safe_div(a: int, b: int) int
    requires b != 0
    requires a >= 0
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
fn method_with_requires_and_invariant() {
    let out = compile_and_run_stdout(
        r#"
class BoundedCounter {
    count: int

    invariant self.count >= 0

    fn add(mut self, n: int)
        requires n > 0
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
#[ignore] // Compiler bug: QualifiedAccess panic in contracts validation (self.field in trait requires)
fn trait_contract_self_field_rejected() {
    compile_should_fail_with(
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
        "field access on non-class type",
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

// ── Self mutation checks (PR 1.4) ────────────────────────────────────

#[test]
fn self_array_index_assign_rejected_in_non_mut_method() {
    // Tests that IndexAssign on self's data is caught in non-mut methods
    // Bug: check_stmt_for_self_mutation had no IndexAssign case
    compile_should_fail_with(
        r#"
class Counter {
    values: [int]

    fn increment(self, index: int) {
        self.values[index] = self.values[index] + 1
    }
}

fn main() {
    let c = Counter { values: [1, 2, 3] }
    c.increment(0)
}
"#,
        "cannot mutate self's data in a non-mut method",
    );
}

#[test]
fn self_array_index_assign_allowed_in_mut_method() {
    // Verify that IndexAssign on self's data IS allowed in mut methods
    let out = compile_and_run_stdout(
        r#"
class Counter {
    values: [int]

    fn increment(mut self, index: int) {
        self.values[index] = self.values[index] + 1
    }
}

fn main() {
    let mut c = Counter { values: [1, 2, 3] }
    c.increment(0)
    print(c.values[0])
}
"#,
    );
    assert_eq!(out, "2\n");
}

// ── Phase 4: assert statement ───────────────────────────────────────────────

#[test]
fn assert_true_runs_ok() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    assert 1 > 0
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_false_aborts() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn main() {
    assert 1 < 0
    print("should not reach")
}
"#,
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("assertion failed"),
        "stderr should contain 'assertion failed', got: {stderr}"
    );
}

#[test]
fn assert_with_variable() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let x = 5
    assert x > 0
    print(x)
}
"#,
    );
    assert_eq!(out, "5\n");
}

#[test]
fn assert_non_bool_rejected() {
    compile_should_fail_with(
        r#"
fn main() {
    assert 42
}
"#,
        "assert expression must be bool",
    );
}

#[test]
fn assert_with_function_call() {
    let out = compile_and_run_stdout(
        r#"
fn is_positive(x: int) bool {
    return x > 0
}

fn main() {
    assert is_positive(5)
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_in_function() {
    let out = compile_and_run_stdout(
        r#"
fn check_positive(x: int) {
    assert x > 0
}

fn main() {
    check_positive(10)
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_with_comparison() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a = 10
    let b = 5
    assert a >= b
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_complex_expression() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let x = 50
    let y = 20
    assert (x > 0) && (y < 100)
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_with_field_access() {
    let out = compile_and_run_stdout(
        r#"
class Config {
    max_retries: int
}

fn main() {
    let c = Config { max_retries: 3 }
    assert c.max_retries > 0
    print("ok")
}
"#,
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn assert_failure_shows_expression() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn main() {
    let x = -1
    assert x > 0
}
"#,
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("assertion failed"),
        "stderr should contain 'assertion failed', got: {stderr}"
    );
    assert!(
        stderr.contains("x > 0"),
        "stderr should contain the expression 'x > 0', got: {stderr}"
    );
}

// ============================================================
// If-Expression Integration Tests
// ============================================================
// Note: If-expressions are not allowed in contract clauses per the decidable
// fragment restriction. Contract expressions are limited to comparisons, arithmetic,
// logical ops, .len(), field access, and literals. No function calls, indexing,
// closures, casts, or if-expressions.

// ── Static invariant discharge (verification RFC phase 2) ───────────────────
// Invariants are compile-time proof obligations, strict mode: every
// construction and write site must be statically proven to preserve the
// invariant, or compilation fails. See src/typeck/discharge.rs.

#[test]
fn discharge_guarded_write_proves() {
    // The classic pattern: a guard whose failure path raises establishes
    // the fact the write needs.
    let out = compile_and_run_stdout(
        r#"
error Insufficient { msg: string }

class Account {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amt: int)
        requires amt > 0
    {
        if amt > self.balance {
            raise Insufficient { msg: "insufficient" }
        }
        self.balance = self.balance - amt
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    a.withdraw(30) catch e {
        print("caught")
    }
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "70\n");
}

#[test]
fn discharge_requires_clause_proves() {
    // requires clauses are entry facts for the proof.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amt: int) int
        requires amt > 0
        requires self.balance >= amt
    {
        self.balance = self.balance - amt
        return self.balance
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    print(a.withdraw(30))
}
"#,
    );
    assert_eq!(out, "70\n");
}

#[test]
fn discharge_unguarded_write_rejected_with_guidance() {
    // The diagnostic names the invariant, the site, the symbolic state,
    // and teaches the fix.
    compile_should_fail_with(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn drain(mut self, amt: int) {
        self.balance = self.balance - amt
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    a.drain(30)
}
"#,
        "guard, a 'requires' clause, or an 'assert'",
    );
}

#[test]
fn discharge_construction_from_guard_proves() {
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn make(x: int) {
    if x >= 0 {
        let a = Account { balance: x }
        print(a.balance)
    } else {
        print("rejected")
    }
}

fn main() {
    make(5)
    make(0 - 3)
}
"#,
    );
    assert_eq!(out, "5\nrejected\n");
}

#[test]
fn discharge_construction_unknown_rejected() {
    // An initializer the prover cannot bound is a compile error, not a
    // runtime check.
    compile_should_fail_with(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn make(x: int) Account {
    return Account { balance: x }
}

fn main() {
    let a = make(5)
    print(a.balance)
}
"#,
        "for this construction",
    );
}

#[test]
fn discharge_subtract_then_add_proves_at_exit() {
    // Temporary violation between writes is fine — only boundaries matter.
    // The symbolic forms cancel: balance ends exactly where it started.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn adjust(mut self, amt: int) {
        self.balance = self.balance - amt
        self.balance = self.balance + amt
    }
}

fn main() {
    let mut a = Account { balance: 5 }
    a.adjust(100)
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "5\n");
}

#[test]
fn discharge_call_while_broken_rejected() {
    // Reentrancy conservatism: any call while the invariant may be broken
    // is rejected — the callee (or an alias holder) could observe the
    // object mid-violation.
    compile_should_fail_with(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn log(self) {
        print(self.balance)
    }

    fn weird(mut self, amt: int)
        requires amt >= 0
    {
        self.balance = self.balance - amt
        self.log()
        self.balance = self.balance + amt
    }
}

fn main() {
    let mut a = Account { balance: 5 }
    a.weird(10)
}
"#,
        "the callee may observe the object",
    );
}

#[test]
fn discharge_foreign_write_with_requires_proves() {
    // Writes outside the class's own methods are proven immediately,
    // using the caller's requires facts and parameter invariants.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn transfer(mut from: Account, mut to: Account, amount: int)
    requires from.balance >= amount
    requires amount > 0
{
    from.balance = from.balance - amount
    to.balance = to.balance + amount
}

fn main() {
    let mut a = Account { balance: 100 }
    let mut b = Account { balance: 0 }
    transfer(a, b, 40)
    print(a.balance)
    print(b.balance)
}
"#,
    );
    assert_eq!(out, "60\n40\n");
}

#[test]
fn discharge_foreign_violating_write_rejected() {
    compile_should_fail_with(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn main() {
    let mut a = Account { balance: 100 }
    a.balance = 0 - 1
}
"#,
        "violates invariant 'self.balance >= 0'",
    );
}

#[test]
fn discharge_foreign_unprovable_write_rejected() {
    compile_should_fail_with(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn set_balance(mut a: Account, v: int) {
    a.balance = v
}

fn main() {
    let mut a = Account { balance: 100 }
    set_balance(a, 5)
}
"#,
        "after this write",
    );
}

#[test]
fn discharge_loop_body_write_proves() {
    let out = compile_and_run_stdout(
        r#"
class Balance {
    amount: int
    invariant self.amount >= 0

    fn do_deposits(mut self) {
        let mut i = 0
        while i < 100 {
            self.amount = self.amount + 1
            i = i + 1
        }
    }
}

fn main() {
    let mut b = Balance { amount: 0 }
    b.do_deposits()
    print(b.amount)
}
"#,
    );
    assert_eq!(out, "100\n");
}

#[test]
fn discharge_branch_guarded_write_proves() {
    // A write inside a guarded branch proves at the branch end using the
    // branch's facts.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amt: int) {
        if amt >= 0 && amt <= self.balance {
            self.balance = self.balance - amt
        }
    }
}

fn main() {
    let mut a = Account { balance: 50 }
    a.withdraw(20)
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "30\n");
}

#[test]
fn discharge_nonzero_invariant_with_guard_proves() {
    // The != fragment extension: a `v != 0` guard proves `self.x != 0`.
    let out = compile_and_run_stdout(
        r#"
class NonZero {
    x: int
    invariant self.x != 0

    fn set(mut self, v: int) {
        if v != 0 {
            self.x = v
        }
    }
}

fn main() {
    let mut n = NonZero { x: 42 }
    n.set(7)
    print(n.x)
}
"#,
    );
    assert_eq!(out, "7\n");
}

#[test]
fn discharge_assert_establishes_fact() {
    // assert is the documented escape hatch: after it passes, the prover
    // may assume the condition.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0

    fn set(mut self, v: int) {
        assert v >= 0
        self.balance = v
    }
}

fn main() {
    let mut a = Account { balance: 1 }
    a.set(9)
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "9\n");
}

#[test]
fn discharge_parameter_invariant_assumed() {
    // A class-typed parameter satisfies its invariants at entry — enough
    // to prove an increment immediately.
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int
    invariant self.balance >= 0
}

fn bump(mut a: Account) {
    a.balance = a.balance + 1
}

fn main() {
    let mut a = Account { balance: 0 }
    bump(a)
    print(a.balance)
}
"#,
    );
    assert_eq!(out, "1\n");
}

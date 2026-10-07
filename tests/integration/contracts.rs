mod common;
use common::{
    compile_and_run_output, compile_and_run_stdout, compile_should_fail, compile_should_fail_with,
    compile_test_and_run_with_stdlib,
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
fn invariant_with_len_accepted() {
    // Collection length invariants became provable via ghost `len()`
    // (rfc-number-types.md §4). The field is read through a method (door (b)
    // rejects a raw external read of a length-covered collection field).
    let out = compile_and_run_stdout(
        r#"
class NonEmptyList {
    items: [int]

    invariant self.items.len() > 0

    fn count(self) int {
        return self.items.len()
    }
}

fn main() {
    let list = NonEmptyList { items: [1, 2, 3] }
    print(list.count())
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
fn requires_rejects_chained_comparison() {
    // Chained comparisons (#451) are not modeled by the contract prover;
    // the explicit conjunction is required.
    compile_should_fail_with(
        r#"
fn foo(x: int) int
    requires 0 <= x < 10
{
    return x
}

fn main() {
    print(foo(1))
}
"#,
        "chained comparisons are not supported in contract expressions",
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
fn invariant_on_generic_class_param_independent_accepted() {
    // Invariants on generic classes are supported when their vocabulary is
    // independent of the type parameters: validated and proven once on the
    // template, stamped onto every instantiation.
    let out = compile_and_run_stdout(
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
    );
    assert_eq!(out.trim(), "0");
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

// ── Invariants at DI construction (zero-state proof) ─────────────────────────

#[test]
fn di_construction_arithmetic_invariant_holds_at_zero() {
    let output = compile_and_run_stdout(
        "class Ledger {\n    debits: int\n    credits: int\n    invariant self.debits + self.credits >= 0\n\n    fn total(self) int {\n        return self.debits + self.credits\n    }\n}\n\napp MyApp[l: Ledger] {\n    fn main(self) {\n        print(self.l.total())\n    }\n}",
    );
    assert_eq!(output.trim(), "0");
}

#[test]
fn di_construction_arithmetic_invariant_refuted_at_zero() {
    compile_should_fail_with(
        "class Ledger {\n    debits: int\n    credits: int\n    invariant self.debits + self.credits >= 10\n}\n\napp MyApp[l: Ledger] {\n    fn main(self) {\n    }\n}",
        "class 'Ledger' is constructed by dependency injection at startup, and its invariant 'self.debits + self.credits >= 10' does not hold for the zero-initialized state",
    );
}

#[test]
fn di_scope_auto_created_constant_invariant_refuted() {
    // An auto-created scoped class has only injected (class-typed) fields, so
    // the only invariants it can declare are field-free constants — but even
    // those must hold, since no struct literal ever proves them.
    compile_should_fail_with(
        "scoped class Flag {\n    invariant 1 > 2\n}\n\nscoped class Handler[f: Flag] {\n    tag: int\n\n    fn run(self) int {\n        return self.tag\n    }\n}\n\napp MyApp {\n    fn main(self) {\n        scope(Handler { tag: 1 }) |h: Handler| {\n            print(h.run())\n        }\n    }\n}",
        "class 'Flag' is constructed by dependency injection when auto-created in this scope block",
    );
}

// ── Length terms in invariant discharge (fact-fragment extension) ────────────

#[test]
fn len_guard_discharges_invariant_write() {
    // `s.len()` resolves to a ghost length term carrying the automatic
    // `>= 0` bound, so writing it into the field is proven directly.
    let out = compile_and_run_stdout(
        r#"
class Cursor {
    pos: int

    invariant self.pos >= 0

    fn clamp(mut self, s: string) {
        if self.pos > s.len() {
            self.pos = s.len()
        }
    }
}

fn main() {
    let mut c = Cursor { pos: 9 }
    c.clamp("ab")
    print(c.pos)
}
"#,
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn len_binding_discharges_arithmetic_write() {
    // The json match_word shape: a `let n = s.len()` binding carries the
    // automatic >= 0 into the ghost vocabulary, proving pos + n >= 0 with
    // no bridging assert.
    let out = compile_and_run_stdout(
        r#"
class Cursor {
    pos: int

    invariant self.pos >= 0

    fn advance(mut self, w: string) {
        let wlen = w.len()
        self.pos = self.pos + wlen
    }
}

fn main() {
    let mut c = Cursor { pos: 1 }
    c.advance("xyz")
    print(c.pos)
}
"#,
    );
    assert_eq!(out.trim(), "4");
}

#[test]
fn len_binding_discharges_construction() {
    // The json parse shape: the binding's >= 0 fact flows into the
    // construction proof of the invariant.
    let out = compile_and_run_stdout(
        r#"
class Cursor {
    pos: int

    invariant self.pos >= 0

    fn get(self) int {
        return self.pos
    }
}

fn main() {
    let s = "abc"
    let n = s.len()
    let c = Cursor { pos: n }
    print(c.get())
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn len_minus_one_is_not_provable() {
    // >= 0 is all a length term carries: len() - 1 may be negative.
    compile_should_fail_with(
        r#"
class Cursor {
    pos: int

    invariant self.pos >= 0

    fn set_last(mut self, s: string) {
        self.pos = s.len() - 1
    }
}

fn main() {
    let mut c = Cursor { pos: 0 }
    c.set_last("abc")
    print(c.pos)
}
"#,
        "cannot prove invariant",
    );
}

#[test]
fn len_guard_bounds_subtraction() {
    // An upper-bound guard composes with the automatic lower bound:
    // len() <= 10 and len() >= 0 prove 10 - len() >= 0.
    let out = compile_and_run_stdout(
        r#"
class Budget {
    left: int

    invariant self.left >= 0

    fn consume(mut self, xs: [int]) {
        if xs.len() <= 10 {
            self.left = 10 - xs.len()
        }
    }
}

fn main() {
    let mut b = Budget { left: 10 }
    b.consume([1, 2, 3])
    print(b.left)
}
"#,
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn len_guard_goes_stale_across_call() {
    // Ghost length terms are epoch-stamped: a call boundary (the callee
    // may mutate the collection through an alias) re-anchors and the guard
    // fact no longer applies to the post-call length.
    compile_should_fail_with(
        r#"
class Budget {
    left: int

    invariant self.left >= 0

    fn touch(mut self) {
        print(self.left)
    }

    fn consume(mut self, xs: [int]) {
        if xs.len() <= 10 {
            self.touch()
            self.left = 10 - xs.len()
        }
    }
}

fn main() {
    let mut b = Budget { left: 10 }
    b.consume([1, 2, 3])
    print(b.left)
}
"#,
        "cannot prove invariant",
    );
}

#[test]
fn post_call_len_guard_uses_fresh_epoch() {
    // A guard evaluated after the call boundary speaks about the current
    // epoch's length term, so it discharges the write it dominates.
    let out = compile_and_run_stdout(
        r#"
class Budget {
    left: int

    invariant self.left >= 0

    fn touch(mut self) {
        print(self.left)
    }

    fn consume(mut self, xs: [int]) {
        self.touch()
        if xs.len() <= 10 {
            self.left = 10 - xs.len()
        }
    }
}

fn main() {
    let mut b = Budget { left: 10 }
    b.consume([1, 2, 3])
    print(b.left)
}
"#,
    );
    assert_eq!(out.trim(), "10\n7");
}

// ── Static requires discharge at call sites (contracts.md phase 6) ───────────
//
// A direct call site whose requires clauses the caller's live flow facts
// prove routes to the callee's unchecked twin (`<name>$nochk`); every other
// site keeps the checked entry, so runtime behavior at unproven sites is
// exactly as before. The twin's presence in the object's symbol table is the
// observable surface for elision (a proven site can never fire its check).

/// Does the compiled object contain the given symbol name?
fn object_contains_symbol(source: &str, symbol: &str) -> bool {
    let obj = pluto::compile_to_object(source).expect("program should compile");
    obj.windows(symbol.len()).any(|w| w == symbol.as_bytes())
}

#[test]
fn proven_requires_site_emits_unchecked_twin() {
    assert!(object_contains_symbol(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn main() {
    let a = 5
    if a > 0 {
        print(positive(a))
    }
}
"#,
        "positive$nochk",
    ));
}

#[test]
fn unproven_requires_site_emits_no_twin() {
    // `let a = 5` deliberately transfers no constant fact (binding_facts is
    // narrow); without a guard the site is unproven and no twin exists —
    // the only call target is the checked entry.
    assert!(!object_contains_symbol(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn main(a: int) {
    print(positive(a))
}
"#,
        "positive$nochk",
    ));
}

#[test]
fn generic_callee_requires_never_elides() {
    assert!(!object_contains_symbol(
        r#"
fn pick<T>(v: T, n: int) T
    requires n > 0
{
    return v
}

fn main() {
    print(pick(7, 3))
}
"#,
        "$nochk",
    ));
}

#[test]
fn requires_proven_site_runs_and_violating_site_still_aborts() {
    // Same callee, two sites: the guarded one is proven (elided), the
    // violating one keeps the runtime check and aborts exactly as before.
    let (stdout, stderr, code) = compile_and_run_output(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn main() {
    let a = 5
    if a > 0 {
        print(positive(a))
    }
    print(positive(0 - 1))
}
"#,
    );
    assert_eq!(stdout.trim(), "10");
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x > 0"), "stderr: {stderr}");
}

#[test]
fn requires_facts_killed_by_interleaved_call_keep_runtime_check() {
    // The guard proves `a.balance >= 50` before `drain` runs, but the
    // interleaved call may mutate the receiver through its alias — the
    // facts are dead at the `withdraw` site, the check is retained, and
    // the actual (violated) state aborts at runtime.
    let (_, stderr, code) = compile_and_run_output(
        r#"
class Account {
    balance: int

    fn withdraw(mut self, amt: int) int
        requires self.balance >= amt
    {
        self.balance = self.balance - amt
        return self.balance
    }
}

fn drain(mut a: Account) {
    a.balance = 0
}

fn main() {
    let mut a = Account { balance: 100 }
    if a.balance >= 50 {
        drain(a)
        print(a.withdraw(50))
    }
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("self.balance >= amt"), "stderr: {stderr}");
}

#[test]
fn requires_partial_conjunct_proof_keeps_runtime_check() {
    // The guard proves `x > 0` but not `x < 100`: the site is unproven
    // (all-or-nothing) and the retained check catches the violation.
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn bounded(x: int) int
    requires x > 0
    requires x < 100
{
    return x
}

fn main() {
    let v = 200
    if v > 0 {
        print(bounded(v))
    }
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("x < 100"), "stderr: {stderr}");
}

#[test]
fn requires_proven_method_site_runs_correctly() {
    let out = compile_and_run_stdout(
        r#"
class Account {
    balance: int

    fn withdraw(mut self, amt: int) int
        requires self.balance >= amt
        requires amt > 0
    {
        self.balance = self.balance - amt
        return self.balance
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    if a.balance >= 10 {
        print(a.withdraw(10))
    }
    print(a.balance)
}
"#,
    );
    assert_eq!(out.trim(), "90\n90");
}

#[test]
fn requires_fn_ref_call_keeps_checked_entry() {
    // A function reference eta-expands into a wrapper whose call site was
    // never proven: calls through the value hit the checked entry.
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn positive(x: int) int
    requires x > 0
{
    return x * 2
}

fn apply(f: fn(int) int, v: int) int {
    return f(v)
}

fn main() {
    print(apply(positive, 0 - 3))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
}

#[test]
fn requires_trait_dispatch_keeps_checked_entry() {
    // Dynamic dispatch goes through the vtable, which only ever holds the
    // checked entry; the trait-propagated requires still aborts.
    let (_, stderr, code) = compile_and_run_output(
        r#"
trait Sink {
    fn put(mut self, n: int)
        requires n > 0
}

class Box impl Sink {
    total: int

    fn put(mut self, n: int) {
        self.total = self.total + n
    }
}

fn feed(mut s: Sink, n: int) {
    s.put(n)
}

fn main() {
    let mut b = Box { total: 0 }
    feed(b, 0 - 1)
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
}

#[test]
fn requires_recursive_proven_site_elides_and_runs() {
    // The recursive site proves (`x > 0` implies `x - 1 >= 0`), so the twin
    // calls itself; the top-level literal site proves too.
    let out = compile_and_run_stdout(
        r#"
fn count_down(x: int) int
    requires x >= 0
{
    if x > 0 {
        return count_down(x - 1)
    }
    return x
}

fn main() {
    print(count_down(3))
}
"#,
    );
    assert_eq!(out.trim(), "0");
}

// ── Contracts on generic classes (template-proven, param-independent) ───────
// Contracts whose vocabulary is independent of the type parameters are
// validated once on the template, proven once on the template body under
// skolem substitution, and stamped onto every instantiation (see
// docs/design/contracts.md "Generics").

#[test]
fn generic_invariant_template_proven_and_runs() {
    let out = compile_and_run_stdout(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0

    fn add(mut self, d: int) {
        if d >= 0 {
            self.v = self.v + d
        }
    }
}

fn main() {
    let mut a = Gauge<int> { tag: none, v: 1 }
    let mut b = Gauge<string> { tag: none, v: 2 }
    a.add(10)
    b.add(20)
    print(a.v)
    print(b.v)
}
"#,
    );
    assert_eq!(out, "11\n22\n");
}

#[test]
fn generic_invariant_violating_template_rejected() {
    // The violating write is caught at the TEMPLATE, under skolem
    // substitution — one proof covers every instantiation, so the error
    // fires even though only Gauge<string> is ever constructed.
    compile_should_fail_with(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0

    fn bad(mut self) {
        self.v = self.v - 1
    }
}

fn main() {
    let g = Gauge<string> { tag: none, v: 1 }
    print(g.v)
}
"#,
        "cannot prove invariant 'self.v >= 0' of class 'Gauge<T>'",
    );
}

#[test]
fn generic_invariant_violating_uninstantiated_template_rejected() {
    // Template-proven means proven even when the template is never used.
    compile_should_fail_with(
        r#"
class Gauge<T> {
    v: int

    invariant self.v >= 0

    fn bad(mut self) {
        self.v = 0 - 1
    }
}

fn main() {
    print(1)
}
"#,
        "invariant 'self.v >= 0' of class 'Gauge<T>' is violated",
    );
}

#[test]
fn generic_invariant_param_typed_field_rejected() {
    compile_should_fail_with(
        r#"
class Box<T> {
    x: T

    invariant self.x >= 0
}

fn main() {
    print(1)
}
"#,
        "mentions field 'x' whose type involves a type parameter of 'Box'",
    );
}

#[test]
fn generic_invariant_nested_param_typed_field_rejected() {
    // The param-dependence check sees through structure ([T], T?, ...).
    compile_should_fail_with(
        r#"
class Box<T> {
    xs: [T]

    invariant self.xs >= 0
}

fn main() {
    print(1)
}
"#,
        "mentions field 'xs' whose type involves a type parameter of 'Box'",
    );
}

#[test]
fn generic_construction_violating_initializer_rejected() {
    // Construction sites name instantiations; the initializer proof
    // evaluates against the template's clauses.
    compile_should_fail_with(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0
}

fn main() {
    let g = Gauge<int> { tag: none, v: 0 - 5 }
    print(g.v)
}
"#,
        "construction of 'Gauge<int>' violates its invariant 'self.v >= 0'",
    );
}

#[test]
fn generic_construction_unproven_initializer_rejected() {
    compile_should_fail_with(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0
}

fn main(args: [string]) {
    let n = args.len() - 3
    let g = Gauge<int> { tag: none, v: n }
    print(g.v)
}
"#,
        "cannot prove invariant 'self.v >= 0' of class 'Gauge<int>' for this construction",
    );
}

#[test]
fn generic_foreign_write_obligation_enforced() {
    compile_should_fail_with(
        r#"
class Gauge<T> {
    tag: T?
    v: int

    invariant self.v >= 0
}

fn main() {
    let mut g = Gauge<int> { tag: none, v: 5 }
    g.v = 0 - 2
    print(g.v)
}
"#,
        "violates invariant 'self.v >= 0'",
    );
}

#[test]
fn generic_typestate_invariant_proven_across_transitions() {
    // State-gated methods (`where S == ...`) are registered per
    // instantiation; the invariant obligation applies to whichever methods
    // each instantiation has, and the fresh values transitions construct
    // carry the construction obligation (single-state only — a transition
    // has no pre-state for two-state clauses to relate across).
    let out = compile_and_run_stdout(
        r#"
class Idle { tag: int }
class Held { tag: int }

class Lease<S> {
    id: int
    epoch: int

    invariant self.epoch >= 0

    fn acquire(self) Lease<Held> where S == Idle {
        return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
    }

    fn release(self) Lease<Idle> where S == Held {
        return Lease<Idle> { id: self.id, epoch: self.epoch }
    }
}

fn main() {
    let l = Lease<Idle> { id: 7, epoch: 0 }
    let h = l.acquire()
    let done = h.release()
    print(done.epoch)
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn generic_typestate_violating_transition_construction_rejected() {
    compile_should_fail_with(
        r#"
class Idle { tag: int }
class Held { tag: int }

class Lease<S> {
    id: int
    epoch: int

    invariant self.epoch >= 0

    fn acquire(self) Lease<Held> where S == Idle {
        return Lease<Held> { id: self.id, epoch: 0 - 1 }
    }
}

fn main() {
    let l = Lease<Idle> { id: 7, epoch: 0 }
    let h = l.acquire()
    print(h.epoch)
}
"#,
        "violates its invariant 'self.epoch >= 0'",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4.5 precision: branch-join anchoring + construction facts
// ─────────────────────────────────────────────────────────────────────────────

/// The #357 census join bug, pinned: a call inside an `if` branch kills
/// field facts from every frame (kills are flow events), and before the fix
/// nothing restored invariant-level anchoring at the join — so a subsequent
/// call's ensures instantiation had no usable pre-state and a bare
/// `try_withdraw` after a branch failed to shrink. Fails on master.
#[test]
fn ensures_composition_survives_branch_join() {
    let out = compile_and_run_stdout(
        r#"
error Insufficient {
    needed: int
}

class BankAccount {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) + amount
    {
        self.balance = self.balance + amount
    }

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
        ensures self.balance == old(self.balance) - amount
    {
        if amount > self.balance {
            raise Insufficient { needed: amount }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    // Literal construction: the exact fact (balance == 100) is killed by
    // the call inside the branch; only the join's invariant-level
    // re-anchoring makes the composition below work.
    let mut acc = BankAccount { balance: 100 }
    if acc.balance > 50 {
        acc.deposit(10)
    }
    acc.deposit(80)
    let x = acc.try_withdraw(80)
    print(x)
}
"#,
    );
    assert_eq!(out.trim(), "110");
}

/// Loop exits get the same invariant-level re-anchoring: facts dropped by
/// the loop havoc are restored to invariant level for ensures composition.
#[test]
fn ensures_composition_survives_loop_exit() {
    let out = compile_and_run_stdout(
        r#"
error Insufficient {
}

class BankAccount {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) + amount
    {
        self.balance = self.balance + amount
    }

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
    {
        if amount > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut acc = BankAccount { balance: 5 }
    for i in 0..3 {
        acc.deposit(1)
    }
    acc.deposit(80)
    let x = acc.try_withdraw(80)
    print(x)
}
"#,
    );
    assert_eq!(out.trim(), "8");
}

/// Construction is fully transparent: the caller's fact env knows the
/// exact int-field values of a struct literal, so ensures instantiation
/// composes from the construction with no guard — `balance == 100` plus
/// deposit's `+80` refutes `try_withdraw(180)`'s raise condition exactly.
#[test]
fn construction_facts_feed_ensures_instantiation() {
    let out = compile_and_run_stdout(
        r#"
error Insufficient {
}

class BankAccount {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) + amount
    {
        self.balance = self.balance + amount
    }

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
    {
        if amount > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut acc = BankAccount { balance: 100 }
    acc.deposit(80)
    let x = acc.try_withdraw(180)
    print(x)
}
"#,
    );
    assert_eq!(out.trim(), "0");
}

/// Construction facts die under the usual kill rules: a mut call whose
/// ensures does not pin the field leaves only invariant-level knowledge,
/// so the later bare call needs handling again.
#[test]
fn construction_facts_killed_by_mut_call() {
    compile_should_fail_with(
        r#"
error Insufficient {
}

class BankAccount {
    balance: int

    invariant self.balance >= 0

    fn touch(mut self) {
        self.balance = self.balance + 0
    }

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
    {
        if amount > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut acc = BankAccount { balance: 100 }
    acc.touch()
    let x = acc.try_withdraw(50)
    print(x)
}
"#,
        "call to fallible method 'try_withdraw' must be handled",
    );
}

/// Reassignment of the binding kills the old construction facts; the new
/// literal's facts take over (10 < 50, so the raise is *provable*, and the
/// call still requires handling).
#[test]
fn construction_facts_killed_by_reassignment() {
    compile_should_fail_with(
        r#"
error Insufficient {
}

class BankAccount {
    balance: int

    invariant self.balance >= 0

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
    {
        if amount > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut acc = BankAccount { balance: 100 }
    acc = BankAccount { balance: 10 }
    let x = acc.try_withdraw(50)
    print(x)
}
"#,
        "call to fallible method 'try_withdraw' must be handled",
    );
}

/// Entities are excluded: entity fields never carry flow facts, so
/// constructing an object grants the caller nothing to shrink with.
#[test]
fn construction_facts_not_assumed_for_entities() {
    compile_should_fail_with(
        r#"
error Insufficient {
}

object Vault {
    balance: int

    fn try_withdraw(mut self, amount: int) int
        requires amount > 0
    {
        if amount > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amount
        return self.balance
    }
}

fn main() {
    let mut v = Vault { balance: 100 }
    let x = v.try_withdraw(50)
    print(x)
}
"#,
        "call to fallible method 'try_withdraw' must be handled",
    );
}

// ── Contracts on bytes APIs (#368 §4) ────────────────────────────────────────

/// `requires frame.len() >= 4` on a bytes-taking API: the fact fragment
/// supports bytes len() terms, so the satisfied call runs and the violated
/// call aborts with the contract message.
#[test]
fn requires_on_bytes_len_satisfied_runs() {
    let out = compile_and_run_stdout(
        r#"
fn header_tag(frame: bytes) int
    requires frame.len() >= 4
{
    return frame[0] as int
}

fn main() {
    let mut frame = bytes_new()
    frame.push(7 as byte)
    frame.push(0 as byte)
    frame.push(0 as byte)
    frame.push(0 as byte)
    print(header_tag(frame))
}
"#,
    );
    assert_eq!(out, "7\n");
}

#[test]
fn requires_on_bytes_len_violated_aborts() {
    let (_, stderr, code) = compile_and_run_output(
        r#"
fn header_tag(frame: bytes) int
    requires frame.len() >= 4
{
    return frame[0] as int
}

fn main() {
    let mut frame = bytes_new()
    frame.push(7 as byte)
    print(header_tag(frame))
}
"#,
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("requires violation"), "stderr: {stderr}");
    assert!(stderr.contains("frame.len() >= 4"), "stderr: {stderr}");
}

// ── Issue #416 honest-behavior pins: overflow traps keep proofs honest ───────
//
// These are the attack shapes from the soundness report: the prover models
// ints mathematically, and before overflow trapping each shape made a
// discharged proof observably false at runtime. Now the overflow is a defect
// and the process aborts BEFORE the proven predicate can be falsified.

// Repro 1: strict invariant discharge. `self.x = self.x + 1` discharges
// against invariant self.x >= 0 mathematically; the overflowing bump now
// traps instead of writing i64::MIN into a field proven non-negative.
#[test]
fn invariant_bump_overflow_traps_instead_of_falsifying() {
    let (_stdout, stderr, code) = compile_and_run_output(
        r#"
class Counter {
    x: int

    invariant self.x >= 0

    fn bump(mut self) {
        self.x = self.x + 1
    }
}

fn main() {
    let mut c = Counter { x: 9223372036854775807 }
    c.bump()
    print(f"x = {c.x}")
}
"#,
    );
    assert_ne!(code, 0, "overflowing bump must trap, not falsify the invariant");
    assert!(
        stderr.contains("pluto: defect: integer overflow in '+': 9223372036854775807 + 1"),
        "stderr: {stderr}"
    );
}

// Repro 2: verify.monotonic (two-state stdlib property). The epoch can no
// longer decrease by wrapping — the advance past i64::MAX traps.
#[test]
fn monotonic_epoch_overflow_traps_instead_of_decreasing() {
    let (stdout, stderr, code) = compile_test_and_run_with_stdlib(
        r#"
import std.verify

class Epoch satisfies verify.monotonic(self.e) {
    e: int

    fn advance(mut self) {
        self.e = self.e + 1
    }
}

test "epoch cannot wrap" {
    let mut a = Epoch { e: 9223372036854775807 }
    a.advance()
    expect(a.e).to_equal(0)
}
"#,
    );
    assert_ne!(code, 0, "overflowing advance must trap; stdout: {stdout}");
    assert!(
        stderr.contains("pluto: defect: integer overflow in '+': 9223372036854775807 + 1"),
        "stderr: {stderr}"
    );
}

// Repro 3: requires discharge routed the call to the unchecked twin because
// the caller proved `big + 1 > 0` mathematically. The overflowing argument
// expression now traps before the call, so f never runs with a false
// precondition.
#[test]
fn requires_nochk_twin_overflow_traps_before_entry() {
    let (stdout, stderr, code) = compile_and_run_output(
        r#"
fn f(x: int)
    requires x > 0
{
    print(f"inside f, x = {x}")
}

fn main() {
    let big = 9223372036854775807
    if big > 0 {
        f(big + 1)
    }
}
"#,
    );
    assert_ne!(code, 0);
    assert!(
        !stdout.contains("inside f"),
        "f must not run with a false precondition; stdout: {stdout}"
    );
    assert!(
        stderr.contains("pluto: defect: integer overflow in '+': 9223372036854775807 + 1"),
        "stderr: {stderr}"
    );
}

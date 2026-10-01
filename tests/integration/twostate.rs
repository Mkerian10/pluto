//! Two-state proofs (docs/design/rfc-properties.md phases 1–2):
//! `ensures` postconditions with `old()`, caller-side assumption of the
//! relation, and two-state (`old()`-carrying) class/object invariants.

mod common;

use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

// ── Phase 1: `old()` in ensures ─────────────────────────────────────────────

#[test]
fn ensures_simple_arithmetic_proven() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn main() {
    let mut c = Counter { n: 41 }
    c.bump()
    print(c.n)
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn ensures_subtract_then_add_proven() {
    // The relation may be broken between writes — only the exit matters.
    assert_eq!(
        compile_and_run(
            r#"
class Holder {
    x: int

    fn shuffle(mut self) ensures self.x == old(self.x) {
        self.x = self.x - 5
        self.x = self.x + 5
    }
}

fn main() {
    let mut h = Holder { x: 10 }
    h.shuffle()
}
"#,
        ),
        0
    );
}

#[test]
fn ensures_branches_proven() {
    // A guarded increment proves monotonicity across the branch join: the
    // then-path adds 1, the implicit fall-through adds 0.
    assert_eq!(
        compile_and_run(
            r#"
class Clamp {
    x: int

    fn bump(mut self) ensures self.x >= old(self.x) {
        if self.x < 10 {
            self.x = self.x + 1
        }
    }
}

fn main() {
    let mut c = Clamp { x: 3 }
    c.bump()
}
"#,
        ),
        0
    );
}

#[test]
fn ensures_raise_path_exempt() {
    // The raise path exits without owing the postcondition; the normal exit
    // proves the exact relation (guard gives amt <= balance).
    let out = compile_and_run_stdout(
        r#"
error Insufficient {
}

class Account {
    balance: int

    fn withdraw(mut self, amt: int) requires amt > 0
        ensures self.balance == old(self.balance) - amt {
        if amt > self.balance {
            raise Insufficient { }
        }
        self.balance = self.balance - amt
    }
}

fn main() {
    let mut a = Account { balance: 100 }
    a.withdraw(30) catch err {
        print("no funds")
    }
    print(a.balance)
}
"#,
    );
    assert_eq!(out.trim(), "70");
}

#[test]
fn ensures_self_call_composition() {
    // A method's own ensures proof sees through calls to sibling methods via
    // the callees' declared relations (equality clauses pin the symbolic
    // field value).
    let out = compile_and_run_stdout(
        r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }

    fn double_bump(mut self) ensures self.n == old(self.n) + 2 {
        self.bump()
        self.bump()
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.double_bump()
    print(c.n)
}
"#,
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn ensures_violated_rejected() {
    compile_should_fail_with(
        r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 2 {
        self.n = self.n + 1
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
}
"#,
        "ensures clause 'self.n == old(self.n) + 2' of method 'bump' of class 'Counter' is violated",
    );
}

#[test]
fn ensures_unprovable_rejected() {
    // v is unconstrained — the relation cannot be decided.
    compile_should_fail_with(
        r#"
class Holder {
    x: int

    fn add(mut self, v: int) ensures self.x >= old(self.x) {
        self.x = self.x + v
    }
}

fn main() {
    let mut h = Holder { x: 0 }
    h.add(3)
}
"#,
        "cannot prove ensures clause 'self.x >= old(self.x)' of method 'add'",
    );
}

#[test]
fn ensures_requires_makes_provable() {
    // The same method proves once requires bounds the parameter.
    assert_eq!(
        compile_and_run(
            r#"
class Holder {
    x: int

    fn add(mut self, v: int) requires v >= 0
        ensures self.x >= old(self.x) {
        self.x = self.x + v
    }
}

fn main() {
    let mut h = Holder { x: 0 }
    h.add(3)
}
"#,
        ),
        0
    );
}

#[test]
fn ensures_call_severs_two_state_knowledge() {
    // An intermediate call may mutate the receiver through an alias — the
    // exact relation does not survive it (conservative by design).
    compile_should_fail_with(
        r#"
fn note() {
    print("mid")
}

class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        note()
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
}
"#,
        "cannot prove ensures clause",
    );
}

// ── old() placement and fragment ────────────────────────────────────────────

#[test]
fn old_outside_contracts_rejected() {
    compile_should_fail_with(
        r#"
fn main() {
    let x = old(3)
    print(x)
}
"#,
        "'old(...)' is only usable inside 'ensures' and 'invariant' clauses",
    );
}

#[test]
fn old_in_requires_rejected() {
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn set(mut self, v: int)
        requires old(self.x) > 0
    {
        self.x = v
    }
}

fn main() {
}
"#,
        "'old(...)' is only meaningful in 'ensures' and 'invariant' clauses",
    );
}

#[test]
fn nested_old_rejected() {
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn set(mut self, v: int) ensures self.x > old(old(self.x)) {
        self.x = v
    }
}

fn main() {
}
"#,
        "nested 'old(...)' is not allowed",
    );
}

#[test]
fn ensures_out_of_fragment_float_rejected() {
    compile_should_fail_with(
        r#"
class C {
    f: float

    fn set(mut self, v: float) ensures self.f >= old(self.f) {
        self.f = v
    }
}

fn main() {
}
"#,
        "outside the provable fragment",
    );
}

#[test]
fn ensures_out_of_fragment_len_rejected() {
    compile_should_fail_with(
        r#"
class C {
    xs: [int]

    fn grow(mut self) ensures self.xs.len() > old(self.xs.len()) {
        self.xs.push(1)
    }
}

fn main() {
}
"#,
        "outside the provable fragment",
    );
}

#[test]
fn ensures_on_free_function_rejected() {
    compile_should_fail_with(
        r#"
fn f(x: int) int
    ensures x > 0
{
    return x
}

fn main() {
}
"#,
        "'ensures' clauses are only supported on methods of classes and objects",
    );
}

#[test]
fn ensures_on_generic_class_rejected() {
    // Generic bodies are checked against skolem type parameters; ensures on
    // them is rejected at declaration (template checking must never see it).
    compile_should_fail_with(
        r#"
class Box<T> {
    v: T
    n: int

    fn tick(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn main() {
    let mut b = Box { v: 1, n: 0 }
    b.tick()
}
"#,
        "ensures clauses on methods of generic classes are not yet supported",
    );
}

#[test]
fn generic_class_calling_ensures_method_ok() {
    // A generic template calling an ensures-carrying concrete method must
    // not crash skolem checking.
    assert_eq!(
        compile_and_run(
            r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

class Box<T> {
    v: T

    fn poke(self, c: Counter) int {
        let mut local = c
        local.bump()
        return local.n
    }
}

fn main() {
    let b = Box<string> { v: "x" }
    let c = Counter { n: 0 }
    print(b.poke(c))
}
"#,
        ),
        0
    );
}

// ── Caller-side assumption ──────────────────────────────────────────────────

#[test]
fn caller_assumes_ensures_for_construction_proof() {
    // Pos requires v >= 1; the caller only knows it via bump's ensures
    // (n == old(n) + 1) over the invariant-level pre-state (n >= 0).
    let out = compile_and_run_stdout(
        r#"
class Pos {
    v: int
    invariant self.v >= 1
}

class Counter {
    n: int
    invariant self.n >= 0

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn make(mut c: Counter) Pos {
    c.bump()
    return Pos { v: c.n }
}

fn main() {
    let mut c = Counter { n: 5 }
    let p = make(c)
    print(p.v)
}
"#,
    );
    assert_eq!(out.trim(), "6");
}

#[test]
fn caller_assumption_requires_the_call() {
    // Without the bump() call the same construction is unprovable — the
    // assumption really comes from the callee's ensures.
    compile_should_fail_with(
        r#"
class Pos {
    v: int
    invariant self.v >= 1
}

class Counter {
    n: int
    invariant self.n >= 0

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn make(mut c: Counter) Pos {
    return Pos { v: c.n }
}

fn main() {
    let mut c = Counter { n: 5 }
    let p = make(c)
    print(p.v)
}
"#,
        "cannot prove invariant 'self.v >= 1' of class 'Pos'",
    );
}

#[test]
fn caller_assumption_feeds_error_shrinking() {
    // check() raises only when v < 1; after bump() the caller's facts carry
    // n >= 1 from the ensures, so the raise is refuted and the call site
    // needs no handling.
    let out = compile_and_run_stdout(
        r#"
error Neg {
}

fn check(v: int) int {
    if v < 1 {
        raise Neg {
        }
    }
    return v
}

class Counter {
    n: int
    invariant self.n >= 0

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
    print(check(c.n))
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

// ── Phase 2: two-state invariants ───────────────────────────────────────────

#[test]
fn two_state_invariant_monotonic_proven() {
    assert_eq!(
        compile_and_run(
            r#"
object Epoch {
    e: int
    invariant self.e >= old(self.e)

    fn advance(mut self) {
        self.e = self.e + 1
    }

    fn now(self) int {
        return self.e
    }
}

fn main() {
    let mut x = Epoch { e: 0 }
    x.advance()
    print(x.now())
}
"#,
        ),
        0
    );
}

#[test]
fn two_state_invariant_decrease_rejected() {
    compile_should_fail_with(
        r#"
object Epoch {
    e: int
    invariant self.e >= old(self.e)

    fn reset(mut self) {
        self.e = 0
    }
}

fn main() {
}
"#,
        "cannot prove invariant 'self.e >= old(self.e)' of class 'Epoch'",
    );
}

#[test]
fn two_state_invariant_foreign_write_proven() {
    assert_eq!(
        compile_and_run(
            r#"
class Epoch {
    e: int
    invariant self.e >= old(self.e)
}

fn main() {
    let mut x = Epoch { e: 1 }
    x.e = x.e + 5
    print(x.e)
}
"#,
        ),
        0
    );
}

#[test]
fn two_state_invariant_foreign_write_rejected() {
    compile_should_fail_with(
        r#"
class Epoch {
    e: int
    invariant self.e >= old(self.e)
}

fn main() {
    let mut x = Epoch { e: 1 }
    x.e = 0
}
"#,
        "cannot prove invariant 'self.e >= old(self.e)' of class 'Epoch' after this write",
    );
}

#[test]
fn two_state_invariant_construction_unconstrained() {
    // Construction has no pre-state: any initial value is legal, including
    // one a later transition could never move to.
    assert_eq!(
        compile_and_run(
            r#"
class Epoch {
    e: int
    invariant self.e >= old(self.e)
}

fn main() {
    let x = Epoch { e: -100 }
    print(x.e)
}
"#,
        ),
        0
    );
}

#[test]
fn two_state_invariant_survives_calls() {
    // Monotonicity composes across call boundaries (transitive relation):
    // the segment before the call and the segment after both prove, and the
    // boundary carries the relation through.
    assert_eq!(
        compile_and_run(
            r#"
fn note() {
    print("mid")
}

object Epoch {
    e: int
    invariant self.e >= old(self.e)

    fn advance_twice(mut self) {
        self.e = self.e + 1
        note()
        self.e = self.e + 1
    }
}

fn main() {
    let mut x = Epoch { e: 0 }
    x.advance_twice()
}
"#,
        ),
        0
    );
}

#[test]
fn two_state_invariant_on_generic_rejected() {
    // Invariants on generic classes were already rejected; old() does not
    // change that.
    compile_should_fail_with(
        r#"
class Box<T> {
    v: T
    n: int
    invariant self.n >= old(self.n)
}

fn main() {
    let b = Box { v: 1, n: 0 }
    print(b.n)
}
"#,
        "invariants on generic classes are not yet supported",
    );
}

#[test]
fn mixed_invariants_single_state_still_checked() {
    // A class carrying both kinds: the single-state half still guards
    // construction.
    compile_should_fail_with(
        r#"
class Epoch {
    e: int
    invariant self.e >= 0
    invariant self.e >= old(self.e)
}

fn main() {
    let x = Epoch { e: -1 }
    print(x.e)
}
"#,
        "construction of 'Epoch' violates its invariant 'self.e >= 0'",
    );
}

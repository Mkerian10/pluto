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
    // An intermediate call that may reach the receiver — here a free
    // function whose declared parameter is the receiver's class (an alias,
    // coarse by type) — may mutate it, so the exact relation does not
    // survive (conservative by design). Reach-free calls are exempt; see
    // the purity-aware severing tests below.
    compile_should_fail_with(
        r#"
class Holder {
    n: int

    fn bump(mut self, peer: Holder) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        observe(peer)
    }
}

fn observe(h: Holder) {
    print(h.n)
}

fn main() {
    let mut c = Holder { n: 0 }
    let p = Holder { n: 5 }
    c.bump(p)
}
"#,
        "cannot prove ensures clause",
    );
}

// ── Purity-aware severing (phase 4.5 precision) ─────────────────────────────

#[test]
fn ensures_proves_through_trailing_print() {
    // print() provably cannot reach the receiver (its argument is a
    // string), so exact two-state knowledge survives it — the #357 census
    // case (`OrderService: ensures count == old(count) + 1`).
    let out = compile_and_run_stdout(
        r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        print(f"now {self.n}")
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
    print(c.n)
}
"#,
    );
    assert_eq!(out.trim(), "now 1\n1");
}

#[test]
fn ensures_proves_through_reach_free_fn() {
    // A free function whose declared parameters cannot reach the
    // receiver's class cannot mutate (or observe) it.
    assert_eq!(
        compile_and_run(
            r#"
fn note(x: int) int {
    return x + 1
}

class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        let v = note(self.n)
        print(v)
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
}
"#,
        ),
        0
    );
}

#[test]
fn frame_ensures_proves_through_builtin_on_param() {
    // The blob census case: a builtin primitive method on a *parameter*
    // (`d.to_bytes()`) cannot reach the receiver — the frame ensures
    // (`epoch == old(epoch)`) survives it.
    assert_eq!(
        compile_and_run(
            r#"
class Store {
    data: bytes
    epoch: int
    applied: int

    fn apply(mut self, tok: int, d: string)
        ensures self.epoch == old(self.epoch)
    {
        self.applied = tok
        self.data = d.to_bytes()
    }
}

fn main() {
    let mut s = Store { data: "x".to_bytes(), epoch: 0, applied: 0 }
    s.apply(1, "payload")
}
"#,
        ),
        0
    );
}

#[test]
fn ensures_proves_through_builtin_on_own_collection_field() {
    // A builtin collection mutator on the receiver's own collection field
    // cannot write int fields (builtins move references and change
    // lengths, never the fields of class instances — the load-bearing
    // survey on facts::CallSeverity::Collections) — the exact int relation
    // survives the push.
    assert_eq!(
        compile_and_run(
            r#"
class Log {
    entries: [int]
    count: int

    fn add(mut self, v: int) ensures self.count == old(self.count) + 1 {
        self.count = self.count + 1
        self.entries.push(v)
    }
}

fn main() {
    let mut l = Log { entries: [], count: 0 }
    l.add(7)
}
"#,
        ),
        0
    );
}

#[test]
fn ensures_severed_by_free_fn_taking_receiver() {
    // Passing the receiver itself to a free function severs: the callee
    // can reach and mutate it.
    compile_should_fail_with(
        r#"
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        audit(self)
    }
}

fn audit(c: Counter) {
    print(c.n)
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump()
}
"#,
        "cannot prove ensures clause",
    );
}

#[test]
fn ensures_severed_by_mut_method_on_same_class_alias() {
    // A mut method on another binding of the same class may be a method on
    // an alias of the receiver — severs.
    compile_should_fail_with(
        r#"
class Counter {
    n: int

    fn poke(mut self) {
        self.n = self.n + 1
    }

    fn bump(mut self, other: Counter) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        let mut o = other
        o.poke()
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    let d = Counter { n: 5 }
    c.bump(d)
}
"#,
        "cannot prove ensures clause",
    );
}

#[test]
fn ensures_severed_by_fn_taking_wrapper_of_receiver() {
    // Reachability is transitive: a free function whose signature mentions
    // a class that *contains* the receiver's class can reach the receiver
    // — severs.
    compile_should_fail_with(
        r#"
class Counter {
    n: int

    fn bump(mut self, w: Wrapper) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        touch_wrapper(w)
    }
}

class Wrapper {
    inner: Counter
}

fn touch_wrapper(w: Wrapper) {
    print(w.inner.n)
}

fn main() {
    let mut c = Counter { n: 0 }
    let w = Wrapper { inner: Counter { n: 1 } }
    c.bump(w)
}
"#,
        "cannot prove ensures clause",
    );
}

#[test]
fn ensures_severed_by_closure_call() {
    // A call through a fn-typed value is opaque — its captures may hold an
    // alias of the receiver.
    compile_should_fail_with(
        r#"
class Counter {
    n: int

    fn bump(mut self, f: fn() int) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        let v = f()
        print(v)
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.bump(() => 3)
}
"#,
        "cannot prove ensures clause",
    );
}

#[test]
fn two_state_invariant_exact_increment_through_print() {
    // The purity exemption also benefits two-state invariants: an exact
    // equality ensures (which non-transitive composition would lose at a
    // composed boundary) survives a reach-free call.
    assert_eq!(
        compile_and_run(
            r#"
object Epoch {
    e: int
    invariant self.e >= old(self.e)

    fn advance(mut self) ensures self.e == old(self.e) + 1 {
        self.e = self.e + 1
        print("advanced")
    }
}

fn main() {
    let mut x = Epoch { e: 0 }
    x.advance()
}
"#,
        ),
        0
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
fn ensures_on_generic_class_param_independent_accepted() {
    // Ensures on generic class methods are supported when the vocabulary is
    // param-independent: proven once on the template under skolems.
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    v: T
    n: int

    fn tick(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn main() {
    let mut b = Box<int> { v: 1, n: 0 }
    b.tick()
    print(b.n)
}
"#,
    );
    assert_eq!(out.trim(), "1");
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
    // Construction facts carry `x.e == 1` into the caller, so the
    // rewinding write is *refuted* outright (not merely unprovable).
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
        "this write to 'x.e' violates invariant 'self.e >= old(self.e)' of class 'Epoch'",
    );
}

#[test]
fn two_state_invariant_foreign_write_unprovable() {
    // With no exact construction knowledge (opaque initializer), the same
    // write is Unknown — the "cannot prove" diagnostic.
    compile_should_fail_with(
        r#"
fn seed() int {
    return 1
}

class Epoch {
    e: int
    invariant self.e >= old(self.e)
}

fn main() {
    let mut x = Epoch { e: seed() }
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
fn two_state_invariant_on_generic_accepted() {
    // Two-state invariants on generic classes are supported (template-proven,
    // param-independent vocabulary); construction owes nothing (no pre-state).
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    v: T
    n: int
    invariant self.n >= old(self.n)
}

fn main() {
    let b = Box<int> { v: 1, n: 0 }
    print(b.n)
}
"#,
    );
    assert_eq!(out.trim(), "0");
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

// ── Two-state contracts on generic classes (template-proven) ────────────────
// Param-independent vocabulary only; one skolem proof covers every
// instantiation (docs/design/contracts.md "Generics").

#[test]
fn generic_two_state_invariant_monotone_object() {
    // The PR #357 census case: Topic<T>.published is a true monotone claim.
    // The proof composes through the self-call inside the loop.
    let out = compile_and_run_stdout(
        r#"
object Topic<T> {
    latest: T?
    published: int

    invariant self.published >= old(self.published)

    fn publish(mut self, msg: T) {
        self.latest = msg
        self.published = self.published + 1
    }

    fn flood(mut self, msg: T) {
        let mut i = 0
        while i < 100 {
            self.publish(msg)
            i = i + 1
        }
    }
}

fn main() {
    let mut t = Topic<string> { latest: none, published: 0 }
    t.publish("a")
    t.flood("b")
    print(t.published)
}
"#,
    );
    assert_eq!(out.trim(), "101");
}

#[test]
fn generic_two_state_invariant_violating_template_rejected() {
    compile_should_fail_with(
        r#"
object Topic<T> {
    latest: T?
    published: int

    invariant self.published >= old(self.published)

    fn rewind(mut self) {
        self.published = self.published - 1
    }
}

fn main() {
    let mut t = Topic<int> { latest: none, published: 0 }
    t.rewind()
}
"#,
        "invariant 'self.published >= old(self.published)' of class 'Topic<T>' is violated",
    );
}

#[test]
fn generic_ensures_proven_and_caller_assumes_relation() {
    // The ensures proof runs once on the template; the caller-side
    // assumption works through the instantiation's stamped specs: after
    // deposit(80) the exact relation refutes nothing and proves the
    // downstream construction (balance - 50 >= 0).
    let out = compile_and_run_stdout(
        r#"
class Acc<T> {
    tag: T?
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amt: int)
        requires amt >= 0
        ensures self.balance == old(self.balance) + amt {
        self.balance = self.balance + amt
    }
}

class Floor {
    v: int
    invariant self.v >= 0
}

fn main() {
    let mut a = Acc<string> { tag: none, balance: 0 }
    a.deposit(80)
    let f = Floor { v: a.balance - 50 }
    print(f.v)
}
"#,
    );
    assert_eq!(out.trim(), "30");
}

#[test]
fn generic_ensures_violating_template_rejected() {
    compile_should_fail_with(
        r#"
class Acc<T> {
    tag: T?
    balance: int

    fn bump(mut self)
        ensures self.balance == old(self.balance) + 1 {
        self.balance = self.balance + 2
    }
}

fn main() {
    let mut a = Acc<int> { tag: none, balance: 0 }
    a.bump()
}
"#,
        "ensures clause 'self.balance == old(self.balance) + 1' of method 'bump' of class 'Acc<T>' is violated",
    );
}

#[test]
fn generic_ensures_param_typed_parameter_rejected() {
    compile_should_fail_with(
        r#"
class Acc<T> {
    tag: T
    balance: int

    fn bad(mut self, x: T)
        ensures self.balance == x {
        self.balance = 0
    }
}

fn main() {
    print(1)
}
"#,
        "mentions parameter 'x' whose type involves a type parameter of 'Acc'",
    );
}

#[test]
fn generic_ensures_param_typed_field_rejected() {
    compile_should_fail_with(
        r#"
class Acc<T> {
    held: T

    fn bad(mut self)
        ensures self.held == old(self.held) {
    }
}

fn main() {
    print(1)
}
"#,
        "mentions field 'held' whose type involves a type parameter of 'Acc'",
    );
}

#[test]
fn generic_sibling_ensures_compose_in_template() {
    // apply_self_call_ensures works under skolems: double_bump's exact +2
    // proof sees through the two bump() calls via their stamped ensures.
    let out = compile_and_run_stdout(
        r#"
class Acc<T> {
    tag: T?
    balance: int

    fn bump(mut self)
        ensures self.balance == old(self.balance) + 1 {
        self.balance = self.balance + 1
    }

    fn double_bump(mut self)
        ensures self.balance == old(self.balance) + 2 {
        self.bump()
        self.bump()
    }
}

fn main() {
    let mut a = Acc<int> { tag: none, balance: 0 }
    a.double_bump()
    print(a.balance)
}
"#,
    );
    assert_eq!(out.trim(), "2");
}

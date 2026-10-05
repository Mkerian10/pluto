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

// ── Receiver aliasing (issue #417) ──────────────────────────────────────────
//
// A foreign write through another binding of the receiver's own class may
// go through an alias of `self` (classes alias freely within a task), which
// would desync the ghost scope's symbolic field tracking from the real
// state and certify false ensures/invariants. Such writes are rejected
// outright inside contract-carrying methods, and the caller-side ensures
// assumption is skipped when an argument may alias the receiver.

#[test]
fn ensures_receiver_alias_foreign_write_rejected() {
    // `c.bump2(c)` makes `other` alias `self`: the declared relation would
    // be false at runtime (x advances by 2, not 1).
    compile_should_fail_with(
        r#"
class Cell {
    x: int

    fn bump2(mut self, mut other: Cell) ensures self.x == old(self.x) + 1 {
        other.x = other.x + 1
        self.x = self.x + 1
    }
}

fn main() {
    let mut c = Cell { x: 5 }
    c.bump2(c)
    print(c.x)
}
"#,
        "may alias 'self'",
    );
}

#[test]
fn ensures_receiver_alias_via_self_link_rejected() {
    // No parameter needed: the alias arrives through a self-referential
    // nullable field (`c.link = c`).
    compile_should_fail_with(
        r#"
class Cell {
    x: int
    link: Cell?

    fn bump(mut self) ensures self.x == old(self.x) + 1 {
        let mut l = self.link
        if l != none {
            l.x = l.x + 1
        }
        self.x = self.x + 1
    }
}

fn main() {
    let mut c = Cell { x: 5, link: none }
    c.link = c
    c.bump()
    print(c.x)
}
"#,
        "may alias 'self'",
    );
}

#[test]
fn invariant_receiver_alias_foreign_write_rejected() {
    // The escalation from the audit: a false ensures (proven under a
    // no-alias assumption) feeds the caller's facts, which then discharge a
    // write that makes a STRICT single-state invariant observably false
    // (`a.dec(a)` leaves bal at 8, not 9; `a.bal - 9` lands at -1). Both
    // the callee body and the caller-side assumption must refuse.
    compile_should_fail_with(
        r#"
class Acc {
    bal: int

    invariant self.bal >= 0

    fn dec(mut self, mut other: Acc)
        requires self.bal >= 1
        ensures self.bal == old(self.bal) - 1
    {
        if other.bal >= 1 {
            other.bal = other.bal - 1
        }
        self.bal = self.bal - 1
    }
}

fn main() {
    let mut a = Acc { bal: 10 }
    a.dec(a)
    a.bal = a.bal - 9
    print(a.bal)
}
"#,
        "invariant 'self.bal >= 0'",
    );
}

#[test]
fn receiver_alias_write_rejected_without_parameter_contracts() {
    // The callee body alone (no caller involved) is rejected, and the
    // diagnostic names both sides: the aliasing binding and the clauses it
    // threatens.
    compile_should_fail_with(
        r#"
class Acc {
    bal: int

    invariant self.bal >= 0

    fn dec(mut self, mut other: Acc)
        requires self.bal >= 1
        ensures self.bal == old(self.bal) - 1
    {
        if other.bal >= 1 {
            other.bal = other.bal - 1
        }
        self.bal = self.bal - 1
    }
}

fn main() {
    print(0)
}
"#,
        "goes through 'other', another 'Acc' binding that may alias 'self'",
    );
}

#[test]
fn receiver_alias_indirect_field_flow_rejected() {
    // The alias blind spot is not limited to fields the clauses mention:
    // a stale symbolic value of ANY tracked int field can flow into the
    // proof (`other.y` write, then `self.x = self.y`). Same-class foreign
    // writes to tracked fields are rejected wholesale.
    compile_should_fail_with(
        r#"
class C {
    x: int
    y: int

    invariant self.x >= 0

    fn m(mut self, mut other: C)
        requires self.y >= 5
    {
        other.y = 0 - 10
        let t = self.y
        self.x = t
    }
}

fn main() {
    print(0)
}
"#,
        "may alias 'self'",
    );
}

#[test]
fn same_class_write_without_contracts_still_allowed() {
    // No proof scope, no ghost state to protect: a contract-free method may
    // write other bindings of its own class.
    let out = compile_and_run_stdout(
        r#"
class Cell {
    x: int

    fn poke(mut self, mut other: Cell) {
        other.x = other.x + 1
        self.x = self.x + 1
    }
}

fn main() {
    let mut a = Cell { x: 1 }
    let mut b = Cell { x: 10 }
    a.poke(b)
    print(f"{a.x} {b.x}")
}
"#,
    );
    assert_eq!(out.trim(), "2 11");
}

#[test]
fn caller_assumption_skipped_when_arg_may_alias_receiver() {
    // The callee compiles (it never writes `other`), but the caller must
    // not assume the ensures relation when an argument may alias the
    // receiver: the assumption's independence from the argument is exactly
    // what receiver aliasing broke in the audit.
    compile_should_fail_with(
        r#"
class Pos {
    v: int
    invariant self.v >= 1
}

class Counter {
    n: int

    fn bump(mut self, other: Counter) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        print(other.n)
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    let mut d = Counter { n: 5 }
    c.bump(d)
    let p = Pos { v: c.n }
    print(p.v)
}
"#,
        "cannot prove invariant 'self.v >= 1' of class 'Pos' for this construction",
    );
}

#[test]
fn caller_assumption_survives_primitive_args() {
    // Primitive arguments can never alias the receiver: the caller-side
    // assumption still flows for them.
    let out = compile_and_run_stdout(
        r#"
class Pos {
    v: int
    invariant self.v >= 1
}

class Counter {
    n: int

    fn add(mut self, k: int) requires k >= 1
        ensures self.n == old(self.n) + k {
        self.n = self.n + k
    }
}

fn main() {
    let mut c = Counter { n: 0 }
    c.add(3)
    let p = Pos { v: c.n }
    print(p.v)
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

// ── Issue #454: calls that provably cannot write the receiver ──────────────
//
// A call boundary keeps obligation (a) — the invariant must hold at the
// call, an observer may see the receiver — but exact two-state knowledge
// survives when every call in the statement provably cannot WRITE the
// receiver's int fields: a non-mut-self method of the receiver's own class,
// no alias-capable parameter, and a transitively write-free body (the body
// check closes the shallow-mutability laundering doors).

#[test]
fn two_state_invariant_survives_readonly_self_call() {
    // The issue #454 repro: an immutable-self helper call must not havoc
    // the old() relations the return-site proof needs.
    let out = compile_and_run_stdout(
        r#"
class Node {
    current_term: int
    voted_for: int

    invariant self.current_term > old(self.current_term) || self.voted_for == old(self.voted_for) || old(self.voted_for) == 0

    fn helper(self) int {
        return self.current_term
    }

    fn go(mut self) int {
        let x = self.helper()
        self.current_term = self.current_term + 1
        return x
    }
}

fn main() {
    let mut n = Node { current_term: 1, voted_for: 0 }
    print(n.go())
    print(n.current_term)
}
"#,
    );
    assert_eq!(out.trim(), "1\n2");
}

#[test]
fn raft_shaped_dispatcher_proves_two_state_invariant() {
    // Term monotonicity as a two-state invariant on the node itself: the
    // dispatcher consults read-only helpers (which call each other) before
    // and after its writes. None of those calls may drop the relation.
    let out = compile_and_run_stdout(
        r#"
class Node {
    term: int
    commit: int

    invariant self.term >= old(self.term) && self.commit >= old(self.commit)

    fn current(self) int {
        return self.term
    }

    fn behind(self, t: int) bool {
        return self.current() < t
    }

    fn step(mut self, t: int) {
        let seen = self.current()
        if t > self.term {
            self.term = t
        }
        let late = self.behind(t)
        self.commit = self.commit + 1
        print(seen)
    }
}

fn main() {
    let mut n = Node { term: 3, commit: 0 }
    n.step(5)
    n.step(2)
    print(n.term)
    print(n.commit)
}
"#,
    );
    assert_eq!(out.trim(), "3\n5\n5\n2");
}

#[test]
fn ensures_survives_readonly_self_call() {
    let out = compile_and_run_stdout(
        r#"
class Counter {
    n: int

    fn peek(self) int {
        return self.n
    }

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        let seen = self.peek()
        print(seen)
    }
}

fn main() {
    let mut c = Counter { n: 41 }
    c.bump()
    print(c.n)
}
"#,
    );
    assert_eq!(out.trim(), "42\n42");
}

#[test]
fn two_state_survives_reach_free_free_function() {
    // Verify-and-align: a free function whose declared parameters cannot
    // reach the receiver's class was already a non-boundary (shared
    // call_severity classification) — exact two-state knowledge survives.
    let out = compile_and_run_stdout(
        r#"
fn double(v: int) int {
    return v * 2
}

class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
        let d = double(self.n)
        print(d)
    }
}

fn main() {
    let mut c = Counter { n: 20 }
    c.bump()
    print(c.n)
}
"#,
    );
    assert_eq!(out.trim(), "42\n21");
}

#[test]
fn mut_self_callee_still_havocs() {
    // Conservatism pin: a mut-self callee havocs two-state knowledge even
    // when its body happens to write nothing.
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn noop(mut self) {
    }

    fn stable(mut self) ensures self.x == old(self.x) {
        self.noop()
    }
}

fn main() {
    let mut c = C { x: 10 }
    c.stable()
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x == old(self.x)'",
    );
}

#[test]
fn alias_capable_param_callee_still_havocs() {
    // Conservatism pin: a callee taking a parameter of the receiver's own
    // class may be handed a writable alias of the receiver.
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn observe(self, other: C) int {
        return other.x
    }

    fn stable(mut self, o: C) ensures self.x == old(self.x) {
        let v = self.observe(o)
        print(v)
    }
}

fn main() {
    let mut c = C { x: 10 }
    let o = C { x: 1 }
    c.stable(o)
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x == old(self.x)'",
    );
}

#[test]
fn trait_dispatch_call_still_havocs() {
    // Conservatism pin: trait-dispatched calls stay opaque (the concrete
    // callee — and what it reaches — is unknown).
    compile_should_fail_with(
        r#"
trait Sink {
    fn accept(self, v: int)
}

class Console impl Sink {
    tag: int

    fn accept(self, v: int) {
        print(v)
    }
}

class C {
    x: int

    fn stable(mut self, s: Sink) ensures self.x == old(self.x) {
        s.accept(self.x)
    }
}

fn main() {
    let con = Console { tag: 0 }
    let mut c = C { x: 10 }
    c.stable(con)
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x == old(self.x)'",
    );
}

#[test]
fn laundered_alias_write_still_havocs() {
    // THE soundness pin for the refinement: binding mutability is shallow,
    // so an immutable-self callee can mint a writable alias of the receiver
    // (`let mut me = self`) and write through it. The signature alone must
    // not qualify the callee — the transitive body summary sees the foreign
    // write and keeps the full havoc. (Without it, `stable` would certify
    // `self.x == old(self.x)` while sneaky changes x at runtime.)
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn sneaky(self) {
        let mut me = self
        me.x = me.x - 5
    }

    fn stable(mut self) ensures self.x == old(self.x) {
        self.sneaky()
    }
}

fn main() {
    let mut c = C { x: 10 }
    c.stable()
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x == old(self.x)'",
    );
}

#[test]
fn transitive_laundered_mut_call_still_havocs() {
    // Same laundering door one call deeper: the innocent-looking
    // immutable-self callee re-binds the receiver mutably and calls a
    // mut-self method through the alias.
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn deep_write(mut self) {
        self.x = self.x + 1
    }

    fn looks_innocent(self) {
        let mut me = self
        me.deep_write()
    }

    fn stable(mut self) ensures self.x == old(self.x) {
        self.looks_innocent()
    }
}

fn main() {
    let mut c = C { x: 10 }
    c.stable()
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x == old(self.x)'",
    );
}

// ── Issue #455: guard-established ensures relations at branch joins ────────
//
// An ensures relation that EVERY surviving branch proves at its own end
// (under that branch's facts) holds of the joined state — the runtime join
// state is one of the survivors. This mirrors exactly how invariant
// checking treats joins; a branch that cannot prove the relation simply
// contributes nothing and the exit proof decides.

#[test]
fn ensures_guarded_monotonic_write_proves() {
    // The issue #455 repro: the only writing path is guarded by n > self.x,
    // the fall-through path leaves x unchanged — both satisfy >= old(x).
    let out = compile_and_run_stdout(
        r#"
class C {
    x: int

    fn bump(mut self, n: int) ensures self.x >= old(self.x) {
        if n > self.x {
            self.x = n
        }
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.bump(5)
    print(c.x)
    c.bump(3)
    print(c.x)
}
"#,
    );
    assert_eq!(out.trim(), "5\n5");
}

#[test]
fn ensures_guarded_with_else_branch_proves() {
    let out = compile_and_run_stdout(
        r#"
class C {
    x: int

    fn raise_to(mut self, n: int) ensures self.x >= old(self.x) {
        if n > self.x {
            self.x = n
        } else {
            self.x = self.x + 1
        }
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.raise_to(5)
    print(c.x)
    c.raise_to(2)
    print(c.x)
}
"#,
    );
    assert_eq!(out.trim(), "5\n6");
}

#[test]
fn ensures_guarded_match_arms_prove() {
    let out = compile_and_run_stdout(
        r#"
enum Cmd {
    Set { v: int }
    Bump
}

class C {
    x: int

    fn apply(mut self, cmd: Cmd) ensures self.x >= old(self.x) {
        match cmd {
            Cmd.Set { v } {
                if v > self.x {
                    self.x = v
                }
            }
            Cmd.Bump {
                self.x = self.x + 1
            }
        }
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.apply(Cmd.Set { v: 7 })
    print(c.x)
    c.apply(Cmd.Bump)
    print(c.x)
    c.apply(Cmd.Set { v: 2 })
    print(c.x)
}
"#,
    );
    assert_eq!(out.trim(), "7\n8\n8");
}

#[test]
fn ensures_write_after_guarded_join_proves() {
    // The join fact (x >= old(x), proven on both paths) composes with a
    // later unconditional write: x@join + 1 >= old(x) + 1 >= old(x).
    let out = compile_and_run_stdout(
        r#"
class C {
    x: int

    fn step(mut self, n: int) ensures self.x >= old(self.x) {
        if n > self.x {
            self.x = n
        }
        self.x = self.x + 1
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.step(5)
    print(c.x)
    c.step(0)
    print(c.x)
}
"#,
    );
    assert_eq!(out.trim(), "6\n7");
}

#[test]
fn ensures_failing_on_one_branch_still_errors() {
    // Conservatism pin: a branch that genuinely violates the relation (and
    // is never repaired) must still fail the exit proof.
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn bad(mut self, n: int) ensures self.x >= old(self.x) {
        if n > 0 {
            self.x = self.x + 1
        } else {
            self.x = self.x - 1
        }
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.bad(1)
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x >= old(self.x)'",
    );
}

#[test]
fn ensures_failing_guarded_write_still_errors() {
    // Else-less variant: the guarded path violates, the fall-through path
    // trivially holds — the join must not certify the relation.
    compile_should_fail_with(
        r#"
class C {
    x: int

    fn bad(mut self, n: int) ensures self.x >= old(self.x) {
        if n > 0 {
            self.x = self.x - 1
        }
    }
}

fn main() {
    let mut c = C { x: 1 }
    c.bad(1)
    print(c.x)
}
"#,
        "cannot prove ensures clause 'self.x >= old(self.x)'",
    );
}

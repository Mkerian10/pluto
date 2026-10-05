// Object construct phase 1 (docs/design/rfc-objects.md): entity semantics.

mod common;
use common::{
    compile_and_run_stdout, compile_and_run_stdout_timeout, compile_should_fail_with,
    compile_test_and_run_stdout,
};

/// Objects have reference identity: `==` compares identity, not structure.
#[test]
fn object_identity_equality() {
    let out = compile_and_run_stdout(
        r#"
        object Vault {
            secret: int
            fn reveal(self) int {
                return self.secret
            }
        }

        fn main() {
            let v1 = Vault { secret: 1 }
            let v2 = Vault { secret: 1 }
            let alias = v1
            print(v1 == v2)
            print(v1 == alias)
            print(v1.reveal())
        }
        "#,
    );
    assert_eq!(out.trim(), "false\ntrue\n1");
}

/// Spawn SHARES objects (an entity is one thing — copying would mint a second
/// identity), made safe by serialized methods: a spawned worker and the main
/// thread increment the same entity and no update is lost. The class version
/// of this program relies on the synchronized-singleton analysis; objects get
/// the guarantee unconditionally, and through an alias.
#[test]
fn object_spawn_shares_entity() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn work(mut self) {
                let mut i = 0
                while i < 1000 {
                    self.value = self.value + 1
                    i = i + 1
                }
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            let alias = c
            let t = spawn c.work()
            let mut i = 0
            while i < 1000 {
                c.increment()
                i = i + 1
            }
            t.get()
            print(c.get())
            print(alias.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "2000\n2000");
}

/// Top-level entities CROSS domain boundaries — as identity handles
/// (rfc-objects.md phase 2), never as copied values. The colocated plan
/// resolves the "handle" to the entity directly.
#[test]
fn object_crosses_boundary_as_handle() {
    let out = compile_and_run_stdout(
        r#"
        object Vault {
            secret: int
            fn reveal(self) int {
                return self.secret
            }
        }

        class PayService {
            fn check(self, v: Vault) int {
                return v.reveal()
            }
        }

        app A[pay: domain PayService] {
            fn main(self) {
                let v = Vault { secret: 5 }
                let r = at self.pay { check(v) } catch -1
                print(r)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

// ── Generic objects (rfc-objects.md phase 3, slice 1) ──
//
// Each monomorphized instantiation is a distinct entity TYPE: its own
// identity space, its own method lock, its own boundary interface hash.

/// A generic object declares, instantiates, and runs — each instantiation
/// keeps identity `==` (false for distinct instances of the same
/// instantiation, true through an alias) and mutation through an alias is
/// visible (one shared entity, not a copied value).
#[test]
fn generic_object_identity_per_instantiation() {
    let out = compile_and_run_stdout(
        r#"
        object Topic<T> {
            name: string
            count: int

            fn bump(mut self) {
                self.count = self.count + 1
            }
            fn get(self) int {
                return self.count
            }
        }

        fn main() {
            let mut a = Topic<int> { name: "a", count: 0 }
            let mut b = Topic<int> { name: "a", count: 0 }
            let alias = a
            print(a == b)
            print(a == alias)
            a.bump()
            print(alias.get())
            let s = Topic<string> { name: "s", count: 5 }
            print(s.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "false\ntrue\n1\n5");
}

/// Distinct instantiations are distinct types: passing a `Cell<string>`
/// where a `Cell<int>` is expected is a type error.
#[test]
fn generic_object_wrong_instantiation_is_type_error() {
    compile_should_fail_with(
        r#"
        object Cell<T> {
            v: T
        }

        fn take_int_cell(c: Cell<int>) int {
            return 1
        }

        fn main() {
            let c = Cell<string> { v: "x" }
            print(take_int_cell(c))
        }
        "#,
        "expected Cell<int>, found Cell<string>",
    );
}

/// Instantiations have separate identity spaces even for `==` itself:
/// comparing across instantiations is a type error, not `false`.
#[test]
fn generic_object_cross_instantiation_eq_is_type_error() {
    compile_should_fail_with(
        r#"
        object Cell<T> {
            v: T
        }

        fn main() {
            let a = Cell<int> { v: 1 }
            let mut b = Cell<string> { v: "x" }
            print(a == b)
        }
        "#,
        "cannot compare Cell<int> with Cell<string>",
    );
}

/// Spawn SHARES a generic-object instance exactly like a non-generic one,
/// and its methods are serialized (per-instantiation lock): concurrent
/// increments from a spawned task and the main thread lose no update.
#[test]
fn generic_object_spawn_shares_entity() {
    let out = compile_and_run_stdout(
        r#"
        object Counter<T> {
            label: T
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }
            fn work(mut self) {
                let mut i = 0
                while i < 1000 {
                    self.value = self.value + 1
                    i = i + 1
                }
            }
            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter<string> { label: "jobs", value: 0 }
            let alias = c
            let t = spawn c.work()
            let mut i = 0
            while i < 1000 {
                c.increment()
                i = i + 1
            }
            t.get()
            print(c.get())
            print(alias.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "2000\n2000");
}

/// The per-instantiation method lock also covers instantiations minted only
/// during MONOMORPHIZATION (inside a generic function body whose signature
/// never names the object) — the lock registration must not depend on the
/// instantiation being known at type-check time.
#[test]
fn generic_object_serialized_methods_mono_time_instantiation() {
    let out = compile_and_run_stdout(
        r#"
        object Cell<T> {
            v: T
            n: int

            fn bump(mut self) {
                self.n = self.n + 1
            }
            fn count(self) int {
                return self.n
            }
            fn work(mut self) {
                let mut i = 0
                while i < 1000 {
                    self.n = self.n + 1
                    i = i + 1
                }
            }
        }

        fn agg<T>(x: T) int {
            let mut c = Cell<T> { v: x, n: 0 }
            let t = spawn c.work()
            let mut i = 0
            while i < 1000 {
                c.bump()
                i = i + 1
            }
            t.get()
            return c.count()
        }

        fn main() {
            print(agg(9))
        }
        "#,
    );
    assert_eq!(out.trim(), "2000");
}

/// The entity GC tag survives monomorphization: a generic-object instance
/// nested inside a class value handed to spawn is NOT deep-copied with the
/// wrapper — both sides increment the same entity.
#[test]
fn generic_object_nested_in_spawned_class_stays_shared() {
    let out = compile_and_run_stdout(
        r#"
        object Counter<T> {
            tag: T
            value: int

            fn bump(mut self) {
                self.value = self.value + 1
            }
            fn get(self) int {
                return self.value
            }
        }

        class Holder {
            c: Counter<int>
            tag: int
        }

        fn work(mut h: Holder) {
            let mut i = 0
            while i < 500 {
                h.c.bump()
                i = i + 1
            }
        }

        fn main() {
            let mut shared = Counter<int> { tag: 7, value: 0 }
            let h = Holder { c: shared, tag: 1 }
            let t = spawn work(h)
            let mut i = 0
            while i < 500 {
                shared.bump()
                i = i + 1
            }
            t.get()
            print(shared.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "1000");
}

/// A generic-object instantiation crosses a domain boundary as an identity
/// handle, exactly like a non-generic entity — the colocated plan resolves
/// the handle to the entity directly.
#[test]
fn generic_object_crosses_boundary_as_handle() {
    let out = compile_and_run_stdout(
        r#"
        object Vault<T> {
            secret: T
            fn reveal(self) T {
                return self.secret
            }
        }

        class PayService {
            fn check(self, v: Vault<int>) int {
                return v.reveal()
            }
        }

        app A[pay: domain PayService] {
            fn main(self) {
                let v = Vault<int> { secret: 5 }
                let r = at self.pay { check(v) } catch -1
                print(r)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

/// Entity placement on a generic instantiation: `at v { m() }` runs where
/// the entity lives, with the mandatory boundary catch — plan-symmetric with
/// the routed case.
#[test]
fn generic_object_entity_placement_colocated() {
    let out = compile_and_run_stdout(
        r#"
        object Vault<T> {
            tag: T
            secret: int
            fn reveal(self) int {
                return self.secret
            }
            fn rotate(mut self) {
                self.secret = self.secret + 100
            }
        }

        fn main() {
            let mut v = Vault<string> { tag: "k", secret: 7 }
            let s1 = at v { reveal() } catch -2
            print(s1)
            at v { rotate() } catch err {}
            let s2 = at v { reveal() } catch -2
            print(s2)
        }
        "#,
    );
    assert_eq!(out.trim(), "7\n107");
}

/// Values nesting a generic-object instantiation stay untransferable — a
/// copy would fork the entity's identity, generic or not.
#[test]
fn generic_object_nested_in_value_rejected_at_boundary() {
    compile_should_fail_with(
        r#"
        object Vault<T> {
            secret: T
        }

        class Wrap {
            v: Vault<int>
        }

        class Escrow {
            fn hold(self, w: Wrap) int {
                return 1
            }
        }

        app A[esc: domain Escrow] {
            fn main(self) {
                let v = Vault<int> { secret: 1 }
                let w = Wrap { v: v }
                let r = at self.esc { hold(w) } catch -1
                print(r)
            }
        }
        "#,
        "a value containing an object cannot enter domain 'Escrow'",
    );
}

// ── Value/entity split: structural equality vs identity ──

/// Classes are values: == compares structure recursively — nested classes,
/// strings, arrays, maps, enums, and nullables included. Objects stay
/// identity-compared, including when nested inside compared values.
#[test]
fn value_equality_split() {
    let out = compile_and_run_stdout(
        r#"
        class Point {
            x: int
            y: int
        }

        class Line {
            a: Point
            b: Point
            label: string
        }

        enum Color {
            Red
            Rgb { r: int, g: int }
        }

        object Vault {
            secret: int
        }

        class Wrap {
            v: Vault
            tag: int
        }

        fn main() {
            print(Point { x: 1, y: 2 } == Point { x: 1, y: 2 })
            print(Point { x: 1, y: 2 } == Point { x: 1, y: 3 })
            let l1 = Line { a: Point { x: 1, y: 2 }, b: Point { x: 3, y: 4 }, label: "l" }
            let l2 = Line { a: Point { x: 1, y: 2 }, b: Point { x: 3, y: 4 }, label: "l" }
            print(l1 == l2)
            print(l1 != l2)

            let arr1 = [1, 2, 3]
            print(arr1 == [1, 2, 3])
            print(arr1 == [1, 2, 4])

            let mut m1 = Map<string, int> {}
            m1.insert("a", 1)
            let mut m2 = Map<string, int> {}
            m2.insert("a", 1)
            print(m1 == m2)

            print(Color.Rgb { r: 1, g: 2 } == Color.Rgb { r: 1, g: 2 })
            print(Color.Rgb { r: 1, g: 2 } == Color.Red)

            let v1 = Vault { secret: 9 }
            let v2 = Vault { secret: 9 }
            print(v1 == v2)
            print(v1 == v1)

            // Entities nested in compared VALUES keep identity semantics:
            // equal-state wrappers around different entities are not equal.
            let w1 = Wrap { v: v1, tag: 1 }
            let w2 = Wrap { v: v2, tag: 1 }
            let w3 = Wrap { v: v1, tag: 1 }
            print(w1 == w2)
            print(w1 == w3)

            let n1: int? = 5
            let n2: int? = 5
            let n3: int? = none
            print(n1 == n2)
            print(n1 == none)
            print(n3 == none)
        }
        "#,
    );
    assert_eq!(
        out.trim(),
        "true\nfalse\ntrue\nfalse\ntrue\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\nfalse\ntrue\ntrue\nfalse\ntrue"
    );
}

/// The entity GC tag makes sharing survive nesting: an object inside a class
/// value handed to spawn is NOT deep-copied with the wrapper — both sides
/// increment the same counter.
#[test]
fn object_nested_in_spawned_class_stays_shared() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            value: int
            fn bump(mut self) {
                self.value = self.value + 1
            }
            fn get(self) int {
                return self.value
            }
        }

        class Holder {
            c: Counter
            tag: int
        }

        fn work(mut h: Holder) {
            let mut i = 0
            while i < 500 {
                h.c.bump()
                i = i + 1
            }
        }

        fn main() {
            let mut shared = Counter { value: 0 }
            let h = Holder { c: shared, tag: 1 }
            let t = spawn work(h)
            let mut i = 0
            while i < 500 {
                shared.bump()
                i = i + 1
            }
            t.get()
            print(shared.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "1000");
}

/// Entity placement is plan-symmetric: `at v { m() }` on a LOCAL entity is a
/// direct call — the same expression that routes to a remote home when v is
/// a handle. The boundary contract (mandatory catch) applies identically.
#[test]
fn entity_placement_colocated() {
    let out = compile_and_run_stdout(
        r#"
        object Vault {
            secret: int
            fn reveal(self) int {
                return self.secret
            }
            fn rotate(mut self) {
                self.secret = self.secret + 100
            }
        }

        fn main() {
            let mut v = Vault { secret: 7 }
            let s1 = at v { reveal() } catch -2
            print(s1)
            at v { rotate() } catch err {}
            let s2 = at v { reveal() } catch -2
            print(s2)
        }
        "#,
    );
    assert_eq!(out.trim(), "7\n107");
}

// ── Per-instance entity locks ──
//
// Entity method serialization is per INSTANCE, not per type: each entity
// carries its own rwlock (hidden trailing slot of the allocation), so two
// instances of one object type run methods concurrently while concurrent
// calls to the SAME instance still serialize (object_spawn_shares_entity
// above covers the latter).

/// Two instances of one object type do NOT serialize against each other.
/// Deterministic distinguisher: a spawned task enters instance `a`'s method
/// (taking a's write lock), signals readiness, then blocks on a channel that
/// only instance `b`'s method sends to. Under a per-type lock b.push can
/// never start (deadlock — the timeout catches a regression); under
/// per-instance locks it completes and unblocks a.
#[test]
fn object_instances_do_not_serialize_against_each_other() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Relay {
            n: int

            fn hold_and_recv(mut self, ready: Sender<int>, data: Receiver<int>) int {
                ready.send(1)!
                let v = data.recv()!
                self.n = v
                return v
            }

            fn push(mut self, data: Sender<int>) {
                self.n = 1
                data.send(42)!
            }
        }

        fn main() {
            let (ready_tx, ready_rx) = chan<int>(1)
            let (data_tx, data_rx) = chan<int>(1)
            let mut a = Relay { n: 0 }
            let mut b = Relay { n: 0 }
            let t = spawn a.hold_and_recv(ready_tx, data_rx)
            ready_rx.recv()!
            b.push(data_tx)!
            print(t.get()!)
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "42");
}

/// Instances of the SAME generic-object instantiation get per-instance
/// behavior too: same channel-handshake distinguisher as above, with two
/// Relay<int> entities.
#[test]
fn generic_object_instances_do_not_serialize_against_each_other() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Relay<T> {
            item: T
            n: int

            fn hold_and_recv(mut self, ready: Sender<int>, data: Receiver<int>) int {
                ready.send(1)!
                let v = data.recv()!
                self.n = v
                return v
            }

            fn push(mut self, data: Sender<int>) {
                self.n = 1
                data.send(77)!
            }
        }

        fn main() {
            let (ready_tx, ready_rx) = chan<int>(1)
            let (data_tx, data_rx) = chan<int>(1)
            let mut a = Relay<int> { item: 0, n: 0 }
            let mut b = Relay<int> { item: 0, n: 0 }
            let t = spawn a.hold_and_recv(ready_tx, data_rx)
            ready_rx.recv()!
            b.push(data_tx)
            print(t.get())
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "77");
}

// ── Entity self-call reentrancy (rfc-objects.md open question 6, RESOLVED) ──
//
// A self-call is part of processing the SAME message: serialized methods
// mean one message at a time, not one stack frame. Two layers make it work:
// codegen skips the lock entirely when the receiver is literally `self`
// inside a method of the same entity class, and the runtime lock is
// owner-aware (same-thread reacquisition while write-held bumps a depth
// count), covering aliases and call chains that reenter the same instance.
// Before this, pthread rwlocks made these deadlock on glibc and silently
// break serialization on macOS (EDEADLK ignored; the nested unlock released
// the OUTER hold). Timeouts turn a deadlock regression into a test failure.

/// (a) A `mut self` method calls another `mut self` method on self.
#[test]
fn entity_self_call_mut_in_mut() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Counter {
            value: int

            fn bump(mut self) {
                self.value = self.value + 1
            }

            fn bump_twice(mut self) {
                self.bump()
                self.bump()
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.bump_twice()
            print(c.value)
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "2");
}

/// (b) A read method calls another read method on self.
#[test]
fn entity_self_call_read_in_read() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Counter {
            value: int

            fn get(self) int {
                return self.value
            }

            fn get_doubled(self) int {
                return self.get() * 2
            }
        }

        fn main() {
            let c = Counter { value: 21 }
            print(c.get_doubled())
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "42");
}

/// (c) A read method calling a `mut self` method on self is rejected at
/// type-check time — the write-within-read upgrade (which the owner-aware
/// lock does NOT support) is unreachable from the language.
#[test]
fn entity_read_method_cannot_call_mut_method() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int

            fn bump(mut self) {
                self.value = self.value + 1
            }

            fn weird(self) int {
                self.bump()
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            print(c.weird())
        }
        "#,
        "cannot call 'mut self' method 'bump' on self in a non-mut method",
    );
}

/// (d) Reentry through an ALIAS: a method passes self to a free function
/// that calls back into the same instance. The receiver is not literally
/// `self`, so the codegen fast path cannot fire — this exercises the
/// owner-aware runtime lock (same thread, write-held → depth bump).
#[test]
fn entity_self_call_through_alias() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Counter {
            value: int

            fn bump(mut self) {
                self.value = self.value + 1
            }

            fn via_helper(mut self) {
                poke(self)
            }

            fn get(self) int {
                return self.value
            }
        }

        fn poke(mut c: Counter) {
            c.bump()
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.via_helper()
            print(c.get())
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "1");
}

/// (e) Mutual recursion between two `mut self` methods (nested reacquisition
/// several levels deep — the depth count must balance).
#[test]
fn entity_self_call_mutual_recursion() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Walker {
            n: int

            fn even(mut self, k: int) bool {
                if k == 0 { return true }
                return self.odd(k - 1)
            }

            fn odd(mut self, k: int) bool {
                if k == 0 { return false }
                return self.even(k - 1)
            }
        }

        fn main() {
            let mut w = Walker { n: 0 }
            print(w.even(10))
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "true");
}

/// Reentry through an entity PLACEMENT on an alias: `at other { get() }`
/// where `other` is the same instance the running method holds the write
/// lock on. The colocated plan is a direct locked call — same owner-aware
/// reacquisition, through the `at` lowering path.
#[test]
fn entity_placement_reenters_same_instance() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Node {
            tag: int

            fn get(self) int {
                return self.tag
            }

            fn probe(mut self, other: Node) int {
                self.tag = self.tag + 1
                return at other { get() } catch -1
            }
        }

        fn main() {
            let mut n = Node { tag: 10 }
            print(n.probe(n))
        }
        "#,
        30,
    );
    assert_eq!(out.trim(), "11");
}

/// Same-instance cross-THREAD serialization still holds with self-calling
/// methods: racing increments through a self-call double-bump lose no
/// updates. (Reentrancy is same-thread only; distinct threads still
/// exclude each other.)
#[test]
fn entity_self_calls_keep_cross_thread_serialization() {
    let out = compile_and_run_stdout_timeout(
        r#"
        object Counter {
            value: int

            fn bump(mut self) {
                self.value = self.value + 1
            }

            fn bump_twice(mut self) {
                self.bump()
                self.bump()
            }

            fn work(mut self) {
                let mut i = 0
                while i < 500 {
                    self.bump_twice()
                    i = i + 1
                }
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            let t = spawn c.work()
            let mut i = 0
            while i < 500 {
                c.bump_twice()
                i = i + 1
            }
            t.get()
            print(c.get())
        }
        "#,
        60,
    );
    assert_eq!(out.trim(), "2000");
}

/// `pluto test` binaries with objects link and run: the test-mode runtime
/// provides no-op lock stubs (the fiber scheduler is single-threaded), and
/// entity allocations still carry the hidden lock slot.
#[test]
fn object_methods_run_in_test_mode() {
    let out = compile_test_and_run_stdout(
        r#"
        object Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn get(self) int {
                return self.value
            }
        }

        test "entities work in test mode" {
            let mut c = Counter { value: 0 }
            let d = Counter { value: 10 }
            c.increment()
            assert c.get() == 1
            assert d.get() == 10
        }
        "#,
    );
    assert!(out.contains("1 tests passed"), "unexpected output: {out}");
}

// ── External field writes are rejected (#427) ──────────────────────────────
//
// The object construct's concurrency contract is per-instance METHOD
// serialization: spawn may share an entity precisely because its methods run
// one at a time. An external `e.field = ...` is a plain store that takes no
// lock, so it races any concurrently executing method. Before the rejection,
// this program demonstrated classic lost updates (printing e.g. 110611
// instead of 200000):
//
//     object Counter {
//         value: int
//         fn work(mut self) {
//             let mut i = 0
//             while i < 100000 {
//                 self.value = self.value + 1
//                 i = i + 1
//             }
//         }
//         fn get(self) int { return self.value }
//     }
//
//     fn main() {
//         let mut c = Counter { value: 0 }
//         let t = spawn c.work()
//         let mut i = 0
//         while i < 100000 {
//             c.value = c.value + 1      // external write — took no lock
//             i = i + 1
//         }
//         t.get()
//         print(c.get())
//     }
//
// That shape can no longer be a runtime test: the external write is a type
// error. The tests below pin the rejection in each syntactic form.

/// Direct external field write on an entity is a compile error.
#[test]
fn object_external_field_write_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int
            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.value = 1
            print(c.get())
        }
        "#,
        "cannot assign to field 'value' of entity 'Counter' from outside its own methods",
    );
}

/// Compound assignment desugars to a field write — same rejection.
#[test]
fn object_external_compound_assign_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int
            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.value += 1
            print(c.get())
        }
        "#,
        "cannot assign to field 'value' of entity 'Counter' from outside its own methods",
    );
}

/// An alias binding doesn't launder the write: entities have reference
/// identity, so the alias IS the entity.
#[test]
fn object_external_write_through_alias_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int
            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            let mut alias = c
            alias.value = 7
            print(c.get())
        }
        "#,
        "cannot assign to field 'value' of entity 'Counter' from outside its own methods",
    );
}

/// A write reached through a field chain (value class holding the entity)
/// is still an external entity-field write.
#[test]
fn object_external_write_through_field_chain_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int
            fn get(self) int {
                return self.value
            }
        }

        class Wrap {
            c: Counter
        }

        fn main() {
            let c = Counter { value: 0 }
            let mut w = Wrap { c: c }
            w.c.value = 5
            print(c.get())
        }
        "#,
        "cannot assign to field 'value' of entity 'Counter' from outside its own methods",
    );
}

/// Writes from ANOTHER entity's methods are still external: only the
/// entity's own `self` may write its fields.
#[test]
fn object_write_from_other_entity_method_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            value: int
        }

        object Poker {
            fn poke(self, c: Counter) {
                c.value = 3
            }
        }

        fn main() {
            let c = Counter { value: 0 }
            let p = Poker {}
            p.poke(c)
        }
        "#,
        "cannot assign to field 'value' of entity 'Counter' from outside its own methods",
    );
}

/// The legal mutation paths keep working: `self` writes inside the entity's
/// own methods, and external READS of entity fields (reads already route
/// through the normal field-load path and stay legal).
#[test]
fn object_self_writes_and_external_reads_still_work() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
                self.value += 1
            }
        }

        fn main() {
            let mut c = Counter { value: 0 }
            c.increment()
            print(c.value)
        }
        "#,
    );
    assert_eq!(out.trim(), "2");
}

/// Channels SHARE entities (#429): where channel send deep-copies value
/// payloads, an object crosses as a shared identity handle — mutation through
/// the received handle is visible to the sender, and `==` identity holds.
#[test]
fn object_channel_send_shares_entity() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn get(self) int {
                return self.value
            }
        }

        fn main() {
            let (tx, rx) = chan<Counter>(1)
            let mut c = Counter { value: 10 }
            tx.send(c)!
            let mut got = rx.recv()!
            print(got == c)
            got.increment()
            print(c.get())
            c.increment()
            print(got.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "true\n11\n12");
}

/// A VALUE sent over a channel is deep-copied, but an entity nested inside it
/// stays shared (__pluto_deep_copy shares GC_TAG_ENTITY): the copy is a fresh
/// class shell whose entity field is the same identity as the original's.
#[test]
fn object_nested_in_sent_value_stays_shared() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            value: int

            fn increment(mut self) {
                self.value = self.value + 1
            }

            fn get(self) int {
                return self.value
            }
        }

        class Wrapper {
            label: int
            counter: Counter
        }

        fn main() {
            let (tx, rx) = chan<Wrapper>(1)
            let c = Counter { value: 0 }
            let mut w = Wrapper { label: 1, counter: c }
            tx.send(w)!
            w.label = 100
            let mut got = rx.recv()!
            print(got.label)
            print(got.counter == c)
            got.counter.increment()
            print(c.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "1\ntrue\n1");
}

// ── External writes through entity container fields are rejected (#440 gap 2)
//
// #427 rejected `e.field = v` from outside the entity, but an IndexAssign's
// immediate object is the CONTAINER, not the entity, so `e.arr[0] = 1`
// slipped past it — mutating entity state without the per-instance lock.
// The tests below pin the extended rule: any lvalue path whose root
// traverses an entity field through an index from outside the entity is
// rejected; `self.` forms inside the entity's methods (which hold the lock)
// and the same shapes on plain value classes stay legal.

/// Array-index write through an entity field from outside is a compile
/// error.
#[test]
fn object_external_index_assign_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            arr: [int]
            fn get(self, i: int) int {
                return self.arr[i]
            }
        }

        fn main() {
            let mut c = Counter { arr: [1, 2, 3] }
            c.arr[0] = 9
            print(c.get(0))
        }
        "#,
        "cannot assign through field 'arr' of entity 'Counter' from outside its own methods",
    );
}

/// Map-key write through an entity field from outside is rejected the same
/// way.
#[test]
fn object_external_map_key_assign_rejected() {
    compile_should_fail_with(
        r#"
        object Registry {
            m: Map<string, int>
        }

        fn main() {
            let mut r = Registry { m: Map<string, int> { "a": 1 } }
            r.m["a"] = 2
        }
        "#,
        "cannot assign through field 'm' of entity 'Registry' from outside its own methods",
    );
}

/// Nested lvalue path: a field write on an ELEMENT of an entity's array
/// (`e.a[0].b = v`) still writes entity state — rejected.
#[test]
fn object_external_nested_path_assign_rejected() {
    compile_should_fail_with(
        r#"
        class Point {
            x: int
        }

        object Board {
            pts: [Point]
        }

        fn main() {
            let b = Board { pts: [Point { x: 0 }] }
            b.pts[0].x = 5
        }
        "#,
        "cannot read field 'pts' of entity 'Board' from outside its own methods",
    );
}

/// Nested containers: `e.arr[0][1] = v` is rejected too.
#[test]
fn object_external_nested_container_assign_rejected() {
    compile_should_fail_with(
        r#"
        object Grid {
            rows: [[int]]
        }

        fn main() {
            let g = Grid { rows: [[1, 2], [3, 4]] }
            g.rows[0][1] = 9
        }
        "#,
        "cannot assign through field 'rows' of entity 'Grid' from outside its own methods",
    );
}

/// Compound assignment desugars to an index assign — same rejection.
#[test]
fn object_external_index_compound_assign_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            arr: [int]
        }

        fn main() {
            let mut c = Counter { arr: [1] }
            c.arr[0] += 1
        }
        "#,
        "cannot assign through field 'arr' of entity 'Counter' from outside its own methods",
    );
}

/// Increment statements desugar the same way — rejected.
#[test]
fn object_external_index_increment_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            arr: [int]
        }

        fn main() {
            let mut c = Counter { arr: [1] }
            c.arr[0]++
        }
        "#,
        "cannot assign through field 'arr' of entity 'Counter' from outside its own methods",
    );
}

/// An alias binding doesn't launder the indexed write: entities have
/// reference identity, so the alias IS the entity.
#[test]
fn object_external_index_assign_through_alias_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            arr: [int]
        }

        fn main() {
            let mut c = Counter { arr: [1] }
            let mut alias = c
            alias.arr[0] = 7
        }
        "#,
        "cannot assign through field 'arr' of entity 'Counter' from outside its own methods",
    );
}

/// An entity nested inside a value class reached via a local: the path
/// still traverses the entity's field, so the indexed write is rejected —
/// matching #427's field-chain rule (`w.c.value = 5` is rejected the same
/// way).
#[test]
fn object_index_assign_through_value_wrapper_rejected() {
    compile_should_fail_with(
        r#"
        object Counter {
            arr: [int]
        }

        class Wrap {
            c: Counter
        }

        fn main() {
            let c = Counter { arr: [1] }
            let mut w = Wrap { c: c }
            w.c.arr[0] = 5
        }
        "#,
        "cannot assign through field 'arr' of entity 'Counter' from outside its own methods",
    );
}

/// The legal paths keep working: `self.` indexed writes inside the entity's
/// own methods (the method holds the lock), and external READS of entity
/// container fields.
#[test]
fn object_self_index_writes_still_work() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            arr: [int]
            m: Map<string, int>
            rows: [[int]]

            fn bump(mut self) {
                self.arr[0] = self.arr[0] + 1
                self.arr[0] += 1
                self.m["k"] = 7
                self.rows[0][1] = 9
            }

            fn a0(self) int { return self.arr[0] }
            fn mk(self) int { return self.m["k"] }
            fn r01(self) int { return self.rows[0][1] }
        }

        fn main() {
            let mut c = Counter {
                arr: [0],
                m: Map<string, int> {},
                rows: [[1, 2]]
            }
            c.bump()
            // Verification goes through methods: external reads of
            // reference-shaped entity fields are rejected (section 3).
            print(c.a0())
            print(c.mk())
            print(c.r01())
        }
        "#,
    );
    assert_eq!(out.trim(), "2\n7\n9");
}

/// A plain value CLASS with the same shapes is unaffected: indexed writes
/// through class fields are ordinary container mutation.
#[test]
fn class_index_assign_shapes_unaffected() {
    let out = compile_and_run_stdout(
        r#"
        class Point {
            x: int
        }

        class Bag {
            arr: [int]
            m: Map<string, int>
            pts: [Point]
        }

        fn main() {
            let mut b = Bag {
                arr: [1],
                m: Map<string, int> { "a": 1 },
                pts: [Point { x: 0 }]
            }
            b.arr[0] = 9
            b.arr[0] += 1
            b.m["a"] = 2
            b.pts[0].x = 5
            print(b.arr[0])
            print(b.m["a"])
            print(b.pts[0].x)
        }
        "#,
    );
    assert_eq!(out.trim(), "10\n2\n5");
}

// ── Entity field reads (rfc-module-semantics.md section 3) ──────────────────
//
// Writes never (pinned above, #427/#432); scalar-shaped reads are allowed
// and loaded under the instance read lock; reference-shaped reads go
// through methods returning deliberate copies; cross-field consistency is
// a method, by doctrine.

/// Scalar-shaped fields read externally: int, string, unit enum, nullable
/// scalar, and an entity handle (identity is meant to be shared).
#[test]
fn entity_scalar_field_reads_allowed() {
    let out = compile_and_run_stdout(
        r#"
        enum Mode {
            Idle
            Busy
        }

        object Peer {
            id: int
        }

        object Server {
            port: int
            name: string
            mode: Mode
            limit: int?
            peer: Peer

            fn noop(self) int {
                return 0
            }
        }

        fn main() {
            let s = Server { port: 8080, name: "api", mode: Mode.Idle, limit: none, peer: Peer { id: 7 } }
            print(s.port)
            print(s.name)
            let l = s.limit ?? 0 - 1
            print(l)
            print(s.peer.id)
        }
        "#,
    );
    assert_eq!(out, "8080\napi\n-1\n7\n");
}

/// Reference-shaped fields are unreadable from outside: the raw read would
/// hand out a live reference into lock-serialized state.
#[test]
fn entity_array_field_read_rejected() {
    compile_should_fail_with(
        r#"
        object Log {
            entries: [string]
            fn add(mut self, e: string) { self.entries.push(e) }
        }

        fn main() {
            let l = Log { entries: [] }
            print(l.entries.len())
        }
        "#,
        "reference-shaped",
    );
}

#[test]
fn entity_map_field_read_rejected() {
    compile_should_fail_with(
        r#"
        object Registry {
            names: Map<string, int>
        }

        fn main() {
            let r = Registry { names: Map<string, int> {} }
            let n = r.names
            print(1)
        }
        "#,
        "reference-shaped",
    );
}

#[test]
fn entity_bytes_field_read_rejected() {
    compile_should_fail_with(
        r#"
        object Buffer {
            data: bytes
        }

        fn main() {
            let b = Buffer { data: bytes_new() }
            print(b.data.len())
        }
        "#,
        "reference-shaped",
    );
}

/// A value-class field is the `e.cfg.x = 1` aliasing hole: rejected.
#[test]
fn entity_value_class_field_read_rejected() {
    compile_should_fail_with(
        r#"
        class Config {
            retries: int
        }

        object Service {
            cfg: Config
        }

        fn main() {
            let s = Service { cfg: Config { retries: 3 } }
            print(s.cfg.retries)
        }
        "#,
        "reference-shaped",
    );
}

/// Data-carrying enums are heap state; unit enums are tags (allowed above).
#[test]
fn entity_data_enum_field_read_rejected() {
    compile_should_fail_with(
        r#"
        enum State {
            Empty
            Holding { what: string }
        }

        object Slot {
            state: State
        }

        fn main() {
            let s = Slot { state: State.Empty }
            let st = s.state
            print(1)
        }
        "#,
        "reference-shaped",
    );
}

/// Inside the entity's own methods, self reads of reference-shaped fields
/// are untouched — the method holds the lock and the copy discipline is
/// the method author's to apply.
#[test]
fn entity_self_reads_unrestricted_in_methods() {
    let out = compile_and_run_stdout(
        r#"
        object Log {
            entries: [string]

            fn add(mut self, e: string) {
                self.entries.push(e)
            }

            fn count(self) int {
                return self.entries.len()
            }
        }

        fn main() {
            let mut l = Log { entries: [] }
            l.add("a")
            l.add("b")
            print(l.count())
        }
        "#,
    );
    assert_eq!(out, "2\n");
}

/// The owner-aware lock: a method reading a scalar field through an alias
/// of its own instance re-acquires on the same thread and proceeds.
#[test]
fn entity_alias_to_self_scalar_read_in_method() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            count: int

            fn read_via(self, other: Counter) int {
                return other.count
            }
        }

        fn main() {
            let c = Counter { count: 41 }
            print(c.read_via(c))
        }
        "#,
    );
    assert_eq!(out, "41\n");
}

/// Scalar reads after spawned mutation observe the joined state — the
/// locked read orders with the serialized methods' stores.
#[test]
fn entity_scalar_read_after_spawned_mutation() {
    let out = compile_and_run_stdout(
        r#"
        object Counter {
            count: int

            fn bump_n(mut self, n: int) {
                let mut i = 0
                while i < n {
                    self.count = self.count + 1
                    i = i + 1
                }
            }
        }

        fn work(mut c: Counter) int {
            c.bump_n(1000)
            return 0
        }

        fn main() {
            let c = Counter { count: 0 }
            let t = spawn work(c)
            let u = spawn work(c)
            t.get()
            u.get()
            print(c.count)
        }
        "#,
    );
    assert_eq!(out, "2000\n");
}

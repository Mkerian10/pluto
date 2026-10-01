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
            let b = Topic<int> { name: "a", count: 0 }
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
            let b = Cell<string> { v: "x" }
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

            let m1 = Map<string, int> {}
            m1.insert("a", 1)
            let m2 = Map<string, int> {}
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

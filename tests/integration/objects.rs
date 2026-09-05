// Object construct phase 1 (docs/design/rfc-objects.md): entity semantics.

mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

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

/// Objects cannot cross a domain boundary as values — an entity crosses by
/// reference (identity handles, a later phase), never by copy.
#[test]
fn object_rejected_at_domain_boundary() {
    compile_should_fail_with(
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
        "an object cannot enter domain 'PayService' as a value",
    );
}

/// Generic objects are a later phase — rejected with a pointer to the RFC.
#[test]
fn generic_object_rejected() {
    compile_should_fail_with(
        r#"
        object Topic<T> {
            name: string
        }

        fn main() {
            let x = 1
            print(x)
        }
        "#,
        "object 'Topic' cannot have type parameters yet",
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

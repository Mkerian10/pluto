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

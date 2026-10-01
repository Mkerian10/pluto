//! `guarded_by` dominance proofs (properties RFC atom 3, phase 3).
//!
//! A field clause `data: bytes guarded_by (g: WriteGrant) g.token ==
//! self.epoch` obligates every write site of the field to be dominated by a
//! conditional the fact engine proves implies the predicate for some
//! in-scope value of the binder type. See src/typeck/dominance.rs.

mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Shared scaffolding ───────────────────────────────────────────────────────

/// The fence shape from examples/blob: guard + write in one serialized
/// method, with the token read through a local.
const FENCE_LOCAL: &str = r#"
error StaleGrant {
    token: int
    epoch: int
}

class WriteGrant {
    token: int
}

object BlobAuthority {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int
    applied: int

    invariant self.epoch >= 0
    invariant self.applied <= self.epoch

    fn grant_write(mut self) WriteGrant {
        self.epoch = self.epoch + 1
        return WriteGrant { token: self.epoch }
    }

    fn apply(mut self, grant: WriteGrant, d: string) {
        let tok = grant.token
        if tok != self.epoch {
            raise StaleGrant { token: tok, epoch: self.epoch }
        }
        self.applied = tok
        self.data = d.to_bytes()
    }

    fn read(self) string {
        return self.data.to_string()
    }
}

fn main() {
    let mut blob = BlobAuthority { data: "genesis".to_bytes(), epoch: 0, applied: 0 }
    let g = blob.grant_write()
    blob.apply(g, "fenced write") catch err {
        print("unexpected rejection")
    }
    print(blob.read())
}
"#;

// ── Proven: fence patterns discharge ─────────────────────────────────────────

#[test]
fn fence_through_local_discharges() {
    // The blob acceptance shape: `let tok = grant.token` then a raise-guard
    // on `tok`, with an interleaved write to another field and a builtin
    // call in the written value — all must survive to the write.
    let out = compile_and_run_stdout(FENCE_LOCAL);
    assert_eq!(out, "fenced write\n");
}

#[test]
fn fence_direct_field_compare_discharges() {
    // Guard phrased directly on the binder's field, no temp local.
    let out = compile_and_run_stdout(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
        self.data = d.to_bytes()
    }

    fn read(self) string {
        return self.data.to_string()
    }
}

fn main() {
    let mut s = Store { data: "".to_bytes(), epoch: 0 }
    s.apply(WriteGrant { token: 0 }, "direct") catch err {
        print("rejected")
    }
    print(s.read())
}
"#,
    );
    assert_eq!(out, "direct\n");
}

#[test]
fn fence_if_else_shape_discharges() {
    // The if-else shape: write in the then-branch of an equality check.
    let out = compile_and_run_stdout(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token == self.epoch {
            self.data = d.to_bytes()
        } else {
            print("stale, skipped")
        }
    }

    fn read(self) string {
        return self.data.to_string()
    }
}

fn main() {
    let mut s = Store { data: "".to_bytes(), epoch: 0 }
    s.apply(WriteGrant { token: 0 }, "branched")
    print(s.read())
}
"#,
    );
    assert_eq!(out, "branched\n");
}

#[test]
fn fence_inequality_predicate_discharges() {
    // A non-equality predicate (`g.token >= self.epoch`) proven from a
    // `<`-shaped raise-guard: fall-through of `tok < epoch → raise` gives
    // `tok >= epoch`.
    let out = compile_and_run_stdout(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token >= self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token < self.epoch {
            raise MathError { message: "stale" }
        }
        self.data = d.to_bytes()
    }

    fn read(self) string {
        return self.data.to_string()
    }
}

fn main() {
    let mut s = Store { data: "".to_bytes(), epoch: 0 }
    s.apply(WriteGrant { token: 3 }, "monotone") catch err {
        print("rejected")
    }
    print(s.read())
}
"#,
    );
    assert_eq!(out, "monotone\n");
}

#[test]
fn guard_inside_loop_discharges() {
    // The guard dominates within the same loop iteration.
    let out = compile_and_run_stdout(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply_many(mut self, grant: WriteGrant, d: string) {
        for i in 0..3 {
            if grant.token != self.epoch {
                raise MathError { message: "stale" }
            }
            self.data = d.to_bytes()
        }
    }

    fn read(self) string {
        return self.data.to_string()
    }
}

fn main() {
    let mut s = Store { data: "".to_bytes(), epoch: 0 }
    s.apply_many(WriteGrant { token: 0 }, "looped") catch err {
        print("rejected")
    }
    print(s.read())
}
"#,
    );
    assert_eq!(out, "looped\n");
}

#[test]
fn guarded_field_on_value_class_discharges() {
    // guarded_by is accepted on plain classes too (values don't share, so
    // no concurrent writer can race the check — see dominance.rs docs).
    let out = compile_and_run_stdout(
        r#"
class Grant {
    token: int
}

class Cell {
    value: int guarded_by (g: Grant) g.token == self.epoch
    epoch: int

    fn set(mut self, grant: Grant, v: int) {
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
        self.value = v
    }
}

fn main() {
    let mut c = Cell { value: 0, epoch: 1 }
    c.set(Grant { token: 1 }, 42) catch err {
        print("rejected")
    }
    print(c.value)
}
"#,
    );
    assert_eq!(out, "42\n");
}

// ── Unproven: dominance violations ───────────────────────────────────────────

#[test]
fn write_without_check_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        self.data = d.to_bytes()
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn check_after_write_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        self.data = d.to_bytes()
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn check_in_wrong_branch_rejected() {
    // The write sits on the branch where the predicate is REFUTED.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token == self.epoch {
            print("ok")
        } else {
            self.data = d.to_bytes()
        }
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn check_killed_by_interleaved_call_rejected() {
    // A user-code call between check and write may mutate the compared
    // fields (entities even allow reentrant self-calls): facts die.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn bump(mut self) {
        self.epoch = self.epoch + 1
    }

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
        self.bump()
        self.data = d.to_bytes()
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn check_killed_by_write_to_compared_field_rejected() {
    // Writing the compared field between check and write kills the fact.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
        self.epoch = self.epoch + 1
        self.data = d.to_bytes()
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn guard_outside_loop_write_inside_rejected() {
    // Conservative loop rule: facts do not survive loop entry.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise MathError { message: "stale" }
        }
        for i in 0..3 {
            self.data = d.to_bytes()
        }
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

#[test]
fn no_binder_value_in_scope_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn clobber(mut self, d: string) {
        self.data = d.to_bytes()
    }
}

fn main() { }
"#,
        "no value of the binder type 'WriteGrant' is in scope",
    );
}

#[test]
fn stale_check_of_reassigned_binder_rejected() {
    // Reassigning the binder variable kills the facts its guard proved.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn apply(mut self, grant: WriteGrant, d: string) {
        let mut g = grant
        if g.token != self.epoch {
            raise MathError { message: "stale" }
        }
        g = WriteGrant { token: 0 }
        self.data = d.to_bytes()
    }
}

fn main() { }
"#,
        "cannot prove guard",
    );
}

// ── Foreign writes ───────────────────────────────────────────────────────────

#[test]
fn foreign_write_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int
}

fn main() {
    let mut s = Store { data: "x".to_bytes(), epoch: 0 }
    s.data = "y".to_bytes()
}
"#,
        "may only be written through 'self' inside the class's own methods",
    );
}

#[test]
fn foreign_write_with_local_check_still_rejected() {
    // Even a "correct-looking" caller-side check does not license a foreign
    // write: guarded fields are protocol-internal.
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn epoch_now(self) int {
        return self.epoch
    }
}

fn clobber(mut s: Store, g: WriteGrant) {
    if g.token == s.epoch_now() {
        s.data = "y".to_bytes()
    }
}

fn main() { }
"#,
        "may only be written through 'self' inside the class's own methods",
    );
}

// ── Declaration-time validation ──────────────────────────────────────────────

#[test]
fn guard_on_generic_class_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

class Box<T> {
    data: T guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int
}

fn main() { }
"#,
        "guarded_by on generic classes is not yet supported",
    );
}

#[test]
fn out_of_fragment_predicate_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
    name: string
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.name == self.tag
    tag: string
}

fn main() { }
"#,
        "outside the provable fragment",
    );
}

#[test]
fn entity_binder_rejected() {
    compile_should_fail_with(
        r#"
object Registry {
    epoch: int
}

object Store {
    data: bytes guarded_by (g: Registry) g.epoch == self.epoch
    epoch: int
}

fn main() { }
"#,
        "is an object (entity)",
    );
}

#[test]
fn unknown_binder_type_rejected() {
    compile_should_fail_with(
        r#"
object Store {
    data: bytes guarded_by (g: Nonexistent) g.token == self.epoch
    epoch: int
}

fn main() { }
"#,
        "is not a declared class",
    );
}

#[test]
fn non_bool_predicate_rejected() {
    compile_should_fail_with(
        r#"
class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token + self.epoch
    epoch: int
}

fn main() { }
"#,
        "guarded_by predicate must be bool",
    );
}

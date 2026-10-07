//! Collection length invariants via derived ghost fields
//! (rfc-number-types.md §4). A class invariant may constrain the length of
//! its own array/map/set/bytes field through `self.<f>.len()`; the compiler
//! models that length as a ghost int updated by the builtin mutators as
//! transfer functions, and proves the invariant at every boundary. The four
//! aliasing doors keep the model sound: fresh-only construction/assignment,
//! no external read, no `mut`-parameter passing. STRICT: an Unknown verdict
//! is a compile error.

mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

// ─────────────────────────────────────────────────────────────────────────────
// Discharging positives
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn push_preserves_non_empty() {
    // push is +1, so len > 0 trivially survives it.
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn append(mut self, v: int) {
        self.segs.push(v)
    }
    fn count(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut l = Log { segs: [0] }
    l.append(1)
    l.append(2)
    print(l.count())
}
"#,
    );
    assert_eq!(out, "3\n");
}

#[test]
fn guarded_pop_preserves_non_empty() {
    // The Styx shape: invariant len > 0, a pop guarded by len > 1.
    let out = compile_and_run_stdout(
        r#"
class SegmentIndex {
    segs: [int]
    invariant self.segs.len() > 0
    fn append(mut self, v: int) {
        self.segs.push(v)
    }
    fn pop_guarded(mut self) {
        if self.segs.len() > 1 {
            self.segs.pop()
        }
    }
    fn count(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut s = SegmentIndex { segs: [0] }
    s.append(1)
    s.append(2)
    s.pop_guarded()
    print(s.count())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn segment_index_compact_worked_example() {
    // RFC worked example: a SegmentIndex that compacts by repeatedly popping a
    // trailing element while more than one remains. Each pop is dominated by a
    // `len() > 1` guard, so `invariant self.segs.len() > 0` holds throughout.
    let out = compile_and_run_stdout(
        r#"
class SegmentIndex {
    segs: [int]
    invariant self.segs.len() > 0

    fn append(mut self, base: int) {
        self.segs.push(base)
    }

    fn compact(mut self) {
        while self.segs.len() > 1 {
            self.segs.pop()
        }
    }

    fn len(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut idx = SegmentIndex { segs: [0] }
    idx.append(10)
    idx.append(20)
    idx.append(30)
    idx.compact()
    print(idx.len())
}
"#,
    );
    assert_eq!(out, "1\n");
}

#[test]
fn clear_then_refill() {
    // clear sets the length to 0; a following push makes len > 0 provable
    // again before the method boundary.
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn reset(mut self, v: int) {
        self.segs.clear()
        self.segs.push(v)
    }
    fn count(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut l = Log { segs: [1, 2, 3] }
    l.reset(9)
    print(l.count())
}
"#,
    );
    assert_eq!(out, "1\n");
}

#[test]
fn map_insert_monotonic() {
    // map insert is modeled as a monotone non-decrease, so a `len >= 0`
    // invariant (and any lower bound already established) survives it.
    let out = compile_and_run_stdout(
        r#"
class Reg {
    ids: Map<string, int>
    invariant self.ids.len() >= 0
    fn add(mut self, k: string, v: int) {
        self.ids.insert(k, v)
    }
    fn size(self) int {
        return self.ids.len()
    }
}
fn main() {
    let mut r = Reg { ids: Map<string, int> {} }
    r.add("a", 1)
    r.add("b", 2)
    print(r.size())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn set_insert_preserves_non_empty() {
    let out = compile_and_run_stdout(
        r#"
class Tags {
    xs: Set<int>
    invariant self.xs.len() > 0
    fn add(mut self, v: int) {
        self.xs.insert(v)
    }
    fn count(self) int {
        return self.xs.len()
    }
}
fn main() {
    let mut t = Tags { xs: Set<int> { 1 } }
    t.add(2)
    print(t.count())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn bytes_push_preserves_non_empty() {
    let out = compile_and_run_stdout(
        r#"
class Buf {
    data: bytes
    invariant self.data.len() > 0
    fn put(mut self, b: byte) {
        self.data.push(b)
    }
    fn size(self) int {
        return self.data.len()
    }
}
fn main() {
    let mut b = Buf { data: bytes_filled(1, 0 as byte) }
    b.put(7 as byte)
    print(b.size())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn index_assign_leaves_length_unchanged() {
    // `xs[i] = v` does not change the length; the invariant still holds.
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn set_first(mut self, v: int) {
        self.segs[0] = v
    }
    fn count(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut l = Log { segs: [1, 2] }
    l.set_first(9)
    print(l.count())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn generic_class_collection_invariant() {
    let out = compile_and_run_stdout(
        r#"
class Stack<T> {
    items: [T]
    invariant self.items.len() > 0
    fn push_one(mut self, v: T) {
        self.items.push(v)
    }
    fn size(self) int {
        return self.items.len()
    }
}
fn main() {
    let mut s = Stack<int> { items: [1] }
    s.push_one(2)
    print(s.size())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn reassign_from_literal_is_fresh() {
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn replace(mut self) {
        self.segs = [1, 2, 3]
    }
    fn count(self) int {
        return self.segs.len()
    }
}
fn main() {
    let mut l = Log { segs: [0] }
    l.replace()
    print(l.count())
}
"#,
    );
    assert_eq!(out, "3\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Unknown / Refuted verdicts — the fix-suggesting diagnostics
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unguarded_pop_cannot_prove_non_empty() {
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn drop_one(mut self) {
        self.segs.pop()
    }
}
fn main() { print(0) }
"#,
        "cannot prove invariant 'self.segs.len() > 0'",
    );
}

#[test]
fn pop_from_possibly_empty_is_rejected() {
    // A `len >= 0` invariant does not prove the collection is non-empty, so
    // the removal itself is rejected (the −1 delta would be unsound).
    compile_should_fail_with(
        r#"
class Bag {
    xs: [int]
    invariant self.xs.len() >= 0
    fn take(mut self) {
        self.xs.pop()
    }
}
fn main() { print(0) }
"#,
        "non-empty",
    );
}

#[test]
fn empty_literal_construction_refuted() {
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
}
fn main() {
    let l = Log { segs: [] }
    print(0)
}
"#,
        "violates its invariant",
    );
}

#[test]
fn fresh_but_unknown_length_construction_rejected() {
    // A slice is fresh (door (a) passes) but its length is not statically
    // known, so `len > 0` cannot be proven at construction.
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn count(self) int { return self.segs.len() }
}
fn make(src: [int]) Log {
    return Log { segs: src.slice(0, 1) }
}
fn main() {
    let l = make([1, 2, 3])
    print(l.count())
}
"#,
        "cannot prove invariant",
    );
}

#[test]
fn nested_collection_len_outside_fragment() {
    // Only a *direct* own collection field is provable; a nested field's
    // length is not tracked.
    compile_should_fail_with(
        r#"
class Inner {
    xs: [int]
}
class Outer {
    inner: Inner
    invariant self.inner.xs.len() > 0
}
fn main() { print(0) }
"#,
        "provable only on the class's own",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Door (a): construction / assignment must be a fresh collection
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn door_a_construction_from_alias_rejected() {
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
}
fn main() {
    let arr = [1, 2]
    let l = Log { segs: arr }
    print(0)
}
"#,
        "fresh collection the class alone owns",
    );
}

#[test]
fn door_a_construction_from_literal_ok() {
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn count(self) int { return self.segs.len() }
}
fn main() {
    let l = Log { segs: [1, 2] }
    print(l.count())
}
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn door_a_assignment_from_alias_rejected() {
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn adopt(mut self, other: [int]) {
        self.segs = other
    }
}
fn main() { print(0) }
"#,
        "fresh collection the class alone owns",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Door (b): no external read of a covered collection field
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn door_b_external_read_rejected() {
    compile_should_fail_with(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
}
fn main() {
    let l = Log { segs: [1] }
    let taken = l.segs
    print(0)
}
"#,
        "cannot read collection field 'segs'",
    );
}

#[test]
fn door_b_read_through_method_ok() {
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn count(self) int {
        return self.segs.len()
    }
    fn first(self) int {
        return self.segs[0]
    }
}
fn main() {
    let l = Log { segs: [7, 8] }
    print(l.count())
    print(l.first())
}
"#,
    );
    assert_eq!(out, "2\n7\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Door (c): no passing a covered collection field to a `mut` parameter
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn door_c_pass_to_mut_param_rejected() {
    compile_should_fail_with(
        r#"
fn grow(mut xs: [int]) {
    xs.push(99)
}
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn widen(mut self) {
        grow(self.segs)
    }
}
fn main() { print(0) }
"#,
        "to a 'mut' parameter",
    );
}

#[test]
fn door_c_pass_to_readonly_param_ok() {
    // Non-`mut` parameters are read-only, so they cannot desync the ghost.
    let out = compile_and_run_stdout(
        r#"
fn total(xs: [int]) int {
    return xs.len()
}
class Log {
    segs: [int]
    invariant self.segs.len() > 0
    fn report(self) int {
        return total(self.segs)
    }
}
fn main() {
    let l = Log { segs: [1, 2, 3] }
    print(l.report())
}
"#,
    );
    assert_eq!(out, "3\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// Boundary interaction: a call that may reach the receiver degrades the exact
// length to invariant-level knowledge (mirrors the int-field boundary rule).
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn call_reaching_receiver_degrades_to_invariant_level() {
    // After `self.touch()` (a self-call that may write the receiver), exact
    // length knowledge is gone, but the invariant still holds by assumption,
    // so a guarded pop afterward re-proves cleanly.
    let out = compile_and_run_stdout(
        r#"
class Log {
    segs: [int]
    tag: int
    invariant self.segs.len() > 0
    fn touch(mut self) {
        self.tag = self.tag + 1
    }
    fn step(mut self) {
        self.segs.push(1)
        self.touch()
        if self.segs.len() > 1 {
            self.segs.pop()
        }
    }
    fn count(self) int { return self.segs.len() }
}
fn main() {
    let mut l = Log { segs: [0], tag: 0 }
    l.step()
    print(l.count())
}
"#,
    );
    assert_eq!(out, "1\n");
}

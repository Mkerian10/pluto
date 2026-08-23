//! Implicit coercion coverage across all value-consuming contexts (#148):
//! Class→Trait, Class→Trait?, and T→T? in let bindings, assignments,
//! returns, call arguments, struct-literal fields, field assignments,
//! array/map literals, index assignments, and generator yields.
//!
//! Every trait-handle test dispatches a method through the coerced value —
//! a raw class pointer stored where a trait handle belongs also compares
//! non-none, so `!= none` alone would not catch a missed vtable wrap.
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

const PREAMBLE: &str = r#"
trait Worker {
    fn work(self) int
}

class W impl Worker {
    fn work(self) int {
        return 9
    }
}

class V impl Worker {
    fn work(self) int {
        return 4
    }
}
"#;

fn run(body: &str) -> String {
    compile_and_run_stdout(&format!("{PREAMBLE}\n{body}"))
}

// ── let bindings and assignment ──────────────────────────────────────────────

#[test]
fn let_class_to_trait_dispatch() {
    let out = run(r#"
fn main() {
    let t: Worker = W {}
    print(t.work())
}
"#);
    assert_eq!(out.trim(), "9");
}

#[test]
fn let_class_to_nullable_trait_dispatch() {
    let out = run(r#"
fn main() {
    let t: Worker? = W {}
    if t != none {
        print(t.work())
    }
}
"#);
    assert_eq!(out.trim(), "9");
}

#[test]
fn assign_class_to_nullable_trait_dispatch() {
    let out = run(r#"
fn main() {
    let mut m: Worker? = none
    m = V {}
    if m != none {
        print(m.work())
    }
}
"#);
    assert_eq!(out.trim(), "4");
}

#[test]
fn assign_value_to_nullable_then_narrowed_arithmetic() {
    // A raw int stored unboxed where int? belongs would be dereferenced as
    // a pointer by the narrowed read and crash.
    let out = run(r#"
fn main() {
    let mut x: int? = none
    x = 5
    if x != none {
        print(x + 1)
    }
}
"#);
    assert_eq!(out.trim(), "6");
}

// ── returns ──────────────────────────────────────────────────────────────────

#[test]
fn return_class_to_nullable_trait_dispatch() {
    let out = run(r#"
fn make() Worker? {
    return W {}
}

fn main() {
    let r = make()
    if r != none {
        print(r.work())
    }
}
"#);
    assert_eq!(out.trim(), "9");
}

// ── call arguments ───────────────────────────────────────────────────────────

#[test]
fn arg_class_to_nullable_trait_dispatch() {
    let out = run(r#"
fn take(w: Worker?) int {
    if w != none {
        return w.work()
    }
    return -1
}

fn main() {
    print(take(W {}))
    print(take(none))
}
"#);
    assert_eq!(out.trim(), "9\n-1");
}

// ── field assignment ─────────────────────────────────────────────────────────

#[test]
fn field_assign_class_to_nullable_trait_dispatch() {
    let out = run(r#"
class Slot {
    w: Worker?
}

fn main() {
    let mut s = Slot { w: none }
    s.w = V {}
    let w: Worker? = s.w
    if w != none {
        print(w.work())
    }
}
"#);
    assert_eq!(out.trim(), "4");
}

#[test]
fn field_assign_value_to_nullable_field() {
    let out = run(r#"
class Counter {
    n: int?
}

fn main() {
    let mut c = Counter { n: none }
    c.n = 7
    print(c.n ?? -1)
}
"#);
    assert_eq!(out.trim(), "7");
}

// ── array literals ───────────────────────────────────────────────────────────

#[test]
fn array_literal_annotated_trait_elements() {
    // The annotation supplies the element type; every element coerces to it.
    let out = run(r#"
fn main() {
    let arr: [Worker] = [W {}, V {}]
    print(arr[0].work())
    print(arr[1].work())
}
"#);
    assert_eq!(out.trim(), "9\n4");
}

#[test]
fn array_literal_annotated_nullable_elements() {
    let out = run(r#"
fn main() {
    let ints: [int?] = [1, none, 3]
    print(ints[0] ?? -1)
    print(ints[1] ?? -1)
    print(ints[2] ?? -1)
}
"#);
    assert_eq!(out.trim(), "1\n-1\n3");
}

#[test]
fn array_literal_without_annotation_still_strict() {
    compile_should_fail_with(
        &format!("{PREAMBLE}\nfn main() {{\n    let arr = [W {{}}, V {{}}]\n}}"),
        "array element type mismatch",
    );
}

#[test]
fn array_literal_incompatible_element_rejected() {
    compile_should_fail_with(
        &format!("{PREAMBLE}\nclass NotAWorker {{\n    n: int\n}}\nfn main() {{\n    let arr: [Worker] = [W {{}}, NotAWorker {{ n: 1 }}]\n}}"),
        "array element type mismatch",
    );
}

// ── index assignment ─────────────────────────────────────────────────────────

#[test]
fn index_assign_class_to_trait_dispatch() {
    let out = run(r#"
fn main() {
    let mut arr: [Worker] = [W {}]
    arr[0] = V {}
    print(arr[0].work())
}
"#);
    assert_eq!(out.trim(), "4");
}

// ── map literals and map insertion ───────────────────────────────────────────

#[test]
fn map_literal_trait_values_dispatch() {
    let out = run(r#"
fn main() {
    let m: Map<string, Worker> = Map<string, Worker> { "w": W {}, "v": V {} }
    print(m["w"].work())
    print(m["v"].work())
}
"#);
    assert_eq!(out.trim(), "9\n4");
}

#[test]
fn map_index_assign_class_to_trait_dispatch() {
    let out = run(r#"
fn main() {
    let m = Map<string, Worker> {}
    m["v"] = V {}
    print(m["v"].work())
}
"#);
    assert_eq!(out.trim(), "4");
}

// ── generator yields ─────────────────────────────────────────────────────────

#[test]
fn yield_class_to_trait_dispatch() {
    let out = run(r#"
fn gen() stream Worker {
    yield W {}
    yield V {}
}

fn main() {
    for w in gen() {
        print(w.work())
    }
}
"#);
    assert_eq!(out.trim(), "9\n4");
}

#[test]
fn yield_value_to_nullable() {
    let out = run(r#"
fn gen() stream int? {
    yield 1
    yield none
}

fn main() {
    for x in gen() {
        print(x ?? -1)
    }
}
"#);
    assert_eq!(out.trim(), "1\n-1");
}

// ── struct literals (regression guard; worked before this change) ────────────

#[test]
fn struct_literal_nullable_trait_field_dispatch() {
    let out = run(r#"
class Slot {
    w: Worker?
}

fn main() {
    let s = Slot { w: W {} }
    let w: Worker? = s.w
    if w != none {
        print(w.work())
    }
}
"#);
    assert_eq!(out.trim(), "9");
}

// ── rejections stay strict ───────────────────────────────────────────────────

#[test]
fn field_assign_non_impl_rejected() {
    compile_should_fail_with(
        &format!("{PREAMBLE}\nclass Plain {{\n    n: int\n}}\nclass Slot {{\n    w: Worker?\n}}\nfn main() {{\n    let mut s = Slot {{ w: none }}\n    s.w = Plain {{ n: 1 }}\n}}"),
        "expected trait Worker?, found Plain",
    );
}

#[test]
fn yield_incompatible_rejected() {
    compile_should_fail_with(
        &format!("{PREAMBLE}\nclass Plain {{\n    n: int\n}}\nfn gen() stream Worker {{\n    yield Plain {{ n: 1 }}\n}}\nfn main() {{}}"),
        "yield type mismatch",
    );
}

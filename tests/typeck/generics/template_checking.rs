//! Generic template body checking (issue #159, #182).
//!
//! Generic bodies are checked before monomorphization by substituting each
//! type parameter with an opaque skolem type implementing its bounds. Errors
//! hold for all instantiations and are reported even when the template is
//! never called, with original template spans.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Errors caught without any instantiation ──────────────────────────────────

#[test]
fn uninstantiated_return_conflict_detected() {
    compile_should_fail_with(
        r#"
fn bad<T>(x: T) T {
    if true {
        return x
    }
    return 42
}
fn main() {}
"#,
        "return type mismatch: expected T, found int",
    );
}

#[test]
fn uninstantiated_let_annotation_conflict_detected() {
    compile_should_fail_with(
        r#"
fn bad<T>(x: T) T {
    let y: int = x
    print(y)
    return x
}
fn main() {}
"#,
        "type mismatch: expected int, found T",
    );
}

#[test]
fn generic_class_method_body_checked() {
    compile_should_fail_with(
        r#"
class Box<T> {
    value: T

    fn wrong(self) T {
        return 42
    }
}
fn main() {}
"#,
        "return type mismatch: expected T, found int",
    );
}

#[test]
fn generic_class_invariant_checked() {
    compile_should_fail_with(
        r#"
class Counter<T> {
    value: T
    count: int

    invariant self.count + 1

    fn get(self) T {
        return self.value
    }
}
fn main() {}
"#,
        "invariant expression must be bool",
    );
}

#[test]
fn arithmetic_on_type_param_rejected() {
    // T is opaque: the body must be well-typed for every T, and no bound
    // grants arithmetic.
    compile_should_fail_with(
        r#"
fn double<T>(x: T) T {
    return x + x
}
fn main() {}
"#,
        "operator not supported for type T",
    );
}

#[test]
fn unbounded_method_call_on_param_rejected() {
    compile_should_fail_with(
        r#"
fn shout<T>(x: T) string {
    return x.to_string()
}
fn main() {}
"#,
        "type parameter 'T' has no method 'to_string'; add a trait bound",
    );
}

// ── Bounds make trait methods available ──────────────────────────────────────

#[test]
fn bounded_param_can_call_trait_method() {
    let out = compile_and_run_stdout(
        r#"
trait Describes {
    fn describe(self) string
}

class Dog impl Describes {
    name: string

    fn describe(self) string {
        return self.name
    }
}

fn announce<T: Describes>(x: T) string {
    return x.describe()
}

fn main() {
    print(announce(Dog { name: "rex" }))
}
"#,
    );
    assert_eq!(out.trim(), "rex");
}

#[test]
fn calling_bounded_generic_requires_bound() {
    // A template calling a generic with stricter bounds than its own must fail.
    compile_should_fail_with(
        r#"
trait Marker {
    fn tag(self) int
}

fn strict<U: Marker>(x: U) U {
    return x
}

fn loose<T>(x: T) T {
    return strict(x)
}
fn main() {}
"#,
        "type T does not satisfy bound 'U: Marker' required by 'strict'",
    );
}

// ── Valid parametric templates still compile ─────────────────────────────────

#[test]
fn valid_templates_unaffected() {
    let out = compile_and_run_stdout(
        r#"
fn id<T>(x: T) T {
    return x
}

fn pick<T>(a: T, b: T, first: bool) T {
    if first {
        return a
    }
    return b
}

class Holder<T> {
    value: T

    fn get(self) T {
        return self.value
    }

    fn replace(self, v: T) Holder<T> {
        return Holder<T> { value: v }
    }
}

fn main() {
    print(id(1))
    print(pick("a", "b", false))
    let h = Holder<int> { value: 3 }
    print(h.replace(4).get())
}
"#,
    );
    assert_eq!(out.trim(), "1\nb\n4");
}

#[test]
fn generic_calling_generic_with_param_typed_arg() {
    // Passing an instance of a generic enum/class of T to another generic —
    // unification recovers args from instantiated (mangled) types.
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    value: T
}

fn unbox<T>(b: Box<T>) T {
    return b.value
}

fn rewrap<T>(b: Box<T>) Box<T> {
    return Box<T> { value: unbox(b) }
}

fn main() {
    let b = Box<int> { value: 9 }
    print(unbox(rewrap(b)))
}
"#,
    );
    assert_eq!(out.trim(), "9");
}

#[test]
fn set_of_type_param_deferred_to_instantiation() {
    // No hashable bound exists, so Set<T> in a template defers key validation
    // to each instantiation.
    let out = compile_and_run_stdout(
        r#"
fn singleton<T>(val: T) Set<T> {
    let s = Set<T> {}
    s.insert(val)
    return s
}

fn main() {
    let s = singleton(42)
    print(s.contains(42))
}
"#,
    );
    assert_eq!(out.trim(), "true");
}

// ── Self-referencing generic classes ─────────────────────────────────────────

#[test]
fn self_referencing_method_signature_works() {
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    value: T

    fn boxed(self) Box<T> {
        return Box<T> { value: self.value }
    }
}

fn main() {
    let b = Box<int> { value: 5 }
    print(b.boxed().value)
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn nullable_self_reference_linked_list() {
    let out = compile_and_run_stdout(
        r#"
class Node<T> {
    value: T
    next: Node<T>?
}

fn main() {
    let tail = Node<int> { value: 2, next: none }
    let head = Node<int> { value: 1, next: tail }
    print(head.value)
    print(tail.value)
}
"#,
    );
    assert_eq!(out.trim(), "1\n2");
}

#[test]
fn expanding_self_reference_rejected() {
    compile_should_fail_with(
        r#"
class Box<T> {
    inner: Box<Box<T>>?
}
fn main() {}
"#,
        "expanding recursive reference",
    );
}

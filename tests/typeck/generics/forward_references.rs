//! Forward references (#175): top-level declaration order does not matter.
//!
//! Any declaration may reference any other declared anywhere in the file —
//! concrete or generic, class, enum, trait, function, or bracket dep.
//! Registration records skeletons for every generic first and defers eager
//! instantiation until all signatures are known (see
//! `defer_eager_instantiation` / `normalize_registered_types`), so the only
//! ordering error left is a genuinely non-terminating one: expanding
//! recursive instantiation on a reference cycle.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Generic ↔ concrete, both directions ─────────────────────────────────────

#[test]
fn generic_field_references_later_class() {
    let out = compile_and_run_stdout(
        r#"
class Box<T> {
    value: T
    tag: Late
}

class Late {
    n: int
}

fn main() {
    let b = Box<int> { value: 1, tag: Late { n: 7 } }
    print(b.tag.n)
}
"#,
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn use_generic_before_decl() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let b = Box<int> { value: 42 }
    print(b.value)
}

class Box<T> {
    value: T
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn explicit_type_arg_forward() {
    let out = compile_and_run_stdout(
        r#"
fn id<T>(x: T) T {
    return x
}

fn main() {
    print(id<Forward>(Forward { x: 1 }).x)
}

class Forward {
    x: int
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

// ── Generic → later generic ─────────────────────────────────────────────────

#[test]
fn generic_field_references_later_generic() {
    let out = compile_and_run_stdout(
        r#"
class A<T> {
    b: B<T>
}

class B<U> {
    x: U
}

fn main() {
    let a = A<int> { b: B<int> { x: 5 } }
    print(a.b.x)
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn generic_fn_param_references_later_generic() {
    let out = compile_and_run_stdout(
        r#"
fn open<T>(w: Wrapper<T>) T {
    return w.inner
}

class Wrapper<T> {
    inner: T
}

fn main() {
    print(open(Wrapper<int> { inner: 3 }))
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

// ── Enums ───────────────────────────────────────────────────────────────────

#[test]
fn generic_enum_variant_references_later_class() {
    let out = compile_and_run_stdout(
        r#"
enum Holder<T> {
    Has { v: T, tag: Late }
    Empty
}

class Late {
    n: int
}

fn main() {
    let h = Holder<int>.Has { v: 1, tag: Late { n: 9 } }
    match h {
        Holder.Has { v, tag } {
            print(tag.n)
        }
        Holder.Empty {
            print(-1)
        }
    }
}
"#,
    );
    assert_eq!(out.trim(), "9");
}

#[test]
fn fn_signature_references_later_generic_enum() {
    let out = compile_and_run_stdout(
        r#"
fn get(o: Opt<int>) int {
    match o {
        Opt.Some { v } {
            return v
        }
        Opt.None {
            return -1
        }
    }
    return -2
}

enum Opt<T> {
    Some { v: T }
    None
}

fn main() {
    print(get(Opt<int>.Some { v: 3 }))
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

// ── Traits and bounds ───────────────────────────────────────────────────────

#[test]
fn generic_bound_references_later_trait() {
    let out = compile_and_run_stdout(
        r#"
fn describe<T: Named>(x: T) string {
    return x.name()
}

trait Named {
    fn name(self) string
}

class Dog impl Named {
    tag: string

    fn name(self) string {
        return self.tag
    }
}

fn main() {
    print(describe(Dog { tag: "rex" }))
}
"#,
    );
    assert_eq!(out.trim(), "rex");
}

// ── DI bracket deps ─────────────────────────────────────────────────────────

#[test]
fn bracket_dep_references_later_class() {
    let out = compile_and_run_stdout(
        r#"
class Service[dep: Late] {
    fn tag(self) int {
        return self.dep.n()
    }
}

class Late {
    fn n(self) int {
        return 11
    }
}

app Main[svc: Service] {
    fn main(self) {
        print(self.svc.tag())
    }
}
"#,
    );
    assert_eq!(out.trim(), "11");
}

// ── Recursion: legal (non-expanding) cycles ─────────────────────────────────

#[test]
fn mutually_recursive_generic_classes() {
    let out = compile_and_run_stdout(
        r#"
class A<T> {
    b: B<T>?
}

class B<U> {
    a: A<U>?
    val: U
}

fn main() {
    let b = B<int> { a: none, val: 9 }
    let a = A<int> { b: b }
    print(a.b == none)
    print(b.val)
}
"#,
    );
    assert_eq!(out.trim(), "false\n9");
}

#[test]
fn recursive_generic_enum_linked_list() {
    let out = compile_and_run_stdout(
        r#"
enum List<T> {
    Cons { head: T, tail: List<T> }
    Nil
}

fn sum(l: List<int>) int {
    match l {
        List.Cons { head, tail } {
            return head + sum(tail)
        }
        List.Nil {
            return 0
        }
    }
    return 0
}

fn main() {
    let l = List<int>.Cons { head: 1, tail: List<int>.Cons { head: 2, tail: List<int>.Nil } }
    print(sum(l))
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn circular_through_param() {
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
}
"#,
    );
    assert_eq!(out.trim(), "1");
}

// ── Rejection: expanding instantiation cycles ───────────────────────────────

#[test]
fn expanding_mutual_cycle_rejected() {
    compile_should_fail_with(
        r#"
class A<T> {
    b: B<Box<T>>?
}

class B<U> {
    a: A<U>?
}

class Box<T> {
    v: T
}

fn main() {}
"#,
        "expanding recursive reference",
    );
}

#[test]
fn expanding_self_reference_still_rejected() {
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

#[test]
fn expanding_indirect_cycle_rejected() {
    compile_should_fail_with(
        r#"
class A<T> {
    b: B<T>?
}

class B<U> {
    c: C<[U]>?
}

class C<V> {
    a: A<V>?
}

fn main() {}
"#,
        "expanding recursive reference",
    );
}

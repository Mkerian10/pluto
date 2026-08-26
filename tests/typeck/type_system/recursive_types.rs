//! Recursive types.
//!
//! Classes and enums are GC references, so recursive and mutually recursive
//! declarations are legal and constructible (with a nullable or enum base
//! case). The rejected shapes are *expanding* generic instantiation cycles,
//! DI bracket-dependency cycles, and reference forms the language doesn't
//! have (error fields referencing errors, unknown generic containers).
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail_with};

// ── Legal recursive declarations ─────────────────────────────────────────────

#[test]
fn direct_recursive_class() {
    // Declarable; unconstructible without a base case, but that's the
    // program's problem, not the type system's
    assert_eq!(compile_and_run("class C {\n    x: C\n}\n\nfn main(){}"), 0);
}

#[test]
fn indirect_recursive_class() {
    assert_eq!(
        compile_and_run("class A {\n    b: B\n}\n\nclass B {\n    a: A\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn three_class_cycle() {
    assert_eq!(
        compile_and_run("class A {\n    b: B\n}\n\nclass B {\n    c: C\n}\n\nclass C {\n    a: A\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn deep_recursive_type() {
    assert_eq!(
        compile_and_run("class A {\n    b: B\n}\n\nclass B {\n    c: C\n}\n\nclass C {\n    d: D\n}\n\nclass D {\n    e: E\n}\n\nclass E {\n    a: A\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn recursive_nullable_type() {
    // The constructible form: nullable gives the base case
    let out = compile_and_run_stdout(
        "class C {\n    x: C?\n    v: int\n}\n\nfn main(){\n    let inner = C { x: none, v: 2 }\n    let outer = C { x: inner, v: 1 }\n    print(outer.v)\n    let n = outer.x\n    if n != none {\n        print(n.v)\n    }\n}",
    );
    assert_eq!(out.trim(), "1\n2");
}

#[test]
fn recursive_enum_variant() {
    let out = compile_and_run_stdout(
        "enum E {\n    Node { next: E }\n    Leaf\n}\n\nfn depth(e: E) int {\n    match e {\n        E.Node { next } {\n            return 1 + depth(next)\n        }\n        E.Leaf {\n            return 0\n        }\n    }\n    return 0\n}\n\nfn main(){\n    print(depth(E.Node { next: E.Node { next: E.Leaf } }))\n}",
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn mutual_enum_recursion() {
    assert_eq!(
        compile_and_run("enum A {\n    HasB { b: B }\n    Stop\n}\n\nenum B {\n    HasA { a: A }\n    Stop\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn recursive_map_type() {
    assert_eq!(
        compile_and_run("class C {\n    m: Map<string, C>\n}\n\nfn main(){}"),
        0
    );
}

#[test]
fn recursive_array_type() {
    let out = compile_and_run_stdout(
        "class C {\n    arr: [C]\n    v: int\n}\n\nfn main(){\n    let leaf = C { arr: [], v: 2 }\n    let root = C { arr: [leaf], v: 1 }\n    print(root.v)\n    print(root.arr[0].v)\n}",
    );
    assert_eq!(out.trim(), "1\n2");
}

#[test]
fn recursive_fn_type() {
    // Nested function types are just types
    let out = compile_and_run_stdout(
        "fn f(g: fn(fn(int) int) int) int {\n    return g((x: int) => x + 1)\n}\n\nfn main(){\n    print(f((h: fn(int) int) => h(41)))\n}",
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn recursive_field_method() {
    let out = compile_and_run_stdout(
        "class C {\n    x: C?\n\n    fn next(self) C? {\n        return self.x\n    }\n}\n\nfn main(){\n    let c = C { x: none }\n    print(c.next() == none)\n}",
    );
    assert_eq!(out.trim(), "true");
}

#[test]
fn recursive_trait_impl() {
    // A trait method returning the trait's own type is fine
    let out = compile_and_run_stdout(
        "trait T {\n    fn me(self) T\n}\n\nclass C impl T {\n    v: int\n\n    fn me(self) T {\n        return self\n    }\n}\n\nfn main(){\n    let c = C { v: 5 }\n    let t: T = c.me()\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

// ── Kept legal cases from earlier passes ─────────────────────────────────────

// Deeply nested generic instantiation is a valid type
#[test]
fn recursive_type_param() {
    let out = compile_and_run_stdout(
        "class C<T> {\n    x: T\n}\n\nfn f() C<C<C<int>>> {\n    return C<C<C<int>>> { x: C<C<int>> { x: C<int> { x: 9 } } }\n}\n\nfn main(){\n    print(f().x.x.x)\n}",
    );
    assert_eq!(out.trim(), "9");
}

// Recursive generic class
#[test]
fn recursive_generic_class() {
    // Non-expanding self-references are legal — classes are GC references.
    // (Expanding ones like `x: C<C<T>>` are rejected at registration.)
    assert_eq!(compile_and_run(r#"class C<T>{x:C<T>} fn main(){}"#), 0);
}

// Nested task type: a spawned fn that itself returns a task
#[test]
fn recursive_task_type() {
    let out = compile_and_run_stdout(
        "fn leaf() int {\n    return 7\n}\n\nfn inner() Task<int> {\n    return spawn leaf()\n}\n\nfn task() Task<Task<int>> {\n    return spawn inner()\n}\n\nfn main(){\n    let t = task()\n    let t2 = t.get()!\n    print(t2.get()!)\n}",
    );
    assert_eq!(out.trim(), "7");
}

// ── Rejected shapes ──────────────────────────────────────────────────────────

#[test]
fn expanding_recursive_instantiation_rejected() {
    compile_should_fail_with(
        "class Box<T> {\n    inner: Box<Box<T>>?\n}\n\nfn main(){}",
        "expanding recursive reference",
    );
}

#[test]
fn cycle_bracket_deps() {
    compile_should_fail_with(
        "class A[b: B] {}\n\nclass B[a: A] {}\n\nfn main(){}",
        "circular",
    );
}

#[test]
fn recursive_error_type() {
    // Error fields cannot reference error declarations
    compile_should_fail_with(
        "error E {\n    cause: E\n}\n\nfn main(){}",
        "unknown type 'E'",
    );
}

#[test]
fn recursive_channel_type() {
    // Channel is not a user-instantiable generic type
    compile_should_fail_with(
        "class C {\n    ch: Channel<C, C>\n}\n\nfn main(){}",
        "unknown",
    );
}

//! Circular references between declarations.
//!
//! Since #175, mutually recursive classes, enums, generics, traits, and
//! functions are legal (heap references make the layouts finite); the only
//! rejected type cycle is an *expanding* recursive instantiation. DI bracket
//! dependencies must still form a DAG, and module import cycles are rejected
//! (covered in tests/integration/modules.rs::circular_import_rejected).
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Legal cycles ─────────────────────────────────────────────────────────────

#[test]
fn circular_classes() {
    let out = compile_and_run_stdout(
        "class A {\n    b: B\n}\n\nclass B {\n    a: A\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn three_way_circular() {
    let out = compile_and_run_stdout(
        "class A {\n    b: B\n}\n\nclass B {\n    c: C\n}\n\nclass C {\n    a: A\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn circular_traits() {
    let out = compile_and_run_stdout(
        "trait T1 {\n    fn foo(self) T2\n}\n\ntrait T2 {\n    fn bar(self) T1\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn circular_enums() {
    let out = compile_and_run_stdout(
        "enum E1 {\n    A { e: E2? }\n}\n\nenum E2 {\n    B { e: E1? }\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn circular_generics() {
    // Non-expanding mutual recursion between generics is legal
    let out = compile_and_run_stdout(
        "class A<T> {\n    value: B<T>?\n}\n\nclass B<U> {\n    value: A<U>?\n}\n\nfn main(){\n    let b = B<int> { value: none }\n    print(b.value == none)\n}",
    );
    assert_eq!(out.trim(), "true");
}

#[test]
fn self_referential_class() {
    let out = compile_and_run_stdout(
        "class Node {\n    next: Node?\n}\n\nfn main(){\n    let n = Node { next: none }\n    print(n.next == none)\n}",
    );
    assert_eq!(out.trim(), "true");
}

#[test]
fn circular_functions() {
    let out = compile_and_run_stdout(
        "fn is_even(n: int) bool {\n    if n == 0 {\n        return true\n    }\n    return is_odd(n - 1)\n}\n\nfn is_odd(n: int) bool {\n    if n == 0 {\n        return false\n    }\n    return is_even(n - 1)\n}\n\nfn main(){\n    print(is_even(4))\n}",
    );
    assert_eq!(out.trim(), "true");
}

#[test]
fn mutual_recursion_methods() {
    let out = compile_and_run_stdout(
        "class C {\n    fn foo(self, n: int) int {\n        if n <= 0 {\n            return 0\n        }\n        return self.bar(n - 1)\n    }\n\n    fn bar(self, n: int) int {\n        return self.foo(n - 1)\n    }\n}\n\nfn main(){\n    let c = C {}\n    print(c.foo(4))\n}",
    );
    assert_eq!(out.trim(), "0");
}

#[test]
fn indirect_circular() {
    let out = compile_and_run_stdout(
        "class A {\n    b: B?\n}\n\nclass B {\n    c: C?\n}\n\nclass C {\n    d: D?\n}\n\nclass D {\n    a: A?\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn circular_nullable() {
    let out = compile_and_run_stdout(
        "class A {\n    b: B?\n}\n\nclass B {\n    a: A?\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn circular_array() {
    let out = compile_and_run_stdout(
        "class A {\n    b: [B]\n}\n\nclass B {\n    a: [A]\n}\n\nfn main(){\n    print(1)\n}",
    );
    assert_eq!(out.trim(), "1");
}

// ── Rejected: expanding recursive instantiation ──────────────────────────────

#[test]
fn expanding_recursive_instantiation_rejected() {
    compile_should_fail_with(
        "class Box<T> {\n    inner: Box<Box<T>>?\n}\n\nfn main(){}",
        "expanding recursive reference",
    );
}

// ── Rejected: DI cycles ──────────────────────────────────────────────────────

#[test]
fn circular_di() {
    compile_should_fail_with(
        "class A[b: B] {\n    x: int\n}\n\nclass B[a: A] {\n    y: int\n}\n\nfn main(){}",
        "circular",
    );
}

#[test]
fn circular_bracket_deps_chain() {
    compile_should_fail_with(
        "class A[b: B] {}\n\nclass B[c: C] {}\n\nclass C[a: A] {}\n\nfn main(){}",
        "circular",
    );
}

#[test]
fn self_di_dependency() {
    compile_should_fail_with(
        "class A[a: A] {}\n\nfn main(){}",
        "circular",
    );
}

// ── Rejected: unsupported reference shapes ───────────────────────────────────

#[test]
fn error_field_referencing_error_rejected() {
    // Error fields cannot reference other error declarations
    compile_should_fail_with(
        "error E1 {\n    e: E2\n}\n\nerror E2 {\n    e: E1\n}\n\nfn main(){}",
        "unknown type 'E2'",
    );
}

#[test]
fn trait_inheritance_syntax_rejected() {
    // Pluto has no trait inheritance; `trait T1: T2` is a syntax error
    compile_should_fail_with(
        "trait T1: T2 {}\n\ntrait T2: T1 {}\n\nfn main(){}",
        "expected {",
    );
}

#[test]
fn double_impl_clause_rejected() {
    // Multiple traits use `impl T1, T2`, not repeated impl clauses
    compile_should_fail_with(
        "trait T1 {}\n\ntrait T2 {}\n\nclass C impl T1 impl T2 {}\n\nfn main(){}",
        "expected {",
    );
}

#[test]
fn circular_generic_bounds() {
    // Bounds must name traits; a type parameter is not a trait
    compile_should_fail_with(
        "fn f<T: U, U: T>(x: T) {}\n\nfn main(){}",
        "unknown trait",
    );
}

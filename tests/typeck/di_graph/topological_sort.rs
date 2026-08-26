//! DI dependency graph: valid wiring shapes and real rejections.
//!
//! Bracket deps `[dep: Type]` are auto-wired via compile-time topological
//! sort. Chains, diamonds, declaration order, generics, traits, enums, and
//! value deps are all valid graph shapes; the rejections are cycles,
//! unknown types, and duplicate dep names.
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Valid shapes ─────────────────────────────────────────────────────────────

#[test]
fn chain_dependency() {
    let out = compile_and_run_stdout(
        "class C {\n    fn v(self) int {\n        return 3\n    }\n}\n\nclass B[c: C] {\n    fn v(self) int {\n        return self.c.v()\n    }\n}\n\nclass A[b: B] {\n    fn v(self) int {\n        return self.b.v()\n    }\n}\n\napp MyApp[a: A] {\n    fn main(self){\n        print(self.a.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn diamond_dependency() {
    let out = compile_and_run_stdout(
        "class A {\n    fn v(self) int {\n        return 1\n    }\n}\n\nclass B[a: A] {}\n\nclass C[a: A] {}\n\nclass D[b: B, c: C] {\n    fn v(self) int {\n        return 4\n    }\n}\n\napp MyApp[d: D] {\n    fn main(self){\n        print(self.d.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "4");
}

#[test]
fn dep_order_is_irrelevant() {
    // Declaration order does not constrain wiring (forward references, #175)
    let out = compile_and_run_stdout(
        "class B[a: A] {\n    fn v(self) int {\n        return 2\n    }\n}\n\nclass A {}\n\napp MyApp[b: B] {\n    fn main(self){\n        print(self.b.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn nested_deps() {
    let out = compile_and_run_stdout(
        "class A {}\n\nclass B[a: A] {}\n\nclass C[b: B] {}\n\nclass D[c: C] {\n    fn v(self) int {\n        return 9\n    }\n}\n\napp MyApp[d: D] {\n    fn main(self){\n        print(self.d.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "9");
}

#[test]
fn generic_class_dep() {
    let out = compile_and_run_stdout(
        "class Box<T> {\n    value: T\n}\n\nclass A[b: Box<int>] {\n    fn v(self) int {\n        return 5\n    }\n}\n\napp MyApp[a: A] {\n    fn main(self){\n        print(self.a.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn dep_on_value_type_allowed() {
    let out = compile_and_run_stdout(
        "class A[x: int] {\n    fn v(self) int {\n        return self.x\n    }\n}\n\napp MyApp[a: A] {\n    fn main(self){\n        print(self.a.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "0");
}

#[test]
fn dep_on_trait_allowed() {
    let out = compile_and_run_stdout(
        "trait T {}\n\nclass A[t: T] {}\n\napp MyApp[a: A] {\n    fn main(self){\n        print(1)\n    }\n}",
    );
    assert_eq!(out.trim(), "1");
}

#[test]
fn dep_on_enum_allowed() {
    let out = compile_and_run_stdout(
        "enum E {\n    A\n}\n\nclass C[e: E] {}\n\napp MyApp[c: C] {\n    fn main(self){\n        print(1)\n    }\n}",
    );
    assert_eq!(out.trim(), "1");
}

// ── Rejections ───────────────────────────────────────────────────────────────

#[test]
fn circular_di() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

#[test]
fn three_way_circular_di() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

#[test]
fn self_dependency() { compile_should_fail_with(r#"class A[a:A]{} app MyApp{fn main(self){}}"#, "circular"); }

#[test]
fn missing_dependency() {
    compile_should_fail_with(
        "class A[b: B] {}\n\napp MyApp[a: A] {\n    fn main(self){}\n}",
        "unknown type 'B'",
    );
}

#[test]
fn duplicate_dep_names() {
    compile_should_fail_with(
        "class A {}\n\nclass B {}\n\nclass C[dep: A, dep: B] {}\n\napp MyApp[c: C] {\n    fn main(self){}\n}",
        "duplicate field 'dep'",
    );
}

// ── Kept from the original file ──────────────────────────────────────────────

#[test]
fn multiple_deps_same() { compile_should_fail_with(r#"class A{} class B[a1:A,a2:A]{} app MyApp{fn main(self){}}"#, ""); }

#[test]
fn dep_on_private() { compile_should_fail_with(r#"private class A{} class B[a:A]{} app MyApp{fn main(self){}}"#, ""); }

#[test]
fn multiple_apps() { compile_should_fail_with(r#"app App1{fn main(self){}} app App2{fn main(self){}}"#, ""); }

#[test]
fn app_no_main() { compile_should_fail_with(r#"app MyApp{fn helper(self){}}"#, ""); }

#[test]
fn app_main_wrong_sig() { compile_should_fail_with(r#"app MyApp{fn main(self)int{return 1}}"#, ""); }

#[test]
fn scoped_class_in_graph_compiles() {
    let out = compile_and_run_stdout(
        "scoped class A {}\n\nclass B {\n    fn v(self) int {\n        return 7\n    }\n}\n\napp MyApp[b: B] {\n    fn main(self){\n        print(self.b.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "7");
}

#[test]
fn transient_class_in_graph_compiles() {
    let out = compile_and_run_stdout(
        "transient class A {}\n\nclass B {\n    fn v(self) int {\n        return 8\n    }\n}\n\napp MyApp[b: B] {\n    fn main(self){\n        print(self.b.v())\n    }\n}",
    );
    assert_eq!(out.trim(), "8");
}

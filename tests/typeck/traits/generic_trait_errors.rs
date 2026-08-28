//! Generic trait errors (#291).
//!
//! Generic traits are templates: `impl T<int>` (or a `T<int>` type mention)
//! stamps out a concrete trait with the type parameters substituted, and
//! conformance/dispatch run against the instantiation. These tests pin the
//! error surface; positive coverage lives in tests/integration/traits.rs.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic generic trait errors
#[test]
fn generic_trait_wrong_type_arg() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) string {\n        return \"hi\"\n    }\n}\n\nfn main(){}",
        "return type mismatch",
    );
}
#[test]
fn generic_trait_missing_type_arg() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass C impl T {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "expects 1 type arguments, got 0",
    );
}

// Multiple type parameters
#[test]
fn generic_trait_two_params_wrong() {
    compile_should_fail_with(
        "trait T<U,V>{\n    fn foo(self, x: U) V\n}\n\nclass C impl T<int,string> {\n    a:int\n\n    fn foo(self, x: string) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "type mismatch",
    );
}
#[test]
fn generic_trait_wrong_param_count() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass C impl T<int,string> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "expects 1 type arguments, got 2",
    );
}

// Type args on a non-generic trait
#[test]
fn non_generic_trait_with_args() {
    compile_should_fail_with(
        "trait T{\n    fn foo(self) int\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "not generic and does not accept type arguments",
    );
}

// Generic class implementing a generic trait with its own type parameter
#[test]
fn generic_class_trait_class_param() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass Box<V> impl T<V> {\n    value:V\n\n    fn foo(self) V {\n        return self.value\n    }\n}\n\nfn main(){}",
        "not supported yet",
    );
}

// Type bounds on generic traits
#[test]
fn generic_trait_bound_not_satisfied() {
    compile_should_fail_with(
        "trait Printable{\n    fn show(self) string\n}\n\ntrait T<U: Printable>{\n    fn foo(self) U\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "does not satisfy trait bound 'Printable'",
    );
}

// Default methods in generic traits: a broken default body still errors
// against the instantiated signature
#[test]
fn generic_trait_default_body_type_error() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n\n    fn bar(self) U {\n        return \"hi\"\n    }\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "",
    );
}

// Missing method in generic trait impl
#[test]
fn generic_trait_missing_method() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n    fn bar(self) U\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "does not implement required method 'bar'",
    );
}

// Wrong type param in method signature
#[test]
fn generic_trait_method_uses_wrong_param() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self, x: U) U\n}\n\nclass C impl T<int> {\n    a:int\n\n    fn foo(self, x: string) int {\n        return 1\n    }\n}\n\nfn main(){}",
        "type mismatch",
    );
}

// Trait object with the wrong instantiation
#[test]
fn generic_trait_object_wrong_args() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){\n    let t: T<string> = C{x:1}\n}",
        "",
    );
}

// Bare template name as a type
#[test]
fn generic_trait_bare_type_mention() {
    compile_should_fail_with(
        "trait T<U>{\n    fn foo(self) U\n}\n\nclass C impl T<int> {\n    x:int\n\n    fn foo(self) int {\n        return 1\n    }\n}\n\nfn main(){\n    let t: T = C{x:1}\n}",
        "",
    );
}

// Duplicate type parameters in a trait declaration
#[test]
fn generic_trait_duplicate_params() {
    compile_should_fail_with(
        "trait T<U, U>{\n    fn foo(self) U\n}\n\nfn main(){}",
        "already declared",
    );
}

// Speculative syntax that is not part of Pluto — pinned as parse failures
#[test]
fn generic_trait_associated_type() {
    compile_should_fail_with(
        "trait T<U>{\n    type Output\n    fn foo(self) Output\n}\n\nfn main(){}",
        "",
    );
}
#[test]
fn generic_trait_default_param() {
    compile_should_fail_with("trait T<U=int>{\n    fn foo(self) U\n}\n\nfn main(){}", "");
}
#[test]
fn generic_trait_where_clause() {
    compile_should_fail_with(
        "trait Printable{\n    fn show(self) string\n}\n\ntrait T<U> where U: Printable {\n    fn foo(self) U\n}\n\nfn main(){}",
        "",
    );
}
#[test]
fn trait_const_generic() {
    compile_should_fail_with(
        "trait T<const N: int>{\n    fn foo(self) int\n}\n\nfn main(){}",
        "",
    );
}

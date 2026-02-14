//! Circular dependency errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Simple circular class dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_classes() { compile_should_fail_with(r#"class A{b:B} class B{a:A} fn main(){}"#, ""); }

// Three-way circular dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn three_way_circular() { compile_should_fail_with(r#"class A{b:B} class B{c:C} class C{a:A} fn main(){}"#, ""); }

// Circular trait dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_traits() { compile_should_fail_with(r#"trait T1{fn foo(self)T2} trait T2{fn bar(self)T1} fn main(){}"#, ""); }

// Circular enum dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_enums() { compile_should_fail_with(r#"enum E1{A{e:E2}} enum E2{B{e:E1}} fn main(){}"#, ""); }

// Circular generic dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_generics() { compile_should_fail_with(r#"class A<T>{value:B<T>} class B<U>{value:A<U>} fn main(){}"#, ""); }

// Circular DI dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_di() { compile_should_fail_with(r#"class A[b:B]{x:int} class B[a:A]{y:int} fn main(){}"#, "circular"); }

// Self-referential class (valid with pointer)
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn self_referential_class() { compile_should_fail_with(r#"class Node{next:Node?} fn main(){}"#, ""); }

// Circular function dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_functions() { compile_should_fail_with(r#"fn f()int{return g()} fn g()int{return f()} fn main(){}"#, ""); }

// Circular type alias (if supported)
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_type_alias() { compile_should_fail_with(r#"type A=B type B=A fn main(){}"#, "circular"); }

// Circular trait bound
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_trait_bound() { compile_should_fail_with(r#"trait T1:T2{} trait T2:T1{} fn main(){}"#, ""); }

// Indirect circular dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn indirect_circular() { compile_should_fail_with(r#"class A{b:B} class B{c:C} class C{d:D} class D{a:A} fn main(){}"#, ""); }

// Circular error dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_errors() { compile_should_fail_with(r#"error E1{e:E2} error E2{e:E1} fn main(){}"#, ""); }

// Circular module dependency
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_modules() { compile_should_fail_with(r#"import mod1 fn main(){}"#, "circular"); }

// Circular bracket deps chain
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_bracket_deps_chain() { compile_should_fail_with(r#"class A[b:B]{} class B[c:C]{} class C[a:A]{} fn main(){}"#, "circular"); }

// Self-dependency in DI
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn self_di_dependency() { compile_should_fail_with(r#"class A[a:A]{} fn main(){}"#, "circular"); }

// Circular with nullable (still circular)
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_nullable() { compile_should_fail_with(r#"class A{b:B?} class B{a:A?} fn main(){}"#, ""); }

// Circular with array
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_array() { compile_should_fail_with(r#"class A{b:[B]} class B{a:[A]} fn main(){}"#, ""); }

// Circular trait implementation
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_trait_impl() { compile_should_fail_with(r#"trait T1{} trait T2{} class C impl T1 impl T2 fn main(){}"#, ""); }

// Mutual recursion in methods
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn mutual_recursion_methods() { compile_should_fail_with(r#"class C{} fn foo(self){self.bar()} fn bar(self){self.foo()} fn main(){}"#, ""); }

// Circular generic bounds
#[test]
#[ignore] // #157: compiler doesn't detect recursive types
fn circular_generic_bounds() { compile_should_fail_with(r#"fn f<T:U,U:T>(x:T){} fn main(){}"#, ""); }

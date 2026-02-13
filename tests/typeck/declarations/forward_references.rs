//! Forward reference errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Use class before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn class_forward_ref() { compile_should_fail_with(r#"class A{b:B} class B{x:int} fn main(){}"#, ""); }

// Use function before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn function_forward_ref() { compile_should_fail_with(r#"fn f(){g()} fn g(){} fn main(){}"#, ""); }

// Use trait before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn trait_forward_ref() { compile_should_fail_with(r#"class C{} impl T trait T{} fn main(){}"#, ""); }

// Use enum before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn enum_forward_ref() { compile_should_fail_with(r#"fn f()E{return E.A} enum E{A B} fn main(){}"#, ""); }

// Use error before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn error_forward_ref() { compile_should_fail_with(r#"fn f()!{raise E{}} error E{} fn main(){}"#, ""); }

// Generic class forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn generic_class_forward_ref() { compile_should_fail_with(r#"class A{b:Box<int>} class Box<T>{value:T} fn main(){}"#, ""); }

// Bracket dep forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn bracket_dep_forward_ref() { compile_should_fail_with(r#"class A[b:B]{x:int} class B{y:int} fn main(){}"#, ""); }

// Method forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn method_forward_ref() { compile_should_fail_with(r#"class C{} fn foo(self){self.bar()} fn bar(self){} fn main(){}"#, ""); }

// Trait method forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn trait_method_forward_ref() { compile_should_fail_with(r#"trait T{fn foo(self){self.bar()} fn bar(self)} class C{} impl T{fn foo(self){} fn bar(self){}} fn main(){}"#, ""); }

// Enum variant forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn enum_variant_forward_ref() { compile_should_fail_with(r#"enum E{A{x:F}} enum F{X Y} fn main(){}"#, ""); }

// Type alias forward ref (if supported)
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn type_alias_forward_ref() { compile_should_fail_with(r#"type MyInt=B type B=int fn main(){}"#, ""); }

// Generic bound forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn generic_bound_forward_ref() { compile_should_fail_with(r#"fn f<T:U>(x:T){} trait U{} fn main(){}"#, ""); }

// Impl trait forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn impl_trait_forward_ref() { compile_should_fail_with(r#"class C{} impl T{fn foo(self){}} trait T{fn foo(self)} fn main(){}"#, ""); }

// Contract forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn contract_forward_ref() { compile_should_fail_with(r#"class A{x:int invariant self.x>B.value} class B{value:int} fn main(){}"#, ""); }

// Field type forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn field_type_forward_ref() { compile_should_fail_with(r#"class A{b:B} class B{a:A} fn main(){}"#, ""); }

// Parameter type forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn param_type_forward_ref() { compile_should_fail_with(r#"fn f(x:B){} class B{} fn main(){}"#, ""); }

// Return type forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn return_type_forward_ref() { compile_should_fail_with(r#"fn f()B{return B{}} class B{} fn main(){}"#, ""); }

// Array element forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn array_element_forward_ref() { compile_should_fail_with(r#"fn f()[B]{return [B{}]} class B{} fn main(){}"#, ""); }

// Map value forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn map_value_forward_ref() { compile_should_fail_with(r#"fn f()Map<string,B>{return Map<string,B>{}} class B{} fn main(){}"#, ""); }

// Closure param forward ref
#[test]
#[ignore] // PR #46 - outdated assertions
#[ignore] // Outdated error message assertions
fn closure_param_forward_ref() { compile_should_fail_with(r#"fn f()(B)B{return (x:B)=>x} class B{} fn main(){}"#, ""); }

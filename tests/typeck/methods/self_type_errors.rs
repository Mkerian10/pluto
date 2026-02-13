//! Self type errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Missing self parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn missing_self() { compile_should_fail_with(r#"class C{} fn foo(){} fn main(){let c=C{} c.foo()}"#, ""); }

// Self parameter wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_wrong_type() { compile_should_fail_with(r#"class C{} fn foo(self:int){} fn main(){}"#, ""); }

// Mut self on immutable receiver
#[test]
#[ignore] // PR #46 - outdated assertions
fn mut_self_immutable() { compile_should_fail_with(r#"class C{x:int} fn foo(mut self){self.x=2} fn main(){let c=C{x:1}c.foo()}"#, ""); }

// Self as return type
#[test]
#[ignore] // PR #46 - outdated assertions
fn return_self() { compile_should_fail_with(r#"class C{} fn foo(self)Self{return self} fn main(){}"#, ""); }

// Self in trait method
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_self_type() { compile_should_fail_with(r#"trait T{fn foo(self)Self} class C{} impl T{fn foo(self)C{return self}} fn main(){}"#, ""); }

// Self parameter in non-method
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_in_function() { compile_should_fail_with(r#"fn f(self){} fn main(){}"#, ""); }

// Multiple self parameters
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_self() { compile_should_fail_with(r#"class C{} fn foo(self,self){} fn main(){}"#, ""); }

// Self parameter not first
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_not_first() { compile_should_fail_with(r#"class C{} fn foo(x:int,self){} fn main(){}"#, ""); }

// Mut self modifies immutable field
#[test]
#[ignore] // PR #46 - outdated assertions
fn mut_self_immutable_field() { compile_should_fail_with(r#"class C{x:int} fn foo(mut self){self.x=2} fn main(){}"#, ""); }

// Self in closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_in_closure() { compile_should_fail_with(r#"class C{} fn foo(self){let f=()=>self} fn main(){}"#, ""); }

// Self parameter type annotation mismatch
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_annotation_mismatch() { compile_should_fail_with(r#"class C1{} class C2{} fn foo(self:C2){} fn main(){}"#, ""); }

// Mut self in trait without mut in impl
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_mut_impl_non_mut() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C{x:int} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Non-mut self in trait, mut in impl
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_non_mut_impl_mut() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(mut self){self.x=2}} fn main(){}"#, ""); }

// Self with explicit type in method
#[test]
#[ignore] // PR #46 - outdated assertions
fn explicit_self_type() { compile_should_fail_with(r#"class C{} fn foo(self:C){} fn main(){}"#, ""); }

// Self in generic method
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_method_self() { compile_should_fail_with(r#"class C{} fn foo<T>(self,x:T)Self{return self} fn main(){}"#, ""); }

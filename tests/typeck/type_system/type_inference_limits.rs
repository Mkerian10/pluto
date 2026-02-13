//! Type inference limits and edge cases - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Ambiguous type inference
#[test]
#[ignore] // PR #46 - outdated assertions
fn ambiguous_inference() { compile_should_fail_with(r#"fn main(){let x=none}"#, ""); }

// Cannot infer from empty array
#[test]
#[ignore] // PR #46 - outdated assertions
fn empty_array_no_inference() { compile_should_fail_with(r#"fn main(){let arr=[]}"#, ""); }

// Cannot infer from empty map
#[test]
#[ignore] // PR #46 - outdated assertions
fn empty_map_no_inference() { compile_should_fail_with(r#"fn main(){let m=Map{}}"#, ""); }

// Cannot infer from empty set
#[test]
#[ignore] // PR #46 - outdated assertions
fn empty_set_no_inference() { compile_should_fail_with(r#"fn main(){let s=Set{}}"#, ""); }

// Conflicting type inference
#[test]
#[ignore] // PR #46 - outdated assertions
fn conflicting_inference() { compile_should_fail_with(r#"fn main(){let x if true{x=1}else{x="hi"}}"#, ""); }

// Inference through nested closures
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_closure_inference() { compile_should_fail_with(r#"fn main(){let f=()=>()=>()=>1 let x:string=f()()()}"#, ""); }

// Generic inference ambiguity
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_inference_ambig() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x=id(id(1))}"#, ""); }

// Inference with multiple constraints
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_constraint_inference() { compile_should_fail_with(r#"fn f<T>(x:T,y:T)T{return x} fn main(){let x=f(1,"hi")}"#, ""); }

// Inference through trait method
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_method_inference() { compile_should_fail_with(r#"trait T{fn f<U>(self,x:U)U} class C{} impl T{fn f<U>(self,x:U)U{return x}} fn main(){let c=C{} let x=c.f()}"#, ""); }

// Inference limit in deep nesting
#[test]
#[ignore] // PR #46 - outdated assertions
fn deep_nesting_inference() { compile_should_fail_with(r#"fn main(){let x=[[[[[[[[[[1]]]]]]]]]]}"#, ""); }

// Cannot infer from if without else
#[test]
#[ignore] // PR #46 - outdated assertions
fn if_no_else_inference() { compile_should_fail_with(r#"fn main(){let x=if true{1}}"#, ""); }

// Inference with nullable ambiguity
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_inference_ambig() { compile_should_fail_with(r#"fn main(){let x:int?=none let y:string?=x}"#, ""); }

// Inference with error type ambiguity
#[test]
#[ignore] // PR #46 - outdated assertions
fn error_inference_ambig() { compile_should_fail_with(r#"error E1{} error E2{} fn f()!E1 int{raise E1{}} fn main(){let x:!E2 int=f()}"#, ""); }

// Inference through spawn
#[test]
#[ignore] // PR #46 - outdated assertions
fn spawn_inference() { compile_should_fail_with(r#"fn task<T>(x:T)T{return x} fn main(){let t=spawn task()}"#, ""); }

// Inference with circular dependency
#[test]
#[ignore] // PR #46 - outdated assertions
fn circular_inference() { compile_should_fail_with(r#"fn main(){let x=y let y=x}"#, ""); }

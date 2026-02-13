//! Const correctness tests - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Const field mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_field_mut() { compile_should_fail_with(r#"class C{const x:int=1} fn update(mut self){self.x=2} fn main(){}"#, ""); }

// Const variable reassignment
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_var_reassign() { compile_should_fail_with(r#"fn main(){const x=1 x=2}"#, ""); }

// Const array element mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_array_mut() { compile_should_fail_with(r#"const arr=[1,2,3] fn main(){arr[0]=5}"#, ""); }

// Const class instance mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_class_mut() { compile_should_fail_with(r#"class C{x:int} const c=C{x:1} fn main(){c.x=2}"#, ""); }

// Const in loop iteration
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_loop_iter() { compile_should_fail_with(r#"fn main(){const x=0 for i in 0..10{x=i}}"#, ""); }

// Const through reference
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_through_ref() { compile_should_fail_with(r#"fn main(){const x=1 let y=x y=2}"#, ""); }

// Const parameter mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_param_mut() { compile_should_fail_with(r#"fn f(const x:int){x=2} fn main(){}"#, ""); }

// Const in closure capture
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_closure_capture() { compile_should_fail_with(r#"fn main(){const x=1 let f=()=>{x=2}}"#, ""); }

// Const nested field mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_nested_field() { compile_should_fail_with(r#"class Inner{x:int} class Outer{const i:Inner} fn main(){let o=Outer{i:Inner{x:1}} o.i.x=2}"#, ""); }

// Const global mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn const_global_mut() { compile_should_fail_with(r#"const GLOBAL=1 fn main(){GLOBAL=2}"#, ""); }

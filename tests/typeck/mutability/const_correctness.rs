//! Const correctness tests - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Const field mutation
#[test]
fn const_field_mut() { compile_should_fail_with(r#"class C{const x:int=1
fn update(mut self){self.x=2}
}
fn main(){}"#, "expected :, found identifier"); }

// Const variable reassignment
#[test]
fn const_var_reassign() { compile_should_fail_with(r#"fn main(){const x=1 x=2}"#, "expected newline after statement"); }

// Const array element mutation
#[test]
fn const_array_mut() { compile_should_fail_with(r#"const arr=[1,2,3] fn main(){arr[0]=5}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found identifier"); }

// Const class instance mutation
#[test]
fn const_class_mut() { compile_should_fail_with(r#"class C{x:int} const c=C{x:1} fn main(){c.x=2}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found identifier"); }

// Const in loop iteration
#[test]
fn const_loop_iter() { compile_should_fail_with(r#"fn main(){const x=0 for i in 0..10{x=i}}"#, "expected newline after statement"); }

// Const through reference
#[test]
fn const_through_ref() { compile_should_fail_with(r#"fn main(){const x=1 let y=x y=2}"#, "expected newline after statement"); }

// Const parameter mutation
#[test]
fn const_param_mut() { compile_should_fail_with(r#"fn f(const x:int){x=2} fn main(){}"#, "expected :, found identifier"); }

// Const in closure capture
#[test]
fn const_closure_capture() { compile_should_fail_with(r#"fn main(){const x=1 let f=()=>{x=2}}"#, "expected newline after statement"); }

// Const nested field mutation
#[test]
fn const_nested_field() { compile_should_fail_with(r#"class Inner{x:int} class Outer{const i:Inner} fn main(){let o=Outer{i:Inner{x:1}} o.i.x=2}"#, "expected :, found identifier"); }

// Const global mutation
#[test]
fn const_global_mut() { compile_should_fail_with(r#"const GLOBAL=1 fn main(){GLOBAL=2}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found identifier"); }

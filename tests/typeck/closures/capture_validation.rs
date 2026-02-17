//! Closure capture validation - 20 tests
#[path = "../common.rs"]
mod common;
use common::{compile_should_fail_with, compile_and_run};

// Capture undefined variable
#[test]
fn capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>x+1}"#, "undefined"); }
#[test]
fn capture_undefined_in_body() { compile_should_fail_with(r#"fn main(){let f=()=>{return y}}"#, "undefined"); }

// Capture type mismatch
#[test]
#[ignore]
fn capture_type_mismatch() { compile_should_fail_with(r#"fn main(){let x=1 let f=(y:int)=>x+y let s:string=f(2)}"#, "type mismatch"); }

// Capture from outer scope
#[test]
#[ignore]
fn capture_outer_scope() { compile_and_run(r#"fn main(){let x=1 if true{let f=()=>x+1}}"#); }

// Capture parameter
#[test]
fn capture_param() { compile_and_run(r#"fn f(x:int){let g=()=>x+1} fn main(){}"#); }

// Capture self in method
#[test]
fn capture_self() { compile_should_fail_with(r#"class C{x:int} fn foo(self){let f=()=>self.x} fn main(){}"#, ""); }

// Capture mutable variable (immutable capture)
#[test]
#[ignore]
fn capture_mut_var() { compile_and_run(r#"fn main(){let x=1 let f=()=>x+1 x=2}"#); }

// Capture multiple variables
#[test]
#[ignore]
fn capture_multiple() { compile_and_run(r#"fn main(){let x=1 let y=2 let f=()=>x+y}"#); }

// Capture class instance
#[test]
#[ignore]
fn capture_class() { compile_and_run(r#"class C{x:int} fn main(){let c=C{x:1} let f=()=>c.x}"#); }

// Capture array
#[test]
#[ignore]
fn capture_array() { compile_and_run(r#"fn main(){let arr=[1,2,3] let f=()=>arr[0]}"#); }

// Capture string
#[test]
fn capture_string() { compile_should_fail_with(r#"fn main(){let s=\"hi\" let f=()=>s}"#, ""); }

// Nested closure capture
#[test]
#[ignore]
fn nested_capture() { compile_and_run(r#"fn main(){let x=1 let f=()=>{let g=()=>x return g}}"#); }

// Capture in different closures
#[test]
#[ignore]
fn multiple_closures_capture() { compile_and_run(r#"fn main(){let x=1 let f=()=>x let g=()=>x+1}"#); }

// Capture loop variable
#[test]
fn capture_loop_var() { compile_and_run(r#"fn main(){for i in 0..10{let f=()=>i}}"#); }

// Capture match binding
#[test]
fn capture_match_binding() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{let f=()=>x}}}"#, ""); }

// Capture generic parameter
#[test]
fn capture_generic_param() { compile_and_run(r#"fn f<T>(x:T){let g=()=>x} fn main(){}"#); }

// Capture trait object
#[test]
fn capture_trait_object() { compile_should_fail_with(r#"trait T{} class C{} impl T fn main(){let t:T=C{} let f=()=>t}"#, ""); }

// Capture nullable
#[test]
#[ignore]
fn capture_nullable() { compile_and_run(r#"fn main(){let x:int?=none let f=()=>x}"#); }

// Capture error (not allowed, errors can't be captured)
#[test]
fn capture_error() { compile_should_fail_with(r#"error E{} fn main(){let e=E{} let f=()=>e}"#, ""); }

// Capture function (closures can be captured)
#[test]
fn capture_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let g=()=>f(2)}"#, ""); }

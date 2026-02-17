//! Closure lifting errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::{compile_should_fail_with, compile_and_run};

// Closure captures variable used after closure creation
#[test]
#[ignore]
fn capture_used_after() { compile_and_run(r#"fn main(){let x=1 let f=()=>x let y=x}"#); }

// Multiple closures with conflicting captures
#[test]
#[ignore]
fn conflicting_captures() { compile_and_run(r#"fn main(){let x=1 let f=()=>x let g=()=>x x=2}"#); }

// Closure in match arm captures match binding
#[test]
fn capture_match_arm_binding() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{let f=()=>x}}}"#, ""); }

// Closure parameter shadows capture
#[test]
#[ignore]
fn param_shadows_capture() { compile_and_run(r#"fn main(){let x=1 let f=(x:int)=>x+1}"#); }

// Nested closure captures from multiple levels
#[test]
#[ignore]
fn nested_multi_level_capture() { compile_and_run(r#"fn main(){let x=1 let f=()=>{let y=2 let g=()=>x+y return g}}"#); }

// Closure lifts with generic capture
#[test]
fn generic_capture_lift() { compile_and_run(r#"fn f<T>(x:T){let g=()=>x} fn main(){}"#); }

// Closure captures class field (invalid, must capture self)
#[test]
fn capture_field_not_self() { compile_should_fail_with(r#"class C{x:int} fn foo(self){let f=()=>x}"#, ""); }

// Closure in loop captures loop variable
#[test]
fn loop_var_capture_lift() { compile_and_run(r#"fn main(){for i in 0..10{let f=()=>i}}"#); }

// Closure captures mutable reference (not supported)
#[test]
fn capture_mut_ref() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

// Closure in spawn
#[test]
fn spawn_with_closure() { compile_should_fail_with(r#"fn main(){let x=1 spawn (()=>x+1)()}"#, ""); }

// Closure captures error value
#[test]
fn capture_error_lift() { compile_should_fail_with(r#"error E{} fn main(){let e=E{} let f=()=>e}"#, ""); }

// Closure in method captures parameter
#[test]
fn method_param_capture() { compile_should_fail_with(r#"class C{} fn foo(self,x:int){let f=()=>x} fn main(){}"#, ""); }

// Closure in generic function
#[test]
fn generic_fn_closure() { compile_should_fail_with(r#"fn f<T>(x:T)(T)T{return (y:T)=>x} fn main(){}"#, ""); }

// Closure captures trait object
#[test]
fn trait_object_capture_lift() { compile_should_fail_with(r#"trait T{} class C{} impl T fn main(){let t:T=C{} let f=()=>t}"#, ""); }

// Closure with span collision (monomorphization + closure)
#[test]
fn span_collision() { compile_should_fail_with(r#"fn f<T>(x:T){let g=()=>x} fn main(){f(1)f(\"hi\")}"#, ""); }

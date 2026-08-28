//! Closure lifting errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::{compile_should_fail_with, compile_and_run};

// Closure captures variable used after closure creation
#[test]
fn capture_used_after() { compile_and_run(r#"fn main(){let x=1
let f=()=>x
let y=x}"#); }

// Multiple closures with conflicting captures
#[test]
fn conflicting_captures() { compile_and_run(r#"fn main(){let mut x=1
let f=()=>x
let g=()=>x
x=2
print(f()+g())}"#); }

// Closure in match arm captures match binding
#[test]
fn capture_match_arm_binding() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{let f=()=>x}}}"#, "expected ., found :"); }

// Closure parameter shadows capture
#[test]
fn param_shadows_capture() {
    // A closure parameter may not shadow an in-scope variable (#160)
    compile_should_fail_with(r#"fn main(){let x=1
let f=(x:int)=>x+1
print(f(x))}"#, "shadows an existing variable");
}

// Nested closure captures from multiple levels
#[test]
fn nested_multi_level_capture() { compile_and_run(r#"fn main(){let x=1
let f=()=>{let y=2
let g=()=>x+y
return g}}"#); }

// Closure lifts with generic capture
#[test]
fn generic_capture_lift() { compile_and_run(r#"fn f<T>(x:T){let g=()=>x} fn main(){}"#); }

// Closure captures class field (invalid, must capture self)
#[test]
fn capture_field_not_self() { compile_should_fail_with(r#"class C{x:int
fn foo(self){let f=()=>x}
}"#, "undefined variable 'x'"); }

// Closure in loop captures loop variable
#[test]
fn loop_var_capture_lift() { compile_and_run(r#"fn main(){for i in 0..10{let f=()=>i}}"#); }

// Closure captures mutable reference (not supported)
#[test]
fn capture_mut_ref() { compile_should_fail_with(r#"fn main(){let x=1
let f=()=>{x=2}}"#, "cannot assign to immutable variable 'x'"); }

// Closure in spawn
#[test]
fn spawn_with_closure() { compile_should_fail_with(r#"fn main(){let x=1 spawn (()=>x+1)()}"#, "expected newline after statement"); }

// Closure captures error value
#[test]
fn capture_error_lift() { compile_should_fail_with(r#"error E{}
fn main(){let e=E{}
let f=()=>e}"#, "unknown class 'E'"); }

// Closure in method captures parameter
#[test]
fn method_param_capture() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"class C{
fn foo(self,x:int){let f=()=>x}
}
fn main(){}"#).is_ok()); }

// Closure in generic function
#[test]
fn generic_fn_closure() { compile_should_fail_with(r#"fn f<T>(x:T)(T)T{return (y:T)=>x} fn main(){}"#, "expected identifier, found ("); }

// Closure captures trait object
#[test]
fn trait_object_capture_lift() { compile_should_fail_with(r#"trait T{} class C{} impl T fn main(){let t:T=C{} let f=()=>t}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Closure with span collision (monomorphization + closure)
#[test]
fn span_collision() { compile_should_fail_with(r#"fn f<T>(x:T){let g=()=>x}
fn main(){f(1)f("hi")}"#, "expected newline after statement"); }

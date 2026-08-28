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
fn capture_type_mismatch() { compile_should_fail_with(r#"fn main(){let x=1
let f=(y:int)=>x+y
let s:string=f(2)}"#, "type mismatch"); }

// Capture from outer scope
#[test]
fn capture_outer_scope() { compile_and_run(r#"fn main(){let x=1
if true{let f=()=>x+1}}"#); }

// Capture parameter
#[test]
fn capture_param() { compile_and_run(r#"fn f(x:int){let g=()=>x+1} fn main(){}"#); }

// Capture self in method
#[test]
fn capture_self() {
    // Capturing self in a method closure is legal (the old source put the
    // method outside the class, so it never parsed)
    compile_and_run(r#"class C{x:int
fn foo(self) int {let f=()=>self.x
return f()}} fn main(){}"#);
}

// Capture mutable variable (immutable capture)
#[test]
fn capture_mut_var() {
    // Reassigning a captured variable requires let mut (#282)
    let out = compile_and_run(r#"fn main(){let mut x=1
let f=()=>x+1
x=2
print(f())}"#);
    assert_eq!(out, 0);
}

// Capture multiple variables
#[test]
fn capture_multiple() { compile_and_run(r#"fn main(){let x=1
let y=2
let f=()=>x+y}"#); }

// Capture class instance
#[test]
fn capture_class() { compile_and_run(r#"class C{x:int}
fn main(){let c=C{x:1}
let f=()=>c.x}"#); }

// Capture array
#[test]
fn capture_array() { compile_and_run(r#"fn main(){let arr=[1,2,3]
let f=()=>arr[0]}"#); }

// Capture string
#[test]
fn capture_string() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn main(){let s="hi"
let f=()=>s}"#).is_ok()); }

// Nested closure capture
#[test]
fn nested_capture() { compile_and_run(r#"fn main(){let x=1
let f=()=>{let g=()=>x
return g}}"#); }

// Capture in different closures
#[test]
fn multiple_closures_capture() { compile_and_run(r#"fn main(){let x=1
let f=()=>x
let g=()=>x+1}"#); }

// Capture loop variable
#[test]
fn capture_loop_var() { compile_and_run(r#"fn main(){for i in 0..10{let f=()=>i}}"#); }

// Capture match binding
#[test]
fn capture_match_binding() {
    // Capturing a match binding in a closure is legal
    compile_and_run(r#"enum E{A{x:int}}
fn main(){
let e = E.A{x:1}
match e {E.A{x}{let f=()=>x
print(f())}}}"#);
}

// Capture generic parameter
#[test]
fn capture_generic_param() { compile_and_run(r#"fn f<T>(x:T){let g=()=>x} fn main(){}"#); }

// Capture trait object
#[test]
fn capture_trait_object() { compile_should_fail_with(r#"trait T{} class C{} impl T fn main(){let t:T=C{} let f=()=>t}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Capture nullable
#[test]
fn capture_nullable() { compile_and_run(r#"fn main(){let x:int?=none
let f=()=>x}"#); }

// Capture error (not allowed, errors can't be captured)
#[test]
fn capture_error() { compile_should_fail_with(r#"error E{}
fn main(){let e=E{}
let f=()=>e}"#, "unknown class 'E'"); }

// Capture function (closures can be captured)
#[test]
fn capture_closure() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn main(){let f=(x:int)=>x+1
let g=()=>f(2)}"#).is_ok()); }

//! Variable capture in closures - 12 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Capture undefined variable
#[test]
fn capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>x}"#, "undefined"); }

// Capture from wrong scope
#[test]
fn capture_wrong_scope() { compile_should_fail_with(r#"fn main(){let f if true{let x=1 f=()=>x}let y=f()}"#, "expected =, found if"); }

// Capture loop variable
#[test]
fn capture_loop_var() { compile_should_fail_with(r#"fn main(){let f for i in 0..10{f=()=>i}}"#, "expected =, found for"); }

// Capture match binding
#[test]
fn capture_match_binding() {
    // A match binding does not escape its arm
    compile_should_fail_with(r#"enum E{A{x:int}}
fn main(){
let e = E.A{x:1}
match e {E.A{x}{}}
let f = () => x}"#, "undefined variable 'x'"); }

// Multi-level capture
#[test]
fn multi_level_capture() { compile_should_fail_with(r#"fn main(){let x=1
let f=()=>{let g=()=>y
return g}}"#, "undefined variable 'y'"); }

// Capture in nested closure
#[test]
fn nested_capture() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1 let g=()=>x return g}let y=f()()}"#, "expected newline after statement"); }

// Capture self outside method
#[test]
fn capture_self() { compile_should_fail_with(r#"fn main(){let f=()=>self.x}"#, "undefined variable 'self'"); }

// Capture parameter
#[test]
fn capture_param() { compile_should_fail_with(r#"fn f(x:int){let g=()=>y} fn main(){}"#, "undefined variable 'y'"); }

// Capture across functions
#[test]
fn capture_cross_function() { compile_should_fail_with(r#"fn f(){let x=1} fn g(){let h=()=>x} fn main(){}"#, "undefined variable 'x'"); }

// Capture with type error
#[test]
fn capture_type_error() { compile_should_fail_with(r#"fn main(){let x=1
let f=(y:int)=>x+y
let z=f("hi")}"#, "argument 1 of 'f': expected int, found string"); }

// Capture in spawn
#[test]
fn capture_in_spawn() { compile_should_fail_with(r#"fn task()int{return x} fn main(){let x=1 spawn task()}"#, "expected newline after statement"); }

// Capture with shadowing
#[test]
fn capture_shadowed() { compile_should_fail_with(r#"fn main(){let x=1
let f=()=>{let x=2
return y}}"#, "undefined variable 'y'"); }

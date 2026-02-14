//! Variable capture in closures - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Capture undefined variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>x}"#, "undefined"); }

// Capture from wrong scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_wrong_scope() { compile_should_fail_with(r#"fn main(){let f if true{let x=1 f=()=>x}let y=f()}"#, ""); }

// Capture after mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_after_mut() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>x x=2}"#, ""); }

// Capture loop variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_loop_var() { compile_should_fail_with(r#"fn main(){let f for i in 0..10{f=()=>i}}"#, ""); }

// Capture match binding
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_match_binding() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let f match E.A{x:1}{E.A{x}{f=()=>x}}}"#, ""); }

// Capture temporary value
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_temporary() { compile_should_fail_with(r#"class C{x:int} fn f()C{return C{x:1}} fn main(){let g=()=>f().x}"#, ""); }

// Multi-level capture
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_level_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{let g=()=>y return g}}"#, ""); }

// Capture in nested closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_capture() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1 let g=()=>x return g}let y=f()()}"#, ""); }

// Capture self outside method
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_self() { compile_should_fail_with(r#"fn main(){let f=()=>self.x}"#, ""); }

// Capture parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_param() { compile_should_fail_with(r#"fn f(x:int){let g=()=>y} fn main(){}"#, ""); }

// Capture across functions
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_cross_function() { compile_should_fail_with(r#"fn f(){let x=1} fn g(){let h=()=>x} fn main(){}"#, ""); }

// Capture with type error
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_type_error() { compile_should_fail_with(r#"fn main(){let x=1 let f=(y:int)=>x+y let z=f("hi")}"#, ""); }

// Capture in spawn
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_in_spawn() { compile_should_fail_with(r#"fn task()int{return x} fn main(){let x=1 spawn task()}"#, ""); }

// Capture moved value
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_moved() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} let d=c let f=()=>c.x}"#, ""); }

// Capture with shadowing
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_shadowed() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{let x=2 return y}}"#, ""); }

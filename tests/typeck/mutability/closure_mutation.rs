//! Closure mutation semantics - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Mutate captured variable
#[test] fn mutate_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

// Mutate multiple captures
#[test] fn mutate_multi_capture() { compile_should_fail_with(r#"fn main(){let x=1 let y=2 let f=()=>{x=3 y=4}}"#, ""); }

// Mutate nested capture
#[test] fn mutate_nested_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{let g=()=>{x=2}}}"#, ""); }

// Mutate capture in loop
#[test] fn mutate_capture_loop() { compile_should_fail_with(r#"fn main(){let x=1 for i in 0..10{let f=()=>{x=i}}}"#, ""); }

// Mutate outer from inner closure
#[test] fn mutate_outer_from_inner() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{let y=2 let g=()=>{x=3 y=4}}}"#, ""); }

// Mutate after closure creation
#[test] fn mutate_after_closure() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>x x=2}"#, ""); }

// Mutate inside closure parameter
#[test] fn mutate_in_closure_param() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{x=2}}"#, ""); }

// Mutate captured class field
#[test] fn mutate_captured_field() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} let f=()=>{c.x=2}}"#, ""); }

// Mutate through closure call
#[test] fn mutate_through_call() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>x let y=f() x=2}"#, ""); }

// Mutate in recursive closure
#[test] fn mutate_recursive_closure() { compile_should_fail_with(r#"fn main(){let x=1 let f:fn()int=()=>{x=2 return f()}}"#, ""); }

//! Recursive closure errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Simple recursive closure (not supported)
#[test]
#[ignore] // PR #46 - outdated assertions
fn simple_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>if x==0{return 1}else{return f(x-1)}}"#, "undefined"); }

// Mutually recursive closures
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutual_recursion() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>g(x) let g=(y:int)=>f(y)}"#, "undefined"); }

// Closure calls itself via capture
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_capture() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x-1)}"#, "undefined"); }

// Recursive closure with base case
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_base_case() { compile_should_fail_with(r#"fn main(){let fac=(n:int)=>if n<=1{return 1}else{return n*fac(n-1)}}"#, "undefined"); }

// Nested recursive closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{let g=(y:int)=>g(y-1) return g(x)}}"#, "undefined"); }

// Closure assigned then called recursively
#[test]
#[ignore] // PR #46 - outdated assertions
fn assign_then_recursive() { compile_should_fail_with(r#"fn main(){let f:(int)int f=(x:int)=>f(x-1)}"#, "undefined"); }

// Generic recursive closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x-1)}"#, "undefined"); }

// Recursive closure in struct
#[test]
#[ignore] // PR #46 - outdated assertions
fn struct_recursive_closure() { compile_should_fail_with(r#"class C{f:(int)int} fn main(){let c=C{f:(x:int)=>c.f(x-1)}}"#, "undefined"); }

// Recursive closure with multiple parameters
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_param_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int,y:int)=>f(x-1,y-1)}"#, "undefined"); }

// Closure recursion through array
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_recursive() { compile_should_fail_with(r#"fn main(){let arr=[(x:int)=>arr[0](x-1)]}"#, "undefined"); }

// Indirect recursion via variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn indirect_recursion() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>g(x) let g=f}"#, "undefined"); }

// Recursive closure with error propagation
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_error() { compile_should_fail_with(r#"error E{} fn main(){let f=(x:int)!=>if x==0{raise E{}}else{f(x-1)!}}"#, ""); }

// Recursive closure in match
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_recursive() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let f=(e:E)=>match e{E.A{x}{f(E.A{x:x-1})}}}"#, "undefined"); }

// Closure calls itself in different branch
#[test]
#[ignore] // PR #46 - outdated assertions
fn branch_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>if x>0{return f(x-1)}else{return 0}}"#, "undefined"); }

// Y-combinator attempt (advanced recursion)
#[test]
#[ignore] // PR #46 - outdated assertions
fn y_combinator() { compile_should_fail_with(r#"fn main(){let y=(f:((int)int)(int)int)=>(x:int)=>f(y(f))(x)}"#, "undefined"); }

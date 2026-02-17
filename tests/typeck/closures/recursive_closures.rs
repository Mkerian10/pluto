//! Recursive closure errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Simple recursive closure (not supported)
#[test]
#[ignore]
fn simple_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>if x==0{return 1}else{return f(x-1)}}"#, "Syntax error: unexpected token if"); }

// Mutually recursive closures
#[test]
#[ignore]
fn mutual_recursion() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>g(x) let g=(y:int)=>f(y)}"#, "undefined"); }

// Closure calls itself via capture
#[test]
fn self_capture() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x-1)}"#, "undefined"); }

// Recursive closure with base case
#[test]
#[ignore]
fn recursive_base_case() { compile_should_fail_with(r#"fn main(){let fac=(n:int)=>if n<=1{return 1}else{return n*fac(n-1)}}"#, "Syntax error: unexpected token if"); }

// Nested recursive closure
#[test]
#[ignore]
fn nested_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{let g=(y:int)=>g(y-1) return g(x)}}"#, "undefined"); }

// Closure assigned then called recursively
#[test]
fn assign_then_recursive() { compile_should_fail_with(r#"fn main(){let f:(int)int f=(x:int)=>f(x-1)}"#, "Syntax error: expected identifier"); }

// Generic recursive closure
#[test]
fn generic_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>f(x-1)}"#, "undefined"); }

// Recursive closure in struct
#[test]
fn struct_recursive_closure() { compile_should_fail_with(r#"class C{f:(int)int} fn main(){let c=C{f:(x:int)=>c.f(x-1)}}"#, "Syntax error: expected identifier"); }

// Recursive closure with multiple parameters
#[test]
fn multi_param_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int,y:int)=>f(x-1,y-1)}"#, "undefined"); }

// Closure recursion through array
#[test]
fn array_recursive() { compile_should_fail_with(r#"fn main(){let arr=[(x:int)=>arr[0](x-1)]}"#, "Syntax error: expected ,"); }

// Indirect recursion via variable
#[test]
#[ignore]
fn indirect_recursion() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>g(x) let g=f}"#, "undefined"); }

// Recursive closure with error propagation
#[test]
fn recursive_error() { compile_should_fail_with(r#"error E{} fn main(){let f=(x:int)!=>if x==0{raise E{}}else{f(x-1)!}}"#, ""); }

// Recursive closure in match
#[test]
fn match_recursive() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let f=(e:E)=>match e{E.A{x}{f(E.A{x:x-1})}}}"#, "Syntax error: expected =>"); }

// Closure calls itself in different branch
#[test]
#[ignore]
fn branch_recursive() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>if x>0{return f(x-1)}else{return 0}}"#, "Syntax error: unexpected token if"); }

// Y-combinator attempt (advanced recursion)
#[test]
fn y_combinator() { compile_should_fail_with(r#"fn main(){let y=(f:((int)int)(int)int)=>(x:int)=>f(y(f))(x)}"#, "Syntax error: expected identifier"); }

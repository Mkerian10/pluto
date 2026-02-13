//! Closure type checking - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Closure parameter type mismatch
#[test]
fn param_type_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 f("hi")}"#, "expected int, found string"); }
#[test]
fn param_count_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 f(1,2)}"#, ""); }

// Closure return type mismatch
#[test]
fn return_type_mismatch() { compile_should_fail_with(r#"fn main(){let f:fn(int) int=(x:int)=>"hi"}"#, "type mismatch"); }

// Closure assigned to wrong type
#[test]
fn assign_wrong_type() { compile_should_fail_with(r#"fn main(){let f:int=(x:int)=>x+1}"#, "type mismatch"); }

// Call non-closure as function
#[test]
fn call_non_closure() { compile_should_fail_with(r#"fn main(){let x=1
x(2)}"#, "undefined function"); }

// Closure in function parameter
#[test]
fn closure_param_type_mismatch() { compile_should_fail_with(r#"fn f(g:fn(int) int){} fn main(){f((x:string)=>1)}"#, "expected fn(int) int, found fn(string) int"); }

// Closure return from function
#[test]
fn return_closure_type_mismatch() { compile_should_fail_with(r#"fn f() fn(int) int{return (x:string)=>1} fn main(){}"#, "type mismatch"); }

// Multiple closure parameters
#[test]
fn multi_param_type_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int,y:string)=>x f(1,2)}"#, "expected string, found int"); }

// Closure with no parameters
#[test]
fn no_param_called_with_arg() { compile_should_fail_with(r#"fn main(){let f=()=>1 f(2)}"#, ""); }

// Closure type annotation wrong
#[test]
fn wrong_annotation() { compile_should_fail_with(r#"fn main(){let f:fn(int) string=(x:int)=>x+1}"#, "type mismatch"); }

// Closure body type error
#[test]
fn body_type_error() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+"hi"}"#, "type mismatch"); }

// Closure parameter used incorrectly
#[test]
fn param_used_wrong() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x.len()}"#, ""); }

// Nested closure type mismatch
#[test]
fn nested_type_mismatch() { compile_should_fail_with(r#"fn main(){let f=()=>(x:int)=>"hi" let g:fn() fn(int) int=f}"#, "type mismatch"); }

// Closure in array
#[test]
fn closure_array_type_mismatch() { compile_should_fail_with(r#"fn main(){let arr:[fn(int) int]=[(x:int)=>x+1,(x:string)=>1]}"#, "type mismatch"); }

// Closure in struct
#[test]
fn closure_in_struct() { compile_should_fail_with(r#"class C{f:fn(int) int} fn main(){let c=C{f:(x:string)=>1}}"#, "expected fn(int) int, found fn(string) int"); }

// Generic closure
#[test]
fn generic_closure_type() { compile_should_fail_with(r#"fn f<T>(g:fn(T) T,x:T)T{return g(x)} fn main(){f((x:int)=>"hi",1)}"#, "cannot infer type parameters"); }

// Closure with void return
#[test]
fn void_return_type() { compile_should_fail_with(r#"fn main(){let f:fn(int) int=(x:int)=>{print("hi")}}"#, "type mismatch"); }

// Closure missing return
#[test]
fn missing_return() { compile_should_fail_with(r#"fn main(){let f:fn(int) int=(x:int)=>{let y=x}}"#, "expected fn(int) int, found fn(int) void"); }

// Closure with error propagation
#[test]
fn closure_error_type() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn main(){let g=()=>f()!}"#, ""); }

// Recursive closure type (not directly supported)
#[test]
fn recursive_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>if x==0{return 1}else{return f(x-1)}}"#, "unexpected token if"); }

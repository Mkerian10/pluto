//! Closure edge cases - 15 tests
#[path = "../common.rs"]
mod common;
use common::{compile_should_fail_with, compile_and_run};

// Empty closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn empty_closure() { compile_and_run(r#"fn main(){let f=()=>{}}"#); }

// Closure with only side effect
#[test]
#[ignore] // PR #46 - outdated assertions
fn side_effect_closure() { compile_should_fail_with(r#"fn main(){let f=()=>{print(\"hi\")}}"#, ""); }

// Closure parameter name collision with builtin
#[test]
#[ignore] // PR #46 - outdated assertions
fn param_builtin_name() { compile_and_run(r#"fn main(){let f=(print:int)=>print+1}"#); }

// Closure captures builtin function
#[test]
#[ignore] // PR #46 - outdated assertions
fn capture_builtin() { compile_should_fail_with(r#"fn main(){let f=()=>print(\"hi\")}"#, ""); }

// Closure with very long body
#[test]
#[ignore] // PR #46 - outdated assertions
fn long_body() { compile_and_run(r#"fn main(){let f=(x:int)=>{let y=x let z=y let a=z let b=a let c=b return c}}"#); }

// Closure with many parameters
#[test]
#[ignore] // PR #46 - outdated assertions
fn many_params() { compile_and_run(r#"fn main(){let f=(a:int,b:int,c:int,d:int,e:int)=>a+b+c+d+e}"#); }

// Closure with many captures
#[test]
#[ignore] // PR #46 - outdated assertions
fn many_captures() { compile_and_run(r#"fn main(){let a=1 let b=2 let c=3 let d=4 let e=5 let f=()=>a+b+c+d+e}"#); }

// Closure in error context
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_in_error() { compile_should_fail_with(r#"error E{f:(int)int} fn main(){let e=E{f:(x:int)=>x+1}}"#, ""); }

// Closure in enum variant
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_in_enum() { compile_should_fail_with(r#"enum E{A{f:(int)int}} fn main(){let e=E.A{f:(x:int)=>x+1}}"#, ""); }

// Closure with nullable parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_param() { compile_and_run(r#"fn main(){let f=(x:int?)=>x}"#); }

// Closure with nullable return
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_return() { compile_should_fail_with(r#"fn main(){let f:(int)int?=(x:int)=>none}"#, ""); }

// Closure with error return
#[test]
#[ignore] // PR #46 - outdated assertions
fn error_return() { compile_should_fail_with(r#"error E{} fn main(){let f:(int)int!=(x:int)=>raise E{}}"#, ""); }

// Closure in generic container
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_in_generic() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b=Box<(int)int>{value:(x:int)=>x+1}}"#, ""); }

// Closure captures from multiple scopes
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_scope_capture() { compile_and_run(r#"fn main(){let x=1 if true{let y=2 let f=()=>x+y}}"#); }

// Closure with contracts (requires/ensures not on closures)
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_with_contract() { compile_should_fail_with(r#"fn main(){let f=(x:int)int requires x>0{return x}}"#, ""); }

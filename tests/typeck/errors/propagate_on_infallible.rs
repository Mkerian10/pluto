//! Propagate on infallible tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic invalid propagation
#[test]
#[ignore]
fn propagate_on_safe_call() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){let x=f()!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_literal() { compile_should_fail_with(r#"fn main(){let x=42!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_binop() { compile_should_fail_with(r#"fn main(){let x=(1+2)!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_string() { compile_should_fail_with(r#"fn main(){let s=\"hi\"!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_array() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3]!}"#, "cannot propagate"); }

// Propagate on safe method calls
#[test]
#[ignore]
fn propagate_on_safe_method() { compile_should_fail_with(r#"class C{x:int fn foo(self)int{return 1}} fn main(){let c=C{x:1}let x=c.foo()!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_builtin_method() { compile_should_fail_with(r#"fn main(){let s=\"hi\" let x=s.len()!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_array_method() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] let x=arr.len()!}"#, "cannot propagate"); }

// Propagate on safe builtin functions
#[test]
#[ignore]
fn propagate_on_print() { compile_should_fail_with(r#"fn main(){print(\"hi\")!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_abs() { compile_should_fail_with(r#"fn main(){let x=abs(-5)!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_min() { compile_should_fail_with(r#"fn main(){let x=min(1,2)!}"#, "cannot propagate"); }

// Propagate in expressions
#[test]
#[ignore]
fn propagate_on_field_access() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1}let x=c.x!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_index() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] let x=arr[0]!}"#, "cannot propagate"); }
#[test]
#[ignore]
fn propagate_on_closure_call() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let y=f(5)!}"#, "cannot propagate"); }

// Propagate on control flow that doesn't raise
#[test]
#[ignore]
fn propagate_on_if_expr() { compile_should_fail_with(r#"fn main(){let x=(if true{1}else{2})!}"#, "cannot propagate"); }

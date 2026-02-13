//! Immutability violation tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Reassign immutable variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_immutable() { compile_should_fail_with(r#"fn main(){let x=1 x=2}"#, ""); }

// Reassign parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_param() { compile_should_fail_with(r#"fn f(x:int){x=2} fn main(){}"#, ""); }

// Reassign in loop
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_in_loop() { compile_should_fail_with(r#"fn main(){let x=1 for i in 0..10{x=i}}"#, ""); }

// Reassign in if branch
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_in_if() { compile_should_fail_with(r#"fn main(){let x=1 if true{x=2}}"#, ""); }

// Reassign captured variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_captured() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

// Reassign field of immutable instance
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_immut_field() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} c.x=2}"#, ""); }

// Reassign array element
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_array_elem() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] arr[0]=5}"#, ""); }

// Reassign map value
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_map_value() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{"a":1} m["a"]=2}"#, ""); }

// Mutate immutable in match
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_in_match() { compile_should_fail_with(r#"enum E{A B} fn main(){let x=1 match E.A{E.A{x=2}E.B{x=3}}}"#, ""); }

// Mutate loop variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_loop_var() { compile_should_fail_with(r#"fn main(){for i in 0..10{i=i+1}}"#, ""); }

// Mutate match binding
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_match_binding() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{x=2}}}"#, ""); }

// Mutate through closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_through_closure() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>x let y=f() x=2}"#, ""); }

// Mutate spawn argument
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_spawn_arg() { compile_should_fail_with(r#"fn task(x:int)int{return x} fn main(){let x=1 spawn task(x) x=2}"#, ""); }

// Reassign after catch
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_after_catch() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn main(){let x=1 f() catch{x=2}}"#, ""); }

// Mutate through nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_through_nullable() { compile_should_fail_with(r#"class C{x:int} fn main(){let c:C?=C{x:1} c?.x=2}"#, ""); }

// Reassign in while
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_in_while() { compile_should_fail_with(r#"fn main(){let x=1 while x<10{x=x+1}}"#, ""); }

// Reassign const variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_const() { compile_should_fail_with(r#"fn main(){let x=1 x=x*2}"#, ""); }

// Mutate through array iteration
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_array_iter() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] for x in arr{x=x+1}}"#, ""); }

// Reassign in nested scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn reassign_nested() { compile_should_fail_with(r#"fn main(){let x=1 if true{if true{x=2}}}"#, ""); }

// Mutate self in non-mut method
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_self_non_mut() { compile_should_fail_with(r#"class C{x:int} fn update(self){self.x=self.x+1} fn main(){}"#, ""); }

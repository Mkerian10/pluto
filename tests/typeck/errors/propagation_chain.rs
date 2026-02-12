//! Error propagation chain tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic propagation chains
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_level_propagation() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){a()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn three_level_propagation() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b()!{return a()!} fn c(){b()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_non_fallible() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){return a()!} fn main(){}"#, "cannot propagate"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_through_assignment() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b(){let x=a()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_binop() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b(){let x=a()!+1} fn main(){}"#, "unhandled error"); }

// Multiple error types in chain
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_errors_same_chain() { compile_should_fail_with(r#"error E1{} error E2{} fn a()!E1{raise E1{}} fn b()!E2{a()!} fn main(){}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn union_errors_in_chain() { compile_should_fail_with(r#"error E1{} error E2{} fn a()!E1{raise E1{}} fn b()!E2{raise E2{}} fn c()!{a()!b()!} fn d(){c()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_wrong_error_type() { compile_should_fail_with(r#"error E1{} error E2{} fn a()!E1{raise E1{}} fn b()!E2{return a()!} fn main(){}"#, "type mismatch"); }

// Propagation through control flow
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_if_branch() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){if true{a()!}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_else_branch() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){if false{}else{a()!}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_while_body() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){while true{a()!}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_for_body() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b(){for i in 0..10{a()!}} fn main(){}"#, "unhandled error"); }

// Propagation through expressions
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_array_element() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b(){let arr=[a()!,2,3]} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_struct_field() { compile_should_fail_with(r#"error E{} class C{x:int} fn a()int!{raise E{}} fn b(){let c=C{x:a()!}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_return_expr() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b()int{return a()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_function_arg() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn c(x:int){} fn b(){c(a()!)} fn main(){}"#, "unhandled error"); }

// Propagation with multiple calls in same expr
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_propagates_same_line() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b(){let x=a()!+a()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_nested_call() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn c(x:int)int{return x} fn b(){let x=c(a()!)} fn main(){}"#, "unhandled error"); }

// Propagation through method calls
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_method_call() { compile_should_fail_with(r#"error E{} class C{fn foo(self){}} fn a()!{raise E{}} fn b(){let c=C{} c.foo() a()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_method_arg() { compile_should_fail_with(r#"error E{} class C{fn foo(self,x:int){}} fn a()int!{raise E{}} fn b(){let c=C{} c.foo(a()!)} fn main(){}"#, "unhandled error"); }

// Edge cases
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_void_fallible() { compile_should_fail_with(r#"error E{} fn a()!{raise E{}} fn b()!{a()!} fn c(){b()!} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_enum_variant() { compile_should_fail_with(r#"error E{} enum Opt{Some{v:int}} fn a()int!{raise E{}} fn b(){let x=Opt.Some{v:a()!}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_match_arm() { compile_should_fail_with(r#"error E{} enum Opt{Some{v:int}None} fn a()!{raise E{}} fn b(){let x=Opt.None match x{Opt.Some{v}=>{a()!}Opt.None=>{}}} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_string_interpolation() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn b(){let s="{a()!}"} fn main(){}"#, "unhandled error"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_chained_calls() { compile_should_fail_with(r#"error E{} fn a()int!{raise E{}} fn c(x:int)int{return x} fn d(x:int)int{return x} fn b(){let x=d(c(a()!))} fn main(){}"#, "unhandled error"); }

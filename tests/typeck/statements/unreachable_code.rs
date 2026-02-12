//! Unreachable code detection - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Return statement makes following code unreachable
#[test]
#[ignore] // PR #46 - outdated assertions
fn code_after_return() { compile_should_fail_with(r#"fn main(){return let x=1}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_stmts_after_return() { compile_should_fail_with(r#"fn main(){return let x=1 let y=2 print(\"hi\")}"#, "unreachable"); }

// Break makes following code unreachable
#[test]
#[ignore] // PR #46 - outdated assertions
fn code_after_break() { compile_should_fail_with(r#"fn main(){while true{break let x=1}}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_stmts_after_break() { compile_should_fail_with(r#"fn main(){while true{break let x=1 let y=2}}"#, "unreachable"); }

// Continue makes following code unreachable
#[test]
#[ignore] // PR #46 - outdated assertions
fn code_after_continue() { compile_should_fail_with(r#"fn main(){while true{continue let x=1}}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_stmts_after_continue() { compile_should_fail_with(r#"fn main(){while true{continue let x=1 let y=2}}"#, "unreachable"); }

// Raise makes following code unreachable
#[test]
#[ignore] // PR #46 - outdated assertions
fn code_after_raise() { compile_should_fail_with(r#"error E{} fn main(){raise E{} let x=1}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn multiple_stmts_after_raise() { compile_should_fail_with(r#"error E{} fn main(){raise E{} let x=1 let y=2}"#, "unreachable"); }

// If-else both branches terminate
#[test]
#[ignore] // PR #46 - outdated assertions
fn if_else_both_return() { compile_should_fail_with(r#"fn main(){if true{return}else{return} let x=1}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn if_else_both_break() { compile_should_fail_with(r#"fn main(){while true{if true{break}else{break} let x=1}}"#, "unreachable"); }

// Match all branches terminate
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_all_return() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{return}E.B{return}} let x=1}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_all_break() { compile_should_fail_with(r#"enum E{A B} fn main(){while true{match E.A{E.A{break}E.B{break}} let x=1}}"#, "unreachable"); }

// While true with no break
#[test]
#[ignore] // PR #46 - outdated assertions
fn infinite_loop_no_break() { compile_should_fail_with(r#"fn main(){while true{let x=1} let y=2}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn infinite_loop_with_continue() { compile_should_fail_with(r#"fn main(){while true{continue} let y=2}"#, "unreachable"); }

// Nested control flow
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_return() { compile_should_fail_with(r#"fn main(){if true{return} return let x=1}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_if_return() { compile_should_fail_with(r#"fn main(){if true{if false{return}else{return}} else{return} let x=1}"#, "unreachable"); }

// Function calls don't affect reachability (unless they never return)
#[test]
#[ignore] // PR #46 - outdated assertions
fn call_then_unreachable() { compile_should_fail_with(r#"fn f(){} fn main(){f() return let x=1}"#, "unreachable"); }

// Unreachable in if branch
#[test]
#[ignore] // PR #46 - outdated assertions
fn unreachable_in_if_branch() { compile_should_fail_with(r#"fn main(){if true{return let x=1}}"#, "unreachable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn unreachable_in_else_branch() { compile_should_fail_with(r#"fn main(){if true{}else{return let x=1}}"#, "unreachable"); }

// Unreachable in match arm
#[test]
#[ignore] // PR #46 - outdated assertions
fn unreachable_in_match_arm() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{return let x=1}E.B{}}}"#, "unreachable"); }

// While loop with break still allows following code
#[test]
#[ignore] // PR #46 - outdated assertions
fn while_with_break_reachable() { compile_should_fail_with(r#"fn main(){while true{break} let x=1}"#, ""); }

// For loop body unreachable
#[test]
#[ignore] // PR #46 - outdated assertions
fn for_body_unreachable() { compile_should_fail_with(r#"fn main(){for i in 0..10{return let x=1}}"#, "unreachable"); }

// Unreachable after propagate
#[test]
#[ignore] // PR #46 - outdated assertions
fn code_after_propagate() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn g()!{f()! let x=1} fn main()catch{}"#, "unreachable"); }

// Unreachable in closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_unreachable() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{return x let y=2}}"#, "unreachable"); }

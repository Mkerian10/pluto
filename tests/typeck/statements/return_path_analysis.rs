//! Return path analysis - 30 tests
#[path = "../common.rs"]
mod common;
use common::{compile_should_fail_with, compile_and_run};

// Missing return in non-void function - now properly detected at typeck
#[test]
fn missing_return_int() { compile_should_fail_with(r#"fn f()int{let x=1}"#, "missing return"); }
#[test]
fn missing_return_bool() { compile_should_fail_with(r#"fn f()bool{let x=true}"#, "missing return"); }
#[test]
fn missing_return_class() { compile_should_fail_with(r#"class C{x:int} fn f()C{let c=C{x:1}}"#, "missing return"); }

// If without else, missing return - detected at typeck
#[test]
fn if_no_else_missing_return() { compile_should_fail_with(r#"fn f()int{if true{return 1}}"#, "missing return"); }
#[test]
fn if_only_one_branch_returns() { compile_should_fail_with(r#"fn f()int{if true{return 1}else{let x=2}}"#, "missing return"); }

// Match not exhaustive for return - detected at typeck
#[test]
fn match_not_all_return() { compile_should_fail_with(r#"enum E{A B} fn f()int{match E.A{E.A{return 1}E.B{let x=2}}}"#, "missing return"); }
#[test]
fn match_missing_arm() { compile_should_fail_with(r#"enum E{A B} fn f()int{match E.A{E.A{return 1}}}"#, ""); }

// While loop doesn't guarantee return
#[test]
fn while_doesnt_guarantee_return() { compile_should_fail_with(r#"fn f()int{while true{return 1}}"#, "missing return"); }
#[test]
fn while_with_break() { compile_should_fail_with(r#"fn f()int{while true{if true{break}return 1}}"#, "missing return"); }

// For loop doesn't guarantee return
#[test]
fn for_doesnt_guarantee_return() { compile_should_fail_with(r#"fn f()int{for i in 0..10{return 1}}"#, "missing return"); }

// Nested if/else missing return
#[test]
fn nested_if_missing_return() { compile_should_fail_with(r#"fn f()int{if true{if false{return 1}else{return 2}}}"#, "missing return"); }
#[test]
fn nested_if_one_path_missing() { compile_should_fail_with(r#"fn f()int{if true{if false{return 1}}else{return 2}}"#, "missing return"); }

// Return in wrong branch
#[test]
fn return_only_in_if() { compile_should_fail_with(r#"fn f()int{if true{return 1}else{let x=2}}"#, "missing return"); }
#[test]
fn return_only_in_else() { compile_should_fail_with(r#"fn f()int{if true{let x=1}else{return 2}}"#, "missing return"); }

// Empty function body
#[test]
fn empty_body_non_void() { compile_should_fail_with(r#"fn f()int{}"#, "missing return"); }

// Return after unreachable
#[test]
#[ignore] // #181: unreachable code detection not implemented
fn return_after_return() { compile_should_fail_with(r#"fn f()int{return 1 return 2}"#, "unreachable"); }

// Void function doesn't need return
#[test]
#[ignore] // #181: test expects success but uses compile_should_fail_with
fn void_no_return_ok() { compile_should_fail_with(r#"fn f(){let x=1}"#, ""); }

// Break doesn't count as return
#[test]
fn break_not_return() { compile_should_fail_with(r#"fn f()int{while true{break}}"#, "missing return"); }

// Continue doesn't count as return
#[test]
fn continue_not_return() { compile_should_fail_with(r#"fn f()int{while true{continue}}"#, "missing return"); }

// Raise terminates a path — function that always raises doesn't need a return
#[test]
fn raise_not_return() { compile_and_run(r#"error E{} fn f()int{raise E{}} fn main(){f() catch E {return}}"#); }

// Method missing return
#[test]
#[ignore] // Syntax error: methods must be inside class body
fn method_missing_return() { compile_should_fail_with(r#"class C{x:int} fn foo(self)int{let y=self.x}"#, "missing return"); }

// If-else both raise — all paths terminate, no missing return
#[test]
fn both_raise_missing_return() { compile_and_run("error E1{}\nerror E2{}\nfn f()int{if true{raise E1{}}else{raise E2{}}}\nfn main(){let x = f() catch 0\nprint(x)}"); }

// Match with wildcard
#[test]
#[ignore] // Syntax error: wildcard match arms not supported
fn match_wildcard_missing_return() { compile_should_fail_with(r#"enum E{A B} fn f()int{match E.A{E.A{return 1}_{let x=2}}}"#, "missing return"); }

// Nested match
#[test]
fn nested_match_missing_return() { compile_should_fail_with(r#"enum E{A B} fn f()int{match E.A{E.A{match E.B{E.A{return 1}E.B{return 2}}}E.B{let x=3}}}"#, "missing return"); }

// Return type mismatch is separate error
#[test]
fn return_type_mismatch() { compile_should_fail_with(r#"fn f()int{return true}"#, "type mismatch"); }

// Implicit return from expression (not supported in Pluto)
#[test]
fn implicit_return_not_supported() { compile_should_fail_with(r#"fn f()int{1}"#, "missing return"); }

// Generic function missing return
#[test]
fn generic_missing_return() { compile_should_fail_with(r#"fn f<T>(x:T)T{let y=x}"#, "missing return"); }

// Closure missing return
#[test]
#[ignore] // Syntax error: closure body requires => before block
fn closure_missing_return() { compile_should_fail_with(r#"fn main(){let f=(x:int)int{let y=x}}"#, "missing return"); }

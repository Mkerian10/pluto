//! Break/continue validation - 20 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Break outside loop
#[test]
fn break_outside_loop() { compile_should_fail_with(r#"fn main(){break}"#, "'break' can only be used inside a loop"); }
#[test]
fn break_in_function() { compile_should_fail_with(r#"fn f(){break} fn main(){}"#, "'break' can only be used inside a loop"); }
#[test]
fn break_in_if() { compile_should_fail_with(r#"fn main(){if true{break}}"#, "'break' can only be used inside a loop"); }

// Continue outside loop
#[test]
fn continue_outside_loop() { compile_should_fail_with(r#"fn main(){continue}"#, "'continue' can only be used inside a loop"); }
#[test]
fn continue_in_function() { compile_should_fail_with(r#"fn f(){continue} fn main(){}"#, "'continue' can only be used inside a loop"); }
#[test]
fn continue_in_if() { compile_should_fail_with(r#"fn main(){if true{continue}}"#, "'continue' can only be used inside a loop"); }

// Break in match outside loop
#[test]
fn break_in_match_outside_loop() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{break}E.B{}}}"#, "'break' can only be used inside a loop"); }

// Continue in match outside loop
#[test]
fn continue_in_match_outside_loop() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{continue}E.B{}}}"#, "'continue' can only be used inside a loop"); }

// Break in closure outside loop (closure doesn't inherit loop context)
#[test]
fn break_in_closure_outside_loop() { compile_should_fail_with(r#"fn main(){while true{let f=()=>{break}}}"#, "'break' can only be used inside a loop"); }

// Continue in closure outside loop
#[test]
fn continue_in_closure_outside_loop() { compile_should_fail_with(r#"fn main(){while true{let f=()=>{continue}}}"#, "'continue' can only be used inside a loop"); }

// Break/continue in nested function
#[test]
fn break_in_nested_function() { compile_should_fail_with(r#"fn main(){while true{fn f(){break}}}"#, ""); }

// Break in for loop is valid
#[test]
fn break_in_for_valid() { compile_and_run(r#"fn main(){for i in 0..10{break}}"#); }

// Continue in for loop is valid
#[test]
fn continue_in_for_valid() { compile_and_run(r#"fn main(){for i in 0..10{continue}}"#); }

// Break in while loop is valid
#[test]
fn break_in_while_valid() { compile_and_run(r#"fn main(){while true{break}}"#); }

// Continue in while loop is valid (use counter to make it terminate)
#[test]
#[ignore]
fn continue_in_while_valid() { compile_and_run(r#"fn main(){let i=0 while i<2{i=i+1 if i==1{continue}}}"#); }

// Break in nested loops (inner break doesn't affect outer)
#[test]
#[ignore]
fn nested_break() { compile_and_run(r#"fn main(){let i=0 while i<2{while true{break}i=i+1}}"#); }

// Continue in nested loops
#[test]
#[ignore]
fn nested_continue() { compile_and_run(r#"fn main(){let i=0 while i<2{let j=0 while j<2{if j==1{j=j+1 continue}j=j+1}i=i+1}}"#); }

// Break in method outside loop
#[test]
fn break_in_method_outside_loop() { compile_should_fail_with(r#"class C{fn foo(self){break}} fn main(){}"#, "'break' can only be used inside a loop"); }

// Continue in method outside loop
#[test]
fn continue_in_method_outside_loop() { compile_should_fail_with(r#"class C{fn foo(self){continue}} fn main(){}"#, "'continue' can only be used inside a loop"); }

// Break/continue in spawn (valid - break is inside the function's loop)
#[test]
#[ignore]
fn break_in_spawn() { compile_and_run(r#"fn f() int{while true{break}return 1} fn main(){let t=spawn f()t.get()}"#); }

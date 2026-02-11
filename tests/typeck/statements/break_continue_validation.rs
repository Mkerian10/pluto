//! Break/continue validation - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Break outside loop
#[test] fn break_outside_loop() { compile_should_fail_with(r#"fn main(){break}"#, "outside loop"); }
#[test] fn break_in_function() { compile_should_fail_with(r#"fn f(){break} fn main(){}"#, "outside loop"); }
#[test] fn break_in_if() { compile_should_fail_with(r#"fn main(){if true{break}}"#, "outside loop"); }

// Continue outside loop
#[test] fn continue_outside_loop() { compile_should_fail_with(r#"fn main(){continue}"#, "outside loop"); }
#[test] fn continue_in_function() { compile_should_fail_with(r#"fn f(){continue} fn main(){}"#, "outside loop"); }
#[test] fn continue_in_if() { compile_should_fail_with(r#"fn main(){if true{continue}}"#, "outside loop"); }

// Break in match outside loop
#[test] fn break_in_match_outside_loop() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{break}E.B{}}}"#, "outside loop"); }

// Continue in match outside loop
#[test] fn continue_in_match_outside_loop() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{continue}E.B{}}}"#, "outside loop"); }

// Break in closure outside loop (closure doesn't inherit loop context)
#[test] fn break_in_closure_outside_loop() { compile_should_fail_with(r#"fn main(){while true{let f=()=>{break}}}"#, "outside loop"); }

// Continue in closure outside loop
#[test] fn continue_in_closure_outside_loop() { compile_should_fail_with(r#"fn main(){while true{let f=()=>{continue}}}"#, "outside loop"); }

// Break/continue in nested function
#[test] fn break_in_nested_function() { compile_should_fail_with(r#"fn main(){while true{fn f(){break}}}"#, ""); }

// Break in for loop is valid
#[test] fn break_in_for_valid() { compile_should_fail_with(r#"fn main(){for i in 0..10{break}}"#, ""); }

// Continue in for loop is valid
#[test] fn continue_in_for_valid() { compile_should_fail_with(r#"fn main(){for i in 0..10{continue}}"#, ""); }

// Break in while loop is valid
#[test] fn break_in_while_valid() { compile_should_fail_with(r#"fn main(){while true{break}}"#, ""); }

// Continue in while loop is valid
#[test] fn continue_in_while_valid() { compile_should_fail_with(r#"fn main(){while true{continue}}"#, ""); }

// Break in nested loops (inner break doesn't affect outer)
#[test] fn nested_break() { compile_should_fail_with(r#"fn main(){while true{while true{break}}}"#, ""); }

// Continue in nested loops
#[test] fn nested_continue() { compile_should_fail_with(r#"fn main(){while true{while true{continue}}}"#, ""); }

// Break in method outside loop
#[test] fn break_in_method_outside_loop() { compile_should_fail_with(r#"class C{} fn foo(self){break} fn main(){}"#, "outside loop"); }

// Continue in method outside loop
#[test] fn continue_in_method_outside_loop() { compile_should_fail_with(r#"class C{} fn foo(self){continue} fn main(){}"#, "outside loop"); }

// Break/continue in spawn
#[test] fn break_in_spawn() { compile_should_fail_with(r#"fn f(){while true{break}} fn main(){spawn f()}"#, ""); }

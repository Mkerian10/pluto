//! Control flow edge cases - 13 tests (removed 2 ACTUALLY_SUCCESS)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Empty while body
#[test] fn empty_while() { compile_should_fail_with(r#"fn main(){while true{}}"#, ""); }

// REMOVED: empty_for - empty for bodies are valid
// REMOVED: empty_if - empty if bodies are valid

// Empty match arm
#[test] fn empty_match_arm() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{}E.B{}}}"#, ""); }

// If with empty else
#[test] fn if_empty_else() { compile_should_fail_with(r#"fn main(){if true{let x=1}else{}}"#, ""); }

// Nested empty blocks
#[test] fn nested_empty_blocks() { compile_should_fail_with(r#"fn main(){{{{}}}}}"#, ""); }

// While false (unreachable body)
#[test] fn while_false() { compile_should_fail_with(r#"fn main(){while false{let x=1}}"#, ""); }

// If false (unreachable then branch)
#[test] fn if_false() { compile_should_fail_with(r#"fn main(){if false{let x=1}}"#, ""); }

// Match with single arm
#[test] fn match_single_arm() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{}}}"#, ""); }

// For with empty range
#[test] fn for_empty_range() { compile_should_fail_with(r#"fn main(){for i in 0..0{}}"#, ""); }

// Infinite loop with only break
#[test] fn infinite_loop_only_break() { compile_should_fail_with(r#"fn main(){while true{break}}"#, ""); }

// If-else chain
#[test] fn if_else_chain() { compile_should_fail_with(r#"fn main(){if true{}else{if true{}else{}}}"#, ""); }

// Match with wildcard only
#[test] fn match_wildcard_only() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{_={}}}"#, ""); }

// Nested loops with multiple breaks
#[test] fn nested_loops_breaks() { compile_should_fail_with(r#"fn main(){while true{while true{break}break}}"#, ""); }

// Empty function with void return
#[test] fn empty_void_function() { compile_should_fail_with(r#"fn f(){} fn main(){}"#, ""); }

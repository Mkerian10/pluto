//! Control flow edge cases - 3 tests (removed 12 ACTUALLY_SUCCESS)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// REMOVED: empty_while - empty while bodies are valid
// REMOVED: empty_for - empty for bodies are valid
// REMOVED: empty_if - empty if bodies are valid
// REMOVED: empty_match_arm - empty match arms are valid
// REMOVED: if_empty_else - empty else blocks are valid

// Nested empty blocks
#[test]
fn nested_empty_blocks() { compile_should_fail_with(r#"fn main(){{{{}}}}}"#, ""); }

// REMOVED: while_false - unreachable code is allowed
// REMOVED: if_false - unreachable code is allowed

// Match with single arm
#[test]
fn match_single_arm() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{}}}"#, ""); }

// REMOVED: for_empty_range - empty ranges are valid
// REMOVED: infinite_loop_only_break - while true {break} is valid
// REMOVED: if_else_chain - if-else chains with empty bodies are valid

// Match with wildcard only
#[test]
fn match_wildcard_only() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{_={}}}"#, ""); }

// REMOVED: nested_loops_breaks - nested loops with breaks are valid
// REMOVED: empty_void_function - empty void functions are valid

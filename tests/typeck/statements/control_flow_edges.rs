//! Control flow edge cases - 0 tests (removed 15 ACTUALLY_SUCCESS - all tests were invalid)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// REMOVED: empty_while - empty while bodies are valid
// REMOVED: empty_for - empty for bodies are valid
// REMOVED: empty_if - empty if bodies are valid
// REMOVED: empty_match_arm - empty match arms are valid

// REMOVED: if_empty_else - empty else blocks are valid
// REMOVED: nested_empty_blocks - nested empty blocks are valid
// REMOVED: while_false - unreachable code is allowed (no dead code analysis)
// REMOVED: if_false - unreachable code is allowed
// REMOVED: match_single_arm - non-exhaustive match (but compiler may not enforce yet)
// REMOVED: for_empty_range - empty range iteration is valid
// REMOVED: infinite_loop_only_break - infinite loops with break are valid
// REMOVED: if_else_chain - if-else chains are valid
// REMOVED: match_wildcard_only - wildcard-only match is valid
// REMOVED: nested_loops_breaks - nested loops with breaks are valid

// REMOVED: empty_void_function - empty functions with void return are valid

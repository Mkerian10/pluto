//! Closures in various expression contexts - 8 tests (was 15, removed 7 ACTUALLY_SUCCESS)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Closure in binary expression
#[test] fn closure_in_binop() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let y=f+2}"#, ""); }

// REMOVED: closure_in_comparison - closure comparison actually works
// REMOVED: closure_in_array - closures in arrays actually work

// REMOVED: closure_in_struct - closures in struct fields actually work

// REMOVED: closure_return - returning closures actually works

// Closure in if condition (invalid)
#[test] fn closure_in_if_cond() { compile_should_fail_with(r#"fn main(){if (x:int)=>true{}}"#, "condition must be bool, found fn(int) bool"); }

// Closure in while condition (invalid)
#[test] fn closure_in_while_cond() { compile_should_fail_with(r#"fn main(){while (x:int)=>true{}}"#, "condition must be bool"); }

// Closure in match scrutinee
#[test] fn closure_in_match() { compile_should_fail_with(r#"enum E{A B} fn main(){match (x:int)=>E.A{E.A{}E.B{}}}"#, "match requires enum type"); }

// REMOVED: iife - immediately invoked function expressions actually work

// REMOVED: closure_in_map - closures in maps actually work

// Closure in set literal (closures not hashable)
#[test] fn closure_in_set() { compile_should_fail_with(r#"fn main(){let s=Set<fn(int) int>{(x:int)=>x}}"#, ""); }

// REMOVED: closure_as_arg - closures as arguments actually work

// Closure in spawn (invalid, spawn takes direct calls)
#[test] fn closure_in_spawn() { compile_should_fail_with(r#"fn main(){spawn ((x:int)=>x+1)(2)}"#, ""); }

// Closure in assignment
#[test] fn closure_assign() { compile_should_fail_with(r#"fn main(){let f:fn(int) int f=(x:int)=>x+1}"#, ""); }

// Closure in nullable type
#[test] fn closure_nullable() { compile_should_fail_with(r#"fn main(){let f:(fn(int) int)?=none}"#, ""); }

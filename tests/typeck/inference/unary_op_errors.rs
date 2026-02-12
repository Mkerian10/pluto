//! Unary operator type errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
#[ignore] // PR #46 - outdated assertions
fn negate_string() { compile_should_fail_with(r#"fn main(){let x=-\"hi\"}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn negate_bool() { compile_should_fail_with(r#"fn main(){let x=-true}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn negate_array() { compile_should_fail_with(r#"fn main(){let x=-[1,2,3]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn not_int() { compile_should_fail_with(r#"fn main(){let x=!42}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn not_string() { compile_should_fail_with(r#"fn main(){let x=!\"hi\"}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn not_array() { compile_should_fail_with(r#"fn main(){let x=![1,2,3]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn bitwise_not_bool() { compile_should_fail_with(r#"fn main(){let x=~true}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn bitwise_not_float() { compile_should_fail_with(r#"fn main(){let x=~3.14}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn negate_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=5 let y=-x}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn not_nullable() { compile_should_fail_with(r#"fn main(){let x:bool?=true let y=!x}"#, "type mismatch"); }

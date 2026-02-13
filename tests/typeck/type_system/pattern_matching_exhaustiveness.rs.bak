//! Pattern matching exhaustiveness tests - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Missing enum variant
#[test]
#[ignore] // PR #46 - outdated assertions
fn missing_enum_variant() { compile_should_fail_with(r#"enum E{A B C} fn main(){match E.A{E.A{}E.B{}}}"#, ""); }

// Duplicate match arms
#[test]
#[ignore] // PR #46 - outdated assertions
fn duplicate_match_arms() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{}E.A{}E.B{}}}"#, ""); }

// Match on wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_wrong_type() { compile_should_fail_with(r#"enum E{A B} fn main(){match 1{E.A{}E.B{}}}"#, ""); }

// Match arm type mismatch
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_arm_type_mismatch() { compile_should_fail_with(r#"enum E{A B} fn f()int{match E.A{E.A{return 1}E.B{return "hi"}}} fn main(){}"#, ""); }

// Match binding type error
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_binding_type_error() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{let y:string=x}}}"#, ""); }

// Match with generic enum wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_generic_wrong_type() { compile_should_fail_with(r#"enum Opt<T>{Some{val:T}None} fn main(){match Opt<int>.Some{val:1}{Opt<string>.Some{val}{}Opt<int>.None{}}}"#, ""); }

// Nested match exhaustiveness
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_match_exhaust() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{match E.B{E.A{}}}E.B{}}}"#, ""); }

// Match arm shadowing
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_arm_shadowing() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let x=1 match E.A{x:2}{E.A{x}{let y=x}}}"#, ""); }

// Match with non-enum type
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_non_enum() { compile_should_fail_with(r#"class C{x:int} fn main(){match C{x:1}{C{x}{}}}"#, ""); }

// Match unreachable arm
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_unreachable_arm() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{}E.B{}E.A{}}}"#, ""); }

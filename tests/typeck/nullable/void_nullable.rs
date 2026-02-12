//! Void nullable rejection tests - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct void?
#[test]
#[ignore] // PR #46 - outdated assertions
fn void_nullable_var() { compile_should_fail_with(r#"fn main(){let x:void?=none}"#, "void nullable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn void_nullable_return() { compile_should_fail_with(r#"fn f()void?{return none} fn main(){}"#, "void nullable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn void_nullable_param() { compile_should_fail_with(r#"fn f(x:void?){} fn main(){}"#, "void nullable"); }

// In collections
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_void_nullable() { compile_should_fail_with(r#"fn main(){let a:[void?]=[none]}"#, "void nullable"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_void_nullable_value() { compile_should_fail_with(r#"fn main(){let m:Map<string,void?>=Map<string,void?>{}}"#, "void nullable"); }

// In class
#[test]
#[ignore] // PR #46 - outdated assertions
fn class_field_void_nullable() { compile_should_fail_with(r#"class C{x:void?} fn main(){}"#, "void nullable"); }

// In enum
#[test]
#[ignore] // PR #46 - outdated assertions
fn enum_variant_void_nullable() { compile_should_fail_with(r#"enum E{A{x:void?}} fn main(){}"#, "void nullable"); }

// Generic with void?
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_instantiated_void_nullable() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<void?>=Box<void?>{value:none}}"#, "void nullable"); }

// Nullable propagation on void
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_void_function() { compile_should_fail_with(r#"fn f()void{} fn g()void?{return f()?} fn main(){}"#, "void nullable"); }

// None infers to void?
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_literal_void_nullable() { compile_should_fail_with(r#"fn main(){let x:void?=none}"#, "void nullable"); }

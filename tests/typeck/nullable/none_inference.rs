//! None literal inference tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// None without context
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_no_context() { compile_should_fail_with(r#"fn main(){let x=none}"#, "cannot infer"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_in_return_no_sig() { compile_should_fail_with(r#"fn f(){return none} fn main(){}"#, "cannot infer"); }

// None in ambiguous contexts
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_in_if_branches() { compile_should_fail_with(r#"fn main(){let x=if true{none}else{42}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_in_match_arms() { compile_should_fail_with(r#"enum E{A B} fn main(){let x=match E.A{E.A=>{none}E.B=>{42}}}"#, "type mismatch"); }

// None in arrays
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_only_none() { compile_should_fail_with(r#"fn main(){let a=[none,none,none]}"#, "cannot infer"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_mixed_none_and_value() { compile_should_fail_with(r#"fn main(){let a=[42,none] let b:[int]=a}"#, "type mismatch"); }

// None in function args
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_fn_none_arg() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(none)}"#, "cannot infer"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn overload_none_ambiguous() { compile_should_fail_with(r#"fn f(x:int?){} fn main(){f(none)}"#, ""); }

// None in binary ops
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_in_comparison() { compile_should_fail_with(r#"fn main(){let b=none==42}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_in_arithmetic() { compile_should_fail_with(r#"fn main(){let x=none+none}"#, "type mismatch"); }

// None in struct fields
#[test]
#[ignore] // PR #46 - outdated assertions
fn struct_field_none_no_type() { compile_should_fail_with(r#"class C<T>{x:T} fn main(){let c=C{x:none}}"#, "cannot infer"); }

// None propagation
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_none() { compile_should_fail_with(r#"fn f(){return none?} fn main(){}"#, "cannot infer"); }

// None in map
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_value_none_no_type() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} m[\"a\"]=none}"#, "type mismatch"); }

// None in ternary-like
#[test]
#[ignore] // PR #46 - outdated assertions
fn none_ternary_mismatch() { compile_should_fail_with(r#"fn main(){let x=if true{42}else{none}}"#, ""); }

// Multiple nones
#[test]
#[ignore] // PR #46 - outdated assertions
fn fn_returns_none_twice() { compile_should_fail_with(r#"fn f(b:bool){if b{return none}return none} fn main(){}"#, "cannot infer"); }

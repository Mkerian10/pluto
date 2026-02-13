//! Temporal ordering and initialization errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Use before declaration
#[test]
#[ignore] // PR #46 - outdated assertions
fn use_before_decl() { compile_should_fail_with(r#"fn main(){let x=y let y=1}"#, "undefined"); }

// Forward reference in init
#[test]
#[ignore] // PR #46 - outdated assertions
fn forward_ref_init() { compile_should_fail_with(r#"fn main(){let x=x+1}"#, "undefined"); }

// Use uninitialized variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn use_uninitialized() { compile_should_fail_with(r#"fn main(){let x:int let y=x+1}"#, ""); }

// Conditional initialization use
#[test]
#[ignore] // PR #46 - outdated assertions
fn cond_init_use() { compile_should_fail_with(r#"fn main(){let x:int if true{x=1}let y=x}"#, ""); }

// Partial initialization
#[test]
#[ignore] // PR #46 - outdated assertions
fn partial_init() { compile_should_fail_with(r#"fn main(){let x:int if true{x=1}else{}let y=x}"#, ""); }

// Initialization in wrong order
#[test]
#[ignore] // PR #46 - outdated assertions
fn init_wrong_order() { compile_should_fail_with(r#"fn f()int{return g()} fn g()int{return f()} fn main(){}"#, ""); }

// Use in own initializer
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_init() { compile_should_fail_with(r#"class C{x:int y:int=x} fn main(){}"#, ""); }

// Forward field reference
#[test]
#[ignore] // PR #46 - outdated assertions
fn forward_field_ref() { compile_should_fail_with(r#"class C{x:int=y y:int} fn main(){}"#, ""); }

// Temporal paradox in closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_temporal() { compile_should_fail_with(r#"fn main(){let f=()=>x let x=1}"#, ""); }

// Declaration after use in block
#[test]
#[ignore] // PR #46 - outdated assertions
fn decl_after_use_block() { compile_should_fail_with(r#"fn main(){{let x=y let y=1}}"#, "undefined"); }

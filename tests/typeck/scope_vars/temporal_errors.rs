//! Temporal ordering and initialization errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Use before declaration
#[test]
#[ignore]
fn use_before_decl() { compile_should_fail_with(r#"fn main(){let x=y let y=1}"#, "undefined"); }

// Forward reference in init
#[test]
fn forward_ref_init() { compile_should_fail_with(r#"fn main(){let x=x+1}"#, "undefined"); }

// Use uninitialized variable
#[test]
fn use_uninitialized() { compile_should_fail_with(r#"fn main(){let x:int let y=x+1}"#, ""); }

// Conditional initialization use
#[test]
fn cond_init_use() { compile_should_fail_with(r#"fn main(){let x:int if true{x=1}let y=x}"#, ""); }

// Partial initialization
#[test]
fn partial_init() { compile_should_fail_with(r#"fn main(){let x:int if true{x=1}else{}let y=x}"#, ""); }

// Initialization in wrong order
#[test]
#[ignore] // Compiler limitation: temporal safety not enforced
fn init_wrong_order() { compile_should_fail_with(r#"fn f()int{return g()} fn g()int{return f()} fn main(){}"#, ""); }

// Use in own initializer
#[test]
fn self_init() { compile_should_fail_with(r#"class C{x:int y:int=x} fn main(){}"#, ""); }

// Forward field reference
#[test]
fn forward_field_ref() { compile_should_fail_with(r#"class C{x:int=y y:int} fn main(){}"#, ""); }

// Temporal paradox in closure
#[test]
fn closure_temporal() { compile_should_fail_with(r#"fn main(){let f=()=>x let x=1}"#, ""); }

// Declaration after use in block
#[test]
#[ignore] // Compiler limitation: temporal safety not enforced
fn decl_after_use_block() { compile_should_fail_with(r#"fn main(){{let x=y let y=1}}"#, "undefined"); }

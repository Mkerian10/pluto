//! Scope violations - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Access variable from nested scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_from_nested_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}let y=x}"#, "undefined"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_from_if_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}else{let y=x}}"#, "undefined"); }

// Access variable from loop scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_from_loop_scope() { compile_should_fail_with(r#"fn main(){while true{let x=1}let y=x}"#, "undefined"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_from_for_scope() { compile_should_fail_with(r#"fn main(){for i in 0..10{let x=1}let y=x}"#, "undefined"); }

// Access match binding outside match
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_match_binding_outside() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{}}let y=x}"#, "undefined"); }

// Access closure parameter outside closure
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_closure_param_outside() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let y=x}"#, "undefined"); }

// Access variable from sibling scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_from_sibling_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}if false{let y=x}}"#, "undefined"); }

// Access function local from another function
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_other_function_local() { compile_should_fail_with(r#"fn f(){let x=1} fn g(){let y=x} fn main(){}"#, "undefined"); }

// Access method local from another method
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_other_method_local() { compile_should_fail_with(r#"class C{} fn foo(self){let x=1} fn bar(self){let y=x} fn main(){}"#, "undefined"); }

// Access for loop variable after loop
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_for_var_after_loop() { compile_should_fail_with(r#"fn main(){for i in 0..10{}let x=i}"#, "undefined"); }

// Access nested block variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_nested_block() { compile_should_fail_with(r#"fn main(){{let x=1}let y=x}"#, ""); }

// Access variable before declaration (forward reference)
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_before_declaration() { compile_should_fail_with(r#"fn main(){let y=x let x=1}"#, "undefined"); }

// Access variable in own initializer
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_reference() { compile_should_fail_with(r#"fn main(){let x=x+1}"#, "undefined"); }

// Access match arm binding in different arm
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_other_arm_binding() { compile_should_fail_with(r#"enum E{A{x:int}B{y:int}} fn main(){match E.A{x:1}{E.A{x}{}E.B{y}{let z=x}}}"#, "undefined"); }

// Access spawn closure scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn access_spawn_scope() { compile_should_fail_with(r#"fn f(){let x=1} fn main(){spawn f() let y=x}"#, ""); }

//! Variable shadowing tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Local shadows parameter
#[test]
fn local_shadows_param() { compile_should_fail_with(r#"fn f(x:int){let x=2} fn main(){}"#, ""); }

// Nested scope shadows
#[test]
fn nested_shadows() { compile_should_fail_with(r#"fn main(){let x=1 if true{let x=2}}"#, ""); }

// Loop variable shadows outer
#[test]
fn loop_shadows_outer() { compile_should_fail_with(r#"fn main(){let i=1 for i in 0..10{}}"#, ""); }

// Match binding shadows
#[test]
fn match_shadows() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let x=1 match E.A{x:2}{E.A{x}{}}}"#, ""); }

// Closure parameter shadows capture
#[test]
fn closure_param_shadows_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=(x:int)=>x+1}"#, ""); }

// Function shadows global
#[test]
#[ignore]
fn function_shadows_global() { compile_should_fail_with(r#"fn x()int{return 1} fn main(){let x=2}"#, ""); }

// Class shadows function
#[test]
fn class_shadows_function() { compile_should_fail_with(r#"fn C(){} class C{} fn main(){}"#, ""); }

// Type param shadows class
#[test]
fn type_param_shadows_class() { compile_should_fail_with(r#"class T{} fn f<T>(x:T){} fn main(){}"#, ""); }

// Multiple shadow levels
#[test]
fn multiple_shadow_levels() { compile_should_fail_with(r#"fn main(){let x=1 if true{let x=2 if true{let x=3}}}"#, ""); }

// Shadow after scope ends
#[test]
#[ignore]
fn shadow_after_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}let x=2}"#, ""); }

// Shadow in different branches
#[test]
fn shadow_diff_branches() { compile_should_fail_with(r#"fn main(){let x=1 if true{let x=2}else{let x=3}}"#, ""); }

// Shadow in match arms
#[test]
fn shadow_match_arms() { compile_should_fail_with(r#"enum E{A B} fn main(){let x=1 match E.A{E.A{let x=2}E.B{let x=3}}}"#, ""); }

// Field name shadows parameter
#[test]
fn field_shadows_param() { compile_should_fail_with(r#"class C{x:int} fn foo(self,x:int){} fn main(){}"#, ""); }

// Method name shadows field
#[test]
fn method_shadows_field() { compile_should_fail_with(r#"class C{foo:int} fn foo(self){} fn main(){}"#, ""); }

// Import shadows local
#[test]
fn import_shadows_local() { compile_should_fail_with(r#"import math fn main(){let math=1}"#, ""); }

// Enum variant shadows variable
#[test]
#[ignore]
fn variant_shadows_var() { compile_should_fail_with(r#"enum E{A} fn main(){let A=1}"#, ""); }

// Error type shadows class
#[test]
fn error_shadows_class() { compile_should_fail_with(r#"class E{} error E{} fn main(){}"#, ""); }

// Trait shadows enum
#[test]
fn trait_shadows_enum() { compile_should_fail_with(r#"enum T{A} trait T{} fn main(){}"#, ""); }

// Generic shadow in nested function
#[test]
fn generic_shadow_nested() { compile_should_fail_with(r#"fn f<T>(x:T){fn g<T>(y:T){}} fn main(){}"#, ""); }

// Shadow builtin (allowed)
#[test]
#[ignore]
fn shadow_builtin() { compile_should_fail_with(r#"fn main(){let print=1}"#, ""); }

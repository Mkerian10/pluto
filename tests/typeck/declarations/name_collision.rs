//! Name collision errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Builtin name as class - compiler doesn't detect collision
#[test]
#[ignore] // #176: compiler doesn't detect builtin name collisions
fn builtin_as_class() { compile_should_fail_with(r#"class print{} fn main(){}"#, ""); }

// Builtin name as function - compiler doesn't detect collision
#[test]
#[ignore] // #176: compiler doesn't detect builtin name collisions
fn builtin_as_function() { compile_should_fail_with(r#"fn len(){} fn main(){}"#, ""); }

// Reserved keyword as identifier - correctly rejected by parser
#[test]
fn keyword_as_identifier() { compile_should_fail_with(r#"fn main(){let fn=1}"#, "Syntax error: expected identifier, found fn"); }

// Module name collision - same as duplicate_declarations::module_class_collision
#[test]
#[ignore] // #174: compiler doesn't detect module/class name collisions
fn module_name_collision() { compile_should_fail_with(r#"import math class math{} fn main(){}"#, "already declared"); }

// Type parameter shadows class - compiler doesn't detect collision
#[test]
#[ignore] // #176: compiler allows type params to shadow class names
fn type_param_shadows_class() { compile_should_fail_with(r#"class C{} fn f<C>(x:C){} fn main(){}"#, ""); }

// Local variable shadows global - compiler doesn't detect collision
#[test]
#[ignore] // #176: compiler allows local variables to shadow global functions
fn local_shadows_global() { compile_should_fail_with(r#"fn g()int{return 1} fn main(){let g=2}"#, ""); }

// Parameter shadows field - compiler allows shadowing
#[test]
#[ignore] // #176: compiler allows method parameters to shadow fields
fn param_shadows_field() { compile_should_fail_with(r#"class C{x:int fn foo(self,x:int){}} fn main(){}"#, ""); }

// Enum variant name collision - correctly requires qualified names
#[test]
fn enum_variant_collision() { compile_should_fail_with(r#"enum E1{A} enum E2{A} fn main(){let e=A}"#, "undefined variable 'A'"); }

// Trait and class in same namespace - same as duplicate_declarations::class_trait_collision
#[test]
#[ignore] // #174: compiler doesn't detect class/trait name collisions
fn trait_class_namespace() { compile_should_fail_with(r#"trait T{} class T{} fn main(){}"#, "already declared"); }

// Error name collision with class - same as duplicate_declarations::error_class_collision
#[test]
#[ignore] // #174: compiler doesn't detect error/class name collisions
fn error_name_collision() { compile_should_fail_with(r#"error E{} class E{} fn main(){}"#, "already declared"); }

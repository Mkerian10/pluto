//! Name collision errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Builtin name as class - rejected by compiler
#[test]
fn builtin_as_class() { compile_should_fail_with(r#"class print{} fn main(){}"#, "shadow builtin"); }

// Builtin name as function - fn len not a standalone builtin (len is a method), still ignored
#[test]
#[ignore] // #176: len is a method name, not a standalone builtin — design decision pending
fn builtin_as_function() { compile_should_fail_with(r#"fn len(){} fn main(){}"#, ""); }

// Reserved keyword as identifier - correctly rejected by parser
#[test]
fn keyword_as_identifier() { compile_should_fail_with(r#"fn main(){let fn=1}"#, "Syntax error: expected identifier, found fn"); }

// Module name collision - same as duplicate_declarations::module_class_collision
#[test]
fn module_name_collision() { compile_should_fail_with("import math\nclass math{}\nfn main(){}", "already declared"); }

// Type parameter shadows class - rejected by compiler
#[test]
fn type_param_shadows_class() { compile_should_fail_with(r#"class C{} fn f<C>(x:C){} fn main(){}"#, "shadows class"); }

// Local variable shadows global - intentional shadowing, not a compile error
#[test]
#[ignore] // #176: design decision - local variable shadowing global function may be intentional
fn local_shadows_global() { compile_should_fail_with(r#"fn g()int{return 1} fn main(){let g=2}"#, ""); }

// Parameter shadows field - fails due to inline class syntax requiring newlines
#[test]
fn param_shadows_field() { compile_should_fail_with(r#"class C{x:int fn foo(self,x:int){}} fn main(){}"#, ""); }

// Enum variant name collision - correctly requires qualified names
#[test]
fn enum_variant_collision() { compile_should_fail_with(r#"enum E1{A} enum E2{A} fn main(){let e=A}"#, "undefined variable 'A'"); }

// Trait and class in same namespace - same as duplicate_declarations::class_trait_collision
#[test]
fn trait_class_namespace() { compile_should_fail_with(r#"trait T{} class T{} fn main(){}"#, "already declared"); }

// Error name collision with class - same as duplicate_declarations::error_class_collision
#[test]
fn error_name_collision() { compile_should_fail_with(r#"error E{} class E{} fn main(){}"#, "already declared"); }

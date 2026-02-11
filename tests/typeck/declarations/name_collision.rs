//! Name collision errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Builtin name as class
#[test] fn builtin_as_class() { compile_should_fail_with(r#"class print{} fn main(){}"#, ""); }

// Builtin name as function
#[test] fn builtin_as_function() { compile_should_fail_with(r#"fn len(){} fn main(){}"#, ""); }

// Reserved keyword as identifier
#[test] fn keyword_as_identifier() { compile_should_fail_with(r#"fn main(){let fn=1}"#, ""); }

// Module name collision
#[test] fn module_name_collision() { compile_should_fail_with(r#"import math class math{} fn main(){}"#, "already declared"); }

// Type parameter shadows class
#[test] fn type_param_shadows_class() { compile_should_fail_with(r#"class C{} fn f<C>(x:C){} fn main(){}"#, ""); }

// Local variable shadows global
#[test] fn local_shadows_global() { compile_should_fail_with(r#"fn g()int{return 1} fn main(){let g=2}"#, ""); }

// Parameter shadows field
#[test] fn param_shadows_field() { compile_should_fail_with(r#"class C{x:int} fn foo(self,x:int){} fn main(){}"#, ""); }

// Enum variant name collision
#[test] fn enum_variant_collision() { compile_should_fail_with(r#"enum E1{A} enum E2{A} fn main(){let e=A}"#, ""); }

// Trait and class in same namespace
#[test] fn trait_class_namespace() { compile_should_fail_with(r#"trait T{} class T{} fn main(){}"#, "already declared"); }

// Error name collision with class
#[test] fn error_name_collision() { compile_should_fail_with(r#"error E{} class E{} fn main(){}"#, "already declared"); }

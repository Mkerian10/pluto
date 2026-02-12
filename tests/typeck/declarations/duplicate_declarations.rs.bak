//! Duplicate declaration errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Duplicate class
#[test] fn duplicate_class() { compile_should_fail_with(r#"class C{} class C{} fn main(){}"#, "already declared"); }

// Duplicate function
#[test] fn duplicate_function() { compile_should_fail_with(r#"fn f(){} fn f(){} fn main(){}"#, "already declared"); }

// Duplicate trait
#[test] fn duplicate_trait() { compile_should_fail_with(r#"trait T{} trait T{} fn main(){}"#, "already declared"); }

// Duplicate enum
#[test] fn duplicate_enum() { compile_should_fail_with(r#"enum E{A} enum E{B} fn main(){}"#, "already declared"); }

// Duplicate error
#[test] fn duplicate_error() { compile_should_fail_with(r#"error E{} error E{} fn main(){}"#, "already declared"); }

// Duplicate app
#[test] fn duplicate_app() { compile_should_fail_with(r#"app A1{fn main(self){}} app A2{fn main(self){}}"#, ""); }

// Duplicate method
#[test] fn duplicate_method() { compile_should_fail_with(r#"class C{} fn foo(self){} fn foo(self){} fn main(){}"#, "already declared"); }

// Duplicate trait method
#[test] fn duplicate_trait_method() { compile_should_fail_with(r#"trait T{fn foo(self) fn foo(self)} fn main(){}"#, "already declared"); }

// Duplicate enum variant
#[test] fn duplicate_enum_variant() { compile_should_fail_with(r#"enum E{A A} fn main(){}"#, "already declared"); }

// Duplicate field
#[test] fn duplicate_field() { compile_should_fail_with(r#"class C{x:int x:string} fn main(){}"#, "already declared"); }

// Duplicate parameter
#[test] fn duplicate_param() { compile_should_fail_with(r#"fn f(x:int,x:string){} fn main(){}"#, "already declared"); }

// Duplicate type parameter
#[test] fn duplicate_type_param() { compile_should_fail_with(r#"fn f<T,T>(x:T){} fn main(){}"#, "already declared"); }

// Class and function same name
#[test] fn class_function_collision() { compile_should_fail_with(r#"class C{} fn C(){} fn main(){}"#, "already declared"); }

// Class and trait same name
#[test] fn class_trait_collision() { compile_should_fail_with(r#"class T{} trait T{} fn main(){}"#, "already declared"); }

// Class and enum same name
#[test] fn class_enum_collision() { compile_should_fail_with(r#"class E{} enum E{A} fn main(){}"#, "already declared"); }

// Trait and enum same name
#[test] fn trait_enum_collision() { compile_should_fail_with(r#"trait T{} enum T{A} fn main(){}"#, "already declared"); }

// Error and class same name
#[test] fn error_class_collision() { compile_should_fail_with(r#"error E{} class E{} fn main(){}"#, "already declared"); }

// Duplicate impl
#[test] fn duplicate_impl() { compile_should_fail_with(r#"trait T{} class C{} impl T{} impl T{} fn main(){}"#, ""); }

// Duplicate generic class
#[test] fn duplicate_generic_class() { compile_should_fail_with(r#"class Box<T>{} class Box<U>{} fn main(){}"#, "already declared"); }

// Module and class same name
#[test] fn module_class_collision() { compile_should_fail_with(r#"import math class math{} fn main(){}"#, "already declared"); }

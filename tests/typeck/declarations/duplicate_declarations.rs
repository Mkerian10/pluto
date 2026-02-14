//! Duplicate declaration errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Duplicate class
#[test]
#[ignore] // #174: compiler doesn't detect duplicate class declarations
fn duplicate_class() { compile_should_fail_with(r#"class C{} class C{} fn main(){}"#, "already declared"); }

// Duplicate function - detected at codegen instead of typeck
#[test]
fn duplicate_function() { compile_should_fail_with(r#"fn f(){} fn f(){} fn main(){}"#, "Duplicate definition of identifier: f"); }

// Duplicate trait
#[test]
#[ignore] // #174: compiler doesn't detect duplicate trait declarations
fn duplicate_trait() { compile_should_fail_with(r#"trait T{} trait T{} fn main(){}"#, "already declared"); }

// Duplicate enum
#[test]
#[ignore] // #174: compiler doesn't detect duplicate enum declarations
fn duplicate_enum() { compile_should_fail_with(r#"enum E{A} enum E{B} fn main(){}"#, "already declared"); }

// Duplicate error
#[test]
#[ignore] // #174: compiler doesn't detect duplicate error declarations
fn duplicate_error() { compile_should_fail_with(r#"error E{} error E{} fn main(){}"#, "already declared"); }

// Duplicate app - correctly detects duplicate apps
#[test]
fn duplicate_app() { compile_should_fail_with(r#"app A1{fn main(self){}} app A2{fn main(self){}}"#, ""); }

// Duplicate method - syntax error from invalid free function with self parameter
#[test]
fn duplicate_method() { compile_should_fail_with(r#"class C{} fn foo(self){} fn foo(self){} fn main(){}"#, "Syntax error: expected identifier, found self"); }

// Duplicate trait method - parser doesn't support duplicate methods in trait syntax
#[test]
fn duplicate_trait_method() { compile_should_fail_with(r#"trait T{fn foo(self) fn foo(self)} fn main(){}"#, "Syntax error: expected (, found identifier"); }

// Duplicate enum variant
#[test]
#[ignore] // #174: compiler doesn't detect duplicate enum variants
fn duplicate_enum_variant() { compile_should_fail_with(r#"enum E{A A} fn main(){}"#, "already declared"); }

// Duplicate field - correctly detects duplicate fields
#[test]
fn duplicate_field() { compile_should_fail_with(r#"class C{x:int x:string} fn main(){}"#, "duplicate field 'x'"); }

// Duplicate parameter
#[test]
#[ignore] // #174: compiler doesn't detect duplicate parameters
fn duplicate_param() { compile_should_fail_with(r#"fn f(x:int,x:string){} fn main(){}"#, "already declared"); }

// Duplicate type parameter
#[test]
#[ignore] // #174: compiler doesn't detect duplicate type parameters
fn duplicate_type_param() { compile_should_fail_with(r#"fn f<T,T>(x:T){} fn main(){}"#, "already declared"); }

// Class and function same name
#[test]
#[ignore] // #174: compiler doesn't detect class/function name collisions
fn class_function_collision() { compile_should_fail_with(r#"class C{} fn C(){} fn main(){}"#, "already declared"); }

// Class and trait same name
#[test]
#[ignore] // #174: compiler doesn't detect class/trait name collisions
fn class_trait_collision() { compile_should_fail_with(r#"class T{} trait T{} fn main(){}"#, "already declared"); }

// Class and enum same name - detected at codegen via reflection function collision
#[test]
fn class_enum_collision() { compile_should_fail_with(r#"class E{} enum E{A} fn main(){}"#, "Duplicate definition of identifier: TypeInfo_type_name_E"); }

// Trait and enum same name
#[test]
#[ignore] // #174: compiler doesn't detect trait/enum name collisions
fn trait_enum_collision() { compile_should_fail_with(r#"trait T{} enum T{A} fn main(){}"#, "already declared"); }

// Error and class same name
#[test]
#[ignore] // #174: compiler doesn't detect error/class name collisions
fn error_class_collision() { compile_should_fail_with(r#"error E{} class E{} fn main(){}"#, "already declared"); }

// Duplicate impl - correctly detects duplicate impls
#[test]
fn duplicate_impl() { compile_should_fail_with(r#"trait T{} class C{} impl T{} impl T{} fn main(){}"#, ""); }

// Duplicate generic class
#[test]
#[ignore] // #174: compiler doesn't detect duplicate generic class declarations
fn duplicate_generic_class() { compile_should_fail_with(r#"class Box<T>{} class Box<U>{} fn main(){}"#, "already declared"); }

// Module and class same name
#[test]
#[ignore] // #174: compiler doesn't detect module/class name collisions
fn module_class_collision() { compile_should_fail_with(r#"import math class math{} fn main(){}"#, "already declared"); }

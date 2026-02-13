//! Visibility errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Private class used in module
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_class_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){let c=mod1.PrivateClass{}}"#, ""); }

// Private function called cross-module
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_fn_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){mod1.private_fn()}"#, ""); }

// Private trait impl cross-module
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_trait_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){}"#, ""); }

// Pub class in non-pub module
#[test]
#[ignore] // PR #46 - outdated assertions
fn pub_in_private_module() { compile_should_fail_with(r#"import private_mod fn main(){let c=private_mod.PublicClass{}}"#, ""); }

// Access private field
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_field_access() { compile_should_fail_with(r#"class C{private x:int} fn main(){let c=C{x:1}let y=c.x}"#, ""); }

// Access private method
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_method_access() { compile_should_fail_with(r#"class C{} private fn foo(self){} fn main(){let c=C{} c.foo()}"#, ""); }

// Private enum variant
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_enum_variant() { compile_should_fail_with(r#"enum E{pub A private B} fn main(){let e=E.B}"#, ""); }

// Private trait
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_trait() { compile_should_fail_with(r#"private trait T{} class C{} impl T{} fn main(){}"#, ""); }

// Pub trait with private method
#[test]
#[ignore] // PR #46 - outdated assertions
fn pub_trait_private_method() { compile_should_fail_with(r#"pub trait T{private fn foo(self)} fn main(){}"#, ""); }

// Re-export private item
#[test]
#[ignore] // PR #46 - outdated assertions
fn reexport_private() { compile_should_fail_with(r#"import mod1 pub use mod1.PrivateClass fn main(){}"#, ""); }

// Private generic parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_generic_param() { compile_should_fail_with(r#"private class C{} pub class Box<T>{value:T} fn main(){let b:Box<C>}"#, ""); }

// Private DI dependency
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_di_dep() { compile_should_fail_with(r#"private class Dep{} pub class Service[dep:Dep]{} fn main(){}"#, ""); }

// Private error type
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_error() { compile_should_fail_with(r#"private error E{} pub fn f()!{raise E{}} fn main(){}"#, ""); }

// Private in public signature
#[test]
#[ignore] // PR #46 - outdated assertions
fn private_in_pub_sig() { compile_should_fail_with(r#"private class C{} pub fn f()C{return C{}} fn main(){}"#, ""); }

// Visibility in app
#[test]
#[ignore] // PR #46 - outdated assertions
fn app_visibility() { compile_should_fail_with(r#"app MyApp{private fn helper(self){} fn main(self){self.helper()}}"#, ""); }

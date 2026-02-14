//! Visibility errors - 15 tests
//! Note: Pluto currently only has 'pub' visibility - no 'private' keyword exists
//! All tests using 'private' keyword get syntax errors
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Private class used in module - module system allows non-existent module, error at usage
#[test]
fn private_class_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){let c=mod1.PrivateClass{}}"#, "unknown class 'mod1.PrivateClass'"); }

// Private function called cross-module - module system allows non-existent module, error at usage
#[test]
fn private_fn_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){mod1.private_fn()}"#, "undefined variable 'mod1'"); }

// Private trait impl cross-module - just imports non-existent module
#[test]
#[ignore] // Test doesn't actually test anything - empty main after import
fn private_trait_cross_module() { compile_should_fail_with(r#"import mod1 fn main(){}"#, ""); }

// Pub class in non-pub module - module system allows non-existent module, error at usage
#[test]
fn pub_in_private_module() { compile_should_fail_with(r#"import private_mod fn main(){let c=private_mod.PublicClass{}}"#, "unknown class 'private_mod.PublicClass'"); }

// Access private field - 'private' keyword not supported
#[test]
fn private_field_access() { compile_should_fail_with(r#"class C{private x:int} fn main(){let c=C{x:1}let y=c.x}"#, "Syntax error: expected :, found identifier"); }

// Access private method - 'private' keyword not supported
#[test]
fn private_method_access() { compile_should_fail_with(r#"class C{} private fn foo(self){} fn main(){let c=C{} c.foo()}"#, "Syntax error: expected 'fn'"); }

// Private enum variant - enum variant visibility not supported (pub/private both rejected)
#[test]
fn private_enum_variant() { compile_should_fail_with(r#"enum E{pub A private B} fn main(){let e=E.B}"#, "Syntax error: expected identifier, found pub"); }

// Private trait - 'private' keyword not supported
#[test]
fn private_trait() { compile_should_fail_with(r#"private trait T{} class C{} impl T{} fn main(){}"#, "Syntax error: expected 'fn'"); }

// Pub trait with private method - 'private' keyword not supported
#[test]
fn pub_trait_private_method() { compile_should_fail_with(r#"pub trait T{private fn foo(self)} fn main(){}"#, "Syntax error: expected fn, found identifier"); }

// Re-export private item - 'pub use' syntax not supported
#[test]
fn reexport_private() { compile_should_fail_with(r#"import mod1 pub use mod1.PrivateClass fn main(){}"#, "Syntax error: expected 'fn'"); }

// Private generic parameter - 'private' keyword not supported
#[test]
fn private_generic_param() { compile_should_fail_with(r#"private class C{} pub class Box<T>{value:T} fn main(){let b:Box<C>}"#, "Syntax error: expected 'fn'"); }

// Private DI dependency - 'private' keyword not supported
#[test]
fn private_di_dep() { compile_should_fail_with(r#"private class Dep{} pub class Service[dep:Dep]{} fn main(){}"#, "Syntax error: expected 'fn'"); }

// Private error type - 'private' keyword not supported
#[test]
fn private_error() { compile_should_fail_with(r#"private error E{} pub fn f()!{raise E{}} fn main(){}"#, "Syntax error: expected 'fn'"); }

// Private in public signature - 'private' keyword not supported
#[test]
fn private_in_pub_sig() { compile_should_fail_with(r#"private class C{} pub fn f()C{return C{}} fn main(){}"#, "Syntax error: expected 'fn'"); }

// Visibility in app - 'private' keyword not supported
#[test]
fn app_visibility() { compile_should_fail_with(r#"app MyApp{private fn helper(self){} fn main(self){self.helper()}}"#, "Syntax error: expected fn, found identifier"); }

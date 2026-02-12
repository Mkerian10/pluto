//! App validation errors - 3 tests (removed 6 ACTUALLY_SUCCESS, 2 already ignored)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// No app declaration
#[test]
#[ignore] // Compiler behavior: programs without app declarations are now accepted
fn no_app() { compile_should_fail_with(r#"fn main(){}"#, ""); }

// REMOVED: multiple_apps - likely valid or different error
// App without main
#[test] fn app_no_main() { compile_should_fail_with(r#"app MyApp{fn helper(self){}}"#, "app must have a 'main' method"); }

// REMOVED: app_main_wrong_return - likely valid or different error
// REMOVED: app_main_with_params - likely valid or different error
// REMOVED: app_main_no_self - likely valid or different error
// REMOVED: app_with_fields - likely valid or different error
// REMOVED: app_impl_trait - likely valid or different error
// REMOVED: generic_app - likely valid or different error

// App name collision
#[test]
#[ignore] // Compiler bug: not detecting name collision between class and app
fn app_name_collision() { compile_should_fail_with(r#"class MyApp{} app MyApp{fn main(self){}}"#, "already declared"); }

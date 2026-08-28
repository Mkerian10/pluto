//! App validation errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// No app declaration
#[test]
#[ignore] // Compiler behavior: programs without app declarations are now accepted
fn no_app() { compile_should_fail_with(r#"fn main(){}"#, ""); }

// Multiple apps
#[test]
fn multiple_apps() { compile_should_fail_with(r#"app A{fn main(self){}} app B{fn main(self){}}"#, "duplicate app declaration"); }

// App without main
#[test]
fn app_no_main() { compile_should_fail_with(r#"app MyApp{fn helper(self){}}"#, "app must have a 'main' method"); }

// App main wrong return type
#[test]
fn app_main_wrong_return() { compile_should_fail_with(r#"app MyApp{fn main(self)int{return 1}}"#, "app main method must not have a return type"); }

// App main with parameters
#[test]
fn app_main_with_params() { compile_should_fail_with(r#"app MyApp{fn main(self,x:int){}}"#, "app main method must not take parameters beyond 'self'"); }

// App main missing self
#[test]
fn app_main_no_self() { compile_should_fail_with(r#"app MyApp{fn main(){}}"#, "app main method must take 'self' as first parameter"); }

// App with fields (not allowed)
#[test]
fn app_with_fields() { compile_should_fail_with(r#"app MyApp{x:int fn main(self){}}"#, "expected fn, found identifier"); }

// App implements trait (not allowed)
#[test]
fn app_impl_trait() { compile_should_fail_with(r#"trait T{} app MyApp impl T{fn main(self){}}"#, "expected {, found impl"); }

// Generic app (not allowed)
#[test]
fn generic_app() { compile_should_fail_with(r#"app MyApp<T>{fn main(self){}}"#, "expected {, found <"); }

// App name collision
#[test]
fn app_name_collision() { compile_should_fail_with(r#"class MyApp{} app MyApp{fn main(self){}}"#, "already declared"); }

//! App validation errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// No app declaration
#[test] fn no_app() { compile_should_fail_with(r#"fn main(){}"#, ""); }

// Multiple apps
#[test] fn multiple_apps() { compile_should_fail_with(r#"app A{fn main(self){}} app B{fn main(self){}}"#, ""); }

// App without main
#[test] fn app_no_main() { compile_should_fail_with(r#"app MyApp{fn helper(self){}}"#, "missing main"); }

// App main wrong return type
#[test] fn app_main_wrong_return() { compile_should_fail_with(r#"app MyApp{fn main(self)int{return 1}}"#, ""); }

// App main with parameters
#[test] fn app_main_with_params() { compile_should_fail_with(r#"app MyApp{fn main(self,x:int){}}"#, ""); }

// App main missing self
#[test] fn app_main_no_self() { compile_should_fail_with(r#"app MyApp{fn main(){}}"#, ""); }

// App with fields (not allowed)
#[test] fn app_with_fields() { compile_should_fail_with(r#"app MyApp{x:int fn main(self){}}"#, ""); }

// App implements trait (not allowed)
#[test] fn app_impl_trait() { compile_should_fail_with(r#"trait T{} app MyApp impl T{fn main(self){}}"#, ""); }

// Generic app (not allowed)
#[test] fn generic_app() { compile_should_fail_with(r#"app MyApp<T>{fn main(self){}}"#, ""); }

// App name collision
#[test] fn app_name_collision() { compile_should_fail_with(r#"class MyApp{} app MyApp{fn main(self){}}"#, "already declared"); }

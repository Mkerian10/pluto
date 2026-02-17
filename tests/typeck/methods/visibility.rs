//! Method visibility errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Private method called from outside
#[test]
fn private_method_external() { compile_should_fail_with(r#"class C{} fn foo(self){} fn main(){let c=C{} c.foo()}"#, ""); }

// Public method (if supported)
#[test]
fn public_method() { compile_should_fail_with(r#"class C{} pub fn foo(self){} fn main(){let c=C{} c.foo()}"#, ""); }

// Method visibility in module
#[test]
fn module_method_visibility() { compile_should_fail_with(r#"import mod1 fn main(){let c=mod1.C{} c.foo()}"#, ""); }

// Trait method visibility
#[test]
fn trait_method_visibility() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Method from imported module
#[test]
fn imported_method() { compile_should_fail_with(r#"import math fn main(){let v=math.Vector{x:1} v.foo()}"#, ""); }

// Private trait method
#[test]
fn private_trait_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T){t.foo()} fn main(){}"#, ""); }

// Method visibility in generic class
#[test]
fn generic_class_visibility() { compile_should_fail_with(r#"class Box<T>{value:T} fn foo(self){} fn main(){let b=Box<int>{value:1}b.foo()}"#, ""); }

// Cross-module method call
#[test]
fn cross_module_method() { compile_should_fail_with(r#"import other fn main(){let c=other.C{} c.private_method()}"#, ""); }

// Method on app class
#[test]
#[ignore]
fn app_method_visibility() { compile_should_fail_with(r#"app MyApp{fn helper(self){} fn main(self){self.helper()}}"#, ""); }

// Method visibility with contracts
#[test]
fn contract_method_visibility() { compile_should_fail_with(r#"class C{} fn foo(self)int ensures result>0{return 1} fn main(){let c=C{} c.foo()}"#, ""); }

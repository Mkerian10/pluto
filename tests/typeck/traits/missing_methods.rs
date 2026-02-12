//! Missing trait method implementation tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic missing method
#[test] fn missing_single_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{} fn main(){}"#, "missing method"); }
#[test] fn missing_one_of_two() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self)} class C{} impl T{fn foo(self){}} fn main(){}"#, "missing method"); }
#[test] fn missing_all_methods() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self) fn baz(self)} class C{} impl T{} fn main(){}"#, "missing method"); }

// Missing with wrong signature present
#[test] fn wrong_sig_not_missing() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self,x:int)int{return x}} fn main(){}"#, "missing method"); }

// Missing generic methods
#[test] fn missing_generic_method() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C{} impl T{} fn main(){}"#, "missing method"); }

// Missing with nullable/error signatures
#[test] fn missing_nullable_method() { compile_should_fail_with(r#"trait T{fn foo(self)int?} class C{} impl T{} fn main(){}"#, "missing method"); }
#[test] fn missing_fallible_method() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C{} impl T{} fn main(){}"#, "missing method"); }

// Multiple traits, one missing method
#[test] fn two_traits_one_incomplete() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{} fn main(){}"#, "missing method"); }

// Missing on generic class
#[test] fn missing_on_generic_class() { compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U>{value:U} impl T{} fn main(){}"#, "missing method"); }

// Partial implementation
#[test] fn three_methods_one_missing() { compile_should_fail_with(r#"trait T{fn a(self) fn b(self) fn c(self)} class C{} impl T{fn a(self){} fn c(self){}} fn main(){}"#, "missing method"); }

// Missing mut self method
#[test] fn missing_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C{} impl T{} fn main(){}"#, "missing method"); }

// Missing method with complex signature
#[test] fn missing_complex_sig() { compile_should_fail_with(r#"trait T{fn foo(self,x:Map<string,int>,f:fn(int)string)[int]} class C{} impl T{} fn main(){}"#, "missing method"); }

// Impl block without trait
#[test] fn impl_wrong_trait() { compile_should_fail_with(r#"trait T{fn foo(self)} trait T2{fn bar(self)} class C{} impl T{fn bar(self){}} fn main(){}"#, "missing method"); }

// Case sensitivity
#[test] fn method_name_case_wrong() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn Foo(self){}} fn main(){}"#, "missing method"); }

// Missing with contracts
#[test] fn missing_method_with_contract() { compile_should_fail_with(r#"trait T{fn foo(self)int requires true ensures result>0} class C{} impl T{} fn main(){}"#, "missing method"); }

// Generic class multiple instantiations
#[test] fn generic_class_missing_per_instance() { compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U>{value:U} impl T{} fn main(){let b1=Box<int>{value:1} let b2=Box<string>{value:\"hi\"}}"#, "missing method"); }

// Missing default method (if Pluto had them)
#[test] fn missing_non_default() { compile_should_fail_with(r#"trait T{fn required(self) fn optional(self){}} class C{} impl T{fn optional(self){}} fn main(){}"#, "missing method"); }

// Trait with only one method, missing
#[test] fn single_method_trait_missing() { compile_should_fail_with(r#"trait Printable{fn print(self)} class C{x:int} impl Printable{} fn main(){}"#, "missing method"); }

// Missing static method (if supported)
#[test] fn missing_static_method() { compile_should_fail_with(r#"trait T{fn create()C} class C{} impl T{} fn main(){}"#, ""); }

// Multiple classes implementing same trait, one missing
#[test] fn one_class_missing_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C1{} impl T{fn foo(self){}} class C2{} impl T{} fn main(){}"#, "missing method"); }

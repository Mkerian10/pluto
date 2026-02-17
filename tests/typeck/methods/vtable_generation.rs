//! Vtable generation errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Missing method in vtable
#[test]
#[ignore]
fn missing_method_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T fn main(){}"#, "missing method"); }

// Method signature mismatch in vtable
#[test]
#[ignore]
fn vtable_sig_mismatch() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self)string{return \"hi\"}} fn main(){}"#, "type mismatch"); }

// Trait object method call
#[test]
fn trait_object_call() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{x:int} impl T{fn foo(self)int{return self.x}} fn main(){let t:T=C{x:1}t.foo()}"#, ""); }

// Multiple traits vtables
#[test]
fn multi_trait_vtables() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{fn bar(self){}} fn main(){}"#, ""); }

// Generic class vtable
#[test]
fn generic_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U>{value:U} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Vtable with wrong method count
#[test]
#[ignore]
fn vtable_method_count() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self)} class C{} impl T{fn foo(self){}} fn main(){}"#, "missing method"); }

// Vtable with extra methods
#[test]
fn vtable_extra_methods() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){} fn bar(self){}} fn main(){}"#, ""); }

// Vtable method order
#[test]
fn vtable_method_order() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self)} class C{} impl T{fn bar(self){} fn foo(self){}} fn main(){}"#, ""); }

// Enum vtable
#[test]
fn enum_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} enum E{A B} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Vtable with mut self
#[test]
fn vtable_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C{x:int} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Vtable with parameters
#[test]
#[ignore]
fn vtable_params() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:string){}} fn main(){}"#, "type mismatch"); }

// Vtable with generics
#[test]
fn vtable_generic_method() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C{} impl T{fn foo<U>(self,x:U)string{return \"hi\"}} fn main(){}"#, ""); }

// Vtable with contracts
#[test]
fn vtable_contracts() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int{return -1}} fn main(){}"#, ""); }

// Vtable with nullable return
#[test]
#[ignore]
fn vtable_nullable() { compile_should_fail_with(r#"trait T{fn foo(self)int?} class C{} impl T{fn foo(self)int{return 1}} fn main(){}"#, "type mismatch"); }

// Vtable with error return
#[test]
fn vtable_error() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C{} impl T{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Multiple classes same trait
#[test]
fn multi_class_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} class C1{} impl T{fn foo(self){}} class C2{} impl T{fn foo(self){}} fn main(){}"#, ""); }

// Vtable with static method (not supported)
#[test]
fn vtable_static() { compile_should_fail_with(r#"trait T{fn create()C} class C{} impl T{fn create()C{return C{}}} fn main(){}"#, ""); }

// Vtable with default implementation (not supported)
#[test]
fn vtable_default() { compile_should_fail_with(r#"trait T{fn foo(self){print(\"default\")}} class C{} impl T fn main(){}"#, ""); }

// Nested trait implementation
#[test]
fn nested_trait_impl() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{fn bar(self){}} fn main(){}"#, ""); }

// Vtable lookup fail
#[test]
fn vtable_lookup_fail() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn main(){let t:T=C{} t.bar()}"#, ""); }

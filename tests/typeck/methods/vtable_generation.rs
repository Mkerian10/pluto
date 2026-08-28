//! Vtable generation errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Missing method in vtable
#[test]
fn missing_method_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)}
class C impl T{}
fn main(){}"#, "does not implement required method"); }

// Method signature mismatch in vtable
#[test]
fn vtable_sig_mismatch() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C impl T{fn foo(self)string{return "hi"}}
fn main(){}"#, "type mismatch"); }

// Trait object method call
#[test]
fn trait_object_call() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C impl T{x:int
fn foo(self)int{return self.x}} fn main(){let t:T=C{x:1}t.foo()}"#, "expected newline after statement"); }

// Multiple traits vtables
#[test]
fn multi_trait_vtables() {
    assert_eq!(compile_and_run(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C impl T1, T2{
fn foo(self){}
fn bar(self){}} fn main(){}"#), 0);
}

// Generic class vtable
#[test]
fn generic_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U>{value:U} impl T{fn foo(self){}} fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Vtable with wrong method count
#[test]
fn vtable_method_count() { compile_should_fail_with(r#"trait T{fn foo(self)
fn bar(self)} class C impl T{fn foo(self){}}
fn main(){}"#, "does not implement required method"); }

// Vtable with extra methods
#[test]
fn vtable_extra_methods() {
    assert_eq!(compile_and_run(r#"trait T{fn foo(self)} class C impl T{
fn foo(self){} fn bar(self){}} fn main(){}"#), 0);
}

// Vtable method order
#[test]
fn vtable_method_order() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{fn foo(self)
fn bar(self)}
class C impl T{
fn bar(self){}
fn foo(self){}}
fn main(){}"#).is_ok()); }

// Enum vtable
#[test]
fn enum_vtable() { compile_should_fail_with(r#"trait T{fn foo(self)} enum E{A B} impl T{fn foo(self){}} fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Vtable with mut self
#[test]
fn vtable_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C impl T{x:int
fn foo(self){}} fn main(){}"#, "method 'foo' in trait 'T' declares 'mut self', but class 'C' does not"); }

// Vtable with parameters
#[test]
fn vtable_params() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C impl T{fn foo(self,x:string){}}
fn main(){}"#, "type mismatch"); }

// Vtable with generics
#[test]
fn vtable_generic_method() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U}
class C impl T{
fn foo<U>(self,x:U)string{return "hi"}}
fn main(){}"#, "expected (, found <"); }

// Vtable with contracts
#[test]
fn vtable_contracts() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C impl T{
fn foo(self)int{return -1}} fn main(){}"#, "'ensures' clauses are not supported: Pluto has no postconditions by design; express guarantees with class invariants or return types (see docs/design/contracts.md)"); }

// Vtable with nullable return
#[test]
fn vtable_nullable() { compile_should_fail_with(r#"trait T{fn foo(self)int?} class C impl T{fn foo(self)int{return 1}}
fn main(){}"#, "type mismatch"); }

// Vtable with error return
#[test]
fn vtable_error() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C impl T{
fn foo(self)int{return 1}} fn main(){}"#, "expected newline after statement"); }

// Multiple classes same trait
#[test]
fn multi_class_vtable() {
    assert_eq!(compile_and_run(r#"trait T{fn foo(self)} class C1 impl T{
fn foo(self){}} class C2 impl T{
fn foo(self){}} fn main(){}"#), 0);
}

// Vtable with static method (not supported)
#[test]
fn vtable_static() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{fn create()C}
class C impl T {
fn create()C{return C{}}}
fn main(){}"#).is_ok()); }

// Vtable with default implementation (not supported)
#[test]
fn vtable_default() { compile_should_fail_with(r#"trait T{fn foo(self){print("default")}}
class C{}
impl T
fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Nested trait implementation
#[test]
fn nested_trait_impl() {
    assert_eq!(compile_and_run(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C impl T1, T2{
fn foo(self){}
fn bar(self){}} fn main(){}"#), 0);
}

// Vtable lookup fail
#[test]
fn vtable_lookup_fail() { compile_should_fail_with(r#"trait T{fn foo(self)} class C impl T{
fn foo(self){}} fn main(){let t:T=C{} t.bar()}"#, "expected newline after statement"); }

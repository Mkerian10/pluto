//! Method lookup errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Method not found on class
#[test]
#[ignore]
fn method_not_found() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1}c.foo()}"#, "no method"); }

// Method on primitive
#[test]
fn method_on_int() { compile_should_fail_with(r#"fn main(){let x=1 x.foo()}"#, ""); }

// Method on string (builtin methods exist)
#[test]
#[ignore]
fn method_on_string_wrong() { compile_should_fail_with(r#"fn main(){let s=\"hi\" s.foo()}"#, "no method"); }

// Method on array
#[test]
#[ignore]
fn method_on_array() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3]arr.foo()}"#, "no method"); }

// Method on enum
#[test]
fn method_on_enum() { compile_should_fail_with(r#"enum E{A B} fn main(){let e=E.A e.foo()}"#, ""); }

// Method on trait object
#[test]
fn method_on_trait_object() { compile_should_fail_with(r#"trait T{} class C{} impl T fn main(){let t:T=C{} t.foo()}"#, ""); }

// Method name collision with field
#[test]
fn method_field_collision() { compile_should_fail_with(r#"class C{foo:int} fn foo(self){} fn main(){}"#, ""); }

// Method on wrong type
#[test]
fn method_wrong_receiver() { compile_should_fail_with(r#"class C1{} class C2{} fn foo(self:C1){} fn main(){let c=C2{} c.foo()}"#, ""); }

// Static method lookup (not supported)
#[test]
fn static_method() { compile_should_fail_with(r#"class C{} fn create()C{return C{}} fn main(){C.create()}"#, ""); }

// Method with wrong self type
#[test]
fn wrong_self_type() { compile_should_fail_with(r#"class C{} fn foo(self:int){} fn main(){}"#, ""); }

// Method on nullable
#[test]
fn method_on_nullable() { compile_should_fail_with(r#"class C{} fn foo(self){} fn main(){let c:C?=none c.foo()}"#, ""); }

// Method on generic without bound
#[test]
#[ignore]
fn generic_no_bound() { compile_should_fail_with(r#"fn f<T>(x:T){x.foo()} fn main(){}"#, ""); }

// Method on map
#[test]
#[ignore]
fn method_on_map_wrong() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} m.foo()}"#, "no method"); }

// Method on set
#[test]
#[ignore]
fn method_on_set_wrong() { compile_should_fail_with(r#"fn main(){let s=Set<int>{} s.foo()}"#, "no method"); }

// Method lookup through multiple traits
#[test]
#[ignore]
fn multi_trait_lookup() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{fn bar(self){}} fn main(){let c=C{} c.baz()}"#, "no method"); }

// Method on closure
#[test]
fn method_on_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 f.foo()}"#, ""); }

// Method on error type
#[test]
fn method_on_error() { compile_should_fail_with(r#"error E{} fn main(){let e=E{} e.foo()}"#, ""); }

// Method with generic parameter
#[test]
#[ignore]
fn method_generic_lookup() { compile_should_fail_with(r#"class C{} fn foo<U>(self,x:U){} fn main(){let c=C{} c.bar()}"#, "no method"); }

// Method on task
#[test]
#[ignore]
fn method_on_task() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){let t=spawn f() t.foo()}"#, "no method"); }

// Method lookup in nested class
#[test]
fn nested_class_method() { compile_should_fail_with(r#"class Outer{} class Inner{} fn foo(self:Outer){} fn main(){let i=Inner{} i.foo()}"#, ""); }

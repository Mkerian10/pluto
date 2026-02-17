//! Trait object type errors - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic trait object type mismatches
#[test]
#[ignore]
fn trait_object_wrong_type() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=42}"#, "type mismatch"); }
#[test]
#[ignore]
fn trait_object_non_impl_class() { compile_should_fail_with(r#"trait T{} class C{x:int} fn main(){let t:T=C{x:1}}"#, "does not implement"); }

// Method calls on trait objects
#[test]
fn trait_object_wrong_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(self){}} fn main(){let t:T=C{x:1}t.bar()}"#, ""); }
#[test]
#[ignore]
fn trait_object_wrong_method_sig() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{x:int} impl T{fn foo(self)int{return 1}} fn main(){let t:T=C{x:1}let s:string=t.foo()}"#, "type mismatch"); }

// Trait object assignment errors
#[test]
#[ignore]
fn assign_wrong_trait() { compile_should_fail_with(r#"trait T1{} trait T2{} class C{x:int} impl T1 fn main(){let t:T2=C{x:1}}"#, "does not implement"); }
#[test]
#[ignore]
fn trait_object_to_concrete() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} let c:C=t}"#, "type mismatch"); }

// Generic function with trait objects
#[test]
fn generic_fn_trait_object() { compile_should_fail_with(r#"trait T{} fn id<U>(x:U)U{return x} fn main(){let t:T id(t)}"#, ""); }

// Trait object in collections
#[test]
fn array_of_trait_objects_mixed() { compile_should_fail_with(r#"trait T{} class C1{x:int} impl T class C2{y:string} fn main(){let arr:[T]=[C1{x:1},C2{y:\"hi\"}]}"#, ""); }
#[test]
#[ignore]
fn array_trait_object_type_mismatch() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let arr:[T]=[C{x:1},42]}"#, "type mismatch"); }

// Nullable trait objects
#[test]
#[ignore]
fn nullable_trait_object() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T?=none let x:T=t}"#, "type mismatch"); }
#[test]
fn trait_object_to_nullable() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} let n:T?=t}"#, ""); }

// Field access on trait objects
#[test]
fn trait_object_field_access() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} let y=t.x}"#, ""); }

// Trait objects with generics
#[test]
fn trait_object_generic_class() { compile_should_fail_with(r#"trait T{} class Box<U>{value:U} impl T fn main(){let t:T=Box<int>{value:42}}"#, ""); }

// Multiple trait objects
#[test]
#[ignore]
fn two_trait_objects_mismatch() { compile_should_fail_with(r#"trait T1{} trait T2{} class C{x:int} impl T1 fn main(){let t1:T1=C{x:1} let t2:T2=t1}"#, "type mismatch"); }

// Trait object return types
#[test]
fn return_trait_object_wrong() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f()T{return 42} fn main(){}"#, "type mismatch"); }
#[test]
fn return_concrete_as_trait() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f()C{return C{x:1}} fn main(){let t:T=f()}"#, ""); }

// Trait object parameters
#[test]
#[ignore]
fn param_trait_object_wrong() { compile_should_fail_with(r#"trait T{} fn use_trait(t:T){} fn main(){use_trait(42)}"#, "type mismatch"); }
#[test]
#[ignore]
fn param_trait_non_impl() { compile_should_fail_with(r#"trait T{} class C{x:int} fn use_trait(t:T){} fn main(){use_trait(C{x:1})}"#, "does not implement"); }

// Casting to trait objects
#[test]
fn cast_to_trait() { compile_should_fail_with(r#"trait T{} class C{x:int} fn main(){let c=C{x:1} let t=c as T}"#, ""); }
#[test]
fn cast_trait_to_concrete() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} let c=t as C}"#, ""); }

// Map/Set with trait objects
#[test]
#[ignore]
fn map_value_trait_object() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let m:Map<string,T>=Map<string,T>{} m[\"a\"]=42}"#, "type mismatch"); }
#[test]
#[ignore]
fn set_trait_object() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let s:Set<T>=Set<T>{} s.insert(42)}"#, "type mismatch"); }

// Trait object with contracts
#[test]
fn trait_object_violates_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{x:int} impl T{fn foo(self)int{return -1}} fn main(){let t:T=C{x:1}}"#, ""); }

// Trait objects in match
#[test]
fn match_on_trait_object() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} match t{}}"#, ""); }

// Trait object size/layout issues
#[test]
fn sizeof_trait_object() { compile_should_fail_with(r#"trait T{} fn main(){let s=sizeof(T)}"#, ""); }

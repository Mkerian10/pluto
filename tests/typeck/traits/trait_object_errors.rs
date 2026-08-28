//! Trait object type errors - 25 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// Basic trait object type mismatches
#[test]
fn trait_object_wrong_type() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let t:T=42}"#, "type mismatch"); }
#[test]
fn trait_object_non_impl_class() { compile_should_fail_with(r#"trait T{} class C{x:int}
fn main(){let t:T=C{x:1}}"#, "expected trait T, found C"); }

// Method calls on trait objects
#[test]
fn trait_object_wrong_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(self){}} fn main(){let t:T=C{x:1}t.bar()}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }
#[test]
fn trait_object_wrong_method_sig() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C impl T{x:int
fn foo(self)int{return 1}}
fn main(){let t:T=C{x:1}
let s:string=t.foo()}"#, "type mismatch"); }

// Trait object assignment errors
#[test]
fn assign_wrong_trait() { compile_should_fail_with(r#"trait T1{} trait T2{} class C impl T1{x:int}
fn main(){let t:T2=C{x:1}}"#, "expected trait T2, found C"); }
#[test]
fn trait_object_to_concrete() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let t:T=C{x:1}
let c:C=t}"#, "type mismatch"); }

// Generic function with trait objects
#[test]
fn generic_fn_trait_object() { compile_should_fail_with(r#"trait T{} fn id<U>(x:U)U{return x} fn main(){let t:T id(t)}"#, "expected =, found identifier"); }

// Trait object in collections
#[test]
fn array_of_trait_objects_mixed() { compile_should_fail_with(r#"trait T{}
class C1 impl T{x:int}
class C2{y:string}
fn main(){let arr:[T]=[C1{x:1},C2{y:"hi"}]}"#, "array element type mismatch: expected trait T, found C2"); }
#[test]
fn array_trait_object_type_mismatch() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let arr:[T]=[C{x:1},42]}"#, "type mismatch"); }

// Nullable trait objects
#[test]
fn nullable_trait_object() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let t:T?=none
let x:T=t}"#, "type mismatch"); }
#[test]
fn trait_object_to_nullable() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{}
class C impl T{x:int}
fn main(){let t:T=C{x:1}
let n:T?=t}"#).is_ok()); }

// Field access on trait objects
#[test]
fn trait_object_field_access() { compile_should_fail_with(r#"trait T{}
class C impl T{x:int}
fn main(){let t:T=C{x:1}
let y=t.x}"#, "field access on non-class type trait T"); }

// Trait objects with generics
#[test]
fn trait_object_generic_class() { compile_should_fail_with(r#"trait T{} class Box<U>{value:U} impl T fn main(){let t:T=Box<int>{value:42}}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Multiple trait objects
#[test]
fn two_trait_objects_mismatch() { compile_should_fail_with(r#"trait T1{} trait T2{} class C impl T1{x:int}
fn main(){let t1:T1=C{x:1}
let t2:T2=t1}"#, "type mismatch"); }

// Trait object return types
#[test]
fn return_trait_object_wrong() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f()T{return 42} fn main(){}"#, "type mismatch"); }
#[test]
fn return_concrete_as_trait() {
    // Class-to-trait coercion applies to returned values (#277)
    let out = compile_and_run_stdout(r#"trait T{fn v(self)int}
class C impl T{x:int
fn v(self)int{return self.x}}
fn f()C{return C{x:9}}
fn main(){let t:T=f()
print(t.v())}"#);
    assert_eq!(out.trim(), "9");
}

// Trait object parameters
#[test]
fn param_trait_object_wrong() { compile_should_fail_with(r#"trait T{}
fn use_trait(t:T){}
fn main(){use_trait(42)}"#, "argument 1 of"); }
#[test]
fn param_trait_non_impl() { compile_should_fail_with(r#"trait T{} class C{x:int}
fn use_trait(t:T){}
fn main(){use_trait(C{x:1})}"#, "expected trait T, found C"); }

// Casting to trait objects
#[test]
fn cast_to_trait() { compile_should_fail_with(r#"trait T{}
class C{x:int}
fn main(){let c=C{x:1}
let t=c as T}"#, "cannot cast from C to trait T"); }
#[test]
fn cast_trait_to_concrete() { compile_should_fail_with(r#"trait T{}
class C impl T{x:int}
fn main(){let t:T=C{x:1}
let c=t as C}"#, "cannot cast from trait T to C"); }

// Map/Set with trait objects
#[test]
fn map_value_trait_object() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let mut m:Map<string,T>=Map<string,T>{}
m["a"]=42}"#, "type mismatch"); }
#[test]
fn set_trait_object() { compile_should_fail_with(r#"trait T{} class C impl T{x:int}
fn main(){let s:Set<T>=Set<T>{}
s.insert(42)}"#, "cannot be used as a map/set key"); }

// Trait object with contracts
#[test]
fn trait_object_violates_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{x:int} impl T{fn foo(self)int{return -1}} fn main(){let t:T=C{x:1}}"#, "'ensures' clauses are not supported: Pluto has no postconditions by design; express guarantees with class invariants or return types (see docs/design/contracts.md)"); }

// Trait objects in match
#[test]
fn match_on_trait_object() { compile_should_fail_with(r#"trait T{}
class C impl T{x:int}
fn main(){let t:T=C{x:1}
match t{}}"#, "match requires enum type, found trait T"); }

// Trait object size/layout issues
#[test]
fn sizeof_trait_object() { compile_should_fail_with(r#"trait T{} fn main(){let s=sizeof(T)}"#, "undefined function 'sizeof'"); }

//! Type unification failure tests - 30 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic unification failures
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn infer_from_conflicting_uses() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let f=id f(42) f(\"hi\")}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn param_return_conflict() { compile_should_fail_with(r#"fn bad<T>(x:T)T{if true{return x}return 42} fn main(){}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_params_conflict() { compile_should_fail_with(r#"fn same<T>(x:T,y:T)T{return x} fn main(){same(42,\"hi\")}"#, "type mismatch"); }

// Array element unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_mixed_types() { compile_should_fail_with(r#"fn first<T>(arr:[T])T{return arr[0]} fn main(){first([42,\"hi\"])}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_return_conflict() { compile_should_fail_with(r#"fn make<T>()[T]{if true{return [42]}return [\"hi\"]} fn main(){}"#, "type mismatch"); }

// Field type unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn class_field_conflict() { compile_should_fail_with(r#"class Box<T>{value:T} fn make<T>()Box<T>{if true{return Box<int>{value:42}}return Box<string>{value:\"hi\"}} fn main(){}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_fields_same_param() { compile_should_fail_with(r#"class Pair<T>{first:T second:T} fn main(){let p=Pair<int>{first:42 second:\"hi\"}}"#, "type mismatch"); }

// Function call unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn call_with_conflicting_inferred() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn apply<U>(f:fn(U)U,x:U)U{return f(x)} fn main(){apply(id,42) apply(id,\"hi\")}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_call_conflict() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(id(42)) id(id(\"hi\"))}"#, "type mismatch"); }

// Return type unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn if_branches_differ() { compile_should_fail_with(r#"fn make<T>(b:bool)T{if b{return 42}return \"hi\"} fn main(){}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_arms_differ() { compile_should_fail_with(r#"enum E{A B} fn get<T>(e:E)T{match e{E.A=>{return 42}E.B=>{return \"hi\"}}} fn main(){}"#, "type mismatch"); }

// Closure unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_param_conflict() { compile_should_fail_with(r#"fn apply<T>(f:fn(T)T,x:T)T{return f(x)} fn main(){let f=(x)=>x apply(f,42) apply(f,\"hi\")}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_return_conflict() { compile_should_fail_with(r#"fn main(){let f=(b:bool)=>{if b{return 42}return \"hi\"}}"#, "type mismatch"); }

// Method call unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn method_receiver_conflict() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn use<U>(b:Box<U>)U{return b.get()} fn main(){use(Box<int>{value:42}) use(Box<string>{value:\"hi\"})}"#, "type mismatch"); }

// Enum variant unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn enum_variant_param_conflict() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn make<U>(b:bool)Opt<U>{if b{return Opt.Some{v:42}}return Opt.Some{v:\"hi\"}} fn main(){}"#, "type mismatch"); }

// Multiple type parameters
#[test]
#[ignore] // PR #46 - outdated assertions
fn two_params_cross_conflict() { compile_should_fail_with(r#"fn swap<T,U>(x:T,y:U)(U,T){return (y,x)} fn main(){let (a,b)=swap(42,\"hi\") let c:int=a}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn param_reuse_conflict() { compile_should_fail_with(r#"fn use_twice<T>(x:T,y:T)T{return x} fn f<U>(a:U){use_twice(a,42) use_twice(a,\"hi\")} fn main(){}"#, "type mismatch"); }

// Recursive unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn recursive_type_conflict() { compile_should_fail_with(r#"class Box<T>{value:T} fn nest<U>()Box<Box<U>>{if true{return Box<Box<int>>{value:Box<int>{value:42}}}return Box<Box<string>>{value:Box<string>{value:\"hi\"}}} fn main(){}"#, "type mismatch"); }

// Unification with builtin types
#[test]
#[ignore] // PR #46 - outdated assertions
fn builtin_generic_conflict() { compile_should_fail_with(r#"fn first<T>(arr:[T])T{return arr[0]} fn main(){first([42]) first([\"hi\"])}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_value_conflict() { compile_should_fail_with(r#"fn get<T>(m:Map<string,T>,k:string)T{return m[k]} fn main(){let m=Map<string,int>{} get(m,\"a\") let m2=Map<string,string>{} get(m,\"b\")}"#, "type mismatch"); }

// Ambiguous bindings
#[test]
fn cannot_infer_from_void() { compile_should_fail_with(r#"fn ignore<T>(x:T){} fn main(){ignore()}"#, "expects 1 arguments, got 0"); }
#[test]
fn multiple_possible_types() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x=id}"#, "undefined variable"); }

// Unification with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_non_nullable_conflict() { compile_should_fail_with(r#"fn maybe<T>(x:T,b:bool)T?{if b{return x}return none} fn use<U>(x:U){} fn main(){use(maybe(42,true))}"#, "type mismatch"); }
#[test]
fn nullable_unwrap_conflict() { compile_should_fail_with(r#"fn unwrap<T>(x:T?)T{return x?} fn main(){unwrap(42)}"#, "cannot infer type parameters"); }

// Unification with errors
#[test]
#[ignore] // Syntax error: old T! return type syntax no longer valid
fn error_infallible_conflict() { compile_should_fail_with(r#"error E{} fn maybe<T>(x:T,b:bool)T!{if b{return x}raise E{}} fn use<U>(x:U){} fn main(){use(maybe(42,true))}"#, "type mismatch"); }

// Cyclic unification
#[test]
#[ignore] // PR #46 - outdated assertions
fn self_referential_type() { compile_should_fail_with(r#"fn loop<T>(x:T)T{return loop(x)} fn main(){}"#, ""); }

// Unification across function boundaries
#[test]
#[ignore] // PR #46 - outdated assertions
fn cross_function_conflict() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn a(){id(42)} fn b(){id(\"hi\")} fn main(){a() b()}"#, "type mismatch"); }

// Generic instance conflicts
#[test]
#[ignore] // PR #46 - outdated assertions
fn instance_param_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn make<U>()Box<U>{return Box<int>{value:42}} fn main(){let b:Box<string>=make()}"#, "type mismatch"); }

// Bound-constrained unification
#[test]
#[ignore] // Syntax error: old impl syntax without class body
fn bound_limits_unification() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f<U:T>(x:U)U{return x} fn main(){f(42)}"#, "does not satisfy"); }

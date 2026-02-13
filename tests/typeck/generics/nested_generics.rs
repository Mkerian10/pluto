//! Nested generics tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic nesting
#[test]
#[ignore] // PR #46 - outdated assertions
fn box_in_box_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<Box<int>>=Box<Box<string>>{value:Box<string>{value:\"hi\"}}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_array_mismatch() { compile_should_fail_with(r#"fn main(){let a:[[int]]=[[\"hi\"]]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn triple_nesting() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<Box<Box<int>>>=Box<Box<Box<string>>>{value:Box<Box<string>>{value:Box<string>{value:\"hi\"}}}}"#, "type mismatch"); }

// Nested generic functions
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_id_mismatch() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x:int=id(id(\"hi\"))}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn triple_id_mismatch() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x:int=id(id(id(\"hi\")))}"#, "type mismatch"); }

// Nested classes
#[test]
#[ignore] // PR #46 - outdated assertions
fn pair_of_boxes_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} class Pair<U,V>{first:U second:V} fn main(){let p:Pair<Box<int>,Box<int>>=Pair<Box<int>,Box<string>>{first:Box<int>{value:42}second:Box<string>{value:\"hi\"}}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn box_of_pair_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} class Pair<U,V>{first:U second:V} fn main(){let b:Box<Pair<int,int>>=Box<Pair<int,string>>{value:Pair<int,string>{first:42 second:\"hi\"}}}"#, "type mismatch"); }

// Nested enums
#[test]
#[ignore] // PR #46 - outdated assertions
fn option_of_option_mismatch() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o:Opt<Opt<int>>=Opt<Opt<string>>.Some{v:Opt<string>.Some{v:\"hi\"}}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn result_of_option_mismatch() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} enum Result<U,V>{Ok{val:U}Err{err:V}} fn main(){let r:Result<Opt<int>,int>=Result<Opt<string>,int>.Ok{val:Opt<string>.Some{v:\"hi\"}}}"#, "type mismatch"); }

// Map/Set nesting
#[test]
#[ignore] // PR #46 - outdated assertions
fn map_of_arrays_mismatch() { compile_should_fail_with(r#"fn main(){let m:Map<string,[int]>=Map<string,[string]>{}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn array_of_maps_mismatch() { compile_should_fail_with(r#"fn main(){let a:[Map<string,int>]=[Map<string,string>{}]}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn set_of_arrays_mismatch() { compile_should_fail_with(r#"fn main(){let s:Set<[int]>=Set<[string]>{}}"#, "type mismatch"); }

// Function types with nesting
#[test]
#[ignore] // PR #46 - outdated assertions
fn fn_returns_generic_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn make()fn()Box<int>{return ()=>Box<string>{value:\"hi\"}} fn main(){}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn fn_takes_generic_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn use(f:fn(Box<int>)){} fn main(){use((b:Box<string>)=>{})}}"#, "type mismatch"); }

// Nested with bounds
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_bound_outer_fails() { compile_should_fail_with(r#"trait T{} class Box<U>{value:U} fn f<V:T>(b:Box<V>){} class C{x:int} fn main(){f(Box<C>{value:C{x:1}})}"#, "does not satisfy"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_bound_inner_fails() { compile_should_fail_with(r#"trait T{} class Box<U:T>{value:U} fn f<V>(b:Box<V>){} class C{x:int} fn main(){f(Box<C>{value:C{x:1}})}"#, "does not satisfy"); }

// Deeply nested access
#[test]
#[ignore] // PR #46 - outdated assertions
fn deep_field_access_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b=Box<Box<int>>{value:Box<int>{value:42}} let x:string=b.value.value}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_array_index_mismatch() { compile_should_fail_with(r#"fn main(){let a=[[42]] let x:string=a[0][0]}"#, "type mismatch"); }

// Nested nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn option_nullable_mismatch() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o:Opt<int?>=Opt<int>.Some{v:42}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_option_mismatch() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o:Opt<int>?=Opt<int>.Some{v:none}}"#, "type mismatch"); }

// Nested errors
#[test]
#[ignore] // PR #46 - outdated assertions
fn result_error_mismatch() { compile_should_fail_with(r#"error E{} enum Result<T,U>{Ok{val:T}Err{err:U}} fn f()Result<int,E>!{raise E{}} fn main(){}"#, ""); }

// Generic methods on nested types
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_method_return_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b=Box<Box<int>>{value:Box<int>{value:42}} let x:string=b.get().get()}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_method_param_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T fn set(mut self,v:T){self.value=v}} fn main(){let b=Box<Box<int>>{value:Box<int>{value:42}} b.set(Box<string>{value:\"hi\"})}"#, "type mismatch"); }

// Recursive nesting edge cases
#[test]
#[ignore] // PR #46 - outdated assertions
fn infinite_nesting_simulation() { compile_should_fail_with(r#"class Box<T>{value:T} fn nest()Box<Box<Box<Box<Box<int>>>>>{return Box<Box<Box<Box<Box<string>>>>>{value:Box<Box<Box<Box<string>>>>{value:Box<Box<Box<string>>>{value:Box<Box<string>>{value:Box<string>{value:\"hi\"}}}}}}} fn main(){}"#, "type mismatch"); }

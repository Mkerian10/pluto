//! Monomorphization span collision tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Multiple instantiations same function
#[test]
#[ignore]
fn two_instances_type_error() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(42) id(true) let x:int=id(true)}"#, "type mismatch"); }
#[test]
#[ignore]
fn three_instances_error() { compile_should_fail_with(r#"fn process<T>(x:T)T{return x} fn main(){process(42) process(3.14) process(true) let x:int=process(3.14)}"#, "type mismatch"); }

// Class instantiation errors
#[test]
#[ignore]
fn box_multi_instance_error() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b1=Box<int>{value:42} let b2=Box<bool>{value:true} let b3:Box<int>=Box<bool>{value:true}}"#, "type mismatch"); }
#[test]
#[ignore]
fn nested_instance_error() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b1=Box<Box<int>>{value:Box<int>{value:42}} let b2:Box<Box<int>>=Box<Box<bool>>{value:Box<bool>{value:true}}}"#, "type mismatch"); }

// Enum instantiation errors
#[test]
#[ignore]
fn option_multi_instance_error() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o1=Opt<int>.Some{v:42} let o2=Opt<bool>.Some{v:true} let o3:Opt<int>=Opt<bool>.Some{v:true}}"#, "type mismatch"); }

// Method on generic class errors
#[test]
#[ignore]
fn generic_method_multi_instance() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b1=Box<int>{value:42} let b2=Box<bool>{value:true} let x:int=b2.get()}"#, "type mismatch"); }
#[test]
#[ignore] // #182: compiler doesn't detect type mismatch in generic method body
fn generic_method_wrong_return() { compile_should_fail_with(r#"class Box<T>{value:T fn wrong(self)T{return 42}} fn main(){let b=Box<bool>{value:true} b.wrong()}"#, "type mismatch"); }

// Closure capture with generics
#[test]
#[ignore]
fn closure_in_generic_capture_error() { compile_should_fail_with(r#"fn make<T>(x:T)fn()T{return ()=>x} fn main(){make(42) make(true) let f=make(true) let x:int=f()}"#, "type mismatch"); }

// Recursive generic errors
#[test]
#[ignore]
fn recursive_generic_error() { compile_should_fail_with(r#"fn rec<T>(x:T,n:int)T{if n==0{return x}return rec(x,n-1)} fn main(){rec(42,5) rec(true,3) let x:int=rec(true,2)}"#, "type mismatch"); }

// Generic with error types
#[test]
#[ignore] // Syntax error: old T! return type syntax no longer valid
fn generic_error_multi_instance() { compile_should_fail_with(r#"error E{} fn maybe<T>(x:T)T!{if true{raise E{}}return x} fn main(){maybe(42) maybe(true) let x:int=maybe(true)}"#, "type mismatch"); }

// Generic with nullable
#[test]
#[ignore]
fn generic_nullable_error() { compile_should_fail_with(r#"fn wrap<T>(x:T)T?{return x} fn main(){wrap(42) wrap(true) let x:int?=wrap(true) let y:int=x}"#, "type mismatch"); }

// Span offset collision check
#[test]
#[ignore]
fn many_instances_span_test() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(42) id(3.14) id(true) id(99) id([1]) let x:int=id(3.14)}"#, "type mismatch"); }

// Monomorphized function body error
#[test]
#[ignore] // #182: compiler doesn't detect type mismatch in generic function body
fn body_error_after_mono() { compile_should_fail_with(r#"fn bad<T>(x:T)T{let y:int=x return x} fn main(){bad(42)}"#, "type mismatch"); }
#[test]
#[ignore] // #182: compiler doesn't detect type mismatch in generic function body
fn body_error_second_instance() { compile_should_fail_with(r#"fn bad<T>(x:T)T{let y:bool=x return x} fn main(){bad(42) bad(true)}"#, "type mismatch"); }

// Match on generic enum errors
#[test]
#[ignore] // Syntax error: match binding with => not supported
fn match_generic_enum_error() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn unwrap<U>(o:Opt<U>)U{match o{Opt.Some{v}=>{return v}Opt.None=>{return 42}}} fn main(){unwrap(Opt<string>.None)}"#, "type mismatch"); }

// Generic trait impl errors
#[test]
#[ignore] // Syntax error: impl without class name
fn generic_trait_impl_error() { compile_should_fail_with(r#"trait T{fn foo(self)int} class Box<U>{value:U} impl T{fn foo(self)int{return self.value}} fn main(){}"#, "type mismatch"); }

// Conflicting type params in calls
#[test]
#[ignore]
fn call_chain_generic_error() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn use<U>(x:U)U{return id(x)} fn main(){use(42) let x:int=use(true)}"#, "type mismatch"); }

// Generic array operations
#[test]
#[ignore]
fn generic_array_error() { compile_should_fail_with(r#"fn first<T>(arr:[T])T{return arr[0]} fn main(){first([42,43]) first([true,false]) let x:int=first([true])}"#, "type mismatch"); }

// Bounds violation after mono
#[test]
#[ignore] // Syntax error: impl without class name
fn bound_check_after_mono() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f<U:T>(x:U){} fn main(){f(C{x:1}) f(42)}"#, "does not satisfy"); }

// Error in monomorphized closure
#[test]
#[ignore]
fn closure_body_mono_error() { compile_should_fail_with(r#"fn apply<T>(f:fn(T)T,x:T)T{return f(x)} fn main(){apply((x:int)=>x+1,42) apply((x:bool)=>x+1,true)}"#, "operator not supported for type bool"); }

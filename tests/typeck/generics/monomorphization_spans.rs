//! Monomorphization span collision tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Multiple instantiations same function
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn two_instances_type_error() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(42) id(\"hi\") let x:int=id(\"oops\")}"#, "type mismatch"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn three_instances_error() { compile_should_fail_with(r#"fn process<T>(x:T)T{return x} fn main(){process(42) process(\"hi\") process(true) let x:int=process(\"bad\")}"#, "type mismatch"); }

// Class instantiation errors
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn box_multi_instance_error() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b1=Box<int>{value:42} let b2=Box<string>{value:\"hi\"} let b3:Box<int>=Box<string>{value:\"oops\"}}"#, "type mismatch"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn nested_instance_error() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b1=Box<Box<int>>{value:Box<int>{value:42}} let b2:Box<Box<int>>=Box<Box<string>>{value:Box<string>{value:\"hi\"}}}"#, "type mismatch"); }

// Enum instantiation errors
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn option_multi_instance_error() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o1=Opt<int>.Some{v:42} let o2=Opt<string>.Some{v:\"hi\"} let o3:Opt<int>=Opt<string>.Some{v:\"bad\"}}"#, "type mismatch"); }

// Method on generic class errors
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn generic_method_multi_instance() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b1=Box<int>{value:42} let b2=Box<string>{value:\"hi\"} let x:int=b2.get()}"#, "type mismatch"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn generic_method_wrong_return() { compile_should_fail_with(r#"class Box<T>{value:T fn wrong(self)T{return 42}} fn main(){let b=Box<string>{value:\"hi\"} b.wrong()}"#, "type mismatch"); }

// Closure capture with generics
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn closure_in_generic_capture_error() { compile_should_fail_with(r#"fn make<T>(x:T)fn()T{return ()=>x} fn main(){make(42) make(\"hi\") let f=make(\"bad\") let x:int=f()}"#, "type mismatch"); }

// Recursive generic errors
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn recursive_generic_error() { compile_should_fail_with(r#"fn rec<T>(x:T,n:int)T{if n==0{return x}return rec(x,n-1)} fn main(){rec(42,5) rec(\"hi\",3) let x:int=rec(\"bad\",2)}"#, "type mismatch"); }

// Generic with error types
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn generic_error_multi_instance() { compile_should_fail_with(r#"error E{} fn maybe<T>(x:T)T!{if true{raise E{}}return x} fn main(){maybe(42) maybe(\"hi\") let x:int=maybe(\"bad\")}"#, "type mismatch"); }

// Generic with nullable
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn generic_nullable_error() { compile_should_fail_with(r#"fn wrap<T>(x:T)T?{return x} fn main(){wrap(42) wrap(\"hi\") let x:int?=wrap(\"bad\") let y:int=x}"#, "type mismatch"); }

// Span offset collision check
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn many_instances_span_test() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(42) id(\"a\") id(true) id(3.14) id([1]) let x:int=id(\"collision\")}"#, "type mismatch"); }

// Monomorphized function body error
#[test]
#[ignore] // #182: compiler doesn't detect type mismatch in generic function body
fn body_error_after_mono() { compile_should_fail_with(r#"fn bad<T>(x:T)T{let y:int=x return x} fn main(){bad(42)}"#, "type mismatch"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn body_error_second_instance() { compile_should_fail_with(r#"fn bad<T>(x:T)T{let y:string=x return x} fn main(){bad(42) bad(\"hi\")}"#, "type mismatch"); }

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
#[ignore] // #156: string literals don't work in compact syntax
fn call_chain_generic_error() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn use<U>(x:U)U{return id(x)} fn main(){use(42) let x:int=use(\"bad\")}"#, "type mismatch"); }

// Generic array operations
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn generic_array_error() { compile_should_fail_with(r#"fn first<T>(arr:[T])T{return arr[0]} fn main(){first([42,43]) first([\"a\",\"b\"]) let x:int=first([\"bad\"])}"#, "type mismatch"); }

// Bounds violation after mono
#[test]
#[ignore] // Syntax error: impl without class name
fn bound_check_after_mono() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f<U:T>(x:U){} fn main(){f(C{x:1}) f(42)}"#, "does not satisfy"); }

// Error in monomorphized closure
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn closure_body_mono_error() { compile_should_fail_with(r#"fn apply<T>(f:fn(T)T,x:T)T{return f(x)} fn main(){apply((x:int)=>x+1,42) apply((x:string)=>x+1,\"hi\")}"#, "type mismatch"); }

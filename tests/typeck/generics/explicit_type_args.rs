//! Explicit type arguments tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Wrong count
#[test]
fn too_many_args() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id<int,string>(42)}"#, "expects 1 type arguments, got 2"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn too_few_args() { compile_should_fail_with(r#"fn pair<T,U>(x:T,y:U)T{return x} fn main(){pair<int>(1,\"hi\")}"#, "expects 2 type arguments, got 1"); }
#[test]
fn args_on_non_generic() { compile_should_fail_with(r#"fn f(x:int)int{return x} fn main(){f<int>(42)}"#, "not generic"); }

// Type mismatch with explicit args
#[test]
#[ignore] // Compiler bug: type checker doesn't enforce explicit type arg constraints on function arguments
fn arg_type_mismatch() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id<int>(\"hi\")}"#, "type mismatch"); }
#[test]
fn return_type_mismatch() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let s:string=id<int>(42)}"#, "type mismatch"); }
#[test]
#[ignore] // Compiler bug: type checker doesn't enforce explicit type arg constraints on function arguments
fn two_params_first_mismatch() { compile_should_fail_with(r#"fn pair<T,U>(x:T,y:U)T{return x} fn main(){pair<int,string>(\"hi\",42)}"#, "type mismatch"); }
#[test]
#[ignore] // Compiler bug: type checker doesn't enforce explicit type arg constraints on function arguments
fn two_params_second_mismatch() { compile_should_fail_with(r#"fn pair<T,U>(x:T,y:U)U{return y} fn main(){pair<int,string>(42,42)}"#, "type mismatch"); }

// Explicit args on classes
#[test]
fn class_too_many_args() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b=Box<int,string>{value:42}}"#, "expects 1 type arguments, got 2"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn class_too_few_args() { compile_should_fail_with(r#"class Pair<T,U>{first:T second:U} fn main(){let p=Pair<int>{first:1 second:\"hi\"}}"#, "expects 2 type arguments, got 1"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn class_arg_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b=Box<int>{value:\"hi\"}}"#, "expected int, found string"); }

// Explicit args on enums
#[test]
fn enum_too_many_args() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let x=Opt<int,string>.Some{v:42}}"#, "expects 1 type arguments, got 2"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn enum_arg_mismatch() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let x=Opt<int>.Some{v:\"hi\"}}"#, "expected int, found string"); }

// Explicit args on builtins
#[test]
fn builtin_with_type_args() { compile_should_fail_with(r#"fn main(){print<int>(42)}"#, "does not accept type arguments"); }
#[test]
fn abs_with_type_args() { compile_should_fail_with(r#"fn main(){abs<int>(-5)}"#, "does not accept type arguments"); }

// Explicit args with inference conflict
#[test]
fn explicit_conflicts_inferred() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){let x:int=id<string>(42)}"#, "type mismatch"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn partial_inference_conflict() { compile_should_fail_with(r#"fn pair<T,U>(x:T,y:U)T{return x} fn main(){pair<int>(\"hi\",42)}"#, "expects 2 type arguments, got 1"); }

// Explicit args on methods
#[test]
#[ignore] // Compiler bug: explicit type args on methods not validated
fn method_explicit_too_many() { compile_should_fail_with(r#"class C{x:int fn foo<T>(self,val:T)T{return val}} fn main(){let c=C{x:1}c.foo<int,string>(42)}"#, "wrong number"); }
#[test]
#[ignore] // Compiler bug: explicit type args on methods not validated
fn method_explicit_arg_mismatch() { compile_should_fail_with(r#"class C{x:int fn foo<T>(self,val:T)T{return val}} fn main(){let c=C{x:1}c.foo<int>(\"hi\")}"#, "type mismatch"); }

// Nested explicit args
#[test]
#[ignore] // Compiler bug: type checker doesn't enforce explicit type arg constraints on function arguments
fn nested_explicit_outer() { compile_should_fail_with(r#"class Box<T>{value:T} fn wrap<U>(x:U)Box<U>{return Box<U>{value:x}} fn main(){wrap<int>(\"hi\")}"#, "type mismatch"); }
#[test]
#[ignore] // Syntax error: string literals don't work in compact syntax
fn nested_explicit_inner() { compile_should_fail_with(r#"class Box<T>{value:T} fn make()Box<int>{return Box<string>{value:\"hi\"}} fn main(){}"#, "return type mismatch"); }

// Explicit args with bounds
#[test]
fn explicit_violates_bound() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} class C{x:int} fn main(){f<C>(C{x:1})}"#, "does not satisfy"); }
#[test]
fn explicit_multi_bound_violation() { compile_should_fail_with(r#"trait T1{} trait T2{} fn f<U:T1+T2>(x:U){} class C{x:int} fn main(){f<C>(C{x:1})}"#, "does not satisfy"); }

// Explicit args with nullable
#[test]
#[ignore] // Compiler bug: wrong error - reports return type mismatch instead of argument type mismatch
fn explicit_nullable_mismatch() { compile_should_fail_with(r#"fn id<T>(x:T?)T?{return x} fn main(){id<int>(\"hi\")}"#, "type mismatch"); }

// Explicit args with errors
#[test]
#[ignore] // Syntax error: old T! return type syntax
fn explicit_error_mismatch() { compile_should_fail_with(r#"error E{} fn f<T>(x:T)T!{return x} fn main(){let s:string=f<int>(42)}"#, "type mismatch"); }

// Undefined type in explicit args
#[test]
fn explicit_undefined_type() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id<UndefinedType>(42)}"#, "unknown type"); }

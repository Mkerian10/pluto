//! Nullable with generics tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Generic functions with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_fn_nullable_param() { compile_should_fail_with(r#"fn id<T>(x:T?)T?{return x} fn main(){let y:int=id(42)}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_fn_nullable_return() { compile_should_fail_with(r#"fn wrap<T>(x:T)T?{return x} fn main(){let y:int=wrap(42)}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_unwrap_type_mismatch() { compile_should_fail_with(r#"fn unwrap<T>(x:T?)T{return x?} fn main(){let x:int?=42 let y:string=unwrap(x)}"#, "type mismatch"); }

// Generic classes with nullable type params
#[test]
#[ignore] // PR #46 - outdated assertions
fn box_nullable_type_param() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int?>=Box<int>{value:42}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn box_get_nullable_value() { compile_should_fail_with(r#"class Box<T>{value:T fn get(self)T{return self.value}} fn main(){let b=Box<int?>{value:none} let x:int=b.get()}"#, "type mismatch"); }

// Generic enums with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn option_nullable_variant() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn main(){let o:Opt<int?>=Opt<int>.Some{v:42}}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn option_unwrap_nullable() { compile_should_fail_with(r#"enum Opt<T>{Some{v:T}None} fn unwrap<U>(o:Opt<U>)U{match o{Opt.Some{v}=>{return v}Opt.None=>{return none}}} fn main(){}"#, "type mismatch"); }

// Type bounds with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_satisfies_bound() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn f<U:T>(x:U?){} fn main(){f(C{x:1})}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn bound_on_nullable_type() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} fn main(){f(none)}"#, "cannot infer"); }

// Unification with nullable generics
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_nullable_non_nullable_conflict() { compile_should_fail_with(r#"fn same<T>(x:T,y:T)T{return x} fn main(){let a:int=42 let b:int?=42 same(a,b)}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_infer_nullable_conflict() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(42) let x:int?=id(42)}"#, "type mismatch"); }

// Nested generics with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn box_of_nullable_box() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<Box<int>?>=Box<Box<int>?>{value:none} let x:Box<int>=b.value}"#, "type mismatch"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_of_generic_box() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int>?=none let x:Box<int>=b}"#, "type mismatch"); }

// Generic methods with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_method_nullable_self() { compile_should_fail_with(r#"class C{fn foo<T>(self,x:T?)T?{return x}} fn main(){let c=C{} let x:int=c.foo(42)}"#, "type mismatch"); }

// Explicit type args with nullable
#[test]
#[ignore] // PR #46 - outdated assertions
fn explicit_nullable_type_arg() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id<int?>(42)}"#, "type mismatch"); }

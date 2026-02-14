//! Implicit T → T? wrapping tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic wrapping (should succeed, so these test inverse cases)
#[test]
fn nullable_to_non_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=42 let y:int=x}"#, "type mismatch"); }
#[test]
fn none_to_non_nullable() { compile_should_fail_with(r#"fn main(){let x:int=none}"#, "type mismatch"); }

// Function return wrapping errors
#[test]
fn return_nullable_from_non_nullable() { compile_should_fail_with(r#"fn f()int{return 42} fn main(){let x:int?=f() let y:int=x}"#, "type mismatch"); }
#[test]
fn return_none_from_non_nullable_fn() { compile_should_fail_with(r#"fn f()int{return none} fn main(){}"#, "type mismatch"); }

// Parameter wrapping errors
#[test]
fn pass_nullable_to_non_nullable() { compile_should_fail_with(r#"fn f(x:int){} fn main(){let y:int?=42 f(y)}"#, "expected int, found int?"); }
#[test]
fn pass_none_to_non_nullable() { compile_should_fail_with(r#"fn f(x:int){} fn main(){f(none)}"#, "expected int, found void?"); }

// Array element wrapping errors
#[test]
fn array_nullable_to_non_nullable() { compile_should_fail_with(r#"fn main(){let a:[int]=[42,none]}"#, "type mismatch"); }
#[test]
fn array_assign_nullable_element() { compile_should_fail_with(r#"fn main(){let a:[int]=[1,2,3] let x:int?=42 a[0]=x}"#, "expected int, found int?"); }

// Class field wrapping errors
#[test]
fn field_nullable_to_non_nullable() { compile_should_fail_with(r#"class C{x:int} fn main(){let y:int?=42 let c=C{x:y}}"#, "expected int, found int?"); }
#[test]
fn field_assign_nullable() { compile_should_fail_with(r#"class C{x:int} fn main(){let mut c=C{x:1} let y:int?=42 c.x=y}"#, "expected int, found int?"); }

// Generic wrapping errors
#[test]
#[ignore] // #170: parser fails on nullable types in generic type arguments
fn generic_nullable_to_non_nullable() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b1:Box<int?>=Box<int?>{value:42} let b2:Box<int>=b1}"#, "expected Box<int>, found Box<int?>"); }

// Method call wrapping errors
#[test]
fn method_nullable_param() { compile_should_fail_with(r#"class C{fn foo(self,x:int){}} fn main(){let c=C{} let y:int?=42 c.foo(y)}"#, "expected int, found int?"); }

// Binary op with nullable
#[test]
fn binop_nullable_int() { compile_should_fail_with(r#"fn main(){let x:int?=42 let y=x+1}"#, "type mismatch"); }
#[test]
fn binop_none_literal() { compile_should_fail_with(r#"fn main(){let x=none+1}"#, "type mismatch"); }

// Map value wrapping
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn map_nullable_value_to_non_nullable() { compile_should_fail_with(r#"fn main(){let m:Map<string,int>=Map<string,int>{} m[\"a\"]=none}"#, "expected int, found void?"); }

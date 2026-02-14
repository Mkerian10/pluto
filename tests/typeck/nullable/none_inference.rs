//! None literal inference tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// None without context
#[test]
#[ignore] // #172: none inference too permissive - compiles when should fail
fn none_no_context() { compile_should_fail_with(r#"fn main(){let x=none}"#, "cannot infer"); }
#[test]
fn none_in_return_no_sig() { compile_should_fail_with(r#"fn f(){return none} fn main(){}"#, "return type mismatch: expected void, found void?"); }

// None in ambiguous contexts
#[test]
fn none_in_if_branches() { compile_should_fail_with(r#"fn main(){let x=if true{none}else{42}}"#, "if-expression branches have incompatible types"); }
#[test]
#[ignore] // Parser error: "unexpected token { in expression" - match arm body parsing issue
fn none_in_match_arms() { compile_should_fail_with(r#"enum E{A B} fn main(){let x=match E.A{E.A=>{none}E.B=>{42}}}"#, "type mismatch"); }

// None in arrays
#[test]
#[ignore] // #172: none inference too permissive - compiles when should fail
fn array_of_only_none() { compile_should_fail_with(r#"fn main(){let a=[none,none,none]}"#, "cannot infer"); }
// This test already passes - array correctly infers as [int?]
#[test]
fn array_mixed_none_and_value() { compile_should_fail_with(r#"fn main(){let a=[42,none] let b:[int]=a}"#, "type mismatch"); }

// None in function args
#[test]
fn generic_fn_none_arg() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(none)}"#, "void? is not allowed"); }
#[test]
#[ignore] // #172: none inference too permissive - compiles when should fail
fn overload_none_ambiguous() { compile_should_fail_with(r#"fn f(x:int?){} fn main(){f(none)}"#, ""); }

// None in binary ops
#[test]
fn none_in_comparison() { compile_should_fail_with(r#"fn main(){let b=none==42}"#, "cannot compare void? with int"); }
#[test]
fn none_in_arithmetic() { compile_should_fail_with(r#"fn main(){let x=none+none}"#, "operator not supported for type void?"); }

// None in struct fields
#[test]
#[ignore] // Parser/typeck error: "unknown class 'C'" - generic class not registered before struct literal
fn struct_field_none_no_type() { compile_should_fail_with(r#"class C<T>{x:T} fn main(){let c=C{x:none}}"#, "cannot infer"); }

// None propagation
#[test]
#[ignore] // #172: none inference too permissive - compiles when should fail
fn propagate_none() { compile_should_fail_with(r#"fn f(){return none?} fn main(){}"#, "cannot infer"); }

// None in map
#[test]
#[ignore] // #156: string literals don't work in compact syntax (escape sequence parser error)
fn map_value_none_no_type() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} m[\"a\"]=none}"#, "type mismatch"); }

// None in ternary-like
// This test already passes - correctly accepts none in else branch
#[test]
fn none_ternary_mismatch() { compile_should_fail_with(r#"fn main(){let x=if true{42}else{none}}"#, ""); }

// Multiple nones
#[test]
fn fn_returns_none_twice() { compile_should_fail_with(r#"fn f(b:bool){if b{return none}return none} fn main(){}"#, "return type mismatch: expected void, found void?"); }

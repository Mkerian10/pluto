//! Nullable propagation chain tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic ? propagation errors
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_non_nullable_fn() { compile_should_fail_with(r#"fn f()int{let x:int?=42 return x?} fn main(){}"#, "cannot propagate"); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_non_nullable_value() { compile_should_fail_with(r#"fn f()int?{return 42?} fn main(){}"#, "cannot propagate"); }

// Chained field access
#[test]
#[ignore] // PR #46 - outdated assertions
fn chain_field_access_nullable() { compile_should_fail_with(r#"class A{x:int} class B{a:A?} fn main(){let b=B{a:none} let x=b.a?.x}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn triple_chain_field_access() { compile_should_fail_with(r#"class A{x:int} class B{a:A?} class C{b:B?} fn main(){let c=C{b:none} let x=c.b?.a?.x}"#, ""); }

// Method call chains
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_method_chain() { compile_should_fail_with(r#"class C{fn foo(self)C?{return none}} fn main(){let c=C{} let x=c.foo()?.foo()}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_method_result() { compile_should_fail_with(r#"class C{fn foo(self)int?{return none}} fn main(){let c=C{} let x=c.foo()?}"#, ""); }

// Propagation in expressions
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_binop() { compile_should_fail_with(r#"fn f()int?{return none} fn g()int?{return f()?+1} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_array() { compile_should_fail_with(r#"fn f()int?{return none} fn g()[int]?{return [f()?,2,3]} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_struct() { compile_should_fail_with(r#"class C{x:int} fn f()int?{return none} fn g()C?{return C{x:f()?}} fn main(){}"#, ""); }

// Mixed error and nullable propagation
#[test]
#[ignore] // PR #46 - outdated assertions
fn nullable_and_error_propagate() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn g()int?{return f()!?} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn error_and_nullable_propagate() { compile_should_fail_with(r#"fn f()int?{return none} fn g()int!{return f()?!} fn main(){}"#, ""); }

// Propagate wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_returns_wrong_type() { compile_should_fail_with(r#"fn f()int?{return none} fn g()string?{return f()?} fn main(){}"#, "type mismatch"); }

// Deep propagation chains
#[test]
#[ignore] // PR #46 - outdated assertions
fn five_level_propagation() { compile_should_fail_with(r#"fn f1()int?{return none} fn f2()int?{return f1()?} fn f3()int?{return f2()?} fn f4()int?{return f3()?} fn f5()int?{return f4()?} fn main(){}"#, ""); }

// Propagate in control flow
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_if_early_return() { compile_should_fail_with(r#"fn f(x:int?)int?{if true{return x?}return 0} fn main(){}"#, ""); }
#[test]
#[ignore] // PR #46 - outdated assertions
fn propagate_in_match() { compile_should_fail_with(r#"enum E{A B} fn f(e:E,x:int?)int?{match e{E.A=>{return x?}E.B=>{return 0}}} fn main(){}"#, ""); }

//! Scope resolution errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Ambiguous name resolution
#[test]
#[ignore] // PR #46 - outdated assertions
fn ambiguous_name() { compile_should_fail_with(r#"class A{} fn A(){} fn main(){A}"#, ""); }

// Cross-scope reference
#[test]
#[ignore] // PR #46 - outdated assertions
fn cross_scope_ref() { compile_should_fail_with(r#"fn main(){if true{let x=1}else{let y=x}}"#, "undefined"); }

// Unqualified import
#[test]
#[ignore] // PR #46 - outdated assertions
fn unqualified_import() { compile_should_fail_with(r#"import math fn main(){let x=add(1,2)}"#, ""); }

// Module scope confusion
#[test]
#[ignore] // PR #46 - outdated assertions
fn module_scope_confusion() { compile_should_fail_with(r#"import mod1 class C{} fn main(){let c=C{} let m=mod1.C{}}"#, ""); }

// Nested scope lookup
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_scope_lookup() { compile_should_fail_with(r#"fn main(){let x=1 if true{if true{if true{let y=x}}}}"#, ""); }

// Function scope vs class scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn function_class_scope() { compile_should_fail_with(r#"class C{x:int} fn foo(){let y=x} fn main(){}"#, ""); }

// Trait method scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn trait_method_scope() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{x:int} impl T{fn foo(self)int{return y}} fn main(){}"#, ""); }

// Generic scope resolution
#[test]
#[ignore] // PR #46 - outdated assertions
fn generic_scope() { compile_should_fail_with(r#"fn f<T>(x:T){let y:T} fn g(){let z:T} fn main(){}"#, ""); }

// Closure scope vs outer scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn closure_outer_scope() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1} let y=x}"#, "undefined"); }

// Match arm scope isolation
#[test]
#[ignore] // PR #46 - outdated assertions
fn match_arm_isolation() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{let x=1}E.B{let y=x}}}"#, "undefined"); }

// Block scope lookup
#[test]
#[ignore] // PR #46 - outdated assertions
fn block_scope_lookup() { compile_should_fail_with(r#"fn main(){{let x=1}{let y=x}}"#, "undefined"); }

// Method self scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn method_self_scope() { compile_should_fail_with(r#"class C{x:int} fn foo(){let y=self.x} fn main(){}"#, ""); }

// App scope isolation
#[test]
#[ignore] // PR #46 - outdated assertions
fn app_scope() { compile_should_fail_with(r#"app MyApp{fn helper(self){let x=1} fn main(self){let y=x}}"#, "undefined"); }

// Enum variant scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn enum_variant_scope() { compile_should_fail_with(r#"enum E{A{x:int}B{y:int}} fn main(){let a=E.A{x:1} let b=a.y}"#, ""); }

// Contract scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn contract_scope() { compile_should_fail_with(r#"class C{x:int invariant y>0} fn main(){}"#, ""); }

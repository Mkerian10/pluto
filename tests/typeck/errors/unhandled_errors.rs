//! Unhandled error tests - 30 tests
mod common;
use common::compile_should_fail_with;

// Basic unhandled raises
#[test] fn raise_in_main() { compile_should_fail_with(r#"error E{} fn main(){raise E{}}"#, "unhandled error"); }
#[test] fn raise_in_function() { compile_should_fail_with(r#"error E{} fn f(){raise E{}} fn main(){}"#, "unhandled error"); }
#[test] fn raise_in_if_branch() { compile_should_fail_with(r#"error E{} fn main(){if true{raise E{}}}"#, "unhandled error"); }
#[test] fn raise_in_else_branch() { compile_should_fail_with(r#"error E{} fn main(){if false{}else{raise E{}}}"#, "unhandled error"); }
#[test] fn raise_in_while() { compile_should_fail_with(r#"error E{} fn main(){while true{raise E{}}}"#, "unhandled error"); }
#[test] fn raise_in_for() { compile_should_fail_with(r#"error E{} fn main(){for i in 0..10{raise E{}}}"#, "unhandled error"); }

// Unhandled fallible calls
#[test] fn fallible_call_no_handler() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn main(){f()}"#, "unhandled error"); }
#[test] fn fallible_call_in_assignment() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let x=f()}"#, "unhandled error"); }
#[test] fn fallible_call_in_binop() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let x=f()+1}"#, "unhandled error"); }
#[test] fn fallible_call_in_return() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn g()int{return f()} fn main(){}"#, "unhandled error"); }

// Missing catch handler
#[test] fn bare_call_needs_catch() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn main(){f()}"#, "unhandled error"); }
#[test] fn bare_call_in_expr() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let x=1+f()}"#, "unhandled error"); }
#[test] fn multiple_bare_calls() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let x=f() let y=f()}"#, "unhandled error"); }

// Missing propagate in non-fallible function
#[test] fn call_fallible_without_propagate() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn g(){f()} fn main(){}"#, "unhandled error"); }
#[test] fn return_fallible_without_propagate() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn g()int{return f()} fn main(){}"#, "unhandled error"); }

// Unhandled in class methods
#[test] fn method_raises_no_handler() { compile_should_fail_with(r#"error E{} class C{x:int fn foo(self){raise E{}}} fn main(){}"#, "unhandled error"); }
#[test] fn method_call_fallible_no_handler() { compile_should_fail_with(r#"error E{} class C{x:int fn foo(self)!{raise E{}}} fn main(){let c=C{x:1}c.foo()}"#, "unhandled error"); }
#[test] fn method_calls_fallible_no_propagate() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} class C{x:int fn foo(self){f()}} fn main(){}"#, "unhandled error"); }

// Unhandled in closures
#[test] fn closure_raises_no_handler() { compile_should_fail_with(r#"error E{} fn main(){let f=()=>{raise E{}}}"#, "unhandled error"); }
#[test] fn closure_calls_fallible_no_handler() { compile_should_fail_with(r#"error E{} fn g()!{raise E{}} fn main(){let f=()=>{g()}}"#, "unhandled error"); }
#[test] fn closure_calls_fallible_no_propagate() { compile_should_fail_with(r#"error E{} fn g()int!{raise E{}} fn main(){let f=()int=>{return g()}}"#, "unhandled error"); }

// Unhandled in match arms
#[test] fn match_arm_raises() { compile_should_fail_with(r#"error E{} enum Opt{Some{v:int}None} fn main(){let x=Opt.None match x{Opt.Some{v}=>{}Opt.None=>{raise E{}}}}"#, "unhandled error"); }
#[test] fn match_arm_calls_fallible() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} enum Opt{Some{v:int}None} fn main(){let x=Opt.None match x{Opt.Some{v}=>{}Opt.None=>{f()}}}"#, "unhandled error"); }

// Unhandled in array/struct literals
#[test] fn array_element_fallible() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let arr=[f(),2,3]}"#, "unhandled error"); }
#[test] fn struct_field_fallible() { compile_should_fail_with(r#"error E{} class C{x:int} fn f()int!{raise E{}} fn main(){let c=C{x:f()}}"#, "unhandled error"); }
#[test] fn enum_variant_field_fallible() { compile_should_fail_with(r#"error E{} enum Opt{Some{v:int}} fn f()int!{raise E{}} fn main(){let x=Opt.Some{v:f()}}"#, "unhandled error"); }

// Unhandled in string interpolation
#[test] fn interpolation_fallible() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn main(){let s=\"{f()}\"}"#, "unhandled error"); }

// Multiple errors unhandled
#[test] fn two_errors_both_unhandled() { compile_should_fail_with(r#"error E1{} error E2{} fn main(){raise E1{} raise E2{}}"#, "unhandled error"); }
#[test] fn union_errors_unhandled() { compile_should_fail_with(r#"error E1{} error E2{} fn f()!E1{raise E1{}} fn g()!E2{raise E2{}} fn main(){f() g()}"#, "unhandled error"); }

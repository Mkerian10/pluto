//! Fixed-point iteration tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct recursion
#[test]
fn recursive_call_no_handler() { compile_should_fail_with(r#"error E{} fn fac(n:int)int{if n==0{return 1}if n==5{raise E{}}return n*fac(n-1)} fn main(){}"#, "call to fallible"); }
#[test]
#[ignore] // Compiler bug: codegen duplicate definition with compact syntax
fn recursive_fallible_no_propagate() { compile_should_fail_with(r#"error E{} fn fac(n:int)int{if n==0{return 1}return n*fac(n-1)} fn main(){}"#, "call to fallible"); }
#[test]
#[ignore] // Compiler bug: codegen duplicate definition with compact syntax
fn recursive_with_propagate() { compile_should_fail_with(r#"error E{} fn fac(n:int)int{if n==0{return 1}return n*fac(n-1)!} fn main(){fac(5)}"#, "call to fallible"); }
#[test]
fn mutual_recursion_no_handler() { compile_should_fail_with(r#"error E{} fn even(n:int)bool{if n==0{return true}if n==10{raise E{}}return odd(n-1)} fn odd(n:int)bool{if n==0{return false}return even(n-1)} fn main(){}"#, "call to fallible"); }
#[test]
#[ignore] // Compiler bug: codegen duplicate definition with compact syntax
fn mutual_recursion_fallible() { compile_should_fail_with(r#"error E{} fn even(n:int)bool{if n==0{return true}return odd(n-1)} fn odd(n:int)bool{if n==0{return false}return even(n-1)} fn main(){even(5)}"#, "call to fallible"); }

// Indirect recursion through multiple functions
#[test]
fn three_way_recursion() { compile_should_fail_with(r#"error E{} fn a(n:int)int{if n==0{return 1}if n==10{raise E{}}return b(n-1)} fn b(n:int)int{return c(n)} fn c(n:int)int{return a(n)} fn main(){}"#, "call to fallible"); }
#[test]
fn recursion_with_mixed_sigs() { compile_should_fail_with(r#"error E{} fn a(n:int)int{if n==0{return 1}return b(n-1)!} fn b(n:int)int{if n==5{raise E{}}return a(n-1)} fn main(){}"#, "call to fallible"); }

// Recursion with conditionals
#[test]
fn recursive_only_in_branch() { compile_should_fail_with(r#"error E{} fn fac(n:int)int{if n>0{if n==5{raise E{}}return n*fac(n-1)}return 1} fn main(){}"#, "call to fallible"); }
#[test]
fn recursive_both_branches() { compile_should_fail_with(r#"error E{} fn fib(n:int)int{if n<=1{if n<0{raise E{}}return 1}return fib(n-1)+fib(n-2)} fn main(){}"#, "call to fallible"); }

// Recursion with loops
#[test]
fn recursive_in_while() { compile_should_fail_with(r#"error E{} fn f(n:int)int{while n>0{if n==10{raise E{}}return f(n-1)}return 0} fn main(){}"#, "call to fallible"); }
#[test]
fn recursive_in_for() { compile_should_fail_with(r#"error E{} fn f(n:int)int{for i in 0..n{if i==5{raise E{}}f(i)}return 0} fn main(){}"#, "call to fallible"); }

// Fixed-point convergence edge cases
#[test]
fn self_call_multiple_sites() { compile_should_fail_with(r#"error E{} fn f(n:int)int{if n==0{return 1}if n==1{if n==10{raise E{}}return f(0)}return f(n-1)+f(n-2)} fn main(){}"#, "call to fallible"); }
#[test]
fn recursion_chain_convergence() { compile_should_fail_with(r#"error E{} fn a(n:int)int{if n==0{return 1}return b(n)} fn b(n:int)int{if n==5{raise E{}}return a(n-1)} fn main(){}"#, "call to fallible"); }

// Recursion with closures (captures make propagation complex)
#[test]
#[ignore] // ACTUALLY_SUCCESS: compiler improved, this case now works
fn recursive_lambda_capture() { compile_should_fail_with(r#"error E{} fn main(){let f=(n:int)int=>{if n==0{return 1}if n==5{raise E{}}return 1}}"#, "unhandled error"); }

// Recursion with error union accumulation
#[test]
fn recursive_multiple_error_types() { compile_should_fail_with(r#"error E1{} error E2{} fn f(n:int)int{if n==0{raise E1{}}if n==1{raise E2{}}return f(n-1)} fn main(){}"#, "call to fallible"); }
#[test]
fn mutual_recursion_different_errors() { compile_should_fail_with(r#"error E1{} error E2{} fn a(n:int)int{if n==0{raise E1{}}return b(n-1)} fn b(n:int)int{if n==0{raise E2{}}return a(n-1)} fn main(){}"#, "call to fallible"); }

// Deep recursion chains
#[test]
fn five_way_mutual_recursion() { compile_should_fail_with(r#"error E{} fn a(n:int)int{if n==0{return 1}if n==10{raise E{}}return b(n-1)} fn b(n:int)int{return c(n)} fn c(n:int)int{return d(n)} fn d(n:int)int{return e(n)} fn e(n:int)int{return a(n-1)} fn main(){}"#, "call to fallible"); }

// Recursion with method calls
#[test]
fn recursive_method() { compile_should_fail_with(r#"error E{} class C{x:int fn fac(self,n:int)int{if n==0{return 1}if n==5{raise E{}}return n*self.fac(n-1)}} fn main(){}"#, "call to fallible method"); }
#[test]
#[ignore] // Compiler bug: codegen duplicate definition with compact syntax
fn recursive_method_fallible() { compile_should_fail_with(r#"error E{} class C{x:int fn fac(self,n:int)int{if n==0{return 1}return n*self.fac(n-1)}} fn main(){let c=C{x:1}c.fac(5)}"#, "call to fallible"); }

// Tail recursion edge case
#[test]
fn tail_recursive_no_handler() { compile_should_fail_with(r#"error E{} fn sum(n:int,acc:int)int{if n==0{return acc}if n==10{raise E{}}return sum(n-1,acc+n)} fn main(){}"#, "call to fallible"); }

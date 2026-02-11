//! Closures in various expression contexts - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Closure in binary expression
#[test] fn closure_in_binop() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let y=f+2}"#, ""); }

// Closure in comparison
#[test] fn closure_in_comparison() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x let g=(y:int)=>y if f==g{}}"#, ""); }

// Closure in array literal
#[test] fn closure_in_array() { compile_should_fail_with(r#"fn main(){let arr=[(x:int)=>x+1,(y:int)=>y*2]}"#, ""); }

// Closure in struct literal
#[test] fn closure_in_struct() { compile_should_fail_with(r#"class C{f:(int)int} fn main(){let c=C{f:(x:int)=>x+1}}"#, ""); }

// Closure as return value
#[test] fn closure_return() { compile_should_fail_with(r#"fn f()(int)int{return (x:int)=>x+1} fn main(){}"#, ""); }

// Closure in if condition (invalid)
#[test] fn closure_in_if_cond() { compile_should_fail_with(r#"fn main(){if (x:int)=>true{}}"#, "type mismatch"); }

// Closure in while condition (invalid)
#[test] fn closure_in_while_cond() { compile_should_fail_with(r#"fn main(){while (x:int)=>true{}}"#, "type mismatch"); }

// Closure in match scrutinee
#[test] fn closure_in_match() { compile_should_fail_with(r#"enum E{A B} fn main(){match (x:int)=>E.A{E.A{}E.B{}}}"#, "type mismatch"); }

// Closure immediately invoked
#[test] fn iife() { compile_should_fail_with(r#"fn main(){let x=((y:int)=>y+1)(2)}"#, ""); }

// Closure in map literal
#[test] fn closure_in_map() { compile_should_fail_with(r#"fn main(){let m=Map<string,(int)int>{\"add\":(x:int)=>x+1}}"#, ""); }

// Closure in set literal (closures not hashable)
#[test] fn closure_in_set() { compile_should_fail_with(r#"fn main(){let s=Set<(int)int>{(x:int)=>x}}"#, ""); }

// Closure as function argument
#[test] fn closure_as_arg() { compile_should_fail_with(r#"fn f(g:(int)int)int{return g(1)} fn main(){f((x:int)=>x+1)}"#, ""); }

// Closure in spawn (invalid, spawn takes direct calls)
#[test] fn closure_in_spawn() { compile_should_fail_with(r#"fn main(){spawn ((x:int)=>x+1)(2)}"#, ""); }

// Closure in assignment
#[test] fn closure_assign() { compile_should_fail_with(r#"fn main(){let f:(int)int f=(x:int)=>x+1}"#, ""); }

// Closure in nullable type
#[test] fn closure_nullable() { compile_should_fail_with(r#"fn main(){let f:((int)int)?=none}"#, ""); }

//! String interpolation type errors - 8 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_class() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} let s=\"{c}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_array() { compile_should_fail_with(r#"fn main(){let a=[1,2,3] let s=\"{a}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let s=\"{f}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_map() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} let s=\"{m}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_enum() { compile_should_fail_with(r#"enum E{A} fn main(){let e=E.A let s=\"{e}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_task() { compile_should_fail_with(r#"fn work()int{return 1} fn main(){let t=spawn work() let s=\"{t}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=none let s=\"{x}\"}"#, "cannot interpolate"); }
#[test]
#[ignore] // #156: string literals don't work in compact syntax
fn interp_trait_object() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1} let s=\"{t}\"}"#, "cannot interpolate"); }

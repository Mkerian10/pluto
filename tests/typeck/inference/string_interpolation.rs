//! String interpolation type errors - 8 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn interp_class() { compile_should_fail_with(r#"class C{x:int}
fn main(){let c=C{x:1}
let s=f"{c}"}"#, "cannot interpolate"); }
#[test]
fn interp_array() { compile_should_fail_with(r#"fn main(){let a=[1,2,3]
let s=f"{a}"}"#, "cannot interpolate"); }
#[test]
fn interp_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1
let s=f"{f}"}"#, "cannot interpolate"); }
#[test]
fn interp_map() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{}
let s=f"{m}"}"#, "cannot interpolate"); }
#[test]
fn interp_enum() { compile_should_fail_with(r#"enum E{A}
fn main(){let e=E.A
let s=f"{e}"}"#, "cannot interpolate"); }
#[test]
fn interp_task() { compile_should_fail_with(r#"fn work()int{return 1}
fn main(){let t=spawn work()
let s=f"{t}"}"#, "cannot interpolate"); }
#[test]
fn interp_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=none
let s=f"{x}"}"#, "cannot interpolate"); }
#[test]
fn interp_trait_object() { compile_should_fail_with(r#"trait T{}
class C impl T{x:int}
fn main(){let t:T=C{x:1}
let s=f"{t}"}"#, "cannot interpolate"); }

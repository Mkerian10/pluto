//! Closure type inference tests - 14 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn closure_return_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)int=>"hi"}"#, "expected int, found string"); }
#[test]
fn closure_param_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x f("hi")}"#, "expected int, found string"); }
#[test]
fn closure_capture_wrong_type() { compile_should_fail_with(r#"fn main(){let s="hi" let f=(x:int)=>s+x}"#, "operand type mismatch"); }
#[test] #[ignore] // #165: parser limitation - params without types not supported
fn closure_no_param_type() { compile_should_fail_with(r#"fn main(){let f=(x)=>x+1}"#, "cannot infer"); }
#[test]
fn closure_no_return_type_ambiguous() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{if x>0{return 1}return "no"}}"#, "return type mismatch"); }
#[test]
fn closure_capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>undefined}"#, "undefined"); }
#[test]
fn closure_wrong_arg_count() { compile_should_fail_with(r#"fn main(){let f=(x:int,y:int)=>x+y f(1)}"#, "expects 2 arguments, got 1"); }
#[test] #[ignore] // #165: parser limitation - explicit void return type syntax
fn closure_return_void_with_value() { compile_should_fail_with(r#"fn main(){let f=()void=>{return 42}}"#, "expected void, found int"); }
#[test]
fn closure_in_binop() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let g=f+1}"#, "operand type mismatch"); }
#[test] #[ignore] // #165: parser limitation - function types in generic params
fn closure_generic_param_unresolved() { compile_should_fail_with(r#"fn apply<T>(f:fn(T)T,x:T)T{return f(x)} fn main(){apply((x)=>x+1,42)}"#, "cannot infer"); }
#[test]
fn closure_field_assign() { compile_should_fail_with(r#"class C{x:int} fn main(){let f=(c:C)=>{c.x="hi"}}"#, "expected int, found string"); }
#[test] #[ignore] // #165: parser limitation - fallible return types (int!) not supported in syntax
fn closure_propagate_invalid() { compile_should_fail_with(r#"fn safe()int{return 1} fn main(){let f=()int!=>{return safe()!}}"#, "cannot propagate"); }
#[test]
fn nested_closure_scope() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1 let g=()=>y}}"#, "undefined"); }
#[test]
fn closure_mut_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

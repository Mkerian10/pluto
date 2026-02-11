//! Closure type inference tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test] fn closure_return_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)int=>\"hi\"}"#, "type mismatch"); }
#[test] fn closure_param_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x f(\"hi\")}"#, "type mismatch"); }
#[test] fn closure_capture_wrong_type() { compile_should_fail_with(r#"fn main(){let s=\"hi\" let f=(x:int)=>s+x}"#, "type mismatch"); }
#[test] fn closure_no_param_type() { compile_should_fail_with(r#"fn main(){let f=(x)=>x+1}"#, "cannot infer"); }
#[test] fn closure_no_return_type_ambiguous() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{if x>0{return 1}return \"no\"}}"#, "type mismatch"); }
#[test] fn closure_capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>undefined}"#, "undefined"); }
#[test] fn closure_wrong_arg_count() { compile_should_fail_with(r#"fn main(){let f=(x:int,y:int)=>x+y f(1)}"#, "argument count"); }
#[test] fn closure_return_void_with_value() { compile_should_fail_with(r#"fn main(){let f=()void=>{return 42}}"#, "type mismatch"); }
#[test] fn closure_in_binop() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 let g=f+1}"#, "type mismatch"); }
#[test] fn closure_generic_param_unresolved() { compile_should_fail_with(r#"fn apply<T>(f:fn(T)T,x:T)T{return f(x)} fn main(){apply((x)=>x+1,42)}"#, "cannot infer"); }
#[test] fn closure_field_assign() { compile_should_fail_with(r#"class C{x:int} fn main(){let f=(c:C)=>{c.x=\"hi\"}}}"#, "type mismatch"); }
#[test] fn closure_raises_not_handled() { compile_should_fail_with(r#"error E{} fn main(){let f=()=>{raise E{}}}"#, "unhandled error"); }
#[test] fn closure_propagate_invalid() { compile_should_fail_with(r#"fn safe()int{return 1} fn main(){let f=()int!=>{return safe()!}}"#, "cannot propagate"); }
#[test] fn nested_closure_scope() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1 let g=()=>y}}"#, "undefined"); }
#[test] fn closure_mut_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

//! Spawn validation tests - 12 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test] fn spawn_closure() { compile_should_fail_with(r#"fn main(){let f=()=>42 spawn f()}"#, "cannot spawn"); }
#[test] fn spawn_method() { compile_should_fail_with(r#"class C{x:int fn foo(self)int{return 1}} fn main(){let c=C{x:1} spawn c.foo()}"#, "cannot spawn"); }
#[test] fn spawn_with_fallible_arg() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn g(x:int)int{return x} fn main(){spawn g(f())}"#, ""); }
#[test] fn spawn_with_propagate_arg() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn g(x:int)int{return x} fn h()!{spawn g(f()!)} fn main(){}"#, ""); }
#[test] fn spawn_builtin() { compile_should_fail_with(r#"fn main(){spawn print(\"hi\")}"#, "cannot spawn"); }
#[test] fn spawn_lambda() { compile_should_fail_with(r#"fn main(){spawn ((x:int)=>x+1)(42)}"#, "cannot spawn"); }
#[test] fn spawn_non_function() { compile_should_fail_with(r#"fn main(){let x=42 spawn x()}"#, "not a function"); }
#[test] fn spawn_undefined() { compile_should_fail_with(r#"fn main(){spawn unknown()}"#, "undefined"); }
#[test] fn spawn_wrong_arg_count() { compile_should_fail_with(r#"fn f(x:int)int{return x} fn main(){spawn f()}"#, "argument count"); }
#[test] fn spawn_wrong_arg_type() { compile_should_fail_with(r#"fn f(x:int)int{return x} fn main(){spawn f(\"hi\")}"#, "type mismatch"); }
#[test] fn spawn_generic_unresolved() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){spawn id()}"#, "cannot infer"); }
#[test] fn double_spawn() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){spawn spawn f()}"#, "cannot spawn"); }

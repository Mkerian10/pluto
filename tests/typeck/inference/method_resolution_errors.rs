//! Method resolution error tests - 20 tests
mod common;
use common::compile_should_fail_with;

#[test] fn method_on_int() { compile_should_fail_with(r#"fn main(){let x=42 x.foo()}"#, "no method"); }
#[test] fn method_on_bool() { compile_should_fail_with(r#"fn main(){let x=true x.foo()}"#, "no method"); }
#[test] fn unknown_method_on_class() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} c.bar()}"#, "no method"); }
#[test] fn unknown_method_on_array() { compile_should_fail_with(r#"fn main(){let x=[1,2,3] x.foo()}"#, "no method"); }
#[test] fn unknown_method_on_string() { compile_should_fail_with(r#"fn main(){let s="hi" s.foo()}"#, "no method"); }
#[test] fn unknown_method_on_map() { compile_should_fail_with(r#"fn main(){let m=Map<string,int>{} m.foo()}"#, "no method"); }
#[test] fn method_wrong_arg_count() { compile_should_fail_with(r#"class C{x:int fn foo(self,y:int)int{return y}} fn main(){let c=C{x:1} c.foo()}"#, "argument count"); }
#[test] fn method_wrong_arg_type() { compile_should_fail_with(r#"class C{x:int fn foo(self,y:int)int{return y}} fn main(){let c=C{x:1} c.foo(\"hi\")}"#, "type mismatch"); }
#[test] fn static_method_on_instance() { compile_should_fail_with(r#"class C{x:int} fn C_foo()int{return 42} fn main(){let c=C{x:1} c.foo()}"#, "no method"); }
#[test] fn method_on_nullable() { compile_should_fail_with(r#"class C{x:int fn foo(self)int{return 1}} fn main(){let c:C?=none c.foo()}"#, "no method"); }
#[test] fn method_on_generic_param() { compile_should_fail_with(r#"fn call<T>(x:T){x.foo()} fn main(){call(42)}"#, "no method"); }
#[test] fn trait_method_on_non_impl() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} fn main(){let c=C{x:1} c.foo()}"#, "no method"); }
#[test] fn method_on_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 f.foo()}"#, "no method"); }
#[test] fn method_on_enum() { compile_should_fail_with(r#"enum E{A} fn main(){let e=E.A e.foo()}"#, "no method"); }
#[test] fn mut_method_on_immut_var() { compile_should_fail_with(r#"class C{x:int fn foo(mut self){self.x=2}} fn main(){let c=C{x:1} c.foo()}"#, ""); }
#[test] fn method_on_task() { compile_should_fail_with(r#"fn work()int{return 1} fn main(){let t=spawn work() t.foo()}"#, "no method"); }
#[test] fn method_chain_first_fails() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} c.foo().bar()}"#, "no method"); }
#[test] fn method_chain_second_fails() { compile_should_fail_with(r#"class C{x:int fn get(self)int{return self.x}} fn main(){let c=C{x:1} c.get().bar()}"#, "no method"); }
#[test] fn method_returns_wrong_type() { compile_should_fail_with(r#"class C{x:int fn foo(self)string{return self.x}} fn main(){let c=C{x:1}}"#, "type mismatch"); }
#[test] fn method_on_bytes() { compile_should_fail_with(r#"fn main(){let b=b\"hi\" b.foo()}"#, "no method"); }

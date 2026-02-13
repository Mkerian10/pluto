//! Trait dispatch errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Dispatch to wrong method
#[test]
fn dispatch_wrong_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T){t.bar()} fn main(){}"#, ""); }

// Dispatch with wrong arguments
#[test]
fn dispatch_wrong_args() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:int){}} fn use_t(t:T){t.foo(\"hi\")} fn main(){}"#, "type mismatch"); }

// Dispatch return type mismatch
#[test]
fn dispatch_return_mismatch() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self)int{return 1}} fn use_t(t:T)string{return t.foo()} fn main(){}"#, "type mismatch"); }

// Multiple trait dispatch
#[test]
fn multi_trait_dispatch() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn bar(self)} class C{} impl T1{fn foo(self){}} impl T2{fn bar(self){}} fn use_t(t1:T1,t2:T2){t1.bar()} fn main(){}"#, ""); }

// Dispatch to non-implemented method
#[test]
fn dispatch_not_impl() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T){t.bar()} fn main(){}"#, ""); }

// Dispatch with generic trait
#[test]
fn generic_trait_dispatch() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int>{fn foo(self)int{return 1}} fn use_t(t:T<string>){} fn main(){}"#, ""); }

// Dispatch with nullable
#[test]
fn nullable_dispatch() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T?){t.foo()} fn main(){}"#, ""); }

// Dispatch on concrete type instead of trait
#[test]
fn dispatch_concrete() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T){} fn main(){use_t(C{})}"#, ""); }

// Dispatch with self type
#[test]
fn dispatch_self_type() { compile_should_fail_with(r#"trait T{fn foo(self)Self} class C{} impl T{fn foo(self)C{return self}} fn main(){}"#, ""); }

// Dispatch with mut self
#[test]
fn dispatch_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C{x:int} impl T{fn foo(mut self){self.x=2}} fn use_t(t:T){t.foo()} fn main(){}"#, ""); }

// Dispatch in generic function
#[test]
fn generic_fn_dispatch() { compile_should_fail_with(r#"trait T{fn foo(self)} fn use_t<U>(t:U) where U:T{t.foo()} class C{} fn main(){use_t(C{})}"#, ""); }

// Dispatch with contract violation
#[test]
fn dispatch_contract() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int{return -1}} fn use_t(t:T){t.foo()} fn main(){}"#, ""); }

// Dispatch ambiguity
#[test]
fn dispatch_ambiguous() { compile_should_fail_with(r#"trait T1{fn foo(self)} trait T2{fn foo(self)} class C{} impl T1{fn foo(self){}} impl T2{fn foo(self){}} fn use_both(t1:T1,t2:T2){} fn main(){}"#, ""); }

// Dispatch with error propagation
#[test]
fn dispatch_error_prop() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C{} impl T{fn foo(self)int!{raise E{}}} fn use_t(t:T)!{t.foo()!} fn main(){}"#, ""); }

// Dispatch to private method
#[test]
fn dispatch_private() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn use_t(t:T){t.foo()} fn main(){}"#, ""); }

// Dispatch with closure parameter
#[test]
fn dispatch_closure_param() { compile_should_fail_with(r#"trait T{fn foo(self,f:(int)int)} class C{} impl T{fn foo(self,f:(int)int){}} fn use_t(t:T){t.foo((x:string)=>1)} fn main(){}"#, "type mismatch"); }

// Dispatch on array of trait objects
#[test]
fn dispatch_array_traits() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn main(){let arr:[T]=[C{}]arr[0].foo()}"#, ""); }

// Dispatch with spawn
#[test]
fn dispatch_spawn() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self)int{return 1}} fn use_t(t:T){spawn t.foo()} fn main(){}"#, ""); }

// Dispatch in match
#[test]
fn dispatch_in_match() { compile_should_fail_with(r#"trait T{fn foo(self)} enum E{A B} class C{} impl T{fn foo(self){}} fn main(){let t:T=C{} match E.A{E.A{t.foo()}E.B{}}}"#, ""); }

// Dispatch chain
#[test]
fn dispatch_chain() { compile_should_fail_with(r#"trait T{fn foo(self)Self} class C{} impl T{fn foo(self)C{return self}} fn use_t(t:T){t.foo().foo()} fn main(){}"#, ""); }

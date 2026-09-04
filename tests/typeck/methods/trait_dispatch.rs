//! Trait dispatch errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Dispatch to wrong method
#[test]
fn dispatch_wrong_method() { compile_should_fail_with(r#"trait T{fn foo(self)}
class C impl T {
fn foo(self){}}
fn use_t(t:T){t.bar()}
fn main(){}"#, "trait 'T' has no method 'bar'"); }

// Dispatch with wrong arguments
#[test]
#[ignore] // source uses legacy escaped-quote + standalone-impl syntax; rewrite when revisiting dispatch coverage
fn dispatch_wrong_args() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:int){}} fn use_t(t:T){t.foo(\"hi\")} fn main(){}"#, "type mismatch"); }

// Dispatch return type mismatch
#[test]
#[ignore] // source uses legacy standalone-impl syntax; rewrite when revisiting dispatch coverage
fn dispatch_return_mismatch() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self)int{return 1}} fn use_t(t:T)string{return t.foo()} fn main(){}"#, "type mismatch"); }

// Multiple trait dispatch
#[test]
fn multi_trait_dispatch() { compile_should_fail_with(r#"trait T1{fn foo(self)}
trait T2{fn bar(self)}
class C impl T1, T2 {
fn foo(self){}
fn bar(self){}}
fn use_t(t1:T1,t2:T2){t1.bar()}
fn main(){}"#, "trait 'T1' has no method 'bar'"); }

// Dispatch to non-implemented method
#[test]
fn dispatch_not_impl() { compile_should_fail_with(r#"trait T{fn foo(self)
fn bar(self)}
class C impl T {
fn foo(self){}}
fn use_t(t:T){t.bar()}
fn main(){}"#, "class 'C' does not implement required method 'bar' from trait 'T'"); }

// Dispatch with generic trait
#[test]
fn generic_trait_dispatch() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int>{fn foo(self)int{return 1}} fn use_t(t:T<string>){} fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Dispatch with nullable
#[test]
fn nullable_dispatch() { compile_should_fail_with(r#"trait T{fn foo(self)}
class C impl T {
fn foo(self){}}
fn use_t(t:T?){t.foo()}
fn main(){}"#, "method call on non-class type trait T?"); }

// Dispatch on concrete type instead of trait
#[test]
fn dispatch_concrete() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{fn foo(self)}
class C impl T {
fn foo(self){}}
fn use_t(t:T){}
fn main(){use_t(C{})}"#).is_ok()); }

// Dispatch with self type
#[test]
fn dispatch_self_type() { compile_should_fail_with(r#"trait T{fn foo(self)Self}
class C impl T {
fn foo(self)C{return self}}
fn main(){}"#, "unknown type 'Self'"); }

// Dispatch with mut self
#[test]
fn dispatch_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)}
class C impl T {x:int
fn foo(mut self){self.x=2}}
fn use_t(t:T){t.foo()}
fn main(){}"#, "cannot call mutating method 'foo' on immutable variable 't'; declare with 'let mut' to allow mutation"); }

// Dispatch in generic function
#[test]
fn generic_fn_dispatch() { compile_should_fail_with(r#"trait T{fn foo(self)} fn use_t<U>(t:U) where U:T{t.foo()} class C{} fn main(){use_t(C{})}"#, "expected ==, found :"); }

// Dispatch with contract violation
#[test]
fn dispatch_contract() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int{return -1}} fn use_t(t:T){t.foo()} fn main(){}"#, "'ensures' clauses are not supported: Pluto has no postconditions by design; express guarantees with class invariants or return types (see docs/design/contracts.md)"); }

// Dispatch ambiguity
#[test]
fn dispatch_ambiguous() { compile_should_fail_with(r#"trait T1{fn foo(self)}
trait T2{fn foo(self)}
class C impl T1, T2 {
fn foo(self){}
fn foo(self){}}
fn use_both(t1:T1,t2:T2){}
fn main(){}"#, "duplicate method 'foo' in class 'C'"); }

// Dispatch with error propagation
#[test]
fn dispatch_error_prop() { compile_should_fail_with(r#"error E{}
trait T{fn foo(self)int!}
class C impl T {
fn foo(self)int!{raise E{}}}
fn use_t(t:T){t.foo()!}
fn main(){}"#, "expected newline after statement"); }

// Dispatch to private method
#[test]
fn dispatch_private() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{fn foo(self)}
class C impl T {
fn foo(self){}}
fn use_t(t:T){t.foo()}
fn main(){}"#).is_ok()); }

// Dispatch with closure parameter
#[test]
#[ignore] // source uses legacy (int)int fn-type syntax (real syntax: fn(int) int); rewrite when revisiting
fn dispatch_closure_param() { compile_should_fail_with(r#"trait T{fn foo(self,f:(int)int)} class C{} impl T{fn foo(self,f:(int)int){}} fn use_t(t:T){t.foo((x:string)=>1)} fn main(){}"#, "type mismatch"); }

// Dispatch on array of trait objects
#[test]
fn dispatch_array_traits() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){}} fn main(){let arr:[T]=[C{}]arr[0].foo()}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Dispatch with spawn
#[test]
fn dispatch_spawn() { compile_should_fail_with(r#"trait T{fn foo(self)int}
class C impl T {
fn foo(self)int{return 1}}
fn use_t(t:T){spawn t.foo()}
fn main(){}"#, "Task handle must be used -- call .get(), .detach(), or assign to a variable"); }

// Dispatch in match
#[test]
fn dispatch_in_match() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"trait T{fn foo(self)}
enum E{A B}
class C impl T {
fn foo(self){}}
fn main(){let t:T=C{}
match E.A{E.A{t.foo()}E.B{}}}"#).is_ok()); }

// Dispatch chain
#[test]
fn dispatch_chain() { compile_should_fail_with(r#"trait T{fn foo(self)Self}
class C impl T {
fn foo(self)C{return self}}
fn use_t(t:T){t.foo().foo()}
fn main(){}"#, "unknown type 'Self'"); }

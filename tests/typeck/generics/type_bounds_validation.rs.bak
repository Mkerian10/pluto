//! Type bounds validation tests - 30 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic bounds violations
#[test] fn fn_bound_not_satisfied() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f<U:T>(x:U){} fn main(){f(C{x:1})}"#, "does not satisfy"); }
#[test] fn class_bound_not_satisfied() { compile_should_fail_with(r#"trait T{} class Box<U:T>{value:U} class C{x:int} fn main(){let b=Box<C>{value:C{x:1}}}"#, "does not satisfy"); }
#[test] fn enum_bound_not_satisfied() { compile_should_fail_with(r#"trait T{} enum Opt<U:T>{Some{v:U}None} class C{x:int} fn main(){let x=Opt<C>.Some{v:C{x:1}}}"#, "does not satisfy"); }

// Multiple bounds
#[test] fn two_bounds_first_fails() { compile_should_fail_with(r#"trait T1{} trait T2{} class C{x:int} impl T2 fn f<U:T1+T2>(x:U){} fn main(){f(C{x:1})}"#, "does not satisfy"); }
#[test] fn two_bounds_second_fails() { compile_should_fail_with(r#"trait T1{} trait T2{} class C{x:int} impl T1 fn f<U:T1+T2>(x:U){} fn main(){f(C{x:1})}"#, "does not satisfy"); }
#[test] fn two_bounds_both_fail() { compile_should_fail_with(r#"trait T1{} trait T2{} class C{x:int} fn f<U:T1+T2>(x:U){} fn main(){f(C{x:1})}"#, "does not satisfy"); }

// Bounds on nested generics
#[test] fn nested_generic_bound_fails() { compile_should_fail_with(r#"trait T{} class Box<U>{value:U} fn f<U:T>(b:Box<U>){} class C{x:int} fn main(){f(Box<C>{value:C{x:1}})}"#, "does not satisfy"); }
#[test] fn generic_in_generic_bound_fails() { compile_should_fail_with(r#"trait T{} class Box<U:T>{value:U} class Wrapper<V>{inner:Box<V>} class C{x:int} fn main(){let w=Wrapper<C>{inner:Box<C>{value:C{x:1}}}}"#, "does not satisfy"); }

// Bounds with primitive types
#[test] fn int_fails_trait_bound() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} fn main(){f(42)}"#, "does not satisfy"); }
#[test] fn string_fails_trait_bound() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} fn main(){f(\"hi\")}"#, "does not satisfy"); }
#[test] fn array_fails_trait_bound() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} fn main(){f([1,2,3])}"#, "does not satisfy"); }

// Bounds with return types
#[test] fn return_type_bound_fails() { compile_should_fail_with(r#"trait T{} class C{x:int} fn make<U:T>()U{return C{x:1}} fn main(){}"#, "type mismatch"); }
#[test] fn generic_return_bound_fails() { compile_should_fail_with(r#"trait T{} fn id<U:T>(x:U)U{return x} class C{x:int} fn main(){id(C{x:1})}"#, "does not satisfy"); }

// Bounds on method calls
#[test] fn method_receiver_bound_fails() { compile_should_fail_with(r#"trait T{} class Box<U:T>{value:U fn get(self)U{return self.value}} class C{x:int} fn main(){let b=Box<C>{value:C{x:1}}}"#, "does not satisfy"); }
#[test] fn method_param_bound_fails() { compile_should_fail_with(r#"trait T{} class Box<U>{value:U fn set<V:T>(mut self,v:V){}} class C{x:int} fn main(){let b=Box<int>{value:1}b.set(C{x:1})}"#, "does not satisfy"); }

// Bounds with closures
#[test] fn closure_param_bound_fails() { compile_should_fail_with(r#"trait T{} fn apply<U:T>(f:fn(U)U,x:U)U{return f(x)} class C{x:int} fn main(){apply((c:C)=>c,C{x:1})}"#, "does not satisfy"); }
#[test] fn closure_return_bound_fails() { compile_should_fail_with(r#"trait T{} fn make<U:T>(f:fn()U)U{return f()} class C{x:int} fn main(){make(()=>C{x:1})}"#, "does not satisfy"); }

// Bounds with explicit type args
#[test] fn explicit_type_arg_bound_fails() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U){} class C{x:int} fn main(){f<C>(C{x:1})}"#, "does not satisfy"); }
#[test] fn explicit_multi_bound_fails() { compile_should_fail_with(r#"trait T1{} trait T2{} fn f<U:T1+T2>(x:U){} class C{x:int} fn main(){f<C>(C{x:1})}"#, "does not satisfy"); }

// Bounds on enum variants
#[test] fn enum_variant_bound_fails() { compile_should_fail_with(r#"trait T{} enum Result<U:T,V>{Ok{val:U}Err{err:V}} class C{x:int} fn main(){let r=Result<C,int>.Ok{val:C{x:1}}}"#, "does not satisfy"); }

// Indirect bound failures
#[test] fn bound_through_typedef() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f<U:T>(x:U){} fn g<V>(x:V){f(x)} fn main(){g(C{x:1})}"#, "does not satisfy"); }
#[test] fn bound_chain() { compile_should_fail_with(r#"trait T{} fn a<U:T>(x:U){} fn b<V:T>(x:V){a(x)} class C{x:int} fn main(){b(C{x:1})}"#, "does not satisfy"); }

// Multiple type params with bounds
#[test] fn two_params_first_fails() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f<U:T,V>(x:U,y:V){} fn main(){f(C{x:1},42)}"#, "does not satisfy"); }
#[test] fn two_params_second_fails() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f<U,V:T>(x:U,y:V){} fn main(){f(42,C{x:1})}"#, "does not satisfy"); }
#[test] fn two_params_both_fail() { compile_should_fail_with(r#"trait T{} class C{x:int} fn f<U:T,V:T>(x:U,y:V){} fn main(){f(C{x:1},C{x:2})}"#, "does not satisfy"); }

// Bounds with nullable types
#[test] fn nullable_bound_fails() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U?){} class C{x:int} fn main(){f(C{x:1})}"#, "does not satisfy"); }

// Bounds with error types
#[test] fn error_bound_fails() { compile_should_fail_with(r#"trait T{} error E{} fn f<U:T>(x:U)!{} class C{x:int} fn main(){f(C{x:1})}"#, "does not satisfy"); }

// Trait not defined
#[test] fn bound_trait_undefined() { compile_should_fail_with(r#"fn f<U:UndefinedTrait>(x:U){} fn main(){}"#, "undefined"); }

// Circular bounds
#[test] fn self_referential_bound() { compile_should_fail_with(r#"trait T{} fn f<U:T>(x:U)U where U:T{return x} fn main(){}"#, ""); }

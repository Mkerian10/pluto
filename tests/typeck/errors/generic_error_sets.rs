//! Generic error sets tests - 20 tests
mod common;
use common::compile_should_fail_with;

// Generic functions with errors
#[test] fn generic_fn_raises_no_handler() { compile_should_fail_with(r#"error E{} fn id<T>(x:T)T{if true{raise E{}}return x} fn main(){id(42)}"#, "unhandled error"); }
#[test] fn generic_fn_fallible_no_propagate() { compile_should_fail_with(r#"error E{} fn id<T>(x:T)T!{if true{raise E{}}return x} fn main(){id(42)}"#, "unhandled error"); }
#[test] fn generic_different_instantiations() { compile_should_fail_with(r#"error E{} fn id<T>(x:T)T!{raise E{}} fn main(){id(42) id(\"hi\")}"#, "unhandled error"); }

// Generic classes with error-raising methods
#[test] fn generic_class_method_raises() { compile_should_fail_with(r#"error E{} class Box<T>{value:T fn get(self)T{if true{raise E{}}return self.value}} fn main(){let b=Box<int>{value:42}b.get()}"#, "unhandled error"); }
#[test] fn generic_class_method_fallible() { compile_should_fail_with(r#"error E{} class Box<T>{value:T fn get(self)T!{raise E{}}} fn main(){let b=Box<int>{value:42}b.get()}"#, "unhandled error"); }
#[test] fn generic_class_multiple_instantiations() { compile_should_fail_with(r#"error E{} class Box<T>{value:T fn get(self)T!{raise E{}}} fn main(){let b1=Box<int>{value:42}b1.get() let b2=Box<string>{value:\"hi\"}b2.get()}"#, "unhandled error"); }

// Generic enums with errors
#[test] fn generic_enum_match_raises() { compile_should_fail_with(r#"error E{} enum Opt<T>{Some{v:T}None} fn unwrap<T>(x:Opt<T>)T{match x{Opt.Some{v}=>{return v}Opt.None=>{raise E{}}}} fn main(){unwrap(Opt<int>.None)}"#, "unhandled error"); }
#[test] fn generic_enum_fallible_unwrap() { compile_should_fail_with(r#"error E{} enum Opt<T>{Some{v:T}None} fn unwrap<T>(x:Opt<T>)T!{match x{Opt.Some{v}=>{return v}Opt.None=>{raise E{}}}} fn main(){unwrap(Opt<int>.None)}"#, "unhandled error"); }

// Type parameter propagation through error boundaries
#[test] fn generic_fn_calls_fallible() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn wrap<T>(maker:fn()T)T{return maker()} fn main(){wrap(f)}"#, "unhandled error"); }
#[test] fn generic_fn_calls_fallible_propagate() { compile_should_fail_with(r#"error E{} fn f()int!{raise E{}} fn wrap<T>(maker:fn()T!)T{return maker()!} fn main(){wrap(f)}"#, "unhandled error"); }

// Error sets differ per instantiation
#[test] fn different_errors_per_instantiation() { compile_should_fail_with(r#"error E1{} error E2{} fn process<T>(x:T)T{if true{raise E1{}}if false{raise E2{}}return x} fn main(){process(42)}"#, "unhandled error"); }
#[test] fn generic_accumulates_errors() { compile_should_fail_with(r#"error E1{} error E2{} fn a()! {raise E1{}} fn b()!{raise E2{}} fn combine<T>(x:T)T{a() b() return x} fn main(){combine(42)}"#, "unhandled error"); }

// Generic type bounds with errors
#[test] fn generic_bounded_fallible() { compile_should_fail_with(r#"error E{} trait T{} class C{x:int} impl T fn process<U:T>(x:U)U!{raise E{}} fn main(){process(C{x:1})}"#, "unhandled error"); }
#[test] fn generic_multi_bound_errors() { compile_should_fail_with(r#"error E{} trait T1{} trait T2{} class C{x:int} impl T1 impl T2 fn process<U:T1+T2>(x:U)U!{raise E{}} fn main(){process(C{x:1})}"#, "unhandled error"); }

// Nested generics with errors
#[test] fn nested_generic_fallible() { compile_should_fail_with(r#"error E{} class Box<T>{value:T} fn unbox<T>(b:Box<T>)T!{raise E{}} fn main(){let b=Box<int>{value:42}unbox(b)}"#, "unhandled error"); }
#[test] fn generic_fn_returns_generic_fallible() { compile_should_fail_with(r#"error E{} class Box<T>{value:T} fn wrap<T>(x:T)Box<T>!{raise E{}} fn main(){wrap(42)}"#, "unhandled error"); }

// Generics with explicit type arguments and errors
#[test] fn explicit_type_arg_fallible() { compile_should_fail_with(r#"error E{} fn id<T>(x:T)T!{raise E{}} fn main(){id<int>(42)}"#, "unhandled error"); }
#[test] fn explicit_type_arg_different_error_sets() { compile_should_fail_with(r#"error E1{} error E2{} fn process<T>(x:T)T{if true{raise E1{}}if false{raise E2{}}return x} fn main(){process<int>(42) process<string>(\"hi\")}"#, "unhandled error"); }

// Generic closures with errors (complex interaction)
#[test] fn generic_with_closure_fallible() { compile_should_fail_with(r#"error E{} fn apply<T>(f:fn(T)T,x:T)T{if true{raise E{}}return f(x)} fn main(){apply((n:int)=>n+1,42)}"#, "unhandled error"); }
#[test] fn generic_closure_param_fallible() { compile_should_fail_with(r#"error E{} fn apply<T>(f:fn(T)T!,x:T)T{return f(x)} fn id(x:int)int!{raise E{}} fn main(){apply(id,42)}"#, "unhandled error"); }

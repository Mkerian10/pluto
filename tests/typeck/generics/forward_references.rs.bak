//! Forward reference tests for generics - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Class forward references
#[test]
#[ignore] // Outdated error message assertions
fn forward_ref_in_generic_param() { compile_should_fail_with(r#"class Box<T>{value:Forward} class Forward{x:int} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn forward_ref_generic_field() { compile_should_fail_with(r#"class Container<T>{data:T next:Forward} fn main(){} class Forward{x:int}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn forward_ref_in_bound() { compile_should_fail_with(r#"fn f<T:ForwardTrait>(x:T){} trait ForwardTrait{} fn main(){}"#, "undefined"); }

// Function forward references
#[test]
#[ignore] // Outdated error message assertions
fn call_forward_generic_fn() { compile_should_fail_with(r#"fn caller(){forward<int>(42)} fn forward<T>(x:T)T{return x} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn generic_returns_forward_class() { compile_should_fail_with(r#"fn make<T>()Forward{return Forward{x:42}} class Forward{x:int} fn main(){}"#, "undefined"); }

// Enum forward references
#[test]
#[ignore] // Outdated error message assertions
fn forward_ref_in_enum_variant() { compile_should_fail_with(r#"enum Container<T>{Some{v:T other:Forward}None} class Forward{x:int} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn generic_enum_forward_variant() { compile_should_fail_with(r#"enum E<T>{A{x:T} B{y:Forward}} fn main(){} class Forward{x:int}"#, "undefined"); }

// Trait forward references
#[test]
#[ignore] // Outdated error message assertions
fn impl_forward_trait() { compile_should_fail_with(r#"class C{x:int} impl ForwardTrait fn main(){} trait ForwardTrait{}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn generic_bound_forward_trait() { compile_should_fail_with(r#"fn f<T:ForwardT>(x:T){} fn main(){} trait ForwardT{}"#, "undefined"); }

// Circular references with generics
#[test]
#[ignore] // Outdated error message assertions
fn circular_generic_classes() { compile_should_fail_with(r#"class A<T>{b:B<T>} class B<U>{a:A<U>} fn main(){}"#, ""); }
#[test]
#[ignore] // Outdated error message assertions
fn circular_through_param() { compile_should_fail_with(r#"class Node<T>{value:T next:Node<T>?} fn main(){}"#, ""); }

// Method forward references
#[test]
#[ignore] // Outdated error message assertions
fn generic_method_forward_return() { compile_should_fail_with(r#"class C{fn foo<T>(self)Forward{return Forward{x:42}}} class Forward{x:int} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn forward_ref_method_param() { compile_should_fail_with(r#"class C{fn foo<T>(self,f:Forward)T{return f.x}} fn main(){} class Forward{x:int}"#, "undefined"); }

// Nested forward references
#[test]
#[ignore] // Outdated error message assertions
fn nested_forward_in_generic() { compile_should_fail_with(r#"class Box<T>{value:T} fn make()Box<Forward>{return Box<Forward>{value:Forward{x:42}}} class Forward{x:int} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn array_of_forward_in_generic() { compile_should_fail_with(r#"class Container<T>{items:[T]} fn make()Container<Forward>{return Container<Forward>{items:[Forward{x:1}]}} class Forward{x:int} fn main(){}"#, "undefined"); }

// Generic instance before class declaration
#[test]
#[ignore] // Outdated error message assertions
fn use_generic_before_decl() { compile_should_fail_with(r#"fn main(){let b=Box<int>{value:42}} class Box<T>{value:T}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn explicit_type_arg_forward() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id<Forward>(Forward{x:1})} class Forward{x:int}"#, "undefined"); }

// Forward ref in type alias context
#[test]
#[ignore] // Outdated error message assertions
fn generic_param_forward_type() { compile_should_fail_with(r#"fn use<T>(x:Forward<T>){} fn main(){} class Forward<U>{value:U}"#, "undefined"); }

// Multiple forward references
#[test]
#[ignore] // Outdated error message assertions
fn two_forward_refs() { compile_should_fail_with(r#"fn f(){let a=A{x:1} let b=B{y:2}} class A{x:int} class B{y:int} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn forward_in_generic_bound_chain() { compile_should_fail_with(r#"fn f<T:Trait1>(x:T){} trait Trait1{} fn main(){}"#, "undefined"); }

// Forward ref with DI
#[test]
#[ignore] // Outdated error message assertions
fn di_forward_ref() { compile_should_fail_with(r#"class Service[dep:Forward]{} class Forward{x:int} app MyApp{fn main(self){}} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn generic_di_forward() { compile_should_fail_with(r#"class Repo<T>[dep:Forward]{value:T} fn main(){} class Forward{x:int}"#, "undefined"); }

// Forward ref in match
#[test]
#[ignore] // Outdated error message assertions
fn match_forward_enum() { compile_should_fail_with(r#"fn use(e:E){match e{E.A=>{}}} enum E{A} fn main(){}"#, "undefined"); }
#[test]
#[ignore] // Outdated error message assertions
fn generic_match_forward() { compile_should_fail_with(r#"fn unwrap<T>(o:Opt<T>)T{match o{Opt.Some{v}=>{return v}Opt.None=>{return none}}} enum Opt<U>{Some{v:U}None} fn main(){}"#, "undefined"); }

// Forward ref in error context
#[test]
#[ignore] // Outdated error message assertions
fn forward_error_type() { compile_should_fail_with(r#"fn f()!E{raise E{}} error E{} fn main(){}"#, "undefined"); }

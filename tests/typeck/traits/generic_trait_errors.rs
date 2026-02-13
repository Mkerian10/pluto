//! Generic trait implementation errors - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic generic trait errors
#[test]
fn generic_trait_wrong_type_arg() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int>{fn foo(self)string{return "hi"}} fn main(){}"#, "type mismatch"); }
#[test]
fn generic_trait_missing_type_arg() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Multiple type parameters
#[test]
fn generic_trait_two_params_wrong() { compile_should_fail_with(r#"trait T<U,V>{fn foo(self,x:U)V} class C{} impl T<int,string>{fn foo(self,x:string)int{return 1}} fn main(){}"#, "type mismatch"); }
#[test]
fn generic_trait_wrong_param_count() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int,string>{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Generic class implementing generic trait
#[test]
fn generic_class_generic_trait_mismatch() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class Box<V>{value:V} impl T<int>{fn foo(self)string{return "hi"}} fn main(){}"#, "type mismatch"); }
#[test]
fn generic_class_trait_wrong_arg() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class Box<V>{value:V} impl T<V>{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Type bounds on generic traits
#[test]
fn generic_trait_bound_not_satisfied() { compile_should_fail_with(r#"trait Printable{} trait T<U:Printable>{fn foo(self)U} class C{x:int} impl T<int>{fn foo(self)int{return 1}} fn main(){}"#, "does not satisfy"); }

// Multiple impls of same generic trait
#[test]
fn two_impls_same_generic_trait() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int>{fn foo(self)int{return 1}} impl T<string>{fn foo(self)string{return "hi"}} fn main(){}"#, ""); }

// Generic trait with generic methods
#[test]
fn generic_trait_generic_method() { compile_should_fail_with(r#"trait T<U>{fn foo<V>(self,x:V)U} class C{} impl T<int>{fn foo<V>(self,x:V)string{return "hi"}} fn main(){}"#, "type mismatch"); }

// Conflicting generic trait impls
#[test]
fn overlapping_generic_trait_impls() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class Box<V>{value:V} impl T<int>{fn foo(self)int{return 1}} impl T<V>{fn foo(self)V{return self.value}} fn main(){}"#, ""); }

// Missing method in generic trait impl
#[test]
fn generic_trait_missing_method() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U fn bar(self)U} class C{} impl T<int>{fn foo(self)int{return 1}} fn main(){}"#, "missing method"); }

// Wrong type param in method signature
#[test]
fn generic_trait_method_uses_wrong_param() { compile_should_fail_with(r#"trait T<U>{fn foo(self,x:U)U} class C{} impl T<int>{fn foo(self,x:string)int{return 1}} fn main(){}"#, "type mismatch"); }

// Trait object from generic trait
#[test]
fn generic_trait_object() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class C{} impl T<int>{fn foo(self)int{return 1}} fn main(){let t:T<int>=C{}}"#, ""); }

// Associated type conflicts (if supported)
#[test]
fn generic_trait_associated_type() { compile_should_fail_with(r#"trait T<U>{type Output fn foo(self)Output} class C{} impl T<int>{type Output=string fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Default type parameters (if supported)
#[test]
fn generic_trait_default_param() { compile_should_fail_with(r#"trait T<U=int>{fn foo(self)U} class C{} impl T{fn foo(self)string{return "hi"}} fn main(){}"#, ""); }

// Variance issues
#[test]
fn generic_trait_covariance() { compile_should_fail_with(r#"class Base{} class Derived{} trait T<U>{fn foo(self)U} class C{} impl T<Base>{fn foo(self)Derived{return Derived{}}} fn main(){}"#, ""); }

// Generic trait with self type
#[test]
fn generic_trait_self_type() { compile_should_fail_with(r#"trait T<U>{fn foo(self)Self} class C{} impl T<int>{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Circular generic trait impls
#[test]
fn circular_generic_trait() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class A{} impl T<B>{fn foo(self)B{return B{}}} class B{} impl T<A>{fn foo(self)A{return A{}}} fn main(){}"#, ""); }

// Generic trait with const generics (if supported)
#[test]
fn trait_const_generic() { compile_should_fail_with(r#"trait T<const N:int>{fn foo(self)[int;N]} class C{} impl T<5>{fn foo(self)[int;10]{return [0;10]}} fn main(){}"#, ""); }

// Higher-kinded types (if supported)
#[test]
fn trait_hkt() { compile_should_fail_with(r#"trait T<F<_>>{fn foo<U>(self,x:F<U>)F<U>} class C{} impl T<Box>{fn foo<U>(self,x:Box<U>)Box<U>{return x}} fn main(){}"#, ""); }

// Generic trait with where clause
#[test]
fn generic_trait_where_clause() { compile_should_fail_with(r#"trait T<U> where U:Printable{fn foo(self)U} trait Printable{} class C{} impl T<int>{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Generic trait implemented for generic type
#[test]
fn generic_for_generic() { compile_should_fail_with(r#"trait T<U>{fn foo(self)U} class Box<V>{value:V} impl T<V>{fn foo(self)V{return self.value}} fn main(){let b=Box<int>{value:1}}"#, ""); }

// Phantom type parameters
#[test]
fn trait_phantom_param() { compile_should_fail_with(r#"trait T<U>{fn foo(self)int} class C{} impl T<string>{fn foo(self)int{return 1}} fn main(){}"#, ""); }

// Generic trait with lifetime bounds (if supported)
#[test]
fn trait_lifetime_bound() { compile_should_fail_with(r#"trait T<'a,U>{fn foo(self,x:&'a U)U} class C{} impl T<'static,int>{fn foo(self,x:&int)int{return *x}} fn main(){}"#, ""); }

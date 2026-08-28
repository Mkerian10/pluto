//! Liskov substitution principle violation tests.
//!
//! Pluto has no `ensures` clauses: postconditions were eliminated by design
//! (docs/design/contracts.md, "Why No `ensures`?") — guarantees are expressed
//! with class invariants and return types. Tests that use `ensures` pin the
//! targeted parser rejection; the Liskov checks themselves cover `requires`.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

const ENSURES_MSG: &str = "'ensures' clauses are not supported";

// Adding requires to implementation (violates LSP)
#[test]
fn impl_adds_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C impl T{fn foo(self,x:int)
requires x>0
{}}
fn main(){}"#, "Liskov"); }
#[test]
fn impl_stronger_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)
requires x>0} class C impl T
{fn foo(self,x:int)
requires x>10
{}}
fn main(){}"#, "Liskov"); }

// `ensures` is rejected outright — on trait signatures and implementations
#[test]
fn trait_sig_ensures_rejected() { compile_should_fail_with(r#"trait T{fn foo(self)int
ensures result>10} class C impl T
{fn foo(self)int
{return 5}}
fn main(){}"#, ENSURES_MSG); }
#[test]
fn impl_ensures_rejected() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C impl T
{fn foo(self)int
ensures result>0
{return 5}}
fn main(){}"#, ENSURES_MSG); }

// Implementations may not declare their own requires at all (the blanket
// Liskov rule) — even "weaker" ones
#[test]
fn impl_weaker_requires_rejected() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)
requires x>10} class C impl T
{fn foo(self,x:int)
requires x>0
{}} fn main(){}"#, "Liskov"); }
#[test]
fn ensures_rejected_everywhere() { compile_should_fail_with(r#"trait T{fn foo(self)int
ensures result>0} class C impl T
{fn foo(self)int
ensures result>10
{return 11}} fn main(){}"#, ENSURES_MSG); }

// Contract conflicts
#[test]
fn impl_contradicts_trait_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)
requires x>0} class C impl T
{fn foo(self,x:int)
requires x<0
{}} fn main(){}"#, "method 'foo' on class 'C' cannot add 'requires' clauses: it implements trait 'T' and adding preconditions would violate the Liskov Substitution Principle"); }
#[test]
fn impl_contradicts_trait_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int
ensures result>0} class C impl T
{fn foo(self)int
ensures result<0
{return -1}} fn main(){}"#, ENSURES_MSG); }

// Multiple requires/ensures
#[test]
fn impl_adds_second_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)
requires x>0} class C impl T
{fn foo(self,x:int)
requires x>0
requires x<100
{}}
fn main(){}"#, "Liskov"); }

// Invariant vs method contracts
#[test]
fn trait_method_vs_class_invariant() { compile_should_fail_with(r#"trait T{fn foo(self)int
ensures result>0} class C
{x:int invariant
self.x<0} impl T{fn foo(self)int{return
self.x}} fn main(){}"#, ENSURES_MSG); }

// Return type covariance with contracts
#[test]
fn subtype_return_with_contract() { compile_should_fail_with(r#"class Base{x:int} class Derived{x:int
y:int} trait T{fn foo(self)Base ensures
result.x>0} class C impl T{fn foo(self)Derived ensures
result.y>0{return Derived{x:-1 y:1}}} fn main(){}"#, ENSURES_MSG); }

// Parameter type contravariance (not supported in Pluto)
#[test]
fn supertype_param() { compile_should_fail_with(r#"class Base{x:int} class Derived{x:int
y:int} trait T{fn foo(self,d:Derived)} class C impl T{fn foo(self,b:Base){}}
fn main(){}"#, "type mismatch"); }

// Nullable with contracts
#[test]
fn nullable_return_with_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int
ensures result>0} class C impl T
{fn foo(self)int?{return none}}
fn main(){}"#, ENSURES_MSG); }

// Error types with contracts
#[test]
fn error_impl_with_ensures() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int
ensures result>0} class C impl T
{fn foo(self)int!{raise E{}}}
fn main(){}"#, ENSURES_MSG); }

// Contract on self parameter
#[test]
fn impl_adds_self_requires() { compile_should_fail_with(r#"trait T{fn foo(self)} class C impl T{x:int
fn foo(self)
requires self.x > 0
{}}
fn main(){}"#, "Liskov"); }

// Multiple contracts, partial override
#[test]
fn impl_changes_one_of_two_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int,y:int)
requires x>0
requires y>0} class C impl T
{fn foo(self,x:int,y:int)
requires x>10
requires y>0
{}}
fn main(){}"#, "Liskov"); }

// Generic method contracts
#[test]
#[ignore] // Unsupported syntax: trait methods cannot declare their own type parameters
fn generic_method_adds_contract() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C impl T{fn foo<U>(self,x:U)U
requires true
{return x}}
fn main(){}"#, "Liskov"); }

// Trait with no contracts, impl adds them
#[test]
fn impl_adds_both_contracts() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)int} class C impl T{fn foo(self,x:int)int
requires x>0
ensures result>0
{return x}}
fn main(){}"#, ENSURES_MSG); }

// Multiple traits with conflicting contracts
#[test]
fn two_traits_conflicting_ensures() { compile_should_fail_with(r#"trait T1{fn foo(self)int
ensures result>0} trait T2
{fn foo(self)int
ensures result<0} class C impl T1
{fn foo(self)int{return 1}} impl T2{fn foo(self)int{return -1}} fn main(){}"#, ENSURES_MSG); }

// Trait composition
#[test]
fn trait_extends_adds_requires() { compile_should_fail_with(r#"trait Base{fn foo(self,x:int)}
trait Derived{fn foo(self,x:int)
requires x>0}
class C impl Base, Derived {fn foo(self,x:int){}
fn foo(self,x:int){}}
fn main(){}"#, "duplicate method 'foo' in class 'C'"); }

// Contract language restrictions
#[test]
fn contract_calls_method() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{fn helper(self)bool{return true}} impl T{fn foo(self)int requires
self.helper(){return 1}} fn main(){}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found impl"); }

// Deep contract violations
#[test]
fn nested_field_contract_added() { compile_should_fail_with(r#"class Inner{x:int} class Outer{inner:Inner} trait T{fn foo(self,o:Outer)} class C impl T{fn foo(self,o:Outer)
requires o.inner.x > 0
{}}
fn main(){}"#, "Liskov"); }

// Ensures on void method
#[test]
fn void_method_with_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)} class C impl T{x:int
fn foo(self)
ensures
self.x>0{}} fn main(){}"#, ENSURES_MSG); }

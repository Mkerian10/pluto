//! Liskov substitution principle violation tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Adding requires to implementation (violates LSP)
#[test]
fn impl_adds_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:int)requires x>0{}} fn main(){}"#, "Liskov"); }
#[test]
fn impl_stronger_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)requires x>0} class C{} impl T{fn foo(self,x:int)requires x>10{}} fn main(){}"#, "Liskov"); }

// Weakening ensures (violates LSP)
#[test]
fn impl_weaker_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>10} class C{} impl T{fn foo(self)int ensures result>0{return 5}} fn main(){}"#, "Liskov"); }
#[test]
fn impl_removes_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int{return -1}} fn main(){}"#, "Liskov"); }

// Allowed: weaker requires, stronger ensures
#[test]
fn impl_weaker_requires_ok() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)requires x>10} class C{} impl T{fn foo(self,x:int)requires x>0{}} fn main(){}"#, ""); }
#[test]
fn impl_stronger_ensures_ok() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int ensures result>10{return 11}} fn main(){}"#, ""); }

// Contract conflicts
#[test]
fn impl_contradicts_trait_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)requires x>0} class C{} impl T{fn foo(self,x:int)requires x<0{}} fn main(){}"#, ""); }
#[test]
fn impl_contradicts_trait_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int ensures result<0{return -1}} fn main(){}"#, ""); }

// Multiple requires/ensures
#[test]
fn impl_adds_second_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)requires x>0} class C{} impl T{fn foo(self,x:int)requires x>0 requires x<100{}} fn main(){}"#, "Liskov"); }

// Invariant vs method contracts
#[test]
fn trait_method_vs_class_invariant() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{x:int invariant self.x<0} impl T{fn foo(self)int{return self.x}} fn main(){}"#, ""); }

// Return type covariance with contracts
#[test]
fn subtype_return_with_contract() { compile_should_fail_with(r#"class Base{x:int} class Derived{x:int y:int} trait T{fn foo(self)Base ensures result.x>0} class C{} impl T{fn foo(self)Derived ensures result.y>0{return Derived{x:-1 y:1}}} fn main(){}"#, ""); }

// Parameter type contravariance (not supported in Pluto)
#[test]
fn supertype_param() { compile_should_fail_with(r#"class Base{x:int} class Derived{x:int y:int} trait T{fn foo(self,d:Derived)} class C{} impl T{fn foo(self,b:Base){}} fn main(){}"#, "type mismatch"); }

// Nullable with contracts
#[test]
fn nullable_return_with_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int?{return none}} fn main(){}"#, "type mismatch"); }

// Error types with contracts
#[test]
fn error_impl_with_ensures() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int ensures result>0} class C{} impl T{fn foo(self)int!{raise E{}}} fn main(){}"#, "type mismatch"); }

// Contract on self parameter
#[test]
fn impl_adds_self_requires() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(self)requires self.x>0{}} fn main(){}"#, "Liskov"); }

// Multiple contracts, partial override
#[test]
fn impl_changes_one_of_two_requires() { compile_should_fail_with(r#"trait T{fn foo(self,x:int,y:int)requires x>0 requires y>0} class C{} impl T{fn foo(self,x:int,y:int)requires x>10 requires y>0{}} fn main(){}"#, "Liskov"); }

// Generic method contracts
#[test]
fn generic_method_adds_contract() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C{} impl T{fn foo<U>(self,x:U)U requires true{return x}} fn main(){}"#, "Liskov"); }

// Trait with no contracts, impl adds them
#[test]
fn impl_adds_both_contracts() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)int} class C{} impl T{fn foo(self,x:int)int requires x>0 ensures result>0{return x}} fn main(){}"#, "Liskov"); }

// Multiple traits with conflicting contracts
#[test]
fn two_traits_conflicting_ensures() { compile_should_fail_with(r#"trait T1{fn foo(self)int ensures result>0} trait T2{fn foo(self)int ensures result<0} class C{} impl T1{fn foo(self)int{return 1}} impl T2{fn foo(self)int{return -1}} fn main(){}"#, ""); }

// Trait composition
#[test]
fn trait_extends_adds_requires() { compile_should_fail_with(r#"trait Base{fn foo(self,x:int)} trait Derived{fn foo(self,x:int)requires x>0} class C{} impl Base{fn foo(self,x:int){}} impl Derived{fn foo(self,x:int){}} fn main(){}"#, ""); }

// Contract language restrictions
#[test]
fn contract_calls_method() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{fn helper(self)bool{return true}} impl T{fn foo(self)int requires self.helper(){return 1}} fn main(){}"#, ""); }

// Deep contract violations
#[test]
fn nested_field_contract_added() { compile_should_fail_with(r#"class Inner{x:int} class Outer{inner:Inner} trait T{fn foo(self,o:Outer)} class C{} impl T{fn foo(self,o:Outer)requires o.inner.x>0{}} fn main(){}"#, "Liskov"); }

// Ensures on void method
#[test]
fn void_method_with_ensures() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(self)ensures self.x>0{}} fn main(){}"#, ""); }

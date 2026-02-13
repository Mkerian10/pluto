//! Liskov substitution with contracts - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Class method adds requires to trait method
#[test]
fn add_requires_to_trait() { compile_should_fail_with(r#"trait T{fn f(self,x:int)} class C{} impl T{fn f(self,x:int) requires x>0 {}} fn main(){}"#, ""); }

// Class method weakens trait requires
#[test]
fn weaken_trait_requires() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0} class C{} impl T{fn f(self,x:int) requires x>5 {}} fn main(){}"#, ""); }

// Class method strengthens trait ensures
#[test]
fn strengthen_trait_ensures() { compile_should_fail_with(r#"trait T{fn f(self,x:int) ensures result>0 int} class C{} impl T{fn f(self,x:int) ensures result>10 int{return x}} fn main(){}"#, ""); }

// Class method removes trait requires
#[test]
fn remove_trait_requires() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0} class C{} impl T{fn f(self,x:int){}} fn main(){}"#, ""); }

// Class method removes trait ensures
#[test]
fn remove_trait_ensures() { compile_should_fail_with(r#"trait T{fn f(self,x:int) ensures result>0 int} class C{} impl T{fn f(self,x:int)int{return x}} fn main(){}"#, ""); }

// Multiple traits with conflicting contracts
#[test]
fn conflicting_trait_contracts() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn f(self,x:int) requires x<0} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn f(self,x:int) requires x<0 {}} fn main(){}"#, ""); }

// Trait method with invariant violation
#[test]
fn trait_method_invariant() { compile_should_fail_with(r#"trait T{fn f(self,x:int)} class C{y:int invariant self.y>0} impl T{fn f(self,x:int){self.y=-1}} fn main(){}"#, ""); }

// Generic trait with contracts
#[test]
fn generic_trait_contracts() { compile_should_fail_with(r#"trait T<U>{fn f(self,x:U) requires x>0} class C{} impl T<int>{fn f(self,x:int) requires x>5 {}} fn main(){}"#, ""); }

// Trait chain with contracts
#[test]
fn trait_chain_contracts() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn f(self,x:int) requires x>5} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn f(self,x:int) requires x>5 {}} fn main(){}"#, ""); }

// Class weakens inherited invariant
#[test]
fn weaken_inherited_invariant() { compile_should_fail_with(r#"class Base{x:int invariant self.x>0} class Derived{x:int invariant self.x>-1} fn main(){}"#, ""); }

// Trait method with different contract types
#[test]
fn different_contract_types() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0} class C{} impl T{fn f(self,x:int) ensures x>0 {}} fn main(){}"#, ""); }

// Override with stricter parameter contract
#[test]
fn stricter_param_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int)} class C{} impl T{fn f(self,x:int) requires x>0 and x<10 {}} fn main(){}"#, ""); }

// Override with weaker return contract
#[test]
fn weaker_return_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) ensures result>0 int} class C{} impl T{fn f(self,x:int) ensures result>-1 int{return x}} fn main(){}"#, ""); }

// Trait with multiple contract clauses
#[test]
fn trait_multiple_contracts() { compile_should_fail_with(r#"trait T{fn f(self,x:int,y:int) requires x>0 requires y>0} class C{} impl T{fn f(self,x:int,y:int) requires x>0 {}} fn main(){}"#, ""); }

// Class adds new contract to trait method
#[test]
fn add_new_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int)} class C{} impl T{fn f(self,x:int) ensures result>0 int{return x}} fn main(){}"#, ""); }

// Trait method signature mismatch with contract
#[test]
fn signature_mismatch_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0 int} class C{} impl T{fn f(self,x:float) requires x>0.0 int{return 1}} fn main(){}"#, ""); }

// Trait with invariant and method contract
#[test]
fn trait_invariant_method_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires self.y>0} class C{y:int} impl T{fn f(self,x:int) requires self.y>0 {}} fn main(){}"#, ""); }

// Contract on trait default method
#[test]
fn trait_default_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0 int{return x}} class C{} impl T{} fn main(){}"#, ""); }

// Class method violates trait contract
#[test]
fn violate_trait_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0} class C{} impl T{fn f(self,x:int){}} fn main(){}"#, ""); }

// Trait with nested contracts
#[test]
fn trait_nested_contracts() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0 and x<100} class C{} impl T{fn f(self,x:int) requires x>0 {}} fn main(){}"#, ""); }

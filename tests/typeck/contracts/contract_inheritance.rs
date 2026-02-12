//! Contract inheritance tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Inherited contract on overridden method
#[test] fn inherited_override_contract() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires x>0} class C{} impl T{fn f(self,y:int){}} fn main(){}"#, ""); }

// Contract propagation through trait chain
#[test] fn contract_trait_chain() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn g(self,y:int) ensures result>0 int} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn g(self,y:int)int{return y}} fn main(){}"#, ""); }

// Multiple traits with same method contract
#[test] fn multiple_trait_same_contract() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn f(self,x:int) requires x>5} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn f(self,x:int) requires x>5 {}} fn main(){}"#, ""); }

// Contract on generic class method
#[test] fn contract_generic_class() { compile_should_fail_with(r#"class C<T>{x:T} fn set(mut self,v:T) requires v>0 {self.x=v} fn main(){}"#, ""); }

// Invariant inheritance through composition
#[test] fn invariant_composition() { compile_should_fail_with(r#"class Inner{x:int invariant self.x>0} class Outer{i:Inner invariant self.i.x<0} fn main(){}"#, ""); }

// Contract on bracket dep method
#[test] fn contract_bracket_dep_method() { compile_should_fail_with(r#"class Dep{x:int} class C[d:Dep]{} fn use_dep(self) requires self.d.x>0 {print(self.d.x)} fn main(){}"#, ""); }

// Trait contract with self reference
#[test] fn trait_contract_self_ref() { compile_should_fail_with(r#"trait T{fn f(self,x:int) requires self.y>0} class C{y:int} impl T{fn f(self,x:int){}} fn main(){}"#, ""); }

// Contract on nested trait impl
#[test] fn nested_trait_impl_contract() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn g(self,y:int) ensures result>0 int} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn g(self,y:int) ensures result>0 int{return self.f(y)}} fn main(){}"#, ""); }

// Invariant on generic class with bounds
#[test] fn invariant_generic_bounds() { compile_should_fail_with(r#"trait Ord{} class C<T:Ord>{x:T invariant self.x>0} fn main(){}"#, ""); }

// Contract propagation to lambda
#[test] fn contract_lambda() { compile_should_fail_with(r#"fn f(g:fn(int)int) requires g(0)>0 {print("ok")} fn main(){let h=(x:int)=>x-1 f(h)}"#, ""); }

// Invariant on app class
#[test] fn invariant_app_class() { compile_should_fail_with(r#"app MyApp{x:int invariant self.x>0 fn main(self){}} fn main(){}"#, ""); }

// Contract on recursive method
#[test] fn contract_recursive_method() { compile_should_fail_with(r#"class C{x:int} fn rec(self,n:int) requires n>=0 int{if n==0{return 1}else{return self.rec(n-1)}} fn main(){}"#, ""); }

// Inherited contract conflict
#[test] fn inherited_contract_conflict() { compile_should_fail_with(r#"trait T1{fn f(self,x:int) requires x>0} trait T2{fn f(self,x:int) requires x<0} class C{} impl T1{fn f(self,x:int) requires x>0 {}} impl T2{fn f(self,x:int) requires x<0 {}} fn main(){}"#, ""); }

// Contract on error-returning method
#[test] fn contract_error_method() { compile_should_fail_with(r#"error E{} class C{x:int} fn check(self) requires self.x>0 !bool{if self.x<0{raise E{}}return true} fn main(){}"#, ""); }

// Contract on nullable-returning method
#[test] fn contract_nullable_method() { compile_should_fail_with(r#"class C{x:int} fn get(self) ensures result?>0 int?{if self.x>0{return self.x}return none} fn main(){}"#, ""); }

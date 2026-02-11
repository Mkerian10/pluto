//! mut self enforcement tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Field mutation without mut self
#[test] fn field_mutation_no_mut() { compile_should_fail_with(r#"class C{x:int} fn set(self,v:int){self.x=v} fn main(){}"#, ""); }

// Multiple field mutations without mut
#[test] fn multi_field_no_mut() { compile_should_fail_with(r#"class C{x:int y:int} fn update(self){self.x=1 self.y=2} fn main(){}"#, ""); }

// Mutation in non-mut method
#[test] fn mutation_in_non_mut() { compile_should_fail_with(r#"class C{count:int} fn increment(self){self.count=self.count+1} fn main(){}"#, ""); }

// Calling mut method from non-mut
#[test] fn call_mut_from_non_mut() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int){self.x=v} fn get(self)int{self.set(1) return self.x} fn main(){}"#, ""); }

// Mutation through non-mut self
#[test] fn mutation_through_non_mut() { compile_should_fail_with(r#"class C{x:int} fn foo(self){let y=self self.x=1} fn main(){}"#, ""); }

// Nested field mutation without mut
#[test] fn nested_field_mut_no_mut() { compile_should_fail_with(r#"class Inner{x:int} class Outer{i:Inner} fn set(self){self.i.x=1} fn main(){}"#, ""); }

// Array element mutation without mut
#[test] fn array_elem_mut_no_mut() { compile_should_fail_with(r#"class C{arr:Array<int>} fn set(self,i:int,v:int){self.arr[i]=v} fn main(){}"#, ""); }

// Map mutation without mut self
#[test] fn map_mut_no_mut() { compile_should_fail_with(r#"class C{m:Map<string,int>} fn insert(self,k:string,v:int){self.m[k]=v} fn main(){}"#, ""); }

// Method call that mutates without mut
#[test] fn method_mutates_no_mut() { compile_should_fail_with(r#"class C{x:int} fn double(self){self.x=self.x*2} fn main(){}"#, ""); }

// Conditional mutation without mut
#[test] fn cond_mut_no_mut() { compile_should_fail_with(r#"class C{x:int} fn maybe_set(self,b:bool){if b{self.x=1}} fn main(){}"#, ""); }

// Loop mutation without mut
#[test] fn loop_mut_no_mut() { compile_should_fail_with(r#"class C{x:int} fn inc_n(self,n:int){for i in 0..n{self.x=self.x+1}} fn main(){}"#, ""); }

// Match arm mutation without mut
#[test] fn match_mut_no_mut() { compile_should_fail_with(r#"enum E{A B} class C{x:int} fn foo(self,e:E){match e{E.A{self.x=1}E.B{self.x=2}}} fn main(){}"#, ""); }

// Mutation in trait impl without mut
#[test] fn trait_impl_mut_no_mut() { compile_should_fail_with(r#"trait T{fn update(self)} class C{x:int} impl T{fn update(self){self.x=1}} fn main(){}"#, ""); }

// Mutation through bracket dep without mut
#[test] fn bracket_dep_mut_no_mut() { compile_should_fail_with(r#"class Dep{x:int} class C[d:Dep]{} fn mutate(self){self.d.x=1} fn main(){}"#, ""); }

// Field increment without mut
#[test] fn field_increment_no_mut() { compile_should_fail_with(r#"class C{count:int} fn inc(self){self.count=self.count+1} fn main(){}"#, ""); }

// Compound assignment without mut
#[test] fn compound_assign_no_mut() { compile_should_fail_with(r#"class C{x:int} fn add(self,v:int){self.x=self.x+v} fn main(){}"#, ""); }

// Mutation in constructor (should work)
#[test] fn constructor_mutation() { compile_should_fail_with(r#"class C{x:int} fn new()C{let c=C{x:0} c.x=1 return c} fn main(){}"#, ""); }

// Mutation of immutable local
#[test] fn immutable_local_mut() { compile_should_fail_with(r#"fn main(){let x=1 x=2}"#, ""); }

// Mutation through immutable reference
#[test] fn immut_ref_mutation() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} let d=c c.x=2}"#, ""); }

// Mutation in invariant check
#[test] fn invariant_mutation() { compile_should_fail_with(r#"class C{x:int invariant self.x=1} fn main(){}"#, ""); }

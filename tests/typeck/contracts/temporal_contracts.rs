//! Temporal contract tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Requires references old value
#[test]
fn requires_old_value() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int) requires old(self.x)>0 {self.x=v} fn main(){}"#, ""); }

// Ensures references old value
#[test]
fn ensures_old_value() { compile_should_fail_with(r#"class C{x:int} fn inc(mut self) ensures self.x>old(self.x) {self.x=self.x+1} fn main(){}"#, ""); }

// Old value on non-mut method
#[test]
fn old_non_mut() { compile_should_fail_with(r#"class C{x:int} fn get(self) ensures result==old(self.x) int{return self.x} fn main(){}"#, ""); }

// Old value on undefined field
#[test]
fn old_undefined_field() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int) requires old(self.y)>0 {self.x=v} fn main(){}"#, ""); }

// Old value in invariant
#[test]
fn old_in_invariant() { compile_should_fail_with(r#"class C{x:int invariant self.x>old(self.x)} fn main(){}"#, ""); }

// Nested old values
#[test]
fn nested_old() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int) ensures self.x>old(old(self.x)) {self.x=v} fn main(){}"#, ""); }

// Old value on parameter
#[test]
fn old_param() { compile_should_fail_with(r#"fn f(x:int) ensures x>old(x) int{return x+1} fn main(){}"#, ""); }

// Old value on local variable
#[test]
fn old_local() { compile_should_fail_with(r#"fn f(x:int)int{let y=x ensures y>old(y) return y+1} fn main(){}"#, ""); }

// Multiple old references
#[test]
fn multiple_old() { compile_should_fail_with(r#"class C{x:int y:int} fn swap(mut self) ensures self.x==old(self.y) and self.y==old(self.x) {let t=self.x self.x=self.y self.y=t} fn main(){}"#, ""); }

// Old value type mismatch
#[test]
fn old_type_mismatch() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int) ensures self.x>old(self.x)+"hi" {self.x=v} fn main(){}"#, ""); }

// Old value on generic field
#[test]
fn old_generic_field() { compile_should_fail_with(r#"class C<T>{x:T} fn set(mut self,v:T) ensures self.x>old(self.x) {self.x=v} fn main(){}"#, ""); }

// Old value on array element
#[test]
fn old_array_elem() { compile_should_fail_with(r#"class C{arr:Array<int>} fn set(mut self,i:int,v:int) ensures self.arr[i]>old(self.arr[i]) {self.arr[i]=v} fn main(){}"#, ""); }

// Old value on map entry
#[test]
fn old_map_entry() { compile_should_fail_with(r#"class C{m:Map<string,int>} fn set(mut self,k:string,v:int) ensures self.m[k]>old(self.m[k]) {self.m[k]=v} fn main(){}"#, ""); }

// Old value in requires and ensures
#[test]
fn old_requires_ensures() { compile_should_fail_with(r#"class C{x:int} fn update(mut self,v:int) requires old(self.x)>0 ensures self.x>old(self.x) {self.x=v} fn main(){}"#, ""); }

// Old value on trait method
#[test]
fn old_trait_method() { compile_should_fail_with(r#"trait T{fn update(mut self,v:int) ensures self.get()>old(self.get())} class C{x:int} fn get(self)int{return self.x} impl T{fn update(mut self,v:int) ensures self.get()>old(self.get()) {self.x=v}} fn main(){}"#, ""); }

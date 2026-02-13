//! Method overloading errors (not supported) - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Two methods same name different params
#[test] fn overload_params() { compile_should_fail_with(r#"class C{} fn foo(self,x:int){} fn foo(self,x:string){} fn main(){}"#, ""); }

// Two methods same name different param count
#[test] fn overload_param_count() { compile_should_fail_with(r#"class C{} fn foo(self){} fn foo(self,x:int){} fn main(){}"#, ""); }

// Two methods same name different return
#[test] fn overload_return() { compile_should_fail_with(r#"class C{} fn foo(self)int{return 1} fn foo(self)string{return \"hi\"} fn main(){}"#, ""); }

// Generic method overload
#[test] fn overload_generic() { compile_should_fail_with(r#"class C{} fn foo(self,x:int){} fn foo<T>(self,x:T){} fn main(){}"#, ""); }

// Method overload with mut self
#[test] fn overload_mut_self() { compile_should_fail_with(r#"class C{x:int} fn foo(self){} fn foo(mut self){self.x=2} fn main(){}"#, ""); }

// Constructor overload
#[test] fn overload_constructor() { compile_should_fail_with(r#"class C{x:int} fn new()C{return C{x:0}} fn new(x:int)C{return C{x:x}} fn main(){}"#, ""); }

// Static method overload (if supported)
#[test] fn overload_static() { compile_should_fail_with(r#"class C{} fn create()C{return C{}} fn create(x:int)C{return C{}} fn main(){}"#, ""); }

// Trait method overload
#[test] fn overload_trait_method() { compile_should_fail_with(r#"trait T{fn foo(self) fn foo(self,x:int)} class C{} impl T{fn foo(self){} fn foo(self,x:int){}} fn main(){}"#, ""); }

// Overload with nullable
#[test] fn overload_nullable() { compile_should_fail_with(r#"class C{} fn foo(self,x:int){} fn foo(self,x:int?){} fn main(){}"#, ""); }

// Overload with error
#[test] fn overload_error() { compile_should_fail_with(r#"error E{} class C{} fn foo(self)int{return 1} fn foo(self)int!{raise E{}} fn main(){}"#, ""); }

// Overload with closure param
#[test] fn overload_closure_param() { compile_should_fail_with(r#"class C{} fn foo(self,f:(int)int){} fn foo(self,f:(string)string){} fn main(){}"#, ""); }

// Overload same signature (duplicate)
#[test] fn duplicate_method() { compile_should_fail_with(r#"class C{} fn foo(self){} fn foo(self){} fn main(){}"#, ""); }

// Overload with generic bound
#[test] fn overload_generic_bound() { compile_should_fail_with(r#"trait T{} class C{} fn foo<U>(self,x:U){} fn foo<V:T>(self,x:V){} fn main(){}"#, ""); }

// Overload on different classes (not overloading, but name collision)
#[test] fn same_name_diff_class() { compile_should_fail_with(r#"class C1{} fn foo(self:C1){} class C2{} fn foo(self:C2){} fn main(){}"#, ""); }

// Operator overloading (not supported)
#[test] fn operator_overload() { compile_should_fail_with(r#"class C{x:int} fn plus(self,other:C)C{return C{x:self.x+other.x}} fn main(){let c1=C{x:1}let c2=C{x:2}let c3=c1+c2}"#, ""); }

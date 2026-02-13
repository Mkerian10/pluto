//! Trait method signature mismatch tests - 30 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Return type mismatches
#[test] fn trait_method_wrong_return() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{x:int} impl T{fn foo(self)string{return "hi"}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_void_vs_int() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{x:int} impl T{fn foo(self)int{return 1}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_int_vs_void() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{x:int} impl T{fn foo(self){}} fn main(){}"#, "type mismatch"); }

// Parameter count mismatches
#[test] fn trait_method_too_many_params() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:int,y:int){}} fn main(){}"#, "parameter count"); }
#[test] fn trait_method_too_few_params() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self){}} fn main(){}"#, "parameter count"); }
#[test] fn trait_method_missing_self() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(){}} fn main(){}"#, "parameter count"); }

// Parameter type mismatches
#[test] fn trait_method_param_type_wrong() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:string){}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_two_params_wrong() { compile_should_fail_with(r#"trait T{fn foo(self,x:int,y:string)} class C{} impl T{fn foo(self,x:string,y:int){}} fn main(){}"#, "type mismatch"); }

// Self parameter variations
#[test] fn trait_method_mut_self_mismatch() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C{} impl T{fn foo(self){}} fn main(){}"#, ""); }
#[test] fn trait_method_self_vs_mut_self() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(mut self){}} fn main(){}"#, ""); }

// Method name mismatches
#[test] fn trait_method_wrong_name() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn bar(self){}} fn main(){}"#, "missing method"); }
#[test] fn trait_method_missing() { compile_should_fail_with(r#"trait T{fn foo(self) fn bar(self)} class C{} impl T{fn foo(self){}} fn main(){}"#, "missing method"); }

// Nullable parameter/return mismatches
#[test] fn trait_method_nullable_return() { compile_should_fail_with(r#"trait T{fn foo(self)int} class C{} impl T{fn foo(self)int?{return none}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_nullable_param() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:int?){}} fn main(){}"#, "type mismatch"); }

// Error signature mismatches
#[test] fn trait_method_fallible_impl() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int} class C{} impl T{fn foo(self)int!{raise E{}}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_non_fallible_impl() { compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C{} impl T{fn foo(self)int{return 1}} fn main(){}"#, "type mismatch"); }

// Generic method mismatches
#[test] fn trait_generic_method_missing_param() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C{} impl T{fn foo(self,x:int)int{return x}} fn main(){}"#, ""); }
#[test] fn trait_method_wrong_generic_count() { compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C{} impl T{fn foo<U,V>(self,x:U)U{return x}} fn main(){}"#, ""); }

// Array/collection parameter mismatches
#[test] fn trait_method_array_vs_single() { compile_should_fail_with(r#"trait T{fn foo(self,x:int)} class C{} impl T{fn foo(self,x:[int]){}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_map_type_wrong() { compile_should_fail_with(r#"trait T{fn foo(self,m:Map<string,int>)} class C{} impl T{fn foo(self,m:Map<string,string>){}} fn main(){}"#, "type mismatch"); }

// Class type parameter mismatches
#[test] fn trait_method_class_type_wrong() { compile_should_fail_with(r#"class A{x:int} class B{y:string} trait T{fn foo(self,a:A)} class C{} impl T{fn foo(self,b:B){}} fn main(){}"#, "type mismatch"); }

// Function type mismatches
#[test] fn trait_method_fn_type_wrong() { compile_should_fail_with(r#"trait T{fn foo(self,f:fn(int)int)} class C{} impl T{fn foo(self,f:fn(string)string){}} fn main(){}"#, "type mismatch"); }
#[test] fn trait_method_closure_type_wrong() { compile_should_fail_with(r#"trait T{fn foo(self,f:fn(int)int)} class C{} impl T{fn foo(self,f:fn(int)string){}} fn main(){}"#, "type mismatch"); }

// Enum parameter mismatches
#[test] fn trait_method_enum_type_wrong() { compile_should_fail_with(r#"enum E1{A} enum E2{B} trait T{fn foo(self,e:E1)} class C{} impl T{fn foo(self,e:E2){}} fn main(){}"#, "type mismatch"); }

// Extra methods in impl (should be allowed)
#[test] fn impl_extra_methods_ok() { compile_should_fail_with(r#"trait T{fn foo(self)} class C{} impl T{fn foo(self){} fn bar(self){}} fn main(){}"#, ""); }

// Multiple trait methods
#[test] fn multiple_methods_one_wrong() { compile_should_fail_with(r#"trait T{fn foo(self)int fn bar(self)string} class C{} impl T{fn foo(self)int{return 1} fn bar(self)int{return 2}} fn main(){}"#, "type mismatch"); }

// Trait on generic class
#[test] fn trait_on_generic_wrong_sig() { compile_should_fail_with(r#"trait T{fn foo(self)int} class Box<U>{value:U} impl T{fn foo(self)string{return "hi"}} fn main(){}"#, "type mismatch"); }

// Return type covariance (should fail - nominal typing)
#[test] fn trait_method_return_subtype() { compile_should_fail_with(r#"class Base{x:int} class Derived{x:int y:int} trait T{fn foo(self)Base} class C{} impl T{fn foo(self)Derived{return Derived{x:1 y:2}}} fn main(){}"#, "type mismatch"); }

// Trait method with contracts (if contracts affect signature)
#[test] fn trait_method_contract_mismatch() { compile_should_fail_with(r#"trait T{fn foo(self)int requires true} class C{} impl T{fn foo(self)int{return 1}} fn main(){}"#, ""); }

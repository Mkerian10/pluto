//! Invariant violation tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Invariant references undefined field
#[test]
fn invariant_undefined_field() { compile_should_fail_with(r#"class C{x:int invariant self.y>0} fn main(){}"#, ""); }

// Invariant references undefined variable
#[test]
fn invariant_undefined_var() { compile_should_fail_with(r#"class C{x:int invariant y>0} fn main(){}"#, ""); }

// Invariant type mismatch
#[test]
fn invariant_type_mismatch() { compile_should_fail_with(r#"class C{x:int invariant self.x=="hi"} fn main(){}"#, ""); }

// Invariant with function call
#[test]
fn invariant_function_call() { compile_should_fail_with(r#"fn f()bool{return true} class C{x:int invariant f()} fn main(){}"#, ""); }

// Invariant with method call
#[test]
fn invariant_method_call() { compile_should_fail_with(r#"class C{x:int invariant self.check()} fn check(self)bool{return true} fn main(){}"#, ""); }

// Invariant with indexing
#[test]
fn invariant_indexing() { compile_should_fail_with(r#"class C{arr:Array<int> invariant self.arr[0]>0} fn main(){}"#, ""); }

// Invariant with closure
#[test]
fn invariant_closure() { compile_should_fail_with(r#"class C{x:int invariant (()=>true)()} fn main(){}"#, ""); }

// Invariant with cast
#[test]
fn invariant_cast() { compile_should_fail_with(r#"class C{x:int invariant (self.x as float)>0.0} fn main(){}"#, ""); }

// Invariant with null propagation
#[test]
fn invariant_null_prop() { compile_should_fail_with(r#"class C{x:int? invariant self.x?>0} fn main(){}"#, ""); }

// Invariant with error propagation
#[test]
fn invariant_error_prop() { compile_should_fail_with(r#"error E{} fn f()!bool{return true} class C{x:int invariant f()!} fn main(){}"#, ""); }

// Multiple invariants with conflict
#[test]
fn invariant_conflict() { compile_should_fail_with(r#"class C{x:int invariant self.x>0 invariant self.x<0} fn main(){}"#, ""); }

// Invariant on bracket dep field
#[test]
fn invariant_bracket_dep() { compile_should_fail_with(r#"class Dep{x:int} class C[d:Dep]{invariant self.d.x>0} fn main(){}"#, ""); }

// Invariant with spawn
#[test]
fn invariant_spawn() { compile_should_fail_with(r#"fn task()bool{return true} class C{x:int invariant spawn task()} fn main(){}"#, ""); }

// Invariant referencing other invariant
#[test]
fn invariant_ref_other() { compile_should_fail_with(r#"class C{x:int y:int invariant self.x>0 invariant self.check_y()} fn check_y(self)bool{return self.y>0} fn main(){}"#, ""); }

// Invariant on generic type param
#[test]
fn invariant_generic_param() { compile_should_fail_with(r#"class C<T>{x:T invariant self.x>0} fn main(){}"#, ""); }

// Invariant with nested field access
#[test]
fn invariant_nested_field() { compile_should_fail_with(r#"class Inner{x:int} class Outer{i:Inner invariant self.i.x>0 invariant self.i.y>0} fn main(){}"#, ""); }

// Invariant with map access
#[test]
fn invariant_map_access() { compile_should_fail_with(r#"class C{m:Map<string,int> invariant self.m["key"]>0} fn main(){}"#, ""); }

// Invariant on trait impl
#[test]
fn invariant_trait_impl() { compile_should_fail_with(r#"trait T{} class C{x:int invariant self.x>0} impl T{} fn main(){}"#, ""); }

// Invariant with array method
#[test]
fn invariant_array_method() { compile_should_fail_with(r#"class C{arr:Array<int> invariant self.arr.contains(1)} fn main(){}"#, ""); }

// Invariant with string interpolation
#[test]
fn invariant_string_interp() { compile_should_fail_with(r#"class C{x:int invariant "value: {self.x}"} fn main(){}"#, ""); }

// Invariant return type not bool
#[test]
fn invariant_non_bool() { compile_should_fail_with(r#"class C{x:int invariant self.x} fn main(){}"#, ""); }

// Invariant with match expression
#[test]
fn invariant_match() { compile_should_fail_with(r#"enum E{A B} class C{x:int e:E invariant match self.e{E.A{true}E.B{false}}} fn main(){}"#, ""); }

// Invariant with if expression
#[test]
fn invariant_if_expr() { compile_should_fail_with(r#"class C{x:int invariant if self.x>0{true}else{false}} fn main(){}"#, ""); }

// Invariant on enum variant
#[test]
fn invariant_enum_variant() { compile_should_fail_with(r#"enum E{A{x:int invariant self.x>0}} fn main(){}"#, ""); }

// Invariant with external variable
#[test]
fn invariant_external_var() { compile_should_fail_with(r#"let global=1 class C{x:int invariant global>0} fn main(){}"#, ""); }

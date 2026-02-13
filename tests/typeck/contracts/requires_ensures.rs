//! Requires and ensures clause tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Requires references undefined parameter
#[test]
fn requires_undefined_param() { compile_should_fail_with(r#"fn f(x:int) requires y>0 int{return x} fn main(){}"#, ""); }

// Requires type mismatch
#[test]
fn requires_type_mismatch() { compile_should_fail_with(r#"fn f(x:int) requires x=="hi" int{return x} fn main(){}"#, ""); }

// Requires with function call
#[test]
fn requires_function_call() { compile_should_fail_with(r#"fn check()bool{return true} fn f(x:int) requires check() int{return x} fn main(){}"#, ""); }

// Ensures references undefined variable
#[test]
fn ensures_undefined_var() { compile_should_fail_with(r#"fn f(x:int) ensures y>0 int{return x} fn main(){}"#, ""); }

// Ensures type mismatch
#[test]
fn ensures_type_mismatch() { compile_should_fail_with(r#"fn f(x:int) ensures result=="hi" int{return x} fn main(){}"#, ""); }

// Ensures with function call
#[test]
fn ensures_function_call() { compile_should_fail_with(r#"fn check()bool{return true} fn f(x:int) ensures check() int{return x} fn main(){}"#, ""); }

// Requires on method references undefined field
#[test]
fn method_requires_undefined_field() { compile_should_fail_with(r#"class C{x:int} fn set(mut self,v:int) requires self.y>0 {self.x=v} fn main(){}"#, ""); }

// Ensures on method references undefined field
#[test]
fn method_ensures_undefined_field() { compile_should_fail_with(r#"class C{x:int} fn get(self) ensures self.y>0 int{return self.x} fn main(){}"#, ""); }

// Requires with closure
#[test]
fn requires_closure() { compile_should_fail_with(r#"fn f(x:int) requires (()=>true)() int{return x} fn main(){}"#, ""); }

// Ensures with closure
#[test]
fn ensures_closure() { compile_should_fail_with(r#"fn f(x:int) ensures (()=>true)() int{return x} fn main(){}"#, ""); }

// Requires with indexing
#[test]
fn requires_indexing() { compile_should_fail_with(r#"fn f(arr:Array<int>) requires arr[0]>0 int{return 1} fn main(){}"#, ""); }

// Ensures with indexing
#[test]
fn ensures_indexing() { compile_should_fail_with(r#"fn f() ensures arr[0]>0 Array<int>{return [1,2,3]} fn main(){}"#, ""); }

// Requires return type not bool
#[test]
fn requires_non_bool() { compile_should_fail_with(r#"fn f(x:int) requires x int{return x} fn main(){}"#, ""); }

// Ensures return type not bool
#[test]
fn ensures_non_bool() { compile_should_fail_with(r#"fn f(x:int) ensures x int{return x} fn main(){}"#, ""); }

// Multiple requires clauses
#[test]
fn multiple_requires() { compile_should_fail_with(r#"fn f(x:int,y:int) requires x>0 requires y>x int{return x+y} fn main(){}"#, ""); }

// Multiple ensures clauses
#[test]
fn multiple_ensures() { compile_should_fail_with(r#"fn f(x:int) ensures result>0 ensures result<10 int{return x} fn main(){}"#, ""); }

// Requires with null propagation
#[test]
fn requires_null_prop() { compile_should_fail_with(r#"fn f(x:int?) requires x?>0 int{return 1} fn main(){}"#, ""); }

// Ensures with null propagation
#[test]
fn ensures_null_prop() { compile_should_fail_with(r#"fn f() ensures result?>0 int?{return 1} fn main(){}"#, ""); }

// Requires with error propagation
#[test]
fn requires_error_prop() { compile_should_fail_with(r#"error E{} fn check()!bool{return true} fn f(x:int) requires check()! int{return x} fn main(){}"#, ""); }

// Ensures with error propagation
#[test]
fn ensures_error_prop() { compile_should_fail_with(r#"error E{} fn check()!bool{return true} fn f(x:int) ensures check()! int{return x} fn main(){}"#, ""); }

// Requires on generic function
#[test]
fn requires_generic() { compile_should_fail_with(r#"fn f<T>(x:T) requires x>0 T{return x} fn main(){}"#, ""); }

// Ensures on generic function
#[test]
fn ensures_generic() { compile_should_fail_with(r#"fn f<T>(x:T) ensures result>0 T{return x} fn main(){}"#, ""); }

// Requires with cast
#[test]
fn requires_cast() { compile_should_fail_with(r#"fn f(x:int) requires (x as float)>0.0 int{return x} fn main(){}"#, ""); }

// Ensures with cast
#[test]
fn ensures_cast() { compile_should_fail_with(r#"fn f(x:int) ensures (result as float)>0.0 int{return x} fn main(){}"#, ""); }

// Requires on void function
#[test]
fn requires_void_func() { compile_should_fail_with(r#"fn f(x:int) requires x>0 {print(x)} fn main(){}"#, ""); }

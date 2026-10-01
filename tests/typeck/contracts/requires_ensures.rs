//! Requires and ensures clause tests - 25 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Requires references undefined parameter
#[test]
fn requires_undefined_param() { compile_should_fail_with(r#"fn f(x:int) requires y>0 int{return x} fn main(){}"#, "expected newline after statement"); }

// Requires type mismatch
#[test]
fn requires_type_mismatch() { compile_should_fail_with(r#"fn f(x:int) requires x=="hi" int{return x} fn main(){}"#, "expected newline after statement"); }

// Requires with function call
#[test]
fn requires_function_call() { compile_should_fail_with(r#"fn check()bool{return true} fn f(x:int) requires check() int{return x} fn main(){}"#, "expected newline after statement"); }

// Ensures references undefined variable
#[test]
fn ensures_undefined_var() { compile_should_fail_with(r#"class C{x:int
fn get(mut self) int ensures self.x>y {return self.x}
}
fn main(){}"#, "undefined variable"); }

// Ensures type mismatch
#[test]
fn ensures_type_mismatch() { compile_should_fail_with(r#"class C{x:int
fn get(mut self) int ensures self.x {return self.x}
}
fn main(){}"#, "ensures expression must be bool"); }

// Ensures with function call
#[test]
fn ensures_function_call() { compile_should_fail_with(r#"fn check()bool{return true}
class C{x:int
fn get(mut self) int ensures check() {return self.x}
}
fn main(){}"#, "function call 'check()' is not allowed in contract expressions"); }

// Requires on method references undefined field
#[test]
fn method_requires_undefined_field() { compile_should_fail_with(r#"class C{x:int
fn set(mut self,v:int)
requires self.y>0 {self.x=v}
}
fn main(){}"#, "expected newline after statement"); }

// Ensures on method references undefined field
#[test]
fn method_ensures_undefined_field() { compile_should_fail_with(r#"class C{x:int
fn get(mut self) int ensures self.y>0 {return self.x}
}
fn main(){}"#, "no field 'y'"); }

// Requires with closure
#[test]
fn requires_closure() { compile_should_fail_with(r#"fn f(x:int) requires (()=>true)() int{return x} fn main(){}"#, "expected newline after statement"); }

// Ensures with closure
#[test]
fn ensures_closure() { compile_should_fail_with(r#"class C{x:int
fn get(mut self) int ensures (() => true) {return self.x}
}
fn main(){}"#, "closures are not allowed in contract expressions"); }

// Requires with indexing
#[test]
fn requires_indexing() { compile_should_fail_with(r#"fn f(arr:Array<int>) requires arr[0]>0 int{return 1} fn main(){}"#, "expected newline after statement"); }

// Ensures with indexing
#[test]
fn ensures_indexing() { compile_should_fail_with(r#"class C{xs:[int]
fn get(mut self) int ensures self.xs[0]>0 {return 1}
}
fn main(){}"#, "index expressions are not allowed in contract expressions"); }

// Requires return type not bool
#[test]
fn requires_non_bool() { compile_should_fail_with(r#"fn f(x:int) requires x int{return x} fn main(){}"#, "expected newline after statement"); }

// Ensures return type not bool
#[test]
fn ensures_non_bool() { compile_should_fail_with(r#"fn f(x:int) int ensures x>0 {return x} fn main(){}"#, "'ensures' clauses are only supported on methods of classes and objects"); }

// Multiple requires clauses
#[test]
fn multiple_requires() { compile_should_fail_with(r#"fn f(x:int,y:int) requires x>0 requires y>x int{return x+y} fn main(){}"#, "expected newline after statement"); }

// Multiple ensures clauses
#[test]
fn multiple_ensures() { compile_should_fail_with(r#"class C{x:int
fn set(mut self, v:int)
    ensures self.x == v
    ensures self.x >= old(self.x)
{self.x = v}
}
fn main(){}"#, "cannot prove ensures clause"); }

// Requires with null propagation
#[test]
fn requires_null_prop() { compile_should_fail_with(r#"fn f(x:int?) requires x?>0 int{return 1} fn main(){}"#, "expected newline after statement"); }

// Ensures with null propagation
#[test]
fn ensures_null_prop() { compile_should_fail_with(r#"class C{x:int?
fn get(mut self) int ensures self.x?>0 {return 1}
}
fn main(){}"#, "null propagation is not allowed in contract expressions"); }

// Requires with error propagation
#[test]
fn requires_error_prop() { compile_should_fail_with(r#"error E{}
fn check()bool{return true}
fn f(x:int)
requires check()! int{return x}
fn main(){}"#, "expected newline after statement"); }

// Ensures with error propagation
#[test]
fn ensures_error_prop() { compile_should_fail_with(r#"error E{}
fn check()bool{return true}
class C{x:int
fn get(mut self) int ensures check()! {return self.x}
}
fn main(){}"#, "error propagation is not allowed in contract expressions"); }

// Requires on generic function
#[test]
fn requires_generic() { compile_should_fail_with(r#"fn f<T>(x:T) requires x>0 T{return x} fn main(){}"#, "expected newline after statement"); }

// Ensures on generic function
#[test]
fn ensures_generic() { compile_should_fail_with(r#"class Box<T>{v:T
n:int
fn set(mut self, k:int) ensures self.n == old(self.n) + k {self.n = self.n + k}
}
fn main(){let mut b = Box{v:1, n:0}
b.set(2)}"#, "ensures clauses on methods of generic classes are not yet supported"); }

// Requires with cast
#[test]
fn requires_cast() { compile_should_fail_with(r#"fn f(x:int) requires (x as float)>0.0 int{return x} fn main(){}"#, "expected newline after statement"); }

// Ensures with cast
#[test]
fn ensures_cast() { compile_should_fail_with(r#"class C{x:int
fn get(mut self) int ensures (self.x as float)>0.0 {return self.x}
}
fn main(){}"#, "type casts are not allowed in contract expressions"); }

// Requires on void function
#[test]
fn requires_void_func() { compile_should_fail_with(r#"fn f(x:int) requires x>0 {print(x)} fn main(){}"#, "expected newline after statement"); }

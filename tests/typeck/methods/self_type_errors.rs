//! Self type errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Missing self parameter
#[test]
fn missing_self() { compile_should_fail_with(r#"class C{} fn foo(){} fn main(){let c=C{} c.foo()}"#, "expected newline after statement"); }

// Self parameter wrong type
#[test]
fn self_wrong_type() { compile_should_fail_with(r#"class C{
fn foo(self:int){}
}
fn main(){}"#, "expected ,, found :"); }

// Mut self on immutable receiver
#[test]
fn mut_self_immutable() { compile_should_fail_with(r#"class C{x:int
fn foo(mut self){self.x=2}
}
fn main(){let c=C{x:1}c.foo()}"#, "expected newline after statement"); }

// Self as return type
#[test]
fn return_self() { compile_should_fail_with(r#"class C{
fn foo(self)Self{return self}
}
fn main(){}"#, "unknown type 'Self'"); }

// Self in trait method
#[test]
fn trait_self_type() { compile_should_fail_with(r#"trait T{fn foo(self)Self}
class C impl T {
fn foo(self)C{return self}}
fn main(){}"#, "unknown type 'Self'"); }

// Self parameter in non-method
#[test]
fn self_in_function() { compile_should_fail_with(r#"fn f(self){} fn main(){}"#, "expected identifier, found self"); }

// Multiple self parameters
#[test]
fn multiple_self() { compile_should_fail_with(r#"class C{
fn foo(self,self){}
}
fn main(){}"#, "expected identifier, found self"); }

// Self parameter not first
#[test]
fn self_not_first() { compile_should_fail_with(r#"class C{} fn foo(x:int,self){} fn main(){}"#, "expected identifier, found self"); }

// Mut self modifies immutable field
#[test]
fn mut_self_immutable_field() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"class C{x:int
fn foo(mut self){self.x=2}
}
fn main(){}"#).is_ok()); }

// Self in closure
#[test]
fn self_in_closure() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"class C{
fn foo(self){let f=()=>self}
}
fn main(){}"#).is_ok()); }

// Self parameter type annotation mismatch
#[test]
fn self_annotation_mismatch() { compile_should_fail_with(r#"class C1{} class C2{} fn foo(self:C2){} fn main(){}"#, "expected identifier, found self"); }

// Mut self in trait without mut in impl
#[test]
fn trait_mut_impl_non_mut() { compile_should_fail_with(r#"trait T{fn foo(mut self)}
class C impl T {x:int
fn foo(self){}}
fn main(){}"#, "method 'foo' in trait 'T' declares 'mut self', but class 'C' does not"); }

// Non-mut self in trait, mut in impl
#[test]
fn trait_non_mut_impl_mut() { compile_should_fail_with(r#"trait T{fn foo(self)}
class C impl T {x:int
fn foo(mut self){self.x=2}}
fn main(){}"#, "method 'foo' in trait 'T' declares 'self', but class 'C' declares 'mut self'"); }

// Self with explicit type in method
#[test]
fn explicit_self_type() { compile_should_fail_with(r#"class C{
fn foo(self:C){}
}
fn main(){}"#, "expected ,, found :"); }

// Self in generic method
#[test]
fn generic_method_self() { compile_should_fail_with(r#"class C{} fn foo<T>(self,x:T)Self{return self} fn main(){}"#, "expected identifier, found self"); }

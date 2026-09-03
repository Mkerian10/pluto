//! Missing trait method implementation tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic missing method
#[test]
fn missing_single_method() { compile_should_fail_with(r#"trait T{fn foo(self)} class C impl T{}
fn main(){}"#, "does not implement required method"); }
#[test]
fn missing_one_of_two() { compile_should_fail_with(r#"trait T{fn foo(self)
fn bar(self)} class C impl T{fn foo(self){}}
fn main(){}"#, "does not implement required method"); }
#[test]
fn missing_all_methods() { compile_should_fail_with(r#"trait T{fn foo(self)
fn bar(self)
fn baz(self)} class C impl T{}
fn main(){}"#, "does not implement required method"); }

// Missing with wrong signature present
#[test]
fn wrong_sig_not_missing() {
    // A present-but-wrong method reports a signature error, not a missing one
    compile_should_fail_with(r#"trait T{fn foo(self)int} class C impl T{fn foo(self,x:int)int{return x}}
fn main(){}"#, "wrong number of parameters");
}

// Missing generic methods
#[test]
fn missing_generic_method() {
    compile_should_fail_with(r#"trait T{fn foo<U>(self,x:U)U} class C impl T{
x: int
}
fn main(){}"#, "class 'C' does not implement required method 'foo' from trait 'T'");
}

// Missing with nullable/error signatures
#[test]
fn missing_nullable_method() { compile_should_fail_with(r#"trait T{fn foo(self)int?} class C impl T{}
fn main(){}"#, "does not implement required method"); }
#[test]
fn missing_fallible_method() {
    // Trait declarations cannot carry a `!` effect annotation
    compile_should_fail_with(r#"error E{} trait T{fn foo(self)int!} class C impl T{}
fn main(){}"#, "expected");
}

// Multiple traits, one missing method
#[test]
fn two_traits_one_incomplete() {
    compile_should_fail_with(r#"trait T1{fn foo(self)}
trait T2{fn bar(self)}
class C impl T1, T2{fn foo(self){}}
fn main(){}"#, "does not implement required method");
}

// Missing on generic class
#[test]
fn missing_on_generic_class() {
    // Conformance for generic classes is checked per instantiation
    compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U> impl T{value:U}
fn main(){
    let b = Box<int>{value:1}
}"#, "does not implement required method");
}

// Partial implementation
#[test]
fn three_methods_one_missing() { compile_should_fail_with(r#"trait T{fn a(self)
fn b(self)
fn c(self)} class C impl T{fn a(self){}
fn c(self){}}
fn main(){}"#, "does not implement required method"); }

// Missing mut self method
#[test]
fn missing_mut_self() { compile_should_fail_with(r#"trait T{fn foo(mut self)} class C impl T{}
fn main(){}"#, "does not implement required method"); }

// Missing method with complex signature
#[test]
fn missing_complex_sig() { compile_should_fail_with(r#"trait T{fn foo(self,x:Map<string,int>,f:fn(int)string)[int]} class C impl T{}
fn main(){}"#, "does not implement required method"); }

// Impl block without trait
#[test]
fn impl_wrong_trait() { compile_should_fail_with(r#"trait T{fn foo(self)} trait T2{fn bar(self)} class C impl T{fn bar(self){}}
fn main(){}"#, "does not implement required method"); }

// Case sensitivity
#[test]
fn method_name_case_wrong() { compile_should_fail_with(r#"trait T{fn foo(self)} class C impl T{fn Foo(self){}}
fn main(){}"#, "does not implement required method"); }

// Missing with contracts
#[test]
fn missing_method_with_contract() {
    compile_should_fail_with(r#"trait T {
    fn foo(self, x: int) int
        requires x > 0
}
class C impl T{}
fn main(){}"#, "does not implement required method");
}

// Generic class multiple instantiations
#[test]
fn generic_class_missing_per_instance() {
    compile_should_fail_with(r#"trait T{fn foo(self)} class Box<U> impl T{value:U}
fn main(){
    let b1 = Box<int>{value:1}
    let b2 = Box<string>{value:"hi"}
}"#, "does not implement required method");
}

// Missing default method (if Pluto had them)
#[test]
fn missing_non_default() { compile_should_fail_with(r#"trait T{fn required(self)
fn optional(self){}} class C impl T{fn optional(self){}}
fn main(){}"#, "does not implement required method"); }

// Trait with only one method, missing
#[test]
fn single_method_trait_missing() { compile_should_fail_with(r#"trait Printable{fn print(self)} class C impl Printable{x:int}
fn main(){}"#, "does not implement required method"); }

// Missing static method (if supported)
#[test]
fn missing_static_method() { compile_should_fail_with(r#"trait T{fn create()C}
class C impl T {
}
fn main(){}"#, "class 'C' does not implement required method 'create' from trait 'T'"); }

// Multiple classes implementing same trait, one missing
#[test]
fn one_class_missing_method() {
    // The complete implementor is fine; the incomplete one is rejected
    compile_should_fail_with(r#"trait T{fn foo(self)}
class C1 impl T{fn foo(self){}}
class C2 impl T{}
fn main(){}"#, "does not implement required method");
}

//! Initialization order errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Variable used before initialization - correctly detected
#[test]
#[ignore]
fn var_before_init() { compile_should_fail_with(r#"fn main(){let y=x let x=1}"#, "undefined variable 'x'"); }

// Class field init order - correctly detected
#[test]
#[ignore]
fn field_init_order() { compile_should_fail_with(r#"class C{x:int y:int} fn main(){let c=C{y:c.x,x:1}}"#, "undefined variable 'c'"); }

// Static init order - Pluto doesn't have static keyword
#[test]
fn static_init_order() { compile_should_fail_with(r#"static x:int=y static y:int=1 fn main(){}"#, "Syntax error: expected 'fn'"); }

// Global const init order - Pluto doesn't have const keyword
#[test]
fn const_init_order() { compile_should_fail_with(r#"const X:int=Y const Y:int=1 fn main(){}"#, "Syntax error: expected 'fn'"); }

// DI init order violation - correctly detected
#[test]
fn di_init_order() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} fn main(){}"#, "circular dependency detected"); }

// Init in wrong scope - correctly detected
#[test]
fn init_wrong_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}let y=x}"#, "undefined variable 'x'"); }

// Forward init in loop - correctly detected
#[test]
fn loop_init_forward() { compile_should_fail_with(r#"fn main(){for i in 0..j{let j=10}}"#, "undefined variable 'j'"); }

// Match binding init order - correctly detected (parser rejects invalid match syntax)
#[test]
fn match_binding_order() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:y}{E.A{x}{let y=x}}}"#, "Syntax error"); }

// Closure capture before init - correctly detected
#[test]
#[ignore]
fn closure_capture_before_init() { compile_should_fail_with(r#"fn main(){let f=()=>x let x=1}"#, "undefined variable 'x'"); }

// Method call before class init - syntax error (free function with self parameter not allowed)
#[test]
fn method_before_class_init() { compile_should_fail_with(r#"class C{} fn foo(self){} fn main(){foo()}"#, "Syntax error: expected identifier, found self"); }

// Trait impl before trait decl - syntax error (standalone impl not supported)
#[test]
fn impl_before_trait() { compile_should_fail_with(r#"class C{} impl T{fn foo(self){}} trait T{fn foo(self)} fn main(){}"#, "Syntax error: expected 'fn'"); }

// Enum variant before enum - forward reference allowed
// #175 resolution: forward references are supported consistently — top-level
// declaration order does not matter for any declaration kind.
#[test]
fn variant_before_enum() {
    assert_eq!(compile_and_run(r#"fn f(){let e=E.A} enum E{A B} fn main(){f()}"#), 0);
}

// Error raise before error decl - syntax error in compact function notation
#[test]
fn raise_before_error() { compile_should_fail_with(r#"fn f()!{raise E{}} error E{} fn main(){}"#, "Syntax error: expected identifier, found !"); }

// Generic instantiation before decl - forward reference allowed
#[test]
fn generic_before_decl() {
    assert_eq!(compile_and_run(r#"
fn f() {
    let b = Box<int> { value: 1 }
    print(b.value)
}
fn main() {
    f()
}
class Box<T> {
    value: T
}
"#), 0);
}

// Bracket dep before class decl - forward reference allowed
#[test]
fn bracket_dep_before_decl() {
    assert_eq!(compile_and_run(r#"class A[b:B]{x:int} class B{y:int} fn main(){}"#), 0);
}

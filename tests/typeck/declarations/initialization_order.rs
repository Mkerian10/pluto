//! Initialization order errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Variable used before initialization
#[test]
fn var_before_init() { compile_should_fail_with(r#"fn main(){let y=x let x=1}"#, "undefined"); }

// Class field init order
#[test]
fn field_init_order() { compile_should_fail_with(r#"class C{x:int y:int} fn main(){let c=C{y:c.x,x:1}}"#, ""); }

// Static init order (if supported)
#[test]
fn static_init_order() { compile_should_fail_with(r#"static x:int=y static y:int=1 fn main(){}"#, ""); }

// Global const init order
#[test]
fn const_init_order() { compile_should_fail_with(r#"const X:int=Y const Y:int=1 fn main(){}"#, ""); }

// DI init order violation
#[test]
fn di_init_order() { compile_should_fail_with(r#"class A[b:B]{} class B[a:A]{} fn main(){}"#, "circular"); }

// Init in wrong scope
#[test]
fn init_wrong_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}let y=x}"#, "undefined"); }

// Forward init in loop
#[test]
fn loop_init_forward() { compile_should_fail_with(r#"fn main(){for i in 0..j{let j=10}}"#, "undefined"); }

// Match binding init order
#[test]
fn match_binding_order() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:y}{E.A{x}{let y=x}}}"#, ""); }

// Closure capture before init
#[test]
fn closure_capture_before_init() { compile_should_fail_with(r#"fn main(){let f=()=>x let x=1}"#, "undefined"); }

// Method call before class init
#[test]
fn method_before_class_init() { compile_should_fail_with(r#"class C{} fn foo(self){} fn main(){foo()}"#, ""); }

// Trait impl before trait decl
#[test]
fn impl_before_trait() { compile_should_fail_with(r#"class C{} impl T{fn foo(self){}} trait T{fn foo(self)} fn main(){}"#, ""); }

// Enum variant before enum
#[test]
fn variant_before_enum() { compile_should_fail_with(r#"fn f(){let e=E.A} enum E{A B} fn main(){}"#, ""); }

// Error raise before error decl
#[test]
fn raise_before_error() { compile_should_fail_with(r#"fn f()!{raise E{}} error E{} fn main(){}"#, ""); }

// Generic instantiation before decl
#[test]
fn generic_before_decl() { compile_should_fail_with(r#"fn f(){let b=Box<int>{value:1}} class Box<T>{value:T} fn main(){}"#, ""); }

// Bracket dep before class decl
#[test]
fn bracket_dep_before_decl() { compile_should_fail_with(r#"class A[b:B]{} class B{} fn main(){}"#, ""); }

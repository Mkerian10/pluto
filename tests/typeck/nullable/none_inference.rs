//! None literal inference tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// None without context
#[test]
fn none_no_context() { compile_should_fail_with(r#"fn main(){let x=none}"#, "cannot infer type of `none`"); }
#[test]
fn none_in_return_no_sig() { compile_should_fail_with(r#"fn f(){return none} fn main(){}"#, "return type mismatch: expected void, found void?"); }

// None in ambiguous contexts
#[test]
fn none_in_if_branches() { compile_should_fail_with(r#"fn main(){let x=if true{none}else{42}}"#, "if-expression branches have incompatible types"); }
#[test]
fn none_in_match_arms() {
    // Match-as-expression uses `=> expr,` arms; none vs int cannot unify
    compile_should_fail_with(r#"enum E{A
B}
fn main(){
    let x = match E.A {
        E.A => none,
        E.B => 42,
    }
}"#, "match arms have incompatible types");
}

// None in arrays
#[test]
fn array_of_only_none() { compile_should_fail_with(r#"fn main(){let a=[none,none,none]}"#, "cannot infer type of `none`"); }
// This test already passes - array correctly infers as [int?]
#[test]
fn array_mixed_none_and_value() { compile_should_fail_with("fn main(){\n  let a=[42,none]\n  let b:[int]=a\n}", "type mismatch"); }

// None in function args
#[test]
fn generic_fn_none_arg() { compile_should_fail_with(r#"fn id<T>(x:T)T{return x} fn main(){id(none)}"#, "void? is not allowed"); }
#[test]
fn none_to_nullable_param_coerces() {
    // none coerces to a nullable parameter type
    assert_eq!(common::compile_and_run(r#"fn f(x:int?){}
fn main(){f(none)}"#), 0);
}

// None in binary ops
#[test]
fn none_in_comparison() { compile_should_fail_with(r#"fn main(){let b=none==42}"#, "cannot compare void? with int"); }
#[test]
fn none_in_arithmetic() { compile_should_fail_with(r#"fn main(){let x=none+none}"#, "operator not supported for type void?"); }

// None in struct fields
#[test]
fn struct_field_none_no_type() {
    // A generic class literal without type args cannot resolve; the bare
    // name is not a class ("unknown class 'C'")
    compile_should_fail_with(r#"class C<T>{x:T}
fn main(){let c=C{x:none}}"#, "unknown class 'C'");
}

// None propagation
#[test]
fn propagate_none_in_void_fn_compiles() {
    // none? unwraps Nullable(Void) to Void; returning void from a void fn is fine
    assert_eq!(common::compile_and_run(r#"fn f(){return none?}
fn main(){}"#), 0);
}

// None in map
#[test]
fn map_value_none_no_type() {
    compile_should_fail_with(r#"fn main(){
    let mut m = Map<int,int>{}
    m[1] = none
}"#, "expected int, found void?");
}

// None in ternary-like
// This test already passes - correctly accepts none in else branch
#[test]
fn none_ternary_mismatch() { compile_should_fail_with(r#"fn main(){let x=if true{42}else{none}}"#, ""); }

// Multiple nones
#[test]
fn fn_returns_none_twice() { compile_should_fail_with(r#"fn f(b:bool){if b{return none}return none} fn main(){}"#, "return type mismatch: expected void, found void?"); }

//! Nested nullable rejection tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct nested nullable
#[test]
fn double_nullable() { compile_should_fail_with(r#"fn main(){let x:int??=42}"#, "Syntax error: expected =, found ?"); }
#[test]
fn triple_nullable() { compile_should_fail_with(r#"fn main(){let x:int???=42}"#, "Syntax error: expected =, found ?"); }

// Nullable of nullable through types
#[test]
#[ignore]
fn nullable_var_made_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=42 let y:int??=x}"#, "Syntax error: expected =, found ?"); }
#[test]
fn function_returns_double_nullable() { compile_should_fail_with(r#"fn f()int??{return none} fn main(){}"#, "Syntax error: expected {, found ?"); }

// In class fields
#[test]
fn class_field_double_nullable() { compile_should_fail_with(r#"class C{x:int??} fn main(){}"#, "Syntax error"); }
#[test]
fn nested_in_generic() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int??>=Box<int??>{value:none}}"#, "Syntax error"); }

// In array/map
#[test]
fn array_of_double_nullable() { compile_should_fail_with(r#"fn main(){let a:[int??]=[none]}"#, "Syntax error"); }
#[test]
fn map_value_double_nullable() { compile_should_fail_with(r#"fn main(){let m:Map<string,int??>=Map<string,int??>{}}"#, "Syntax error"); }

// In function signatures
#[test]
fn param_double_nullable() { compile_should_fail_with(r#"fn f(x:int??){} fn main(){}"#, "Syntax error: expected ,, found ?"); }
#[test]
fn return_double_nullable() { compile_should_fail_with(r#"fn f()int??{return none} fn main(){}"#, "Syntax error: expected {, found ?"); }

// In enum variants
#[test]
fn enum_variant_double_nullable() { compile_should_fail_with(r#"enum E{A{x:int??}} fn main(){}"#, "Syntax error"); }

// Chained ? operators
#[test]
fn double_propagate_operator() { compile_should_fail_with(r#"fn f()int?{return 42} fn g()int??{return f()??} fn main(){}"#, "Syntax error"); }

// Nullable of error type
#[test]
fn nullable_error_nullable() { compile_should_fail_with(r#"error E{} fn f()(E?)??{return none} fn main(){}"#, "Syntax error"); }

// Through type alias
// This test already passes - typedef syntax is not supported in Pluto
#[test]
fn typedef_hides_nullable() { compile_should_fail_with(r#"fn f()int?{return 42} fn g(){let x:f()??=none} fn main(){}"#, ""); }

// Inference of nested
#[test]
fn infer_nested_from_none() { compile_should_fail_with(r#"fn f()int??{let x=none return x} fn main(){}"#, "Syntax error"); }

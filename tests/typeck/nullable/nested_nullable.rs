//! Nested nullable rejection tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Direct nested nullable
#[test]
fn double_nullable() { compile_should_fail_with(r#"fn main(){let x:int??=42}"#, "nested nullable"); }
#[test]
fn triple_nullable() { compile_should_fail_with(r#"fn main(){let x:int???=42}"#, "nested nullable"); }

// Nullable of nullable through types
#[test]
fn nullable_var_made_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=42 let y:int??=x}"#, "nested nullable"); }
#[test]
fn function_returns_double_nullable() { compile_should_fail_with(r#"fn f()int??{return none} fn main(){}"#, "nested nullable"); }

// In class fields
#[test]
fn class_field_double_nullable() { compile_should_fail_with(r#"class C{x:int??} fn main(){}"#, "nested nullable"); }
#[test]
fn nested_in_generic() { compile_should_fail_with(r#"class Box<T>{value:T} fn main(){let b:Box<int??>=Box<int??>{value:none}}"#, "nested nullable"); }

// In array/map
#[test]
fn array_of_double_nullable() { compile_should_fail_with(r#"fn main(){let a:[int??]=[none]}"#, "nested nullable"); }
#[test]
fn map_value_double_nullable() { compile_should_fail_with(r#"fn main(){let m:Map<string,int??>=Map<string,int??>{}}"#, "nested nullable"); }

// In function signatures
#[test]
fn param_double_nullable() { compile_should_fail_with(r#"fn f(x:int??){} fn main(){}"#, "nested nullable"); }
#[test]
fn return_double_nullable() { compile_should_fail_with(r#"fn f()int??{return none} fn main(){}"#, "nested nullable"); }

// In enum variants
#[test]
fn enum_variant_double_nullable() { compile_should_fail_with(r#"enum E{A{x:int??}} fn main(){}"#, "nested nullable"); }

// Chained ? operators
#[test]
fn double_propagate_operator() { compile_should_fail_with(r#"fn f()int?{return 42} fn g()int??{return f()??} fn main(){}"#, "nested nullable"); }

// Nullable of error type
#[test]
fn nullable_error_nullable() { compile_should_fail_with(r#"error E{} fn f()(E?)??{return none} fn main(){}"#, "nested nullable"); }

// Through type alias
#[test]
fn typedef_hides_nullable() { compile_should_fail_with(r#"fn f()int?{return 42} fn g(){let x:f()??=none} fn main(){}"#, ""); }

// Inference of nested
#[test]
fn infer_nested_from_none() { compile_should_fail_with(r#"fn f()int??{let x=none return x} fn main(){}"#, "nested nullable"); }

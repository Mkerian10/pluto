//! Unary operator type errors - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn negate_array_string() { compile_should_fail_with(r#"fn main(){let x=-[true]}"#, "cannot negate type [bool]"); }
#[test]
fn negate_bool() { compile_should_fail_with(r#"fn main(){let x=-true}"#, "cannot negate type bool"); }
#[test]
fn negate_array() { compile_should_fail_with(r#"fn main(){let x=-[1,2,3]}"#, "cannot negate type [int]"); }
#[test]
fn not_int() { compile_should_fail_with(r#"fn main(){let x=!42}"#, "cannot apply '!' to type int"); }
#[test]
fn not_float() { compile_should_fail_with(r#"fn main(){let x=!3.14}"#, "cannot apply '!' to type float"); }
#[test]
fn not_array() { compile_should_fail_with(r#"fn main(){let x=![1,2,3]}"#, "cannot apply '!' to type [int]"); }
#[test]
fn bitwise_not_bool() { compile_should_fail_with(r#"fn main(){let x=~true}"#, "cannot apply '~' to type bool"); }
#[test]
fn bitwise_not_float() { compile_should_fail_with(r#"fn main(){let x=~3.14}"#, "cannot apply '~' to type float"); }
#[test]
fn negate_nullable() { compile_should_fail_with(r#"fn main(){let x:int?=5 let y=-x}"#, "cannot negate type int?"); }
#[test]
fn not_nullable() { compile_should_fail_with(r#"fn main(){let x:bool?=true let y=!x}"#, "cannot apply '!' to type bool?"); }

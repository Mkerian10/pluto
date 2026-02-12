//! Select statement error tests - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Select without default is fallible
#[test] fn select_no_default_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() select{c.receiver=>let x=it}}"#, "unhandled error"); }
#[test] fn select_multiple_arms_no_default() { compile_should_fail_with(r#"fn main(){let c1=chan<int>() let c2=chan<string>() select{c1.receiver=>let x=it c2.receiver=>let y=it}}"#, "unhandled error"); }
#[test] fn select_no_default_in_function() { compile_should_fail_with(r#"fn f(){let c=chan<int>() select{c.receiver=>let x=it}} fn main(){}"#, "unhandled error"); }

// Select without default needs propagation
#[test] fn select_no_default_return_value() { compile_should_fail_with(r#"fn f()int{let c=chan<int>() select{c.receiver=>return it} return 0} fn main(){}"#, "unhandled error"); }
#[test] fn select_no_default_in_assignment() { compile_should_fail_with(r#"fn main(){let c=chan<int>() let x=select{c.receiver=>it}}"#, "unhandled error"); }

// Select with sender (also fallible without default)
#[test] fn select_sender_no_default() { compile_should_fail_with(r#"fn main(){let c=chan<int>() select{c.sender=>c.sender.send(42)}}"#, "unhandled error"); }

// Select in expressions
#[test] fn select_in_binop_no_default() { compile_should_fail_with(r#"fn main(){let c=chan<int>() let x=1+select{c.receiver=>it}}"#, "unhandled error"); }
#[test] fn select_in_array_no_default() { compile_should_fail_with(r#"fn main(){let c=chan<int>() let arr=[select{c.receiver=>it},2,3]}"#, "unhandled error"); }

// Select with multiple types (type checking + error checking)
#[test] fn select_mixed_types_no_default() { compile_should_fail_with(r#"fn main(){let c1=chan<int>() let c2=chan<string>() let x=select{c1.receiver=>it c2.receiver=>it}}"#, ""); }

// Nested select
#[test] fn nested_select_no_default() { compile_should_fail_with(r#"fn main(){let c1=chan<int>() let c2=chan<int>() select{c1.receiver=>select{c2.receiver=>let y=it}}}"#, "unhandled error"); }

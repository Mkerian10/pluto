//! Fallible builtin tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// pow() with negative exponent
#[test]
fn pow_negative_exp_no_handler() { compile_should_fail_with(r#"fn main(){let x=pow(5,-2)}"#, "call to fallible function"); }
#[test]
fn pow_negative_exp_needs_propagate() { compile_should_fail_with(r#"fn f()int{return pow(5,-2)} fn main(){}"#, "call to fallible function"); }
#[test]
fn pow_negative_exp_with_propagate() { compile_should_fail_with(r#"fn f()int{return pow(5,-2)!} fn main(){f()}"#, "call to fallible function"); }
#[test]
fn pow_in_binop_no_handler() { compile_should_fail_with(r#"fn main(){let x=pow(2,-3)+10}"#, "call to fallible function"); }

// Channel send/recv fallibility
#[test]
#[ignore]
fn send_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) tx.send(42)}"#, "call to fallible method"); }
#[test]
#[ignore]
fn recv_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) rx.recv()}"#, "call to fallible method"); }
#[test]
#[ignore]
fn try_send_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) tx.try_send(42)}"#, "call to fallible method"); }
#[test]
#[ignore]
fn try_recv_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) rx.try_recv()}"#, "call to fallible method"); }

// Channel operations in expressions
#[test]
#[ignore]
fn recv_in_assignment_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) let x=rx.recv()}"#, "call to fallible method"); }
#[test]
#[ignore]
fn send_in_function_no_handler() { compile_should_fail_with(r#"fn f(){let (tx,rx)=chan<int>(1) tx.send(42)} fn main(){}"#, "call to fallible method"); }
#[test]
#[ignore]
fn recv_with_propagate_wrong_sig() { compile_should_fail_with(r#"fn f()int{let (tx,rx)=chan<int>(1) return rx.recv()!} fn main(){f()}"#, "call to fallible function"); }

// Mixed pow and channel errors
#[test]
#[ignore]
fn pow_and_chan_same_function() { compile_should_fail_with(r#"fn main(){let x=pow(2,-1) let (tx,rx)=chan<int>(1) tx.send(x)}"#, "call to fallible function"); }
#[test]
#[ignore]
fn fallible_builtins_in_if() { compile_should_fail_with(r#"fn main(){if true{let x=pow(2,-1)}else{let (tx,rx)=chan<int>(1) tx.send(42)}}"#, "call to fallible function"); }

// Channel close (not fallible but used with fallible ops)
#[test]
#[ignore]
fn recv_after_close_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) tx.close() rx.recv()}"#, "call to fallible method"); }
#[test]
#[ignore] // #167: select expressions don't enforce error handling for fallible operations
fn select_no_default_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1) select{val=rx.recv(){print(val)}}}"#, "must be handled"); }

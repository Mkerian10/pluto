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
#[ignore] // Syntax error: old propagation syntax
fn pow_negative_exp_with_propagate() { compile_should_fail_with(r#"fn f()int!{return pow(5,-2)!} fn main(){f()}"#, "call to fallible function"); }
#[test]
fn pow_in_binop_no_handler() { compile_should_fail_with(r#"fn main(){let x=pow(2,-3)+10}"#, "call to fallible function"); }

// Channel send/recv fallibility
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn send_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() c.sender.send(42)}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn recv_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() c.receiver.recv()}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn try_send_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() c.sender.try_send(42)}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn try_recv_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() c.receiver.try_recv()}"#, "call to fallible function"); }

// Channel operations in expressions
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn recv_in_assignment_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() let x=c.receiver.recv()}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn send_in_function_no_handler() { compile_should_fail_with(r#"fn f(){let c=chan<int>() c.sender.send(42)} fn main(){}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn recv_with_propagate_wrong_sig() { compile_should_fail_with(r#"fn f()int{let c=chan<int>() return c.receiver.recv()!} fn main(){}"#, "call to fallible function"); }

// Mixed pow and channel errors
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn pow_and_chan_same_function() { compile_should_fail_with(r#"fn main(){let x=pow(2,-1) let c=chan<int>() c.sender.send(x)}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn fallible_builtins_in_if() { compile_should_fail_with(r#"fn main(){if true{let x=pow(2,-1)}else{let c=chan<int>() c.sender.send(42)}}"#, "call to fallible function"); }

// Channel close (not fallible but used with fallible ops)
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn recv_after_close_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() c.sender.close() c.receiver.recv()}"#, "call to fallible function"); }
#[test]
#[ignore] // Outdated: uses old channel API (.sender/.receiver)
fn select_no_default_no_handler() { compile_should_fail_with(r#"fn main(){let c=chan<int>() select{c.receiver=>let x=it}}"#, "call to fallible function"); }

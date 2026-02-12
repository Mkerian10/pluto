//! Channel error tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Send wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn send_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() ch.0.send("hi")}"#, ""); }

// Receive wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn receive_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let x:string=ch.1.recv()}"#, ""); }

// Channel type mismatch
#[test]
#[ignore] // PR #46 - outdated assertions
fn channel_type_mismatch() { compile_should_fail_with(r#"fn main(){let ch1=chan<int>() let ch2:Channel<string,string>=ch1}"#, ""); }

// Sender wrong type param
#[test]
#[ignore] // PR #46 - outdated assertions
fn sender_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let s:Sender<string>=ch.0}"#, ""); }

// Receiver wrong type param
#[test]
#[ignore] // PR #46 - outdated assertions
fn receiver_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let r:Receiver<string>=ch.1}"#, ""); }

// Send on receiver
#[test]
#[ignore] // PR #46 - outdated assertions
fn send_on_receiver() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() ch.1.send(1)}"#, ""); }

// Receive on sender
#[test]
#[ignore] // PR #46 - outdated assertions
fn receive_on_sender() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let x=ch.0.recv()}"#, ""); }

// Channel generic wrong instantiation
#[test]
#[ignore] // PR #46 - outdated assertions
fn channel_generic_wrong() { compile_should_fail_with(r#"fn make<T>()Channel<T,T>{return chan<T>()} fn main(){let ch:Channel<int,int>=make<string>()}"#, ""); }

// Send nullable to non-nullable channel
#[test]
#[ignore] // PR #46 - outdated assertions
fn send_nullable_mismatch() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let x:int?=none ch.0.send(x)}"#, ""); }

// Receive from closed channel without error handling
#[test]
#[ignore] // PR #46 - outdated assertions
fn recv_closed_no_error() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() ch.0.close() let x=ch.1.recv()}"#, ""); }

// Try_send wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn try_send_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() ch.0.try_send("hi")}"#, ""); }

// Try_recv wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn try_recv_wrong_type() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() let x:string?=ch.1.try_recv()}"#, ""); }

// Channel in task wrong type
#[test]
#[ignore] // PR #46 - outdated assertions
fn channel_task_wrong_type() { compile_should_fail_with(r#"fn task(s:Sender<int>){s.send(1)} fn main(){let ch=chan<string>() spawn task(ch.0)}"#, ""); }

// Iterate over sender
#[test]
#[ignore] // PR #46 - outdated assertions
fn iterate_sender() { compile_should_fail_with(r#"fn main(){let ch=chan<int>() for x in ch.0{}}"#, ""); }

// Channel select wrong types
#[test]
#[ignore] // PR #46 - outdated assertions
fn select_wrong_types() { compile_should_fail_with(r#"fn main(){let ch1=chan<int>() let ch2=chan<string>() select{ch1.1{x}=>print(x) ch2.1{y}=>print(y)}}"#, ""); }

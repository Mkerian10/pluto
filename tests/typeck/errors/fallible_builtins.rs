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
fn send_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
tx.send(42)}"#, "call to fallible method"); }
#[test]
fn recv_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
rx.recv()}"#, "call to fallible method"); }
#[test]
fn try_send_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
tx.try_send(42)}"#, "call to fallible method"); }
#[test]
fn try_recv_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
rx.try_recv()}"#, "call to fallible method"); }

// Channel operations in expressions
#[test]
fn recv_in_assignment_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
let x=rx.recv()}"#, "call to fallible method"); }
#[test]
fn send_in_function_no_handler() { compile_should_fail_with(r#"fn f(){let (tx,rx)=chan<int>(1)
tx.send(42)}
fn main(){}"#, "call to fallible method"); }
#[test]
fn recv_with_propagate_wrong_sig() { compile_should_fail_with(r#"fn f()int{let (tx,rx)=chan<int>(1)
return rx.recv()!}
fn main(){f()}"#, "call to fallible function"); }

// Mixed pow and channel errors
#[test]
fn pow_and_chan_same_function() { compile_should_fail_with(r#"fn main(){let x=pow(2,-1)
let (tx,rx)=chan<int>(1)
tx.send(x)}"#, "call to fallible function"); }
#[test]
fn fallible_builtins_in_if() { compile_should_fail_with(r#"fn main(){if true{let x=pow(2,-1)}else{let (tx,rx)=chan<int>(1)
tx.send(42)}}"#, "call to fallible function"); }

// Channel close (not fallible but used with fallible ops)
#[test]
fn recv_after_close_no_handler() { compile_should_fail_with(r#"fn main(){let (tx,rx)=chan<int>(1)
tx.close()
rx.recv()}"#, "call to fallible method"); }
// #167 resolution: a select arm's operation only fires when ready, so the
// fallible thing is the select statement itself (ChannelClosed when every
// channel closes). Inside a function that makes the *function* fallible and
// callers must handle it (see channels.rs select_all_closed_error). In `main`
// there is no call site to enforce — the escape is caught at runtime instead:
// the process reports "unhandled error escaped main: ChannelClosed" and exits
// nonzero (channels.rs select_all_closed_escaping_main_fails_process).
#[test]
fn select_in_fallible_fn_enforced_at_call_site() {
    compile_should_fail_with(
        r#"
fn pick(rx: Receiver<int>) int {
    select {
        val = rx.recv() {
            return val
        }
    }
    return 0
}

fn main() {
    let (tx, rx) = chan<int>(1)
    print(pick(rx))
}
"#,
        "must be handled",
    );
}

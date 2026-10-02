mod common;
use common::*;

// ── Basic operations ────────────────────────────────────────────────────────

#[test]
fn chan_send_recv_int() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(42)!
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn chan_send_recv_string() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<string>(1)
    tx.send("hello")!
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "hello");
}

#[test]
fn chan_send_recv_float() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<float>(1)
    tx.send(3.14)!
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "3.14");
}

#[test]
fn chan_send_recv_bool() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<bool>(1)
    tx.send(true)!
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "true");
}

#[test]
fn chan_multiple_values() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(3)
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
    print(rx.recv()!)
    print(rx.recv()!)
    print(rx.recv()!)
}
"#);
    assert_eq!(out.trim(), "10\n20\n30");
}

#[test]
fn chan_different_capacities() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx1, rx1) = chan<int>(1)
    tx1.send(1)!
    print(rx1.recv()!)

    let (tx10, rx10) = chan<int>(10)
    for i in 0..10 {
        tx10.send(i)!
    }
    let mut sum = 0
    for i in 0..10 {
        sum = sum + rx10.recv()!
    }
    print(sum)
}
"#);
    assert_eq!(out.trim(), "1\n45");
}

// ── Blocking + concurrency ──────────────────────────────────────────────────

#[test]
fn chan_unbuffered_spawn_producer() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(99)!
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn produce(tx).detach()
    let val = rx.recv()!
    print(val)
}
"#, 15);
    assert_eq!(out.trim(), "99");
}

#[test]
fn chan_buffered_spawn_producer_consumer() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

fn main() {
    let (tx, rx) = chan<int>(3)
    spawn produce(tx).detach()
    let a = rx.recv()!
    let b = rx.recv()!
    let c = rx.recv()!
    print(a + b + c)
}
"#, 15);
    assert_eq!(out.trim(), "6");
}

#[test]
fn chan_unbuffered_multiple_items() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn produce(tx).detach()
    print(rx.recv()!)
    print(rx.recv()!)
    print(rx.recv()!)
}
"#, 15);
    assert_eq!(out.trim(), "10\n20\n30");
}

#[test]
fn chan_fifo_order() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    for i in 0..5 {
        tx.send(i)!
    }
}

fn main() {
    let (tx, rx) = chan<int>(5)
    spawn produce(tx).detach()
    for i in 0..5 {
        print(rx.recv()!)
    }
}
"#, 15);
    assert_eq!(out.trim(), "0\n1\n2\n3\n4");
}

// ── Close behavior ──────────────────────────────────────────────────────────

#[test]
fn chan_close_then_recv_error() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let val = rx.recv() catch 0
    print(val)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn chan_send_after_close_error() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    tx.send(1) catch e { print("send caught") }
}
"#);
    assert_eq!(out.trim(), "send caught");
}

#[test]
fn chan_close_wakes_blocked_receiver() {
    let out = compile_and_run_stdout_timeout(r#"
fn closer(tx: Sender<int>) {
    tx.close()
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn closer(tx).detach()
    let val = rx.recv() catch -1
    print(val)
}
"#, 15);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn chan_buffered_close_drain_then_error() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(3)
    tx.send(1)!
    tx.send(2)!
    tx.close()
    print(rx.recv()!)
    print(rx.recv()!)
    let val = rx.recv() catch -1
    print(val)
}
"#);
    assert_eq!(out.trim(), "1\n2\n-1");
}

// ── Non-blocking (try_send / try_recv) ──────────────────────────────────────

#[test]
fn chan_try_send_success() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.try_send(42)!
    print(rx.recv()!)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn chan_try_send_full() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.try_send(1)!
    tx.try_send(2) catch e { print("full") }
}
"#);
    assert_eq!(out.trim(), "full");
}

#[test]
fn chan_try_send_closed() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    tx.try_send(1) catch e { print("closed") }
}
"#);
    assert_eq!(out.trim(), "closed");
}

#[test]
fn chan_try_recv_success() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(77)!
    let val = rx.try_recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "77");
}

#[test]
fn chan_try_recv_empty() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let val = rx.try_recv() catch -1
    print(val)
}
"#);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn chan_try_recv_closed_empty() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let val = rx.try_recv() catch -1
    print(val)
}
"#);
    assert_eq!(out.trim(), "-1");
}

// ── For-in on Receiver ──────────────────────────────────────────────────────

#[test]
fn chan_for_in_receiver() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.close()
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn produce(tx).detach()
    for val in rx {
        print(val)
    }
    print("done")
}
"#, 15);
    assert_eq!(out.trim(), "1\n2\n3\ndone");
}

#[test]
fn chan_for_in_break() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.close()
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn produce(tx).detach()
    for val in rx {
        if val == 2 {
            break
        }
        print(val)
    }
    print("broke out")
}
"#, 15);
    assert_eq!(out.trim(), "1\nbroke out");
}

#[test]
fn chan_for_in_continue() {
    let out = compile_and_run_stdout_timeout(r#"
fn produce(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.close()
}

fn main() {
    let (tx, rx) = chan<int>()
    spawn produce(tx).detach()
    for val in rx {
        if val == 2 {
            continue
        }
        print(val)
    }
}
"#, 15);
    assert_eq!(out.trim(), "1\n3");
}

#[test]
fn chan_for_in_empty_closed() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    for val in rx {
        print(val)
    }
    print("zero iterations")
}
"#);
    assert_eq!(out.trim(), "zero iterations");
}

// ── Error handling ──────────────────────────────────────────────────────────

#[test]
fn chan_propagate_send() {
    let out = compile_and_run_stdout(r#"
fn try_send(tx: Sender<int>) {
    tx.send(42)!
}

fn main() {
    let (tx, rx) = chan<int>(1)
    try_send(tx)!
    print(rx.recv()!)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn chan_propagate_recv() {
    let out = compile_and_run_stdout(r#"
fn try_recv(rx: Receiver<int>) int {
    return rx.recv()!
}

fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(55)!
    let val = try_recv(rx)!
    print(val)
}
"#);
    assert_eq!(out.trim(), "55");
}

#[test]
fn chan_catch_with_handler() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let val = rx.recv() catch e { -99 }
    print(val)
}
"#);
    assert_eq!(out.trim(), "-99");
}

#[test]
fn chan_bare_send_compile_fail() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(42)
}
"#, "must be handled with ! or catch");
}

#[test]
fn chan_bare_recv_compile_fail() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    rx.recv()
}
"#, "must be handled with ! or catch");
}

// ── Type errors ─────────────────────────────────────────────────────────────

#[test]
fn chan_wrong_type_compile_fail() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send("hello")!
}
"#, "expects int, found string");
}

#[test]
fn chan_non_int_capacity_compile_fail() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>("big")
}
"#, "capacity must be int");
}

#[test]
fn chan_unknown_method_compile_fail() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.foo()
}
"#, "no method");
}

// ── Multi-task ──────────────────────────────────────────────────────────────

#[test]
fn chan_fan_in_multiple_senders() {
    let out = compile_and_run_stdout_timeout(r#"
fn send_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

fn main() {
    let (tx, rx) = chan<int>(3)
    spawn send_val(tx, 10).detach()
    spawn send_val(tx, 20).detach()
    spawn send_val(tx, 30).detach()
    let mut sum = 0
    for i in 0..3 {
        sum = sum + rx.recv()!
    }
    print(sum)
}
"#, 15);
    assert_eq!(out.trim(), "60");
}

#[test]
fn chan_as_function_arg() {
    let out = compile_and_run_stdout(r#"
fn send_value(tx: Sender<string>) {
    tx.send("from function")!
}

fn recv_value(rx: Receiver<string>) string {
    return rx.recv()!
}

fn main() {
    let (tx, rx) = chan<string>(1)
    send_value(tx)!
    let val = recv_value(rx)!
    print(val)
}
"#);
    assert_eq!(out.trim(), "from function");
}

#[test]
fn chan_shorthand_catch() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let val = rx.recv() catch 0
    print(val)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn chan_unbuffered_default_capacity() {
    // chan<T>() with no capacity arg should use capacity 1 (handoff)
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>()
    tx.send(100)!
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "100");
}

// ── Sender reference counting & auto-close ────────────────────────────────

#[test]
fn chan_auto_close_basic() {
    // LetChan in helper fn, return without close -> auto-close on exit
    let out = compile_and_run_stdout(r#"
fn helper() int {
    let (tx, rx) = chan<int>(2)
    tx.send(42)!
    let val = rx.recv()!
    // no tx.close() — sender_dec on function exit auto-closes
    return val
}

fn main() {
    let result = helper() catch 0
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn chan_auto_close_with_spawn() {
    // spawn producer(tx); tx.close(); for val in rx { ... } — terminates correctly
    let out = compile_and_run_stdout_timeout(r#"
fn producer(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

fn main() {
    let (tx, rx) = chan<int>(10)
    spawn producer(tx).detach()
    tx.close()
    let mut sum = 0
    for val in rx {
        sum = sum + val
    }
    print(sum)
}
"#, 15);
    assert_eq!(out.trim(), "6");
}

#[test]
fn chan_multiple_spawn_refs() {
    // Two spawn worker(tx) calls — channel closes only when both finish
    let out = compile_and_run_stdout_timeout(r#"
fn worker(tx: Sender<int>, value: int) {
    tx.send(value)!
}

fn main() {
    let (tx, rx) = chan<int>(10)
    spawn worker(tx, 10).detach()
    spawn worker(tx, 20).detach()
    tx.close()
    let mut sum = 0
    for val in rx {
        sum = sum + val
    }
    print(sum)
}
"#, 15);
    assert_eq!(out.trim(), "30");
}

#[test]
fn chan_early_return_before_letchan() {
    // Pre-declared null safely skipped by null guard in sender_dec
    let out = compile_and_run_stdout(r#"
fn maybe_create(flag: bool) int {
    if flag {
        return 99
    }
    let (tx, rx) = chan<int>(1)
    tx.send(1)!
    return rx.recv()!
}

fn main() {
    let result = maybe_create(true) catch 0
    print(result)
}
"#);
    assert_eq!(out.trim(), "99");
}

#[test]
fn chan_explicit_close_plus_exit_block() {
    // tx.close() then function exit — double-dec with underflow guard
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(2)
    tx.send(42)!
    tx.close()
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn chan_non_spawn_closure_capturing_sender() {
    // Regular closure captures sender, no inc/dec per call, closes at fn exit.
    // The closure propagates ChannelClosed (`tx.send(x)!`), so it is inferred
    // fallible and its calls must be handled.
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(10)
    let send_val = (x: int) => { tx.send(x)! }
    send_val(1)!
    send_val(2)!
    send_val(3)!
    let v1 = rx.recv()!
    let v2 = rx.recv()!
    let v3 = rx.recv()!
    let sum = v1 + v2 + v3
    print(sum)
}
"#);
    assert_eq!(out.trim(), "6");
}

#[test]
fn chan_sender_reassignment_compile_error() {
    // Reassigning a Sender variable should be a type error
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let (tx2, rx2) = chan<int>(1)
    tx = tx2
}
"#, "cannot reassign channel sender/receiver variable");
}

#[test]
fn chan_receiver_reassignment_compile_error() {
    // Reassigning a Receiver variable should be a type error
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let (tx2, rx2) = chan<int>(1)
    rx = rx2
}
"#, "cannot reassign channel sender/receiver variable");
}

// ── Select statement ──────────────────────────────────────────────────────

#[test]
fn select_recv_basic() {
    // One channel with data ready — select should pick it
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(42)!
    select {
        val = rx.recv() {
            print(val)
        }
    }
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn select_send_basic() {
    // Send arm — select should send and execute the body
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        tx.send(99) {
            print("sent")
        }
    }
    let val = rx.recv()!
    print(val)
}
"#);
    assert_eq!(out.trim(), "sent\n99");
}

#[test]
fn select_default_no_ready() {
    // No channels ready, default arm executes
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        val = rx.recv() {
            print(val)
        }
        default {
            print("nothing")
        }
    }
}
"#);
    assert_eq!(out.trim(), "nothing");
}

#[test]
fn select_default_with_ready_channel() {
    // Channel has data — should pick recv arm, not default
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(7)!
    select {
        val = rx.recv() {
            print(val)
        }
        default {
            print("default")
        }
    }
}
"#);
    assert_eq!(out.trim(), "7");
}

#[test]
fn select_blocks_until_ready() {
    // Without default, select blocks until a channel has data
    let out = compile_and_run_stdout_timeout(r#"
fn producer(tx: Sender<int>) {
    tx.send(123)!
}

fn main() {
    let (tx, rx) = chan<int>(1)
    spawn producer(tx).detach()
    select {
        val = rx.recv() {
            print(val)
        }
    }
}
"#, 15);
    assert_eq!(out.trim(), "123");
}

#[test]
fn select_all_closed_error() {
    // All channels closed without default → ChannelClosed error propagates
    let out = compile_and_run_stdout(r#"
fn try_select(rx: Receiver<int>) int {
    select {
        val = rx.recv() {
            return val
        }
    }
    return 0
}

fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let result = try_select(rx) catch -1
    print(result)
}
"#);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn select_all_closed_with_default() {
    // All channels closed but default exists — executes default, no error
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    select {
        val = rx.recv() {
            print("recv")
        }
        default {
            print("closed-default")
        }
    }
}
"#);
    assert_eq!(out.trim(), "closed-default");
}

#[test]
fn select_fan_in_loop() {
    // Fan-in: two producers, one consumer using select in a loop
    let out = compile_and_run_stdout_timeout(r#"
fn producer(tx: Sender<int>, start: int) {
    tx.send(start)!
    tx.send(start + 1)!
}

fn do_select(rx1: Receiver<int>, rx2: Receiver<int>) int {
    let mut sum = 0
    let mut count = 0
    while count < 4 {
        select {
            v1 = rx1.recv() {
                sum = sum + v1
                count = count + 1
            }
            v2 = rx2.recv() {
                sum = sum + v2
                count = count + 1
            }
        }
    }
    return sum
}

fn main() {
    let (tx1, rx1) = chan<int>(10)
    let (tx2, rx2) = chan<int>(10)
    spawn producer(tx1, 10).detach()
    spawn producer(tx2, 20).detach()
    tx1.close()
    tx2.close()

    let sum = do_select(rx1, rx2) catch 0
    print(sum)
}
"#, 15);
    // 10 + 11 + 20 + 21 = 62, or could be less if ChannelClosed fires before all received
    let val: i64 = out.trim().parse().unwrap();
    assert!(val > 0, "expected positive sum, got {val}");
}

#[test]
fn select_multiple_recv_arms() {
    // Two channels, both with data — select picks one (non-deterministic, just verify it works)
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx1, rx1) = chan<int>(1)
    let (tx2, rx2) = chan<int>(1)
    tx1.send(1)!
    tx2.send(2)!
    let mut result = 0
    select {
        v1 = rx1.recv() {
            result = v1
        }
        v2 = rx2.recv() {
            result = v2
        }
    }
    print(result)
}
"#);
    let val: i64 = out.trim().parse().unwrap();
    assert!(val == 1 || val == 2, "expected 1 or 2, got {val}");
}

#[test]
fn select_recv_wrong_type_compile_fail() {
    // Select recv on a Sender should fail
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        val = tx.recv() {
            print(val)
        }
    }
}
"#, "Receiver");
}

#[test]
fn select_send_wrong_type_compile_fail() {
    // Select send on a Receiver should fail
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        rx.send(42) {
            print("sent")
        }
    }
}
"#, "Sender");
}

// ── Unhandled-error exit check + typed runtime errors (#167) ─────────────────

#[test]
fn select_all_closed_escaping_main_fails_process() {
    // A select with no default whose channels are all closed raises
    // ChannelClosed. Inside a function, that makes the function fallible and
    // callers must handle it (covered above). When it escapes `main` itself
    // there is no call site to enforce — instead of vanishing silently, the
    // process reports the error and exits nonzero.
    let (_out, err, code) = compile_and_run_output(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    select {
        val = rx.recv() {
            print(val)
        }
    }
}
"#);
    assert_eq!(code, 1, "unhandled escape must fail the process; stderr: {err}");
    assert!(
        err.contains("unhandled error escaped main: ChannelClosed"),
        "stderr must name the escaped error; got: {err}"
    );
}

#[test]
fn typed_catch_matches_runtime_channel_error() {
    // Runtime-raised channel errors now carry their type name, so a typed
    // catch can discriminate them (previously the type was unset and typed
    // handlers could never match).
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let v = rx.recv() catch err: ChannelClosed {
        -7
    }
    print(v)
}
"#);
    assert_eq!(out.trim(), "-7");
}

#[test]
fn typed_catch_matches_channel_full() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(1)!
    tx.try_send(2) catch err: ChannelFull {
        print(-8)
    }
    print(rx.recv()!)
}
"#);
    assert_eq!(out.trim(), "-8\n1");
}

// ── recv_timeout ────────────────────────────────────────────────────────────

#[test]
fn recv_timeout_delivers_buffered_value() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(42)!
    let v = rx.recv_timeout(1000)!
    print(v)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn recv_timeout_delivers_value_from_producer() {
    // The deadline must not fire when a producer delivers in time.
    let out = compile_and_run_stdout(r#"
fn produce(tx: Sender<int>) {
    tx.send(7)!
}

fn main() {
    let (tx, rx) = chan<int>(0)
    spawn produce(tx).detach()
    let v = rx.recv_timeout(5000)!
    print(v)
}
"#);
    assert_eq!(out.trim(), "7");
}

#[test]
fn recv_timeout_raises_timed_out_on_quiet_channel() {
    // A quiet (open, empty) channel raises TimedOut after the deadline — and
    // the wait really is timed: it must block for roughly the deadline, not
    // return immediately.
    let out = compile_and_run_stdout(r#"
extern fn __pluto_time_ns() int

fn main() {
    let (tx, rx) = chan<int>(1)
    let start = __pluto_time_ns()
    let v = rx.recv_timeout(100) catch err: TimedOut { -1 }
    let elapsed_ms = (__pluto_time_ns() - start) / 1000000
    print(v)
    if elapsed_ms >= 80 {
        print("waited")
    } else {
        print(f"too fast: {elapsed_ms}ms")
    }
    tx.close()
}
"#);
    assert_eq!(out.trim(), "-1\nwaited");
}

#[test]
fn recv_timeout_channel_closed_wins() {
    // A closed-and-drained channel is a definite state, not a bounded wait:
    // ChannelClosed is raised, never TimedOut.
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let v = rx.recv_timeout(1000) catch err: TimedOut { -1 } catch err: ChannelClosed { -2 }
    print(v)
}
"#);
    assert_eq!(out.trim(), "-2");
}

#[test]
fn recv_timeout_drains_buffer_before_closed() {
    // Buffered values still drain from a closed channel before ChannelClosed.
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(2)
    tx.send(1)!
    tx.send(2)!
    tx.close()
    print(rx.recv_timeout(1000)!)
    print(rx.recv_timeout(1000)!)
    let v = rx.recv_timeout(1000) catch err: ChannelClosed { -2 }
    print(v)
}
"#);
    assert_eq!(out.trim(), "1\n2\n-2");
}

#[test]
fn recv_timeout_requires_error_handling() {
    // recv_timeout is fallible ({ChannelClosed, TimedOut}) — an unhandled
    // call is a compile error like recv.
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let v = rx.recv_timeout(100)
    print(v)
}
"#, "call to fallible method 'recv_timeout' must be handled");
}

#[test]
fn recv_timeout_ms_must_be_int() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let v = rx.recv_timeout("soon")!
    print(v)
}
"#, "recv_timeout() expects int milliseconds");
}

// ── select `after` arm ──────────────────────────────────────────────────────

#[test]
fn select_after_fires_on_quiet_channel() {
    // Raft-shaped loop: wait for heartbeats with an election timeout; a
    // quiet channel trips the after arm.
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let mut elections = 0
    let mut running = true
    while running {
        select {
            hb = rx.recv() {
                print(f"heartbeat {hb}")
            }
            after 50 {
                elections = elections + 1
                if elections == 2 {
                    running = false
                }
            }
        }
    }
    print(f"elections {elections}")
    tx.close()
}
"#);
    assert_eq!(out.trim(), "elections 2");
}

#[test]
fn select_after_does_not_fire_when_arm_ready() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(5)!
    select {
        v = rx.recv() {
            print(f"got {v}")
        }
        after 5000 {
            print("should not fire")
        }
    }
}
"#);
    assert_eq!(out.trim(), "got 5");
}

#[test]
fn select_after_waits_roughly_the_deadline() {
    let out = compile_and_run_stdout(r#"
extern fn __pluto_time_ns() int

fn main() {
    let (tx, rx) = chan<int>(1)
    let start = __pluto_time_ns()
    select {
        v = rx.recv() {
            print(v)
        }
        after 100 {
            let elapsed_ms = (__pluto_time_ns() - start) / 1000000
            if elapsed_ms >= 80 {
                print("waited")
            } else {
                print(f"too fast: {elapsed_ms}ms")
            }
        }
    }
    tx.close()
}
"#);
    assert_eq!(out.trim(), "waited");
}

#[test]
fn select_after_duration_reevaluated_each_entry() {
    // The deadline expression runs on EVERY select entry (the Raft
    // randomized-window requirement): a side-effecting duration function
    // must be called once per loop iteration.
    let out = compile_and_run_stdout(r#"
fn next_window() int {
    print("eval")
    return 30
}

fn main() {
    let (tx, rx) = chan<int>(1)
    let mut i = 0
    while i < 3 {
        select {
            v = rx.recv() {
                print(v)
            }
            after next_window() {
                i = i + 1
            }
        }
    }
    tx.close()
}
"#);
    assert_eq!(out.trim(), "eval\neval\neval");
}

#[test]
fn select_after_taking_arm_is_not_an_error() {
    // Taking the after arm runs its block and the select completes normally
    // — no handling (`!`/catch wrapping of the select) beyond the usual
    // all-closed ChannelClosed is demanded, and nothing is raised.
    let code = compile_and_run(r#"
fn wait_once(rx: Receiver<int>) int {
    select {
        v = rx.recv() {
            return v
        }
        after 20 {
            return -1
        }
    }
    return 0
}

fn main() {
    let (tx, rx) = chan<int>(1)
    let r = wait_once(rx) catch -99
    if r == -1 {
        tx.close()
    }
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn select_after_all_closed_still_raises_channel_closed() {
    // A fully closed select can never complete — ChannelClosed wins over
    // waiting out the deadline.
    let out = compile_and_run_stdout(r#"
fn ruled_out(rx: Receiver<int>) int {
    select {
        v = rx.recv() {
            return v
        }
        after 60000 {
            return -1
        }
    }
    return 0
}

fn main() {
    let (tx, rx) = chan<int>(1)
    tx.close()
    let r = ruled_out(rx) catch err: ChannelClosed { -2 }
    print(r)
}
"#);
    assert_eq!(out.trim(), "-2");
}

#[test]
fn select_after_plus_default_rejected() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        v = rx.recv() {
            print(v)
        }
        default {
            print(0)
        }
        after 100 {
            print(1)
        }
    }
}
"#, "select cannot have both a default arm and an after arm");
}

#[test]
fn select_after_duration_must_be_int() {
    compile_should_fail_with(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        v = rx.recv() {
            print(v)
        }
        after "soon" {
            print(1)
        }
    }
}
"#, "select after expects int milliseconds");
}

// ── select `after` / recv_timeout duration edges (#423) ─────────────────────
//
// Shared deadline rule (docs/design/channels.md): a non-positive duration is
// an already-expired deadline — one readiness poll, then the timeout fires;
// a huge duration saturates to a far-future deadline instead of overflowing
// into firing instantly. These run in PRODUCTION mode (real waits), with the
// helper's watchdog guarding against the pre-fix block-forever behavior.

#[test]
fn select_after_negative_duration_fires_immediately() {
    // Pre-fix, a computed negative duration collided with the runtime's
    // "no after arm" sentinel and blocked forever in production.
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let d = 0 - 5
    select {
        v = rx.recv() {
            print("got value")
        }
        after d {
            print("after fired")
        }
    }
    tx.close()
}
"#, 10);
    assert_eq!(out.trim(), "after fired");
}

#[test]
fn select_after_zero_duration_fires_immediately() {
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    select {
        v = rx.recv() {
            print("got value")
        }
        after 0 {
            print("after fired")
        }
    }
    tx.close()
}
"#, 10);
    assert_eq!(out.trim(), "after fired");
}

#[test]
fn select_after_negative_duration_ready_arm_still_wins() {
    // One readiness poll happens before the expired deadline fires, so a
    // ready arm still wins over a non-positive after.
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(7)!
    let d = 0 - 1
    select {
        v = rx.recv() {
            print(f"got {v}")
        }
        after d {
            print("after fired")
        }
    }
}
"#, 10);
    assert_eq!(out.trim(), "got 7");
}

#[test]
fn select_after_huge_duration_does_not_fire_instantly() {
    // i64::MAX ms used to overflow the nanosecond multiply and fire the
    // after arm instantly; it must saturate to a far-future deadline and
    // let the producer's value win.
    let out = compile_and_run_stdout_timeout(r#"
fn producer(tx: Sender<int>) {
    tx.send(42)!
}

fn main() {
    let (tx, rx) = chan<int>(1)
    spawn producer(tx).detach()
    select {
        v = rx.recv() {
            print(f"got {v}")
        }
        after 9223372036854775807 {
            print("after fired")
        }
    }
}
"#, 10);
    assert_eq!(out.trim(), "got 42");
}

#[test]
fn recv_timeout_negative_duration_times_out_immediately() {
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    let d = 0 - 5
    let v = rx.recv_timeout(d) catch err: TimedOut { -1 }
    print(v)
    tx.close()
}
"#, 10);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn recv_timeout_negative_duration_still_delivers_buffered_value() {
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(9)!
    let d = 0 - 5
    let v = rx.recv_timeout(d) catch err: TimedOut { -1 }
    print(v)
}
"#, 10);
    assert_eq!(out.trim(), "9");
}

#[test]
fn recv_timeout_huge_duration_delivers_buffered_value() {
    let out = compile_and_run_stdout_timeout(r#"
fn main() {
    let (tx, rx) = chan<int>(1)
    tx.send(11)!
    let v = rx.recv_timeout(9223372036854775807) catch err: TimedOut { -1 }
    print(v)
}
"#, 10);
    assert_eq!(out.trim(), "11");
}

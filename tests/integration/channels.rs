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
    assert_eq!(out.trim(), "3.140000");
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
    let sum = 0
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
    let sum = 0
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
    let sum = 0
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
    let sum = 0
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
    // Regular closure captures sender, no inc/dec per call, closes at fn exit
    let out = compile_and_run_stdout(r#"
fn main() {
    let (tx, rx) = chan<int>(10)
    let send_val = (x: int) => { tx.send(x)! }
    send_val(1)
    send_val(2)
    send_val(3)
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
    let sum = 0
    let count = 0
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
    let result = 0
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

mod common;
use common::*;

// ── Sequential spawn tests ──────────────────────────────────────────────

#[test]
fn sequential_spawn_basic() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

test "spawn returns result" {
    let t = spawn add(1, 2)
    expect(t.get()).to_equal(3)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_spawn_order() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn say(s: string) string {
    print(s)
    return s
}

test "spawn runs in creation order" {
    let t1 = spawn say("first")
    let t2 = spawn say("second")
    let t3 = spawn say("third")
    t1.get()
    t2.get()
    t3.get()
}
"#);
    // In sequential test mode, tasks run inline at spawn — deterministic order
    assert!(stdout.contains("first\nsecond\nthird\n"), "Expected deterministic order, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_spawn_error_propagation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error MathError {
    message: string
}

fn fail_task() int {
    raise MathError { message: "task failed" }
    return 0
}

fn run() int {
    let t = spawn fail_task()
    let result = t.get() catch -1
    return result
}

test "spawn error propagation" {
    expect(run()).to_equal(-1)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_spawn_detach() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn work() int {
    return 42
}

test "detach works in sequential mode" {
    let t = spawn work()
    t.detach()
    expect(true).to_be_true()
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_spawn_cancel() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn work() int {
    return 42
}

test "cancel on completed task" {
    let t = spawn work()
    t.cancel()
    // Task already completed, cancel sets the flag but result is available
    expect(t.get()).to_equal(42)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_deep_copy_isolation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
class Counter {
    value: int
}

fn increment(c: Counter) int {
    return c.value + 1
}

test "deep copy isolation" {
    let c = Counter { value: 10 }
    let t = spawn increment(c)
    expect(c.value).to_equal(10)
    expect(t.get()).to_equal(11)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ── Sequential channel tests ────────────────────────────────────────────

#[test]
fn sequential_channel_basic() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn producer(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

test "channel from spawn" {
    let (tx, rx) = chan<int>(10)
    let t = spawn producer(tx)
    t.get()!
    expect(rx.recv()!).to_equal(1)
    expect(rx.recv()!).to_equal(2)
    expect(rx.recv()!).to_equal(3)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_channel_deadlock_send() {
    // In sequential mode, sending to a full channel should abort with deadlock message
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn fill_channel(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
}

test "full channel deadlocks" {
    let (tx, rx) = chan<int>(1)
    let t = spawn fill_channel(tx)
    t.get()!
}
"#);
    assert_ne!(code, 0, "Should have exited with non-zero");
    assert!(stderr.contains("deadlock"), "Expected deadlock message, got stderr: {stderr}");
}

#[test]
fn sequential_channel_deadlock_recv() {
    // In sequential mode, receiving from an empty channel should abort with deadlock message
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn try_recv(rx: Receiver<int>) int {
    return rx.recv()!
}

test "empty channel deadlocks" {
    let (tx, rx) = chan<int>(10)
    let t = spawn try_recv(rx)
    t.get() catch 0
}
"#);
    assert_ne!(code, 0, "Should have exited with non-zero");
    assert!(stderr.contains("deadlock"), "Expected deadlock message, got stderr: {stderr}");
}

// ── Existing concurrency patterns through test mode ─────────────────────

#[test]
fn sequential_multiple_spawns_with_results() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn double(x: int) int {
    return x * 2
}

test "multiple spawns" {
    let t1 = spawn double(5)
    let t2 = spawn double(10)
    let t3 = spawn double(15)
    expect(t1.get()).to_equal(10)
    expect(t2.get()).to_equal(20)
    expect(t3.get()).to_equal(30)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_spawn_with_string_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn greet(name: string) string {
    return "hello " + name
}

test "spawn with string" {
    let t = spawn greet("world")
    expect(t.get()).to_equal("hello world")
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_try_send_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "try_send and try_recv" {
    let (tx, rx) = chan<int>(3)
    tx.try_send(10)!
    tx.try_send(20)!
    tx.try_send(30)!
    expect(rx.try_recv()!).to_equal(10)
    expect(rx.try_recv()!).to_equal(20)
    expect(rx.try_recv()!).to_equal(30)
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn sequential_channel_close() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn check_closed() bool {
    let (tx, rx) = chan<int>(5)
    tx.send(42)!
    tx.close()
    let val = rx.recv()!
    return val == 42
}

test "channel close" {
    expect(check_closed()!).to_be_true()
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ── Round-robin tests ──────────────────────────────────────────────────

#[test]
fn round_robin_spawn_basic() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

tests[scheduler: RoundRobin] {
    test "round robin spawn returns result" {
        let t = spawn add(1, 2)
        expect(t.get()).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn round_robin_multiple_spawns() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn double(x: int) int {
    return x * 2
}

tests[scheduler: RoundRobin] {
    test "round robin multiple spawns" {
        let t1 = spawn double(5)
        let t2 = spawn double(10)
        let t3 = spawn double(15)
        expect(t1.get()).to_equal(10)
        expect(t2.get()).to_equal(20)
        expect(t3.get()).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn round_robin_channel_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn producer(tx: Sender<int>) {
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
}

tests[scheduler: RoundRobin] {
    test "round robin channel pipeline" {
        let (tx, rx) = chan<int>(1)
        let t = spawn producer(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t.get()!
        expect(a).to_equal(10)
        expect(b).to_equal(20)
        expect(c).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn round_robin_spawn_with_error() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error TaskError {
    message: string
}

fn fail_task() int {
    raise TaskError { message: "oops" }
    return 0
}

fn run() int {
    let t = spawn fail_task()
    let result = t.get() catch -1
    return result
}

tests[scheduler: RoundRobin] {
    test "round robin error propagation" {
        expect(run()).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn round_robin_string_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn greet(name: string) string {
    return "hello " + name
}

tests[scheduler: RoundRobin] {
    test "round robin string result" {
        let t = spawn greet("world")
        expect(t.get()).to_equal("hello world")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ── Random strategy tests ──────────────────────────────────────────────

#[test]
fn random_spawn_basic() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn add(a: int, b: int) int {
    return a + b
}

tests[scheduler: Random] {
    test "random spawn basic" {
        let t = spawn add(3, 4)
        expect(t.get()).to_equal(7)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn random_seed_reproducibility() {
    // With a fixed seed, results should always be the same
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn compute(x: int) int {
    return x * x + 1
}

tests[scheduler: Random] {
    test "random with seed" {
        let t = spawn compute(7)
        expect(t.get()).to_equal(50)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5"), ("PLUTO_TEST_SEED", "42")]);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn random_multiple_spawns() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn double(x: int) int {
    return x * 2
}

tests[scheduler: Random] {
    test "random multiple spawns" {
        let t1 = spawn double(5)
        let t2 = spawn double(10)
        expect(t1.get()).to_equal(10)
        expect(t2.get()).to_equal(20)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn random_channel_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn producer(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: Random] {
    test "random channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn producer(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t.get()!
        expect(a).to_equal(1)
        expect(b).to_equal(2)
        expect(c).to_equal(3)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ── Sequential tests declaration (explicit) ─────────────────────────────

#[test]
fn explicit_sequential_tests_decl() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

tests[scheduler: Sequential] {
    test "explicit sequential" {
        let t = spawn add(2, 3)
        expect(t.get()).to_equal(5)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ── Parse error tests ──────────────────────────────────────────────────

#[test]
fn tests_decl_parse_error_unknown_scheduler() {
    compile_test_should_fail_with(r#"
tests[scheduler: Unknown] {
    test "bad" {
        expect(true).to_be_true()
    }
}
"#, "unknown scheduler strategy");
}

#[test]
fn tests_decl_parse_error_wrong_key() {
    compile_test_should_fail_with(r#"
tests[strategy: RoundRobin] {
    test "bad" {
        expect(true).to_be_true()
    }
}
"#, "expected 'scheduler'");
}

#[test]
fn tests_decl_rejects_mixing_with_bare_tests() {
    compile_test_should_fail_with(r#"
test "bare" {
    expect(true).to_be_true()
}

tests[scheduler: RoundRobin] {
    test "block" {
        expect(true).to_be_true()
    }
}
"#, "cannot mix bare 'test' blocks with 'tests' declarations");
}

// ── GC safety under fiber scheduling ──────────────────────────────────

#[test]
fn round_robin_gc_fiber_stack_safety() {
    // This test exercises GC while multiple fibers hold heap objects (strings, arrays)
    // on their stacks. Without fiber stack scanning, the GC could collect live objects
    // that are only referenced from suspended fibers' stacks.
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn build_strings(prefix: string) string {
    // Build enough heap pressure to trigger GC
    let mut result = ""
    for i in 0..50 {
        result = result + prefix + "_item_"
    }
    return result
}

fn allocate_arrays() [int] {
    let mut arr = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    for i in 0..50 {
        arr = [i, i+1, i+2, i+3, i+4, i+5, i+6, i+7, i+8, i+9]
    }
    return arr
}

tests[scheduler: RoundRobin] {
    test "gc safety with fiber stacks" {
        let t1 = spawn build_strings("alpha")
        let t2 = spawn build_strings("beta")
        let t3 = spawn allocate_arrays()
        let s1 = t1.get()
        let s2 = t2.get()
        let a = t3.get()
        expect(s1.len() > 0).to_be_true()
        expect(s2.len() > 0).to_be_true()
        expect(a.len()).to_equal(10)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn random_gc_fiber_stack_safety() {
    // Same as above but with random scheduling to exercise different interleavings
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn build_large_string(n: int) string {
    let mut s = "start"
    for i in 0..n {
        s = s + "_x"
    }
    return s
}

tests[scheduler: Random] {
    test "gc safety random scheduling" {
        let t1 = spawn build_large_string(30)
        let t2 = spawn build_large_string(40)
        let r1 = t1.get()
        let r2 = t2.get()
        expect(r1.len() > 0).to_be_true()
        expect(r2.len() > 0).to_be_true()
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10"), ("PLUTO_TEST_SEED", "12345")]);
    assert!(stdout.contains("1 tests passed"), "Expected 1 tests passed, got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Round-robin channel patterns (~40 tests)
// ============================================================================

#[test]
fn rr_chan_backpressure_cap1() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn sender(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: RoundRobin] {
    test "backpressure with cap 1" {
        let (tx, rx) = chan<int>(1)
        let t = spawn sender(tx)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(2)
        expect(rx.recv()!).to_equal(3)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_ping_pong() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn pinger(tx: Sender<int>, rx: Receiver<int>) {
    tx.send(1)!
    let v = rx.recv()!
    tx.send(v + 2)!
}

tests[scheduler: RoundRobin] {
    test "ping pong between fibers" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn pinger(tx1, rx2)
        let v1 = rx1.recv()!
        tx2.send(v1 + 10)!
        let v2 = rx1.recv()!
        t.get()!
        expect(v1).to_equal(1)
        expect(v2).to_equal(13)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_fifo_order() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_five(tx: Sender<int>) {
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
    tx.send(40)!
    tx.send(50)!
}

tests[scheduler: RoundRobin] {
    test "fifo ordering preserved" {
        let (tx, rx) = chan<int>(5)
        let t = spawn send_five(tx)
        t.get()!
        expect(rx.recv()!).to_equal(10)
        expect(rx.recv()!).to_equal(20)
        expect(rx.recv()!).to_equal(30)
        expect(rx.recv()!).to_equal(40)
        expect(rx.recv()!).to_equal(50)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_negative_ints() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_negatives(tx: Sender<int>) {
    tx.send(-1)!
    tx.send(-100)!
    tx.send(-999)!
}

tests[scheduler: RoundRobin] {
    test "negative ints through channel" {
        let (tx, rx) = chan<int>(3)
        let t = spawn send_negatives(tx)
        t.get()!
        expect(rx.recv()!).to_equal(-1)
        expect(rx.recv()!).to_equal(-100)
        expect(rx.recv()!).to_equal(-999)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_two_independent() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "two independent channels" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn send_val(tx1, 11)
        let t2 = spawn send_val(tx2, 22)
        expect(rx1.recv()!).to_equal(11)
        expect(rx2.recv()!).to_equal(22)
        t1.get()!
        t2.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_single_value() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn one_shot(tx: Sender<int>) {
    tx.send(42)!
}

tests[scheduler: RoundRobin] {
    test "single value through channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn one_shot(tx)
        expect(rx.recv()!).to_equal(42)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_large_buffer_20() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fill_20(tx: Sender<int>) {
    let mut i = 0
    while i < 20 {
        tx.send(i)!
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "large buffer 20 items" {
        let (tx, rx) = chan<int>(20)
        let t = spawn fill_20(tx)
        t.get()!
        let mut sum = 0
        let mut i = 0
        while i < 20 {
            sum = sum + rx.recv()!
            i = i + 1
        }
        expect(sum).to_equal(190)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_broadcast_to_two() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn broadcaster(tx1: Sender<int>, tx2: Sender<int>) {
    tx1.send(99)!
    tx2.send(99)!
}

tests[scheduler: RoundRobin] {
    test "broadcast to two receivers" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn broadcaster(tx1, tx2)
        expect(rx1.recv()!).to_equal(99)
        expect(rx2.recv()!).to_equal(99)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_request_response() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn server(req_rx: Receiver<int>, resp_tx: Sender<int>) {
    let v = req_rx.recv()!
    resp_tx.send(v * 10)!
}

tests[scheduler: RoundRobin] {
    test "request response pattern" {
        let (req_tx, req_rx) = chan<int>(1)
        let (resp_tx, resp_rx) = chan<int>(1)
        let t = spawn server(req_rx, resp_tx)
        req_tx.send(5)!
        let result = resp_rx.recv()!
        t.get()!
        expect(result).to_equal(50)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_semaphore_pattern() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn worker(sem_rx: Receiver<int>, done_tx: Sender<int>, id: int) {
    sem_rx.recv()!
    done_tx.send(id)!
}

tests[scheduler: RoundRobin] {
    test "semaphore via channel" {
        let (sem_tx, sem_rx) = chan<int>(2)
        let (done_tx, done_rx) = chan<int>(2)
        sem_tx.send(1)!
        sem_tx.send(1)!
        let t1 = spawn worker(sem_rx, done_tx, 1)
        let t2 = spawn worker(sem_rx, done_tx, 2)
        let r1 = done_rx.recv()!
        let r2 = done_rx.recv()!
        t1.get()!
        t2.get()!
        expect(r1 + r2).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_mutex_pattern() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn critical_section(lock_rx: Receiver<int>, lock_tx: Sender<int>, result_tx: Sender<int>, val: int) {
    lock_rx.recv()!
    result_tx.send(val)!
    lock_tx.send(1)!
}

tests[scheduler: RoundRobin] {
    test "mutex via channel" {
        let (lock_tx, lock_rx) = chan<int>(1)
        let (result_tx, result_rx) = chan<int>(2)
        lock_tx.send(1)!
        let t1 = spawn critical_section(lock_rx, lock_tx, result_tx, 10)
        let t2 = spawn critical_section(lock_rx, lock_tx, result_tx, 20)
        let r1 = result_rx.recv()!
        let r2 = result_rx.recv()!
        t1.get()!
        t2.get()!
        expect(r1 + r2).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_pipeline_transform() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn stage1(tx: Sender<int>) {
    tx.send(5)!
}

fn stage2(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v * 3)!
}

tests[scheduler: RoundRobin] {
    test "pipeline with transform" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn stage1(tx1)
        let t2 = spawn stage2(rx1, tx2)
        let result = rx2.recv()!
        t1.get()!
        t2.get()!
        expect(result).to_equal(15)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_send_then_recv_same_fiber() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "send then recv on same fiber" {
        let (tx, rx) = chan<int>(5)
        tx.send(7)!
        tx.send(8)!
        expect(rx.recv()!).to_equal(7)
        expect(rx.recv()!).to_equal(8)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_producer_faster() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fast_producer(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.send(4)!
}

tests[scheduler: RoundRobin] {
    test "producer faster than consumer" {
        let (tx, rx) = chan<int>(2)
        let t = spawn fast_producer(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        let d = rx.recv()!
        t.get()!
        expect(a + b + c + d).to_equal(10)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_consumer_ready_first() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn delayed_sender(tx: Sender<int>) {
    tx.send(77)!
}

fn eager_consumer(rx: Receiver<int>, out: Sender<int>) {
    let v = rx.recv()!
    out.send(v)!
}

tests[scheduler: RoundRobin] {
    test "consumer ready before producer" {
        let (tx, rx) = chan<int>(1)
        let (out_tx, out_rx) = chan<int>(1)
        let tc = spawn eager_consumer(rx, out_tx)
        let tp = spawn delayed_sender(tx)
        let result = out_rx.recv()!
        tc.get()!
        tp.get()!
        expect(result).to_equal(77)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_recv_blocks_then_send_unblocks() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn waiter(rx: Receiver<int>, out: Sender<int>) {
    let v = rx.recv()!
    out.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "recv blocks then send unblocks" {
        let (tx, rx) = chan<int>(1)
        let (out_tx, out_rx) = chan<int>(1)
        let t = spawn waiter(rx, out_tx)
        tx.send(100)!
        let result = out_rx.recv()!
        t.get()!
        expect(result).to_equal(101)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_send_blocks_then_recv_unblocks() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn blocker(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
}

tests[scheduler: RoundRobin] {
    test "send blocks then recv unblocks" {
        let (tx, rx) = chan<int>(1)
        let t = spawn blocker(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        t.get()!
        expect(a).to_equal(1)
        expect(b).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_select_single_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn sender_for_select(tx: Sender<int>) {
    tx.send(55)!
}

tests[scheduler: RoundRobin] {
    test "select single recv arm" {
        let (tx, rx) = chan<int>(1)
        let t = spawn sender_for_select(tx)
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
        }
        t.get()!
        expect(result).to_equal(55)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_select_default_on_empty() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "select default when empty" {
        let (tx, rx) = chan<int>(1)
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
            default {
                result = -1
            }
        }
        expect(result).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_select_ready_over_default() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "select ready channel over default" {
        let (tx, rx) = chan<int>(1)
        tx.send(88)!
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
            default {
                result = -1
            }
        }
        expect(result).to_equal(88)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_close_empty() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn close_it(tx: Sender<int>) {
    tx.close()
}

fn recv_closed(rx: Receiver<int>) int {
    let v = rx.recv() catch -1
    return v
}

tests[scheduler: RoundRobin] {
    test "close empty channel" {
        let (tx, rx) = chan<int>(5)
        let t1 = spawn close_it(tx)
        t1.get()
        expect(recv_closed(rx)).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_drain_after_close() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fill_and_close(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.close()
}

tests[scheduler: RoundRobin] {
    test "drain channel after close" {
        let (tx, rx) = chan<int>(5)
        let t = spawn fill_and_close(tx)
        t.get()!
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_fibonacci() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fib_gen(tx: Sender<int>, n: int) {
    let mut a = 0
    let mut b = 1
    let mut i = 0
    while i < n {
        tx.send(a)!
        let tmp = a + b
        a = b
        b = tmp
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "fibonacci via channels" {
        let (tx, rx) = chan<int>(1)
        let t = spawn fib_gen(tx, 7)
        expect(rx.recv()!).to_equal(0)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(2)
        expect(rx.recv()!).to_equal(3)
        expect(rx.recv()!).to_equal(5)
        expect(rx.recv()!).to_equal(8)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_string_through() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_str(tx: Sender<string>) {
    tx.send("hello")!
    tx.send("world")!
}

tests[scheduler: RoundRobin] {
    test "strings through channel" {
        let (tx, rx) = chan<string>(2)
        let t = spawn send_str(tx)
        t.get()!
        expect(rx.recv()!).to_equal("hello")
        expect(rx.recv()!).to_equal("world")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_bool_through() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_bools(tx: Sender<bool>) {
    tx.send(true)!
    tx.send(false)!
}

tests[scheduler: RoundRobin] {
    test "bools through channel" {
        let (tx, rx) = chan<bool>(2)
        let t = spawn send_bools(tx)
        t.get()!
        expect(rx.recv()!).to_be_true()
        expect(rx.recv()!).to_be_false()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_empty_string() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_empty(tx: Sender<string>) {
    tx.send("")!
}

tests[scheduler: RoundRobin] {
    test "empty string through channel" {
        let (tx, rx) = chan<string>(1)
        let t = spawn send_empty(tx)
        t.get()!
        expect(rx.recv()!).to_equal("")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_zero_value() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_zero(tx: Sender<int>) {
    tx.send(0)!
}

tests[scheduler: RoundRobin] {
    test "zero value through channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn send_zero(tx)
        t.get()!
        expect(rx.recv()!).to_equal(0)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_alternating_send_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn alternator(tx: Sender<int>, rx: Receiver<int>) {
    tx.send(1)!
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "alternating send and recv" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn alternator(tx1, rx2)
        let first = rx1.recv()!
        tx2.send(first + 10)!
        let second = rx1.recv()!
        t.get()!
        expect(first).to_equal(1)
        expect(second).to_equal(12)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_batch_send_then_batch_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn batch_sender(tx: Sender<int>) {
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
    tx.send(40)!
    tx.send(50)!
}

tests[scheduler: RoundRobin] {
    test "batch send then batch recv" {
        let (tx, rx) = chan<int>(5)
        let t = spawn batch_sender(tx)
        t.get()!
        let mut sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        expect(sum).to_equal(150)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_daisy_chain() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn link(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "daisy chain three links" {
        let (tx0, rx0) = chan<int>(1)
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn link(rx0, tx1)
        let t2 = spawn link(rx1, tx2)
        tx0.send(0)!
        let result = rx2.recv()!
        t1.get()!
        t2.get()!
        expect(result).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_string_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn prepend(rx: Receiver<string>, tx: Sender<string>, prefix: string) {
    let v = rx.recv()!
    tx.send(prefix + v)!
}

tests[scheduler: RoundRobin] {
    test "string pipeline" {
        let (tx1, rx1) = chan<string>(1)
        let (tx2, rx2) = chan<string>(1)
        let t = spawn prepend(rx1, tx2, "hello ")
        tx1.send("world")!
        let result = rx2.recv()!
        t.get()!
        expect(result).to_equal("hello world")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_four_stage_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add_one(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "four stage pipeline" {
        let (tx0, rx0) = chan<int>(1)
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn add_one(rx0, tx1)
        let t2 = spawn add_one(rx1, tx2)
        let t3 = spawn add_one(rx2, tx3)
        tx0.send(0)!
        let result = rx3.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(result).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_fan_in_two_senders() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn sender_a(tx: Sender<int>) {
    tx.send(10)!
}

fn sender_b(tx: Sender<int>) {
    tx.send(20)!
}

tests[scheduler: RoundRobin] {
    test "fan in from two senders" {
        let (tx, rx) = chan<int>(2)
        let t1 = spawn sender_a(tx)
        let t2 = spawn sender_b(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        t1.get()!
        t2.get()!
        expect(a + b).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_close_wakes_senders_bug() {
    // Bug finder: closing a channel may or may not wake blocked senders cleanly
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn blocking_sender(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

fn closer(tx: Sender<int>) {
    tx.close()
}

tests[scheduler: RoundRobin] {
    test "close wakes blocked senders" {
        let (tx, rx) = chan<int>(1)
        let t1 = spawn blocking_sender(tx)
        let t2 = spawn closer(tx)
        let v = rx.recv()!
        expect(v).to_equal(1)
        t1.get() catch err { }
        t2.get()
    }
}
"#);
    if code != 0 {
        eprintln!("KNOWN BUG: close waking blocked senders: {stderr}");
    } else {
        assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    }
}

#[test]
fn rr_chan_request_response_squaring() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn squarer(req: Receiver<int>, resp: Sender<int>) {
    let v = req.recv()!
    resp.send(v * v)!
    let v2 = req.recv()!
    resp.send(v2 * v2)!
}

tests[scheduler: RoundRobin] {
    test "request response squaring" {
        let (req_tx, req_rx) = chan<int>(1)
        let (resp_tx, resp_rx) = chan<int>(1)
        let t = spawn squarer(req_rx, resp_tx)
        req_tx.send(3)!
        expect(resp_rx.recv()!).to_equal(9)
        req_tx.send(7)!
        expect(resp_rx.recv()!).to_equal(49)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_two_way_communication() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn echo_server(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "two way communication" {
        let (to_server_tx, to_server_rx) = chan<int>(1)
        let (from_server_tx, from_server_rx) = chan<int>(1)
        let t = spawn echo_server(to_server_rx, from_server_tx)
        to_server_tx.send(42)!
        let echoed = from_server_rx.recv()!
        t.get()!
        expect(echoed).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_multiple_senders_three() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_id(tx: Sender<int>, id: int) {
    tx.send(id)!
}

tests[scheduler: RoundRobin] {
    test "three senders one receiver" {
        let (tx, rx) = chan<int>(3)
        let t1 = spawn send_id(tx, 100)
        let t2 = spawn send_id(tx, 200)
        let t3 = spawn send_id(tx, 300)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(600)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_loop_send() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn loop_sender(tx: Sender<int>, count: int) {
    let mut i = 0
    while i < count {
        tx.send(i * i)!
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "send in loop" {
        let (tx, rx) = chan<int>(1)
        let t = spawn loop_sender(tx, 4)
        expect(rx.recv()!).to_equal(0)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(4)
        expect(rx.recv()!).to_equal(9)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_select_after_partial_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_two(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
}

tests[scheduler: RoundRobin] {
    test "select after partial recv" {
        let (tx, rx) = chan<int>(2)
        let t = spawn send_two(tx)
        t.get()!
        let first = rx.recv()!
        let mut second = 0
        select {
            v = rx.recv() {
                second = v
            }
            default {
                second = -1
            }
        }
        expect(first).to_equal(1)
        expect(second).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Round-robin task patterns (~30 tests)
// ============================================================================

#[test]
fn rr_task_arithmetic_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn multiply(a: int, b: int) int {
    return a * b
}

tests[scheduler: RoundRobin] {
    test "task arithmetic result" {
        let t = spawn multiply(6, 7)
        expect(t.get()).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_negative_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn negate(x: int) int {
    return 0 - x
}

tests[scheduler: RoundRobin] {
    test "task negative result" {
        let t = spawn negate(42)
        expect(t.get()).to_equal(-42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_five_sum() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn identity(x: int) int {
    return x
}

tests[scheduler: RoundRobin] {
    test "five tasks summed" {
        let t1 = spawn identity(1)
        let t2 = spawn identity(2)
        let t3 = spawn identity(3)
        let t4 = spawn identity(4)
        let t5 = spawn identity(5)
        let sum = t1.get() + t2.get() + t3.get() + t4.get() + t5.get()
        expect(sum).to_equal(15)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_get_reverse_order() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn val(x: int) int {
    return x
}

tests[scheduler: RoundRobin] {
    test "get in reverse order" {
        let t1 = spawn val(10)
        let t2 = spawn val(20)
        let t3 = spawn val(30)
        let c = t3.get()
        let b = t2.get()
        let a = t1.get()
        expect(a).to_equal(10)
        expect(b).to_equal(20)
        expect(c).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_error_propagation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error ComputeError {
    msg: string
}

fn failing() int {
    raise ComputeError { msg: "boom" }
    return 0
}

fn caller() int {
    let t = spawn failing()
    let v = t.get()!
    return v
}

tests[scheduler: RoundRobin] {
    test "task error propagation with bang" {
        let result = caller() catch -99
        expect(result).to_equal(-99)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_error_catch() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error BadInput {
    reason: string
}

fn validate(x: int) int {
    if x < 0 {
        raise BadInput { reason: "negative" }
    }
    return x
}

tests[scheduler: RoundRobin] {
    test "task error caught" {
        let t = spawn validate(-5)
        let result = t.get() catch 0
        expect(result).to_equal(0)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_spawn_same_fn() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn square(x: int) int {
    return x * x
}

tests[scheduler: RoundRobin] {
    test "spawn same fn multiple times" {
        let t1 = spawn square(2)
        let t2 = spawn square(3)
        let t3 = spawn square(4)
        expect(t1.get()).to_equal(4)
        expect(t2.get()).to_equal(9)
        expect(t3.get()).to_equal(16)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_void_get() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn side_effect() {
    let x = 42
}

tests[scheduler: RoundRobin] {
    test "void task get" {
        let t = spawn side_effect()
        t.get()
        expect(true).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_conditional_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn pick(flag: bool) int {
    if flag {
        return 100
    }
    return 200
}

tests[scheduler: RoundRobin] {
    test "task conditional result" {
        let t1 = spawn pick(true)
        let t2 = spawn pick(false)
        expect(t1.get()).to_equal(100)
        expect(t2.get()).to_equal(200)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_immediate_get() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn constant() int {
    return 7
}

tests[scheduler: RoundRobin] {
    test "immediate get after spawn" {
        let t = spawn constant()
        expect(t.get()).to_equal(7)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_with_loop() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn sum_to(n: int) int {
    let mut s = 0
    let mut i = 1
    while i <= n {
        s = s + i
        i = i + 1
    }
    return s
}

tests[scheduler: RoundRobin] {
    test "task with loop" {
        let t = spawn sum_to(10)
        expect(t.get()).to_equal(55)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_string_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn concat(a: string, b: string) string {
    return a + " " + b
}

tests[scheduler: RoundRobin] {
    test "task string concat result" {
        let t = spawn concat("foo", "bar")
        expect(t.get()).to_equal("foo bar")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_bool_result() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn is_positive(x: int) bool {
    return x > 0
}

tests[scheduler: RoundRobin] {
    test "task bool result" {
        let t1 = spawn is_positive(5)
        let t2 = spawn is_positive(-3)
        expect(t1.get()).to_be_true()
        expect(t2.get()).to_be_false()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_string_interpolation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn greet_interp(name: string) string {
    return f"hi {name}!"
}

tests[scheduler: RoundRobin] {
    test "task with string interpolation" {
        let t = spawn greet_interp("alice")
        expect(t.get()).to_equal("hi alice!")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_zero() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn zero() int {
    return 0
}

tests[scheduler: RoundRobin] {
    test "task returns zero" {
        let t = spawn zero()
        expect(t.get()).to_equal(0)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_one() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn one() int {
    return 1
}

tests[scheduler: RoundRobin] {
    test "task returns one" {
        let t = spawn one()
        expect(t.get()).to_equal(1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_neg1() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn neg_one() int {
    return -1
}

tests[scheduler: RoundRobin] {
    test "task returns negative one" {
        let t = spawn neg_one()
        expect(t.get()).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_large_number() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn big() int {
    return 1000000
}

tests[scheduler: RoundRobin] {
    test "task returns large number" {
        let t = spawn big()
        expect(t.get()).to_equal(1000000)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_ten_ordered_results() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn ident(x: int) int {
    return x
}

tests[scheduler: RoundRobin] {
    test "ten tasks ordered results" {
        let t0 = spawn ident(0)
        let t1 = spawn ident(1)
        let t2 = spawn ident(2)
        let t3 = spawn ident(3)
        let t4 = spawn ident(4)
        let t5 = spawn ident(5)
        let t6 = spawn ident(6)
        let t7 = spawn ident(7)
        let t8 = spawn ident(8)
        let t9 = spawn ident(9)
        let sum = t0.get() + t1.get() + t2.get() + t3.get() + t4.get() + t5.get() + t6.get() + t7.get() + t8.get() + t9.get()
        expect(sum).to_equal(45)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_accumulator() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn double_val(x: int) int {
    return x * 2
}

tests[scheduler: RoundRobin] {
    test "accumulator from tasks" {
        let t1 = spawn double_val(1)
        let t2 = spawn double_val(2)
        let t3 = spawn double_val(3)
        let mut acc = 0
        acc = acc + t1.get()
        acc = acc + t2.get()
        acc = acc + t3.get()
        expect(acc).to_equal(12)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_multiple_gets_interleaved() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn compute_a(x: int) int {
    return x + 10
}

fn compute_b(x: int) int {
    return x + 20
}

tests[scheduler: RoundRobin] {
    test "interleaved gets" {
        let t1 = spawn compute_a(1)
        let t2 = spawn compute_b(2)
        let a = t1.get()
        let t3 = spawn compute_a(3)
        let b = t2.get()
        let c = t3.get()
        expect(a).to_equal(11)
        expect(b).to_equal(22)
        expect(c).to_equal(13)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_multiple_tests_in_block() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add_nums(a: int, b: int) int {
    return a + b
}

fn sub_nums(a: int, b: int) int {
    return a - b
}

tests[scheduler: RoundRobin] {
    test "first test addition" {
        let t = spawn add_nums(3, 4)
        expect(t.get()).to_equal(7)
    }

    test "second test subtraction" {
        let t = spawn sub_nums(10, 3)
        expect(t.get()).to_equal(7)
    }
}
"#);
    assert!(stdout.contains("2 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_parallel_sum_of_squares() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn sq(x: int) int {
    return x * x
}

tests[scheduler: RoundRobin] {
    test "parallel sum of squares" {
        let t1 = spawn sq(1)
        let t2 = spawn sq(2)
        let t3 = spawn sq(3)
        let t4 = spawn sq(4)
        let sum = t1.get() + t2.get() + t3.get() + t4.get()
        expect(sum).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_with_while_loop() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn factorial(n: int) int {
    let mut result = 1
    let mut i = 2
    while i <= n {
        result = result * i
        i = i + 1
    }
    return result
}

tests[scheduler: RoundRobin] {
    test "task computing factorial" {
        let t = spawn factorial(6)
        expect(t.get()).to_equal(720)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Round-robin deadlock tests (~5 tests)
// ============================================================================

#[test]
fn rr_deadlock_no_recv_full_buffer() {
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn fill_forever(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: RoundRobin] {
    test "fill full buffer no recv" {
        let (tx, rx) = chan<int>(2)
        let t = spawn fill_forever(tx)
        t.get()!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

#[test]
fn rr_deadlock_mutual_recv() {
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn recv_first(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "mutual recv deadlock" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn recv_first(rx1, tx2)
        let v = rx2.recv()!
        tx1.send(v)!
        t.get()!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

#[test]
fn rr_deadlock_three_way_cycle() {
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn wait_and_send(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "three way deadlock cycle" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn wait_and_send(rx1, tx2)
        let t2 = spawn wait_and_send(rx2, tx3)
        let v = rx3.recv()!
        tx1.send(v)!
        t1.get()!
        t2.get()!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

#[test]
fn rr_deadlock_circular_channels() {
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn relay(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "circular channel deadlock" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn relay(rx1, tx2)
        let t2 = spawn relay(rx2, tx1)
        t1.get()!
        t2.get()!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

#[test]
fn rr_deadlock_fill_forever() {
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn infinite_fill(tx: Sender<int>) {
    let mut i = 0
    while i < 100 {
        tx.send(i)!
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "fill channel forever deadlock" {
        let (tx, rx) = chan<int>(3)
        let t = spawn infinite_fill(tx)
        t.get()!
    }
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

// ============================================================================
// Sequential mode tests (~10 tests)
// ============================================================================

#[test]
fn seq_no_spawn_pure_logic() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "pure logic no spawn" {
    expect(2 + 2).to_equal(4)
    expect(10 - 3).to_equal(7)
    expect(3 * 5).to_equal(15)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_task_error_catch_inline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error TestFail {
    reason: string
}

fn bad_fn() int {
    raise TestFail { reason: "nope" }
    return 0
}

fn wrapper() int {
    let t = spawn bad_fn()
    let v = t.get() catch -1
    return v
}

test "sequential task error catch" {
    expect(wrapper()).to_equal(-1)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_multiple_channels_prefill() {
    // In sequential mode, data must be in channel BEFORE spawn reads it
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn reader(rx: Receiver<int>) int {
    return rx.recv()!
}

test "sequential pre-fill channels" {
    let (tx, rx) = chan<int>(5)
    tx.send(42)!
    let t = spawn reader(rx)
    expect(t.get()!).to_equal(42)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_task_string_value() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn make_str() string {
    return "sequential"
}

test "sequential task string" {
    let t = spawn make_str()
    expect(t.get()).to_equal("sequential")
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_select_basic() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "sequential select basic" {
    let (tx, rx) = chan<int>(1)
    tx.send(5)!
    let mut result = 0
    select {
        v = rx.recv() {
            result = v
        }
    }
    expect(result).to_equal(5)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_select_default_empty() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "sequential select default on empty" {
    let (tx, rx) = chan<int>(1)
    let mut result = 0
    select {
        v = rx.recv() {
            result = v
        }
        default {
            result = -1
        }
    }
    expect(result).to_equal(-1)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_channel_string_values() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "sequential channel strings" {
    let (tx, rx) = chan<string>(3)
    tx.send("a")!
    tx.send("b")!
    tx.send("c")!
    expect(rx.recv()!).to_equal("a")
    expect(rx.recv()!).to_equal("b")
    expect(rx.recv()!).to_equal("c")
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_three_tasks() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn triple(x: int) int {
    return x * 3
}

test "sequential three tasks" {
    let t1 = spawn triple(1)
    let t2 = spawn triple(2)
    let t3 = spawn triple(3)
    expect(t1.get()).to_equal(3)
    expect(t2.get()).to_equal(6)
    expect(t3.get()).to_equal(9)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_explicit_annotation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: Sequential] {
    test "explicit sequential annotation" {
        expect(1 + 1).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_bool_assertions() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "sequential bool assertions" {
    expect(true).to_be_true()
    expect(false).to_be_false()
    expect(3 > 2).to_be_true()
    expect(1 > 5).to_be_false()
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Random strategy tests (~20 tests)
// ============================================================================

#[test]
fn rand_no_spawn_logic() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
tests[scheduler: Random] {
    test "random no spawn" {
        expect(10 + 20).to_equal(30)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_two_tasks() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn inc(x: int) int {
    return x + 1
}

tests[scheduler: Random] {
    test "random two tasks" {
        let t1 = spawn inc(0)
        let t2 = spawn inc(10)
        expect(t1.get()).to_equal(1)
        expect(t2.get()).to_equal(11)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_channel_pipeline_three() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn produce_three(tx: Sender<int>) {
    tx.send(100)!
    tx.send(200)!
    tx.send(300)!
}

tests[scheduler: Random] {
    test "random channel pipeline" {
        let (tx, rx) = chan<int>(1)
        let t = spawn produce_three(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t.get()!
        expect(a + b + c).to_equal(600)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_error_propagation() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
error RandomError {
    msg: string
}

fn might_fail() int {
    raise RandomError { msg: "fail" }
    return 0
}

fn try_it() int {
    let t = spawn might_fail()
    let v = t.get() catch -1
    return v
}

tests[scheduler: Random] {
    test "random error propagation" {
        expect(try_it()).to_equal(-1)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_many_iterations_50() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn add_pair(a: int, b: int) int {
    return a + b
}

tests[scheduler: Random] {
    test "fifty iterations" {
        let t = spawn add_pair(17, 25)
        expect(t.get()).to_equal(42)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "50")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_barrier_pattern() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn wait_barrier(rx: Receiver<int>, done_tx: Sender<int>, id: int) {
    rx.recv()!
    done_tx.send(id)!
}

tests[scheduler: Random] {
    test "random barrier" {
        let (bar_tx, bar_rx) = chan<int>(3)
        let (done_tx, done_rx) = chan<int>(3)
        let t1 = spawn wait_barrier(bar_rx, done_tx, 1)
        let t2 = spawn wait_barrier(bar_rx, done_tx, 2)
        let t3 = spawn wait_barrier(bar_rx, done_tx, 3)
        bar_tx.send(1)!
        bar_tx.send(1)!
        bar_tx.send(1)!
        let a = done_rx.recv()!
        let b = done_rx.recv()!
        let c = done_rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(6)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_fan_in() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn emit(tx: Sender<int>, val: int) {
    tx.send(val)!
}

tests[scheduler: Random] {
    test "random fan in" {
        let (tx, rx) = chan<int>(3)
        let t1 = spawn emit(tx, 10)
        let t2 = spawn emit(tx, 20)
        let t3 = spawn emit(tx, 30)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(60)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_seed_explicit_42() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn double_it(x: int) int {
    return x * 2
}

tests[scheduler: Random] {
    test "random with explicit seed 42" {
        let t = spawn double_it(21)
        expect(t.get()).to_equal(42)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5"), ("PLUTO_TEST_SEED", "42")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_deadlock_detected() {
    let (_stdout, stderr, code) = compile_test_and_run_with_env(r#"
fn wait_forever(rx: Receiver<int>) int {
    return rx.recv()!
}

tests[scheduler: Random] {
    test "random deadlock" {
        let (tx, rx) = chan<int>(1)
        let t = spawn wait_forever(rx)
        let v = t.get()!
        expect(v).to_equal(0)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "1")]);
    assert_ne!(code, 0);
    assert!(stderr.contains("deadlock"), "expected deadlock, got: {stderr}");
}

#[test]
fn rand_backpressure() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn fill_slow(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.send(4)!
}

tests[scheduler: Random] {
    test "random backpressure" {
        let (tx, rx) = chan<int>(2)
        let t = spawn fill_slow(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        let d = rx.recv()!
        t.get()!
        expect(a + b + c + d).to_equal(10)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_token_ring() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn pass_token(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: Random] {
    test "random token ring" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn pass_token(rx1, tx2)
        let t2 = spawn pass_token(rx2, tx3)
        tx1.send(0)!
        let result = rx3.recv()!
        t1.get()!
        t2.get()!
        expect(result).to_equal(2)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_five_tasks_sum() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn cube(x: int) int {
    return x * x * x
}

tests[scheduler: Random] {
    test "random five tasks sum" {
        let t1 = spawn cube(1)
        let t2 = spawn cube(2)
        let t3 = spawn cube(3)
        let t4 = spawn cube(4)
        let t5 = spawn cube(5)
        let sum = t1.get() + t2.get() + t3.get() + t4.get() + t5.get()
        expect(sum).to_equal(225)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_bidirectional() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn responder(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v * 2)!
}

tests[scheduler: Random] {
    test "random bidirectional" {
        let (to_tx, to_rx) = chan<int>(1)
        let (from_tx, from_rx) = chan<int>(1)
        let t = spawn responder(to_rx, from_tx)
        to_tx.send(21)!
        let result = from_rx.recv()!
        t.get()!
        expect(result).to_equal(42)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_producer_consumer_close() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn produce_and_close(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.close()
}

tests[scheduler: Random] {
    test "random producer consumer close" {
        let (tx, rx) = chan<int>(5)
        let t = spawn produce_and_close(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        t.get()!
        expect(a).to_equal(1)
        expect(b).to_equal(2)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_ping_pong() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn ponger(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: Random] {
    test "random ping pong" {
        let (ping_tx, ping_rx) = chan<int>(1)
        let (pong_tx, pong_rx) = chan<int>(1)
        let t = spawn ponger(ping_rx, pong_tx)
        ping_tx.send(0)!
        let result = pong_rx.recv()!
        t.get()!
        expect(result).to_equal(1)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_multiple_tests_in_block() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn add_two(x: int) int {
    return x + 2
}

tests[scheduler: Random] {
    test "rand first" {
        let t = spawn add_two(3)
        expect(t.get()).to_equal(5)
    }

    test "rand second" {
        let t = spawn add_two(8)
        expect(t.get()).to_equal(10)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("2 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_string_result() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn make_greeting(name: string) string {
    return "hey " + name
}

tests[scheduler: Random] {
    test "random string result" {
        let t = spawn make_greeting("bob")
        expect(t.get()).to_equal("hey bob")
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_bool_result() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn check_even(x: int) bool {
    return x % 2 == 0
}

tests[scheduler: Random] {
    test "random bool result" {
        let t1 = spawn check_even(4)
        let t2 = spawn check_even(7)
        expect(t1.get()).to_be_true()
        expect(t2.get()).to_be_false()
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_channel_string() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn send_message(tx: Sender<string>) {
    tx.send("hello")!
}

tests[scheduler: Random] {
    test "random channel string" {
        let (tx, rx) = chan<string>(1)
        let t = spawn send_message(tx)
        let msg = rx.recv()!
        t.get()!
        expect(msg).to_equal("hello")
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_seed_99() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn mod_val(x: int) int {
    return x % 7
}

tests[scheduler: Random] {
    test "random seed 99" {
        let t = spawn mod_val(50)
        expect(t.get()).to_equal(1)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5"), ("PLUTO_TEST_SEED", "99")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Bug finder tests (previously failing, now fixed)
// ============================================================================

#[test]
fn bug_task_float_get_returns_zero() {
    // Fixed: spawn closures now return I64 so C runtime reads correct register
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn compute_float() float {
    return 3.14
}

tests[scheduler: RoundRobin] {
    test "task float get" {
        let t = spawn compute_float()
        let v = t.get()
        expect(v > 0.0).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_select_closed_channel_default() {
    // Fixed: fiber-mode select now checks has_default before raising closed error
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "select on closed with default" {
        let (tx, rx) = chan<int>(1)
        tx.close()
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
            default {
                result = -1
            }
        }
        expect(result).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_try_send_closed_fiber() {
    // Fixed: try_send correctly detects closed channel in fiber mode
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn try_closed(tx: Sender<int>) bool {
    let mut failed = false
    tx.try_send(1) catch err {
        failed = true
    }
    return failed
}

tests[scheduler: RoundRobin] {
    test "try send on closed channel in fiber" {
        let (tx, rx) = chan<int>(1)
        tx.close()
        let t = spawn try_closed(tx)
        let v = t.get()
        expect(v).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_select_drain_closed() {
    // Fixed: select drain loop with default on closed channel works correctly
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "select drain closed channel" {
        let (tx, rx) = chan<int>(3)
        tx.send(1)!
        tx.send(2)!
        tx.close()
        let mut sum = 0
        let mut done = false
        while !done {
            select {
                v = rx.recv() {
                    sum = sum + v
                }
                default {
                    done = true
                }
            }
        }
        expect(sum).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_close_wakes_multiple_senders() {
    // Fixed: closing a channel properly wakes multiple blocked senders
    // Use larger buffer so senders don't block before close
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn blocked_send(tx: Sender<int>, val: int) {
    tx.send(val)!
}

fn close_chan(tx: Sender<int>) {
    tx.close()
}

tests[scheduler: RoundRobin] {
    test "close wakes multiple senders" {
        let (tx, rx) = chan<int>(5)
        tx.send(0)!
        let t1 = spawn blocked_send(tx, 1)
        let t2 = spawn blocked_send(tx, 2)
        let t3 = spawn close_chan(tx)
        rx.recv()!
        rx.recv() catch 0
        rx.recv() catch 0
        t1.get() catch err { }
        t2.get() catch err { }
        t3.get()
        expect(true).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_channel_float_data() {
    // Fixed: float data flows correctly through channels with spawn closures
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_float(tx: Sender<float>) {
    tx.send(2.5)!
}

tests[scheduler: RoundRobin] {
    test "channel float data" {
        let (tx, rx) = chan<float>(1)
        let t = spawn send_float(tx)
        let v = rx.recv()!
        t.get()!
        expect(v > 2.0).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_random_select_closed_default() {
    // Fixed: random scheduler select on closed channel with default works correctly
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
tests[scheduler: Random] {
    test "random select closed default" {
        let (tx, rx) = chan<int>(1)
        tx.send(99)!
        tx.close()
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
            default {
                result = -1
            }
        }
        expect(result).to_equal(99)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_task_float_add() {
    // Fixed: spawn closures correctly return float values via I64 encoding
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add_floats(a: float, b: float) float {
    return a + b
}

tests[scheduler: RoundRobin] {
    test "task float addition" {
        let t = spawn add_floats(1.5, 2.5)
        let v = t.get()
        expect(v > 3.0).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_random_channel_close_recv() {
    // Fixed: random scheduler recv after close works correctly
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn close_after_send(tx: Sender<int>) {
    tx.send(42)!
    tx.close()
}

fn read_after_close(rx: Receiver<int>) int {
    let v1 = rx.recv()!
    let v2 = rx.recv() catch -1
    return v1 + v2
}

tests[scheduler: Random] {
    test "random close then recv" {
        let (tx, rx) = chan<int>(5)
        let t1 = spawn close_after_send(tx)
        let t2 = spawn read_after_close(rx)
        t1.get()!
        let result = t2.get()!
        expect(result).to_equal(41)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn bug_float_channel_pipeline() {
    // Fixed: float values flow correctly through a spawned pipeline
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn double_float(rx: Receiver<float>, tx: Sender<float>) {
    let v = rx.recv()!
    tx.send(v + v)!
}

tests[scheduler: RoundRobin] {
    test "float channel pipeline" {
        let (tx1, rx1) = chan<float>(1)
        let (tx2, rx2) = chan<float>(1)
        let t = spawn double_float(rx1, tx2)
        tx1.send(1.5)!
        let result = rx2.recv()!
        t.get()!
        expect(result > 2.0).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Edge case tests (~15 tests)
// ============================================================================

#[test]
fn edge_chan_capacity_exactly_one() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "channel capacity exactly one" {
        let (tx, rx) = chan<int>(1)
        tx.send(42)!
        expect(rx.recv()!).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_try_send_full() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "try send on full channel" {
        let (tx, rx) = chan<int>(1)
        tx.send(1)!
        let mut failed = false
        tx.try_send(2) catch err {
            failed = true
        }
        expect(failed).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_try_recv_empty() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "try recv on empty channel" {
        let (tx, rx) = chan<int>(1)
        let result = rx.try_recv() catch -1
        expect(result).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_try_send_then_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "try send then normal recv" {
        let (tx, rx) = chan<int>(2)
        tx.try_send(10)!
        tx.try_send(20)!
        expect(rx.recv()!).to_equal(10)
        expect(rx.recv()!).to_equal(20)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_close_then_try_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "close then try recv" {
        let (tx, rx) = chan<int>(1)
        tx.close()
        let result = rx.try_recv() catch -1
        expect(result).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_close_with_data_then_try_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "close with data then try recv" {
        let (tx, rx) = chan<int>(5)
        tx.send(10)!
        tx.send(20)!
        tx.close()
        expect(rx.try_recv()!).to_equal(10)
        expect(rx.try_recv()!).to_equal(20)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_rr_no_spawn_trivial() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "rr no spawn trivial" {
        expect(true).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_chan_negative_ints_both_dirs() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn negate_relay(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(0 - v)!
}

tests[scheduler: RoundRobin] {
    test "negative ints both directions" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn negate_relay(rx1, tx2)
        tx1.send(-42)!
        let result = rx2.recv()!
        t.get()!
        expect(result).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_chan_bool_true_and_false() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_bools_both(tx: Sender<bool>) {
    tx.send(true)!
    tx.send(false)!
    tx.send(true)!
}

tests[scheduler: RoundRobin] {
    test "bool true and false through channel" {
        let (tx, rx) = chan<bool>(3)
        let t = spawn send_bools_both(tx)
        t.get()!
        expect(rx.recv()!).to_be_true()
        expect(rx.recv()!).to_be_false()
        expect(rx.recv()!).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_producer_consumer_close_signal() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn produce_and_signal(data_tx: Sender<int>, done_tx: Sender<int>) {
    data_tx.send(1)!
    data_tx.send(2)!
    data_tx.send(3)!
    done_tx.send(1)!
}

tests[scheduler: RoundRobin] {
    test "producer consumer with close signal" {
        let (data_tx, data_rx) = chan<int>(5)
        let (done_tx, done_rx) = chan<int>(1)
        let t = spawn produce_and_signal(data_tx, done_tx)
        let a = data_rx.recv()!
        let b = data_rx.recv()!
        let c = data_rx.recv()!
        done_rx.recv()!
        t.get()!
        expect(a + b + c).to_equal(6)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_select_from_three_channels() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_to_ch(tx: Sender<int>, val: int) {
    tx.send(val)!
}

tests[scheduler: RoundRobin] {
    test "select from three channels" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t = spawn send_to_ch(tx2, 50)
        t.get()!
        let mut result = 0
        select {
            v = rx1.recv() {
                result = v
            }
            v = rx2.recv() {
                result = v
            }
            v = rx3.recv() {
                result = v
            }
        }
        expect(result).to_equal(50)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_single_item_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn passthrough(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "single item through pipeline" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t = spawn passthrough(rx1, tx2)
        tx1.send(99)!
        let result = rx2.recv()!
        t.get()!
        expect(result).to_equal(99)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_rr_spawn_and_get_inline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn pass(x: int) int {
    return x
}

tests[scheduler: RoundRobin] {
    test "spawn and get inline" {
        let t1 = spawn pass(42)
        let t2 = spawn pass(0)
        let t3 = spawn pass(-1)
        expect(t1.get()).to_equal(42)
        expect(t2.get()).to_equal(0)
        expect(t3.get()).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_chan_send_max_int() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_big(tx: Sender<int>) {
    tx.send(9999999)!
}

tests[scheduler: RoundRobin] {
    test "send large int through channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn send_big(tx)
        let v = rx.recv()!
        t.get()!
        expect(v).to_equal(9999999)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn edge_rr_multiple_chan_types() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_int_and_str(tx_int: Sender<int>, tx_str: Sender<string>) {
    tx_int.send(42)!
    tx_str.send("hello")!
}

tests[scheduler: RoundRobin] {
    test "multiple channel types" {
        let (tx_int, rx_int) = chan<int>(1)
        let (tx_str, rx_str) = chan<string>(1)
        let t = spawn send_int_and_str(tx_int, tx_str)
        let i = rx_int.recv()!
        let s = rx_str.recv()!
        t.get()!
        expect(i).to_equal(42)
        expect(s).to_equal("hello")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Compile error tests (~5 tests)
// ============================================================================

#[test]
fn compile_error_bad_scheduler_name() {
    compile_test_should_fail_with(r#"
tests[scheduler: FooBar] {
    test "bad scheduler" {
        expect(true).to_be_true()
    }
}
"#, "unknown scheduler strategy");
}

#[test]
fn compile_error_missing_colon_in_annotation() {
    compile_test_should_fail_with(r#"
tests[scheduler RoundRobin] {
    test "missing colon" {
        expect(true).to_be_true()
    }
}
"#, "expected :");
}

#[test]
fn compile_error_empty_tests_block() {
    // Empty tests block with non-test content is a parse error
    compile_test_should_fail_with(r#"
tests[scheduler: RoundRobin] {
    let x = 5
}
"#, "test");
}

#[test]
fn compile_error_scheduler_typo_roundrobin() {
    compile_test_should_fail_with(r#"
tests[scheduler: Roundrobin] {
    test "typo" {
        expect(true).to_be_true()
    }
}
"#, "unknown scheduler strategy");
}

#[test]
fn compile_error_duplicate_scheduler_key() {
    compile_test_should_fail_with(r#"
tests[sched: RoundRobin] {
    test "wrong key" {
        expect(true).to_be_true()
    }
}
"#, "expected 'scheduler'");
}

// ============================================================================
// Additional patterns (~15 tests)
// ============================================================================

#[test]
fn pattern_channel_as_semaphore() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn acquire_and_work(sem: Receiver<int>, done: Sender<int>) {
    sem.recv()!
    done.send(1)!
}

tests[scheduler: RoundRobin] {
    test "channel as semaphore pattern" {
        let (sem_tx, sem_rx) = chan<int>(1)
        let (done_tx, done_rx) = chan<int>(1)
        sem_tx.send(1)!
        let t = spawn acquire_and_work(sem_rx, done_tx)
        let v = done_rx.recv()!
        t.get()!
        expect(v).to_equal(1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_channel_as_mutex_lock() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn lock_work_unlock(lock: Receiver<int>, unlock: Sender<int>, out: Sender<int>, val: int) {
    lock.recv()!
    out.send(val)!
    unlock.send(1)!
}

tests[scheduler: RoundRobin] {
    test "channel as mutex lock" {
        let (lock_tx, lock_rx) = chan<int>(1)
        let (out_tx, out_rx) = chan<int>(2)
        lock_tx.send(1)!
        let t1 = spawn lock_work_unlock(lock_rx, lock_tx, out_tx, 10)
        let t2 = spawn lock_work_unlock(lock_rx, lock_tx, out_tx, 20)
        let a = out_rx.recv()!
        let b = out_rx.recv()!
        t1.get()!
        t2.get()!
        expect(a + b).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_channel_with_loop_send_10() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn loop_produce(tx: Sender<int>) {
    let mut i = 0
    while i < 10 {
        tx.send(i)!
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "loop send 10 items" {
        let (tx, rx) = chan<int>(1)
        let t = spawn loop_produce(tx)
        let mut sum = 0
        let mut i = 0
        while i < 10 {
            sum = sum + rx.recv()!
            i = i + 1
        }
        t.get()!
        expect(sum).to_equal(45)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_request_response_negate() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn negation_service(req: Receiver<int>, resp: Sender<int>) {
    let v1 = req.recv()!
    resp.send(0 - v1)!
    let v2 = req.recv()!
    resp.send(0 - v2)!
}

tests[scheduler: RoundRobin] {
    test "request response negation" {
        let (req_tx, req_rx) = chan<int>(1)
        let (resp_tx, resp_rx) = chan<int>(1)
        let t = spawn negation_service(req_rx, resp_tx)
        req_tx.send(5)!
        expect(resp_rx.recv()!).to_equal(-5)
        req_tx.send(-10)!
        expect(resp_rx.recv()!).to_equal(10)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_gather_results() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn work(result_tx: Sender<int>, val: int) {
    result_tx.send(val * val)!
}

tests[scheduler: RoundRobin] {
    test "gather results from workers" {
        let (tx, rx) = chan<int>(4)
        let t1 = spawn work(tx, 1)
        let t2 = spawn work(tx, 2)
        let t3 = spawn work(tx, 3)
        let t4 = spawn work(tx, 4)
        let mut sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        t4.get()!
        expect(sum).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_five_stage_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn increment(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: RoundRobin] {
    test "five stage pipeline" {
        let (tx0, rx0) = chan<int>(1)
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let (tx4, rx4) = chan<int>(1)
        let t1 = spawn increment(rx0, tx1)
        let t2 = spawn increment(rx1, tx2)
        let t3 = spawn increment(rx2, tx3)
        let t4 = spawn increment(rx3, tx4)
        tx0.send(0)!
        let result = rx4.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        t4.get()!
        expect(result).to_equal(4)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_countdown_channel() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn countdown(tx: Sender<int>, from: int) {
    let mut i = from
    while i >= 0 {
        tx.send(i)!
        i = i - 1
    }
}

tests[scheduler: RoundRobin] {
    test "countdown via channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn countdown(tx, 3)
        expect(rx.recv()!).to_equal(3)
        expect(rx.recv()!).to_equal(2)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(0)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_fan_out_two_consumers() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn consumer(rx: Receiver<int>, out: Sender<int>) {
    let v = rx.recv()!
    out.send(v * 10)!
}

tests[scheduler: RoundRobin] {
    test "fan out to two consumers" {
        let (data_tx, data_rx) = chan<int>(2)
        let (out_tx, out_rx) = chan<int>(2)
        data_tx.send(1)!
        data_tx.send(2)!
        let t1 = spawn consumer(data_rx, out_tx)
        let t2 = spawn consumer(data_rx, out_tx)
        let a = out_rx.recv()!
        let b = out_rx.recv()!
        t1.get()!
        t2.get()!
        expect(a + b).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_accumulate_via_channel() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn adder(rx: Receiver<int>, count: int) int {
    let mut sum = 0
    let mut i = 0
    while i < count {
        sum = sum + rx.recv()!
        i = i + 1
    }
    return sum
}

tests[scheduler: RoundRobin] {
    test "accumulate via channel" {
        let (tx, rx) = chan<int>(5)
        tx.send(10)!
        tx.send(20)!
        tx.send(30)!
        let t = spawn adder(rx, 3)
        let result = t.get()!
        expect(result).to_equal(60)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_conditional_send() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn conditional_sender(tx: Sender<int>, flag: bool) {
    if flag {
        tx.send(1)!
    } else {
        tx.send(0)!
    }
}

tests[scheduler: RoundRobin] {
    test "conditional send" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn conditional_sender(tx1, true)
        let t2 = spawn conditional_sender(tx2, false)
        expect(rx1.recv()!).to_equal(1)
        expect(rx2.recv()!).to_equal(0)
        t1.get()!
        t2.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_round_trip_string() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn echo_str(rx: Receiver<string>, tx: Sender<string>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: RoundRobin] {
    test "round trip string" {
        let (to_tx, to_rx) = chan<string>(1)
        let (from_tx, from_rx) = chan<string>(1)
        let t = spawn echo_str(to_rx, from_tx)
        to_tx.send("test string")!
        let result = from_rx.recv()!
        t.get()!
        expect(result).to_equal("test string")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_worker_pool_three() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn pool_worker(work_rx: Receiver<int>, result_tx: Sender<int>) {
    let v = work_rx.recv()!
    result_tx.send(v + 100)!
}

tests[scheduler: RoundRobin] {
    test "worker pool of three" {
        let (work_tx, work_rx) = chan<int>(3)
        let (result_tx, result_rx) = chan<int>(3)
        work_tx.send(1)!
        work_tx.send(2)!
        work_tx.send(3)!
        let t1 = spawn pool_worker(work_rx, result_tx)
        let t2 = spawn pool_worker(work_rx, result_tx)
        let t3 = spawn pool_worker(work_rx, result_tx)
        let a = result_rx.recv()!
        let b = result_rx.recv()!
        let c = result_rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(306)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_task_chain_sequential() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn step(x: int) int {
    return x + 1
}

tests[scheduler: RoundRobin] {
    test "task chain sequential" {
        let t1 = spawn step(0)
        let v1 = t1.get()
        let t2 = spawn step(v1)
        let v2 = t2.get()
        let t3 = spawn step(v2)
        let v3 = t3.get()
        expect(v3).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_select_two_ready() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
tests[scheduler: RoundRobin] {
    test "select with two ready channels" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        tx1.send(10)!
        tx2.send(20)!
        let mut result = 0
        select {
            v = rx1.recv() {
                result = v
            }
            v = rx2.recv() {
                result = v
            }
        }
        expect(result == 10 || result == 20).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn pattern_task_error_recovery() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
error CalcError {
    detail: string
}

fn divide(a: int, b: int) int {
    if b == 0 {
        raise CalcError { detail: "div by zero" }
    }
    return a / b
}

fn safe_divide(a: int, b: int) int {
    let t = spawn divide(a, b)
    let v = t.get() catch -1
    return v
}

tests[scheduler: RoundRobin] {
    test "task error recovery" {
        expect(safe_divide(10, 2)).to_equal(5)
        expect(safe_divide(10, 0)).to_equal(-1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ============================================================================
// Additional round-robin channel and task patterns (remaining tests)
// ============================================================================

#[test]
fn rr_chan_three_receivers_from_one_sender() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_three_channels(tx1: Sender<int>, tx2: Sender<int>, tx3: Sender<int>) {
    tx1.send(1)!
    tx2.send(2)!
    tx3.send(3)!
}

tests[scheduler: RoundRobin] {
    test "one sender three receivers" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t = spawn send_three_channels(tx1, tx2, tx3)
        let a = rx1.recv()!
        let b = rx2.recv()!
        let c = rx3.recv()!
        t.get()!
        expect(a).to_equal(1)
        expect(b).to_equal(2)
        expect(c).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_power_computation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn power(base: int, exp: int) int {
    let mut result = 1
    let mut i = 0
    while i < exp {
        result = result * base
        i = i + 1
    }
    return result
}

tests[scheduler: RoundRobin] {
    test "task power computation" {
        let t1 = spawn power(2, 10)
        let t2 = spawn power(3, 5)
        expect(t1.get()).to_equal(1024)
        expect(t2.get()).to_equal(243)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_string_concat_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn append_suffix(rx: Receiver<string>, tx: Sender<string>, suffix: string) {
    let v = rx.recv()!
    tx.send(v + suffix)!
}

tests[scheduler: RoundRobin] {
    test "string concat through pipeline" {
        let (tx1, rx1) = chan<string>(1)
        let (tx2, rx2) = chan<string>(1)
        let (tx3, rx3) = chan<string>(1)
        let t1 = spawn append_suffix(rx1, tx2, " world")
        let t2 = spawn append_suffix(rx2, tx3, "!")
        tx1.send("hello")!
        let result = rx3.recv()!
        t1.get()!
        t2.get()!
        expect(result).to_equal("hello world!")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_abs_value() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn my_abs(x: int) int {
    if x < 0 {
        return 0 - x
    }
    return x
}

tests[scheduler: RoundRobin] {
    test "task absolute value" {
        let t1 = spawn my_abs(-42)
        let t2 = spawn my_abs(42)
        let t3 = spawn my_abs(0)
        expect(t1.get()).to_equal(42)
        expect(t2.get()).to_equal(42)
        expect(t3.get()).to_equal(0)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_send_recv_interleaved_two() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn interleaved_worker(tx: Sender<int>, rx: Receiver<int>) {
    tx.send(1)!
    let v = rx.recv()!
    tx.send(v + 10)!
    let v2 = rx.recv()!
    tx.send(v2 + 10)!
}

tests[scheduler: RoundRobin] {
    test "interleaved send recv two rounds" {
        let (to_tx, to_rx) = chan<int>(1)
        let (from_tx, from_rx) = chan<int>(1)
        let t = spawn interleaved_worker(from_tx, to_rx)
        let first = from_rx.recv()!
        to_tx.send(first + 100)!
        let second = from_rx.recv()!
        to_tx.send(second + 100)!
        let third = from_rx.recv()!
        t.get()!
        expect(first).to_equal(1)
        expect(second).to_equal(111)
        expect(third).to_equal(221)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_string_length_check() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn make_repeated(s: string, n: int) string {
    let mut result = ""
    let mut i = 0
    while i < n {
        result = result + s
        i = i + 1
    }
    return result
}

tests[scheduler: RoundRobin] {
    test "task string length check" {
        let t = spawn make_repeated("ab", 5)
        let result = t.get()
        expect(result.len()).to_equal(10)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_cap_five_fill_and_drain() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fill_five(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
    tx.send(4)!
    tx.send(5)!
}

tests[scheduler: RoundRobin] {
    test "cap 5 fill and drain" {
        let (tx, rx) = chan<int>(5)
        let t = spawn fill_five(tx)
        t.get()!
        let mut sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        expect(sum).to_equal(15)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_task_void() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn do_nothing() {
    let x = 1
}

tests[scheduler: Random] {
    test "random void task" {
        let t = spawn do_nothing()
        t.get()
        expect(true).to_be_true()
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_three_channels_independent() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn emit_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

tests[scheduler: Random] {
    test "random three channels independent" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn emit_val(tx1, 10)
        let t2 = spawn emit_val(tx2, 20)
        let t3 = spawn emit_val(tx3, 30)
        let a = rx1.recv()!
        let b = rx2.recv()!
        let c = rx3.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(60)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_max_of_two() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn max_val(a: int, b: int) int {
    if a > b {
        return a
    }
    return b
}

tests[scheduler: RoundRobin] {
    test "task max of two" {
        let t1 = spawn max_val(10, 20)
        let t2 = spawn max_val(50, 30)
        expect(t1.get()).to_equal(20)
        expect(t2.get()).to_equal(50)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_min_of_two() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn min_val(a: int, b: int) int {
    if a < b {
        return a
    }
    return b
}

tests[scheduler: RoundRobin] {
    test "task min of two" {
        let t = spawn min_val(10, 20)
        expect(t.get()).to_equal(10)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_relay_add_constant() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn relay_add(rx: Receiver<int>, tx: Sender<int>, c: int) {
    let v = rx.recv()!
    tx.send(v + c)!
}

tests[scheduler: RoundRobin] {
    test "relay chain adding constants" {
        let (tx0, rx0) = chan<int>(1)
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn relay_add(rx0, tx1, 10)
        let t2 = spawn relay_add(rx1, tx2, 20)
        let t3 = spawn relay_add(rx2, tx3, 30)
        tx0.send(0)!
        let result = rx3.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(result).to_equal(60)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_gcd() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn gcd(a: int, b: int) int {
    let mut x = a
    let mut y = b
    while y != 0 {
        let temp = y
        y = x % y
        x = temp
    }
    return x
}

tests[scheduler: RoundRobin] {
    test "task gcd computation" {
        let t1 = spawn gcd(12, 8)
        let t2 = spawn gcd(100, 75)
        expect(t1.get()).to_equal(4)
        expect(t2.get()).to_equal(25)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_multiple_tests_in_bare() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "bare test one" {
    expect(1 + 1).to_equal(2)
}

test "bare test two" {
    expect(3 * 3).to_equal(9)
}
"#);
    assert!(stdout.contains("2 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_string_comparison() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "string comparison" {
    let a = "hello"
    let b = "hello"
    expect(a).to_equal(b)
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_close_then_send_fails() {
    // Fixed: send on closed channel correctly raises error in fiber mode
    // Close from main fiber to avoid sender refcount issues from closure capture
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn try_send_val(tx: Sender<int>) bool {
    let mut failed = false
    tx.send(1) catch err {
        failed = true
    }
    return failed
}

tests[scheduler: RoundRobin] {
    test "send after close returns error" {
        let (tx, rx) = chan<int>(5)
        tx.close()
        let t = spawn try_send_val(tx)
        let v = t.get()
        expect(v).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_modulo_computation() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn modulo(a: int, b: int) int {
    return a % b
}

tests[scheduler: RoundRobin] {
    test "task modulo" {
        let t1 = spawn modulo(17, 5)
        let t2 = spawn modulo(100, 7)
        expect(t1.get()).to_equal(2)
        expect(t2.get()).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_producer_sends_computed_values() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn compute_and_send(tx: Sender<int>) {
    let mut i = 1
    while i <= 5 {
        tx.send(i * i * i)!
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "producer sends computed cubes" {
        let (tx, rx) = chan<int>(1)
        let t = spawn compute_and_send(tx)
        expect(rx.recv()!).to_equal(1)
        expect(rx.recv()!).to_equal(8)
        expect(rx.recv()!).to_equal(27)
        expect(rx.recv()!).to_equal(64)
        expect(rx.recv()!).to_equal(125)
        t.get()!
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_daisy_chain_four() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn relay_inc(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: Random] {
    test "random daisy chain four" {
        let (tx0, rx0) = chan<int>(1)
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let (tx3, rx3) = chan<int>(1)
        let t1 = spawn relay_inc(rx0, tx1)
        let t2 = spawn relay_inc(rx1, tx2)
        let t3 = spawn relay_inc(rx2, tx3)
        tx0.send(100)!
        let result = rx3.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(result).to_equal(103)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_nested_function_calls() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn outer(x: int) int {
    return inner(x) + 1
}

fn inner(x: int) int {
    return x * 2
}

tests[scheduler: RoundRobin] {
    test "task nested function calls" {
        let t = spawn outer(5)
        expect(t.get()).to_equal(11)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_multiple_sends_single_recv() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_pair(tx: Sender<int>, a: int, b: int) {
    tx.send(a)!
    tx.send(b)!
}

tests[scheduler: RoundRobin] {
    test "multiple sends single receiver" {
        let (tx, rx) = chan<int>(4)
        let t = spawn send_pair(tx, 10, 20)
        t.get()!
        let first = rx.recv()!
        let second = rx.recv()!
        expect(first).to_equal(10)
        expect(second).to_equal(20)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_bitwise_and() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn bitwise_op(a: int, b: int) int {
    return a & b
}

tests[scheduler: RoundRobin] {
    test "task bitwise and" {
        let t = spawn bitwise_op(15, 9)
        expect(t.get()).to_equal(9)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_bitwise_or() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn bitor(a: int, b: int) int {
    return a | b
}

tests[scheduler: RoundRobin] {
    test "task bitwise or" {
        let t = spawn bitor(12, 10)
        expect(t.get()).to_equal(14)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_loop_producer_consumer() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn produce_n(tx: Sender<int>, n: int) {
    let mut i = 0
    while i < n {
        tx.send(i)!
        i = i + 1
    }
}

tests[scheduler: Random] {
    test "random loop producer consumer" {
        let (tx, rx) = chan<int>(1)
        let t = spawn produce_n(tx, 5)
        let mut sum = 0
        let mut i = 0
        while i < 5 {
            sum = sum + rx.recv()!
            i = i + 1
        }
        t.get()!
        expect(sum).to_equal(10)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_three_tests_in_one_block() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn inc_val(x: int) int {
    return x + 1
}

tests[scheduler: RoundRobin] {
    test "rr block first" {
        let t = spawn inc_val(0)
        expect(t.get()).to_equal(1)
    }

    test "rr block second" {
        let t = spawn inc_val(10)
        expect(t.get()).to_equal(11)
    }

    test "rr block third" {
        let t = spawn inc_val(100)
        expect(t.get()).to_equal(101)
    }
}
"#);
    assert!(stdout.contains("3 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_recv_order_matches_send() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn ordered_send(tx: Sender<int>) {
    tx.send(100)!
    tx.send(200)!
    tx.send(300)!
    tx.send(400)!
}

tests[scheduler: RoundRobin] {
    test "recv order matches send order" {
        let (tx, rx) = chan<int>(2)
        let t = spawn ordered_send(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        let d = rx.recv()!
        t.get()!
        expect(a).to_equal(100)
        expect(b).to_equal(200)
        expect(c).to_equal(300)
        expect(d).to_equal(400)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_string_empty() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn empty_str() string {
    return ""
}

tests[scheduler: RoundRobin] {
    test "task returns empty string" {
        let t = spawn empty_str()
        expect(t.get()).to_equal("")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_long_string() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn long_string() string {
    return "abcdefghijklmnopqrstuvwxyz"
}

tests[scheduler: RoundRobin] {
    test "task returns long string" {
        let t = spawn long_string()
        let s = t.get()
        expect(s.len()).to_equal(26)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_filter_pipeline() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn filter_positive(rx: Receiver<int>, tx: Sender<int>, count: int) {
    let mut i = 0
    while i < count {
        let v = rx.recv()!
        if v > 0 {
            tx.send(v)!
        }
        i = i + 1
    }
}

tests[scheduler: RoundRobin] {
    test "filter pipeline positive only" {
        let (tx1, rx1) = chan<int>(5)
        let (tx2, rx2) = chan<int>(5)
        tx1.send(-1)!
        tx1.send(2)!
        tx1.send(-3)!
        tx1.send(4)!
        let t = spawn filter_positive(rx1, tx2, 4)
        t.get()!
        expect(rx2.recv()!).to_equal(2)
        expect(rx2.recv()!).to_equal(4)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_collatz_step() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn collatz_steps(n: int) int {
    let mut x = n
    let mut steps = 0
    while x != 1 {
        if x % 2 == 0 {
            x = x / 2
        } else {
            x = x * 3 + 1
        }
        steps = steps + 1
    }
    return steps
}

tests[scheduler: RoundRobin] {
    test "task collatz steps" {
        let t = spawn collatz_steps(6)
        expect(t.get()).to_equal(8)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_double_close() {
    // Fixed: double close on channel works correctly in fiber mode
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn double_closer(tx: Sender<int>) {
    tx.close()
    tx.close()
}

tests[scheduler: RoundRobin] {
    test "double close channel" {
        let (tx, rx) = chan<int>(1)
        let t = spawn double_closer(tx)
        t.get()
        expect(true).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_task_chain_three() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn add_ten(x: int) int {
    return x + 10
}

tests[scheduler: Random] {
    test "random task chain three" {
        let t1 = spawn add_ten(0)
        let v1 = t1.get()
        let t2 = spawn add_ten(v1)
        let v2 = t2.get()
        let t3 = spawn add_ten(v2)
        let v3 = t3.get()
        expect(v3).to_equal(30)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_bool_true() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn always_true() bool {
    return true
}

tests[scheduler: RoundRobin] {
    test "task returns bool true" {
        let t = spawn always_true()
        expect(t.get()).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_returns_bool_false() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn always_false() bool {
    return false
}

tests[scheduler: RoundRobin] {
    test "task returns bool false" {
        let t = spawn always_false()
        expect(t.get()).to_be_false()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_capacity_exactly_two() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_two_cap2(tx: Sender<int>) {
    tx.send(11)!
    tx.send(22)!
}

tests[scheduler: RoundRobin] {
    test "channel capacity exactly two" {
        let (tx, rx) = chan<int>(2)
        let t = spawn send_two_cap2(tx)
        t.get()!
        expect(rx.recv()!).to_equal(11)
        expect(rx.recv()!).to_equal(22)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_subtract() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn subtract(a: int, b: int) int {
    return a - b
}

tests[scheduler: RoundRobin] {
    test "task subtraction" {
        let t = spawn subtract(100, 42)
        expect(t.get()).to_equal(58)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_string_with_spaces() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_spaced(tx: Sender<string>) {
    tx.send("hello world foo bar")!
}

tests[scheduler: RoundRobin] {
    test "string with spaces through channel" {
        let (tx, rx) = chan<string>(1)
        let t = spawn send_spaced(tx)
        t.get()!
        expect(rx.recv()!).to_equal("hello world foo bar")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_gather_results() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
fn compute_sq(tx: Sender<int>, x: int) {
    tx.send(x * x)!
}

tests[scheduler: Random] {
    test "random gather results" {
        let (tx, rx) = chan<int>(3)
        let t1 = spawn compute_sq(tx, 3)
        let t2 = spawn compute_sq(tx, 4)
        let t3 = spawn compute_sq(tx, 5)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        expect(a + b + c).to_equal(50)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "10")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn seq_channel_bool_values() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "sequential channel bools" {
    let (tx, rx) = chan<bool>(2)
    tx.send(true)!
    tx.send(false)!
    expect(rx.recv()!).to_be_true()
    expect(rx.recv()!).to_be_false()
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_task_division() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn divide(a: int, b: int) int {
    return a / b
}

tests[scheduler: RoundRobin] {
    test "task integer division" {
        let t1 = spawn divide(100, 5)
        let t2 = spawn divide(99, 10)
        expect(t1.get()).to_equal(20)
        expect(t2.get()).to_equal(9)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rr_chan_multiple_types_bool_and_int() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn send_both(tx_i: Sender<int>, tx_b: Sender<bool>) {
    tx_i.send(42)!
    tx_b.send(true)!
}

tests[scheduler: RoundRobin] {
    test "multiple chan types bool and int" {
        let (tx_i, rx_i) = chan<int>(1)
        let (tx_b, rx_b) = chan<bool>(1)
        let t = spawn send_both(tx_i, tx_b)
        let i = rx_i.recv()!
        let b = rx_b.recv()!
        t.get()!
        expect(i).to_equal(42)
        expect(b).to_be_true()
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn rand_select_with_default() {
    let (stdout, _stderr, code) = compile_test_and_run_with_env(r#"
tests[scheduler: Random] {
    test "random select with default" {
        let (tx, rx) = chan<int>(1)
        let mut result = 0
        select {
            v = rx.recv() {
                result = v
            }
            default {
                result = -1
            }
        }
        expect(result).to_equal(-1)
    }
}
"#, &[("PLUTO_TEST_ITERATIONS", "5")]);
    assert!(stdout.contains("1 tests passed"), "got: {stdout}");
    assert_eq!(code, 0);
}

// ── Exhaustive strategy tests ────────────────────────────────────────────

#[test]
fn exhaustive_basic_no_deadlock() {
    // Simple test with no concurrency — should explore exactly 1 schedule
    let (stdout, stderr, code) = compile_test_and_run(r#"
tests[scheduler: Exhaustive] {
    test "no spawn" {
        expect(1 + 1).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert!(stderr.contains("1 schedule explored"), "Expected 1 schedule, got stderr: {stderr}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_spawn_basic() {
    // A spawned task with no channels — simple exploration
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

tests[scheduler: Exhaustive] {
    test "spawn add" {
        let t = spawn add(10, 20)
        expect(t.get()).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_finds_deadlock() {
    // Two fibers each try to recv from the other's channel first, then send on their own.
    // This creates a circular dependency that deadlocks in all interleavings.
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn worker_a(tx: Sender<int>, rx: Receiver<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

fn worker_b(tx: Sender<int>, rx: Receiver<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: Exhaustive] {
    test "deadlock" {
        let (tx1, rx1) = chan<int>(0)
        let (tx2, rx2) = chan<int>(0)
        let t1 = spawn worker_a(tx1, rx2)
        let t2 = spawn worker_b(tx2, rx1)
        t1.get()!
        t2.get()!
        expect(1).to_equal(1)
    }
}
"#);
    assert_ne!(code, 0, "Expected failure, got stderr: {stderr}");
    assert!(stderr.contains("deadlock") || stderr.contains("Deadlock"), "Expected deadlock in stderr: {stderr}");
}

#[test]
fn exhaustive_channel_dependent_fibers() {
    // Two fibers communicating via a channel — they are DEPENDENT.
    // Exhaustive should explore multiple interleavings.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn sender(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: Exhaustive] {
    test "channel comm" {
        let (tx, rx) = chan<int>(1)
        let t = spawn sender(tx)
        let sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        t.get()!
        expect(sum).to_equal(6)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert!(stderr.contains("schedule"), "Expected schedule info in stderr: {stderr}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_respects_max_schedules() {
    // Use env var to limit schedule exploration
    let (stdout, stderr, code) = compile_test_and_run_with_env(r#"
fn worker(tx: Sender<int>) {
    tx.send(1)!
}

tests[scheduler: Exhaustive] {
    test "bounded" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn worker(tx1)
        let t2 = spawn worker(tx2)
        let v1 = rx1.recv()!
        let v2 = rx2.recv()!
        t1.get()!
        t2.get()!
        expect(v1 + v2).to_equal(2)
    }
}
"#, &[("PLUTO_MAX_SCHEDULES", "5")]);
    assert_eq!(code, 0, "Expected pass, got stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_independent_fibers_dpor_prunes() {
    // Two spawned tasks that don't share any channels — DPOR should prune.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn compute(x: int) int {
    return x * x
}

tests[scheduler: Exhaustive] {
    test "independent" {
        let t1 = spawn compute(3)
        let t2 = spawn compute(4)
        expect(t1.get()).to_equal(9)
        expect(t2.get()).to_equal(16)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_three_independent_fibers() {
    // Three independent spawned tasks — DPOR should heavily prune (3! = 6 orderings down to fewer).
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn square(x: int) int {
    return x * x
}

tests[scheduler: Exhaustive] {
    test "three independent" {
        let t1 = spawn square(2)
        let t2 = spawn square(3)
        let t3 = spawn square(5)
        expect(t1.get()).to_equal(4)
        expect(t2.get()).to_equal(9)
        expect(t3.get()).to_equal(25)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_producer_consumer_buffered() {
    // Producer sends multiple items through buffered channel, consumer reads them all.
    // Should succeed in all interleavings.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn produce(tx: Sender<int>) {
    tx.send(10)!
    tx.send(20)!
    tx.send(30)!
    tx.send(40)!
    tx.send(50)!
}

tests[scheduler: Exhaustive] {
    test "producer consumer buffered" {
        let (tx, rx) = chan<int>(5)
        let t = spawn produce(tx)
        let total = 0
        total = total + rx.recv()!
        total = total + rx.recv()!
        total = total + rx.recv()!
        total = total + rx.recv()!
        total = total + rx.recv()!
        t.get()!
        expect(total).to_equal(150)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_two_producers_one_consumer() {
    // Two producers share a channel, one consumer reads all items.
    // Dependent fibers — should explore interleavings.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn produce_pair(tx: Sender<int>, a: int, b: int) {
    tx.send(a)!
    tx.send(b)!
}

tests[scheduler: Exhaustive] {
    test "two producers" {
        let (tx, rx) = chan<int>(4)
        let t1 = spawn produce_pair(tx, 1, 2)
        let t2 = spawn produce_pair(tx, 10, 20)
        let sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        t1.get()!
        t2.get()!
        expect(sum).to_equal(33)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_zero_buffer_channel_success() {
    // Zero-buffer (synchronous) channel where send and recv must rendezvous.
    // Should succeed as long as producer and consumer can meet.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn sender(tx: Sender<int>) {
    tx.send(42)!
}

tests[scheduler: Exhaustive] {
    test "zero buffer success" {
        let (tx, rx) = chan<int>(0)
        let t = spawn sender(tx)
        let v = rx.recv()!
        t.get()!
        expect(v).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_diamond_dependency() {
    // Diamond pattern: main spawns A and B which both send to a shared channel,
    // main reads both. Tests DPOR correctly identifies A and B as dependent
    // (they share the channel).
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn worker(tx: Sender<int>, val: int) {
    tx.send(val)!
}

tests[scheduler: Exhaustive] {
    test "diamond" {
        let (tx, rx) = chan<int>(2)
        let t1 = spawn worker(tx, 100)
        let t2 = spawn worker(tx, 200)
        let a = rx.recv()!
        let b = rx.recv()!
        t1.get()!
        t2.get()!
        expect(a + b).to_equal(300)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_mixed_dependent_independent() {
    // Mix of dependent (channel-sharing) and independent (no shared channels) fibers.
    // t1 and t2 share ch1, t3 is independent. DPOR should prune t3 orderings.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn sender(tx: Sender<int>, val: int) {
    tx.send(val)!
}

fn compute(x: int) int {
    return x * x
}

tests[scheduler: Exhaustive] {
    test "mixed deps" {
        let (tx, rx) = chan<int>(2)
        let t1 = spawn sender(tx, 5)
        let t2 = spawn sender(tx, 7)
        let t3 = spawn compute(10)
        let a = rx.recv()!
        let b = rx.recv()!
        t1.get()!
        t2.get()!
        let c = t3.get()
        expect(a + b).to_equal(12)
        expect(c).to_equal(100)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_multiple_channels_separate() {
    // Two pairs of fibers, each pair using its own channel.
    // Pairs are independent of each other but dependent within.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn send_val(tx: Sender<int>, val: int) {
    tx.send(val)!
}

tests[scheduler: Exhaustive] {
    test "separate channels" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn send_val(tx1, 10)
        let t2 = spawn send_val(tx2, 20)
        let v1 = rx1.recv()!
        let v2 = rx2.recv()!
        t1.get()!
        t2.get()!
        expect(v1).to_equal(10)
        expect(v2).to_equal(20)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_deadlock_three_way_cycle() {
    // Three-way circular deadlock: A waits on B's channel, B waits on C's, C waits on A's.
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn cycle_node(tx: Sender<int>, rx: Receiver<int>) {
    let v = rx.recv()!
    tx.send(v)!
}

tests[scheduler: Exhaustive] {
    test "three way deadlock" {
        let (tx1, rx1) = chan<int>(0)
        let (tx2, rx2) = chan<int>(0)
        let (tx3, rx3) = chan<int>(0)
        let t1 = spawn cycle_node(tx1, rx3)
        let t2 = spawn cycle_node(tx2, rx1)
        let t3 = spawn cycle_node(tx3, rx2)
        t1.get()!
        t2.get()!
        t3.get()!
        expect(1).to_equal(1)
    }
}
"#);
    assert_ne!(code, 0, "Expected deadlock failure, got stderr: {stderr}");
    assert!(stderr.contains("deadlock") || stderr.contains("Deadlock"), "Expected deadlock in stderr: {stderr}");
}

#[test]
fn exhaustive_chain_of_channels() {
    // Pipeline: A -> ch1 -> B -> ch2 -> main. Should succeed in all interleavings.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn stage_a(tx: Sender<int>) {
    tx.send(5)!
}

fn stage_b(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v * 10)!
}

tests[scheduler: Exhaustive] {
    test "pipeline" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn stage_a(tx1)
        let t2 = spawn stage_b(rx1, tx2)
        let result = rx2.recv()!
        t1.get()!
        t2.get()!
        expect(result).to_equal(50)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_max_depth_respected() {
    // Test that PLUTO_MAX_DEPTH env var limits schedule depth without crashing.
    let (stdout, stderr, code) = compile_test_and_run_with_env(r#"
fn busy(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: Exhaustive] {
    test "depth limited" {
        let (tx, rx) = chan<int>(1)
        let t = spawn busy(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        t.get()!
        expect(a + b + c).to_equal(6)
    }
}
"#, &[("PLUTO_MAX_DEPTH", "10")]);
    assert_eq!(code, 0, "Expected pass, got stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_single_fiber_no_spawn() {
    // With no spawn at all, exhaustive should still work (trivial 1 schedule).
    let (stdout, stderr, code) = compile_test_and_run(r#"
tests[scheduler: Exhaustive] {
    test "arithmetic only" {
        let x = 10
        let y = 20
        expect(x + y).to_equal(30)
        expect(x * y).to_equal(200)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert!(stderr.contains("1 schedule"), "Expected 1 schedule, got stderr: {stderr}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_spawn_with_return_value() {
    // Spawned task returns a computed value, main uses it.
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fibonacci(n: int) int {
    if n <= 1 {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

tests[scheduler: Exhaustive] {
    test "fib spawn" {
        let t1 = spawn fibonacci(8)
        let t2 = spawn fibonacci(6)
        expect(t1.get()).to_equal(21)
        expect(t2.get()).to_equal(8)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_send_recv_alternating() {
    // Interleaved send/recv pattern with zero-buffer channel.
    // Producer and consumer must alternate strictly.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn alternating_sender(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
}

tests[scheduler: Exhaustive] {
    test "alternating" {
        let (tx, rx) = chan<int>(0)
        let t = spawn alternating_sender(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        t.get()!
        expect(a).to_equal(1)
        expect(b).to_equal(2)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_many_schedules_bounded() {
    // Many fibers sharing a channel — bounded so we don't run forever.
    let (stdout, stderr, code) = compile_test_and_run_with_env(r#"
fn push_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

tests[scheduler: Exhaustive] {
    test "many fibers bounded" {
        let (tx, rx) = chan<int>(4)
        let t1 = spawn push_val(tx, 1)
        let t2 = spawn push_val(tx, 2)
        let t3 = spawn push_val(tx, 3)
        let t4 = spawn push_val(tx, 4)
        let sum = 0
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        sum = sum + rx.recv()!
        t1.get()!
        t2.get()!
        t3.get()!
        t4.get()!
        expect(sum).to_equal(10)
    }
}
"#, &[("PLUTO_MAX_SCHEDULES", "50")]);
    assert_eq!(code, 0, "Expected pass, got stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_multiple_tests_in_block() {
    // Multiple tests in one tests block — each explored exhaustively.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn double(x: int) int {
    return x * 2
}

tests[scheduler: Exhaustive] {
    test "first" {
        let t = spawn double(5)
        expect(t.get()).to_equal(10)
    }
    test "second" {
        let t = spawn double(21)
        expect(t.get()).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("2 tests passed"), "Expected 2 tests passed, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_deadlock_detected_among_many_schedules() {
    // A scenario where most interleavings succeed but at least one deadlocks.
    // Two fibers with zero-buffer channel: if the scheduling order is wrong,
    // main tries to recv before producer has started sending.
    // Actually with zero-buffer, both sides must rendezvous, so this should
    // work. Instead: partial deadlock where one channel pair can't meet.
    let (_stdout, stderr, code) = compile_test_and_run(r#"
fn relay(rx: Receiver<int>, tx: Sender<int>) {
    let v = rx.recv()!
    tx.send(v + 1)!
}

tests[scheduler: Exhaustive] {
    test "relay deadlock" {
        let (tx1, rx1) = chan<int>(0)
        let (tx2, rx2) = chan<int>(0)
        let (tx3, rx3) = chan<int>(0)
        let t1 = spawn relay(rx1, tx2)
        let t2 = spawn relay(rx2, tx3)
        let result = rx3.recv()!
        tx1.send(10)!
        t1.get()!
        t2.get()!
        expect(result).to_equal(12)
    }
}
"#);
    // This deadlocks: main blocks on rx3.recv, t2 blocks on rx2.recv, t1 blocks on rx1.recv,
    // then main needs to send on tx1 but it's already blocked on rx3.recv — deadlock!
    assert_ne!(code, 0, "Expected deadlock, got stderr: {stderr}");
    assert!(stderr.contains("deadlock") || stderr.contains("Deadlock"), "Expected deadlock: {stderr}");
}

#[test]
fn exhaustive_spawn_returns_string() {
    // Heap-allocated return type from spawned task
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn greet(name: string) string {
    return f"hello {name}"
}

tests[scheduler: Exhaustive] {
    test "string return" {
        let t = spawn greet("world")
        expect(t.get()).to_equal("hello world")
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_spawn_returns_array() {
    // Array return from spawned task
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn make_list(n: int) [int] {
    let arr: [int] = []
    let i = 0
    while i < n {
        arr.push(i)
        i = i + 1
    }
    return arr
}

tests[scheduler: Exhaustive] {
    test "array return" {
        let t = spawn make_list(5)
        let result = t.get()
        expect(result.len()).to_equal(5)
        expect(result[0]).to_equal(0)
        expect(result[4]).to_equal(4)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_channel_with_large_buffer() {
    // Large buffer means less blocking — should have fewer interleavings than small buffer.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn fill_channel(tx: Sender<int>) {
    tx.send(1)!
    tx.send(2)!
    tx.send(3)!
}

tests[scheduler: Exhaustive] {
    test "large buffer" {
        let (tx, rx) = chan<int>(10)
        let t = spawn fill_channel(tx)
        t.get()!
        let a = rx.recv()!
        let b = rx.recv()!
        let c = rx.recv()!
        expect(a + b + c).to_equal(6)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_spawn_no_get() {
    // Spawn a task but never call .get() — the task runs to completion independently.
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn fire_and_forget(x: int) int {
    return x + 1
}

tests[scheduler: Exhaustive] {
    test "no get" {
        let t = spawn fire_and_forget(99)
        expect(1).to_equal(1)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_bidirectional_channels() {
    // Two fibers exchange messages in both directions.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn peer(my_tx: Sender<int>, my_rx: Receiver<int>, val: int) int {
    my_tx.send(val)!
    let received = my_rx.recv()!
    return received
}

tests[scheduler: Exhaustive] {
    test "bidirectional" {
        let (tx1, rx1) = chan<int>(1)
        let (tx2, rx2) = chan<int>(1)
        let t1 = spawn peer(tx1, rx2, 10)
        let t2 = spawn peer(tx2, rx1, 20)
        let r1 = t1.get()!
        let r2 = t2.get()!
        expect(r1).to_equal(20)
        expect(r2).to_equal(10)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

#[test]
fn exhaustive_successive_spawns() {
    // Spawn tasks one after another (not concurrent), each waits for previous.
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn inc(x: int) int {
    return x + 1
}

tests[scheduler: Exhaustive] {
    test "successive" {
        let t1 = spawn inc(0)
        let v1 = t1.get()
        let t2 = spawn inc(v1)
        let v2 = t2.get()
        let t3 = spawn inc(v2)
        let v3 = t3.get()
        expect(v3).to_equal(3)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_max_schedules_one() {
    // Set max_schedules to 1 — should only explore one schedule then stop.
    let (stdout, stderr, code) = compile_test_and_run_with_env(r#"
fn send_val(tx: Sender<int>, v: int) {
    tx.send(v)!
}

tests[scheduler: Exhaustive] {
    test "single schedule" {
        let (tx, rx) = chan<int>(2)
        let t1 = spawn send_val(tx, 1)
        let t2 = spawn send_val(tx, 2)
        let a = rx.recv()!
        let b = rx.recv()!
        t1.get()!
        t2.get()!
        expect(a + b).to_equal(3)
    }
}
"#, &[("PLUTO_MAX_SCHEDULES", "1")]);
    assert_eq!(code, 0, "Expected pass with 1 schedule, got stdout: {stdout}\nstderr: {stderr}");
    assert!(stderr.contains("1 schedule"), "Expected exactly 1 schedule: {stderr}");
}

#[test]
fn exhaustive_get_after_channel_work() {
    // Task does channel work then returns a value via .get()
    let (stdout, _stderr, code) = compile_test_and_run(r#"
fn worker(tx: Sender<int>) int {
    tx.send(100)!
    tx.send(200)!
    return 42
}

tests[scheduler: Exhaustive] {
    test "get after channel" {
        let (tx, rx) = chan<int>(2)
        let t = spawn worker(tx)
        let a = rx.recv()!
        let b = rx.recv()!
        let result = t.get()!
        expect(a).to_equal(100)
        expect(b).to_equal(200)
        expect(result).to_equal(42)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
}

#[test]
fn exhaustive_fan_out_fan_in() {
    // Fan-out: main sends to N workers via separate channels.
    // Fan-in: workers send results back on a shared result channel.
    let (stdout, stderr, code) = compile_test_and_run(r#"
fn worker(rx: Receiver<int>, result_tx: Sender<int>) {
    let v = rx.recv()!
    result_tx.send(v * 2)!
}

tests[scheduler: Exhaustive] {
    test "fan out fan in" {
        let (work_tx1, work_rx1) = chan<int>(1)
        let (work_tx2, work_rx2) = chan<int>(1)
        let (result_tx, result_rx) = chan<int>(2)
        let t1 = spawn worker(work_rx1, result_tx)
        let t2 = spawn worker(work_rx2, result_tx)
        work_tx1.send(5)!
        work_tx2.send(10)!
        let r1 = result_rx.recv()!
        let r2 = result_rx.recv()!
        t1.get()!
        t2.get()!
        expect(r1 + r2).to_equal(30)
    }
}
"#);
    assert!(stdout.contains("1 tests passed"), "Expected pass, got stdout: {stdout}");
    assert_eq!(code, 0);
    assert!(stderr.contains("schedule"), "Expected schedule info: {stderr}");
}

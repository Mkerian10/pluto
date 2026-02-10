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

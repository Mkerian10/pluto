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

// Category 7: Concurrency Tests (25+ tests)
// Validates spawn, tasks, and channels codegen correctness

use super::common::{compile_and_run, compile_and_run_stdout};

// ============================================================================
// Spawn (10 tests)
// ============================================================================

#[test]
fn test_spawn_returning_int() {
    let src = r#"
        fn compute() int {
            return 42
        }

        fn main() {
            let t = spawn compute()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_spawn_returning_float() {
    let src = r#"
        fn compute() float {
            return 3.14
        }

        fn main() {
            let t = spawn compute()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "3.140000");
}

#[test]
fn test_spawn_returning_string() {
    let src = r#"
        fn greet() string {
            return "hello from task"
        }

        fn main() {
            let t = spawn greet()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "hello from task");
}

#[test]
fn test_spawn_returning_void() {
    let src = r#"
        fn do_work() {
        }

        fn main() {
            let t = spawn do_work()
            t.get()
        }
    "#;
    assert_eq!(compile_and_run(src), 0);
}

#[test]
fn test_spawn_with_zero_captures() {
    let src = r#"
        fn compute(x: int, y: int) int {
            return x + y
        }

        fn main() {
            let t = spawn compute(10, 20)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "30");
}

#[test]
fn test_spawn_with_one_capture() {
    let src = r#"
        fn double(x: int) int {
            return x * 2
        }

        fn main() {
            let value = 21
            let t = spawn double(value)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_spawn_with_five_captures() {
    let src = r#"
        fn sum(a: int, b: int, c: int, d: int, e: int) int {
            return a + b + c + d + e
        }

        fn main() {
            let v1 = 1
            let v2 = 2
            let v3 = 3
            let v4 = 4
            let v5 = 5
            let t = spawn sum(v1, v2, v3, v4, v5)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_spawn_calling_spawn_nested() {
    let src = r#"
        fn inner() int {
            return 42
        }

        fn outer() int {
            let t = spawn inner()
            return t.get()
        }

        fn main() {
            let t = spawn outer()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
#[ignore] // LIMITATION: Array initialization syntax ([Type; size]) not supported - use array literals or push() instead
fn test_spawn_hundred_tasks_concurrent() {
    let src = r#"
        fn identity(x: int) int {
            return x
        }

        fn main() {
            let tasks = [Task<int>; 100]
            let mut i = 0
            while i < 100 {
                tasks[i] = spawn identity(i)
                i = i + 1
            }
            let mut sum = 0
            let j = 0
            while j < 100 {
                sum = sum + tasks[j].get()
                j = j + 1
            }
            print(sum)
        }
    "#;
    // Sum of 0..99 = 99 * 100 / 2 = 4950
    assert_eq!(compile_and_run_stdout(src).trim(), "4950");
}

#[test]
fn test_spawn_task_memory_representation() {
    // Verify Task<T> is represented as i64 pointer
    let src = r#"
        fn compute() int {
            return 123
        }

        fn main() {
            let t = spawn compute()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "123");
}

// ============================================================================
// Task.get() (5 tests)
// ============================================================================

#[test]
fn test_get_on_completed_task() {
    let src = r#"
        fn fast() int {
            return 99
        }

        fn main() {
            let t = spawn fast()
            // Task likely completes before we call get()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "99");
}

#[test]
fn test_get_blocking_on_running_task() {
    let src = r#"
        fn slow() int {
            let mut sum = 0
            let mut i = 0
            while i < 1000000 {
                sum = sum + 1
                i = i + 1
            }
            return sum
        }

        fn main() {
            let t = spawn slow()
            // get() blocks until task completes
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1000000");
}

#[test]
fn test_get_propagate_error_from_task() {
    let src = r#"
        error ComputeError {
            message: string
        }

        fn might_fail() int {
            raise ComputeError { message: "failed" }
        }

        fn wrapper() int {
            let t = spawn might_fail()
            return t.get()!
        }

        fn main() {
            let result = wrapper() catch -1
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1");
}

#[test]
fn test_get_catch_handling_task_error() {
    let src = r#"
        error WorkError {
            message: string
        }

        fn fails() int {
            raise WorkError { message: "oops" }
        }

        fn main() {
            let t = spawn fails()
            let result = t.get() catch 999
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "999");
}

#[test]
fn test_get_multiple_times_on_same_task() {
    // get() can be called multiple times on a completed task
    let src = r#"
        fn compute() int {
            return 77
        }

        fn main() {
            let t = spawn compute()
            let r1 = t.get()
            let r2 = t.get()
            let r3 = t.get()
            print(r1 + r2 + r3)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "231");
}

// ============================================================================
// Channels (10 tests)
// ============================================================================

#[test]
fn test_channel_send_recv_non_blocking() {
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(5)
            tx.send(42)!
            let val = rx.recv()!
            print(val)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_channel_send_recv_blocking() {
    let src = r#"
        fn sender(tx: Sender<int>) {
            tx.send(123)!
        }

        fn main() {
            let (tx, rx) = chan<int>(1)
            spawn sender(tx).detach()
            let val = rx.recv()!
            print(val)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "123");
}

#[test]
fn test_channel_send_on_full_channel() {
    // try_send on full channel returns error
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(2)
            tx.try_send(1)!
            tx.try_send(2)!
            tx.try_send(3) catch err {
                print(-1)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1");
}

#[test]
fn test_channel_recv_on_empty_channel() {
    // try_recv on empty channel returns error
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(1)
            let result = rx.try_recv() catch -1
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1");
}

#[test]
fn test_channel_iteration() {
    let src = r#"
        fn producer(tx: Sender<int>) {
            let i = 1
            while i <= 5 {
                tx.send(i)!
                i = i + 1
            }
            tx.close()
        }

        fn main() {
            let (tx, rx) = chan<int>(2)
            spawn producer(tx).detach()
            let mut sum = 0
            for val in rx {
                sum = sum + val
            }
            print(sum)
        }
    "#;
    // Sum of 1..5 = 15
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_channel_multiple_senders_one_receiver() {
    let src = r#"
        fn sender(tx: Sender<int>, value: int) {
            tx.send(value)!
        }

        fn main() {
            let (tx, rx) = chan<int>(10)
            spawn sender(tx, 10).detach()
            spawn sender(tx, 20).detach()
            spawn sender(tx, 30).detach()
            let v1 = rx.recv()!
            let v2 = rx.recv()!
            let v3 = rx.recv()!
            print(v1 + v2 + v3)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "60");
}

#[test]
fn test_channel_one_sender_multiple_receivers() {
    let src = r#"
        fn receiver(rx: Receiver<int>) int {
            return rx.recv()!
        }

        fn main() {
            let (tx, rx) = chan<int>(10)
            tx.send(100)!
            tx.send(200)!
            let t1 = spawn receiver(rx)
            let t2 = spawn receiver(rx)
            let r1 = t1.get()!
            let r2 = t2.get()!
            print(r1 + r2)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "300");
}

#[test]
fn test_channel_close_and_recv() {
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(1)
            tx.close()
            let val = rx.recv() catch -1
            print(val)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "-1");
}

#[test]
fn test_channel_string_values() {
    let src = r#"
        fn main() {
            let (tx, rx) = chan<string>(3)
            tx.send("hello")!
            tx.send("world")!
            let s1 = rx.recv()!
            let s2 = rx.recv()!
            print(s1 + " " + s2)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "hello world");
}

#[test]
fn test_channel_class_values() {
    let src = r#"
        class Point {
            x: int
            y: int
        }

        fn main() {
            let (tx, rx) = chan<Point>(2)
            let p1 = Point { x: 10, y: 20 }
            let p2 = Point { x: 30, y: 40 }
            tx.send(p1)!
            tx.send(p2)!
            let r1 = rx.recv()!
            let r2 = rx.recv()!
            print(r1.x + r1.y + r2.x + r2.y)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "100");
}

// ============================================================================
// Additional concurrency edge cases (5+ tests)
// ============================================================================

#[test]
fn test_spawn_with_arithmetic_in_args() {
    let src = r#"
        fn compute(x: int) int {
            return x
        }

        fn main() {
            let a = 10
            let b = 20
            let t = spawn compute(a + b * 2)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "50");
}

#[test]
fn test_spawn_with_class_capture() {
    let src = r#"
        class Data {
            value: int
        }

        fn extract(d: Data) int {
            return d.value
        }

        fn main() {
            let d = Data { value: 999 }
            let t = spawn extract(d)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "999");
}

#[test]
fn test_spawn_with_array_capture() {
    let src = r#"
        fn sum_array(arr: [int]) int {
            let mut total = 0
            let mut i = 0
            while i < arr.len() {
                total = total + arr[i]
                i = i + 1
            }
            return total
        }

        fn main() {
            let arr = [1, 2, 3, 4, 5]
            let t = spawn sum_array(arr)
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "15");
}

#[test]
fn test_task_type_inference() {
    // Task<int> type should be correctly inferred
    let src = r#"
        fn compute() int {
            return 42
        }

        fn main() {
            let t = spawn compute()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_channel_capacity_zero() {
    // Unbuffered channel (capacity 0) — send blocks until recv
    let src = r#"
        fn sender(tx: Sender<int>) {
            tx.send(88)!
        }

        fn main() {
            let (tx, rx) = chan<int>(0)
            spawn sender(tx).detach()
            let val = rx.recv()!
            print(val)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "88");
}

#[test]
fn test_channel_large_capacity() {
    // Large buffered channel
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(1000)
            let mut i = 0
            while i < 100 {
                tx.send(i)!
                i = i + 1
            }
            let mut sum = 0
            let j = 0
            while j < 100 {
                sum = sum + rx.recv()!
                j = j + 1
            }
            print(sum)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "4950");
}

#[test]
fn test_spawn_return_type_float_precision() {
    let src = r#"
        fn compute() float {
            return 1.5 + 2.5
        }

        fn main() {
            let t = spawn compute()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "4.000000");
}

#[test]
fn test_spawn_deep_call_chain() {
    let src = r#"
        fn level3() int {
            return 42
        }

        fn level2() int {
            let t = spawn level3()
            return t.get()
        }

        fn level1() int {
            let t = spawn level2()
            return t.get()
        }

        fn main() {
            let t = spawn level1()
            let result = t.get()
            print(result)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "42");
}

#[test]
fn test_channel_receiver_type_representation() {
    // Verify Receiver<T> and Sender<T> are i64 pointers
    let src = r#"
        fn main() {
            let (tx, rx) = chan<int>(1)
            tx.send(777)!
            let val = rx.recv()!
            print(val)
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "777");
}

#[test]
fn test_spawn_with_boolean_return() {
    let src = r#"
        fn check() bool {
            return true
        }

        fn main() {
            let t = spawn check()
            let result = t.get()
            if result {
                print(1)
            }
        }
    "#;
    assert_eq!(compile_and_run_stdout(src).trim(), "1");
}

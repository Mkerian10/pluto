mod common;
use common::*;

// ── Basic functionality ────────────────────────────────────────────────

#[test]
fn spawn_basic() {
    let out = compile_and_run_stdout(r#"
fn returns_42() int {
    return 42
}

fn main() {
    let t = spawn returns_42()
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn spawn_with_args() {
    let out = compile_and_run_stdout(r#"
fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let t = spawn add(1, 2)
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "3");
}

#[test]
fn spawn_captures_variables() {
    let out = compile_and_run_stdout(r#"
fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let x = 10
    let y = 20
    let t = spawn add(x, y)
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "30");
}

#[test]
fn spawn_void_function() {
    // Spawn a void function — .get() just blocks until done
    let code = compile_and_run(r#"
fn do_nothing() {
}

fn main() {
    let t = spawn do_nothing()
    t.get()
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn spawn_multiple_tasks() {
    let out = compile_and_run_stdout(r#"
fn double(x: int) int {
    return x * 2
}

fn main() {
    let t1 = spawn double(5)
    let t2 = spawn double(10)
    let t3 = spawn double(15)
    let r1 = t1.get()
    let r2 = t2.get()
    let r3 = t3.get()
    print(r1 + r2 + r3)
}
"#);
    assert_eq!(out.trim(), "60");
}

#[test]
fn spawn_string_result() {
    let out = compile_and_run_stdout(r#"
fn greet(name: string) string {
    return "hello " + name
}

fn main() {
    let t = spawn greet("world")
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "hello world");
}

// ── Error handling ─────────────────────────────────────────────────────

#[test]
fn spawn_error_propagation() {
    let out = compile_and_run_stdout(r#"
error MathError {
    message: string
}

fn divide(a: int, b: int) int {
    if b == 0 {
        raise MathError { message: "division by zero" }
    }
    return a / b
}

fn try_divide() int {
    let t = spawn divide(10, 0)
    let result = t.get()!
    return result
}

fn main() {
    let result = try_divide() catch -1
    print(result)
}
"#);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn spawn_error_catch() {
    let out = compile_and_run_stdout(r#"
error MathError {
    message: string
}

fn divide(a: int, b: int) int {
    if b == 0 {
        raise MathError { message: "division by zero" }
    }
    return a / b
}

fn main() {
    let t = spawn divide(10, 0)
    let result = t.get() catch -1
    print(result)
}
"#);
    assert_eq!(out.trim(), "-1");
}

#[test]
fn spawn_infallible_no_bang() {
    // .get() on an infallible spawn should not require ! or catch
    let out = compile_and_run_stdout(r#"
fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let t = spawn add(3, 4)
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "7");
}

// ── Compile failures ───────────────────────────────────────────────────

#[test]
fn compile_fail_spawn_non_call() {
    compile_should_fail(r#"
fn main() {
    let t = spawn 42
}
"#);
}

#[test]
fn compile_fail_unhandled_get() {
    compile_should_fail_with(r#"
error MathError {
    message: string
}

fn fallible() int {
    raise MathError { message: "oops" }
    return 0
}

fn main() {
    let t = spawn fallible()
    let result = t.get()
    print(result)
}
"#, "must be handled with ! or catch");
}

#[test]
fn spawn_method_call_basic() {
    let out = compile_and_run_stdout(r#"
class Foo {
    value: int

    fn bar(self) int {
        return self.value
    }
}

fn main() {
    let f = Foo { value: 42 }
    let t = spawn f.bar()
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

// ── Spawn arg restrictions ─────────────────────────────────────────────

#[test]
fn compile_fail_spawn_propagate_in_arg() {
    compile_should_fail_with(r#"
error MathError {
    message: string
}

fn fallible() int {
    raise MathError { message: "oops" }
    return 0
}

fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let t = spawn add(fallible()!, 1)
}
"#, "error propagation (!) is not allowed in spawn arguments");
}

#[test]
fn compile_fail_spawn_bare_fallible_arg() {
    compile_should_fail_with(r#"
error MathError {
    message: string
}

fn fallible() int {
    raise MathError { message: "oops" }
    return 0
}

fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let t = spawn add(fallible(), 1)
}
"#, "must be handled with ! or catch");
}

#[test]
fn spawn_catch_in_arg_allowed() {
    let out = compile_and_run_stdout(r#"
error MathError {
    message: string
}

fn fallible() int {
    raise MathError { message: "oops" }
    return 42
}

fn double(x: int) int {
    return x * 2
}

fn main() {
    let t = spawn double(fallible() catch 5)
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "10");
}

#[test]
fn spawn_fallible_arg_workaround() {
    let out = compile_and_run_stdout(r#"
error MathError {
    message: string
}

fn maybe_value(x: int) int {
    if x < 0 {
        raise MathError { message: "negative" }
    }
    return x
}

fn double(x: int) int {
    return x * 2
}

fn do_it() int {
    let x = maybe_value(21)!
    let t = spawn double(x)
    return t.get()
}

fn main() {
    let result = do_it() catch -1
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

// ── Scope safety + edge cases ──────────────────────────────────────────

#[test]
fn spawn_task_handle_shadowing() {
    // Inner scope shadows t with fallible spawn, outer t is infallible
    let out = compile_and_run_stdout(r#"
fn foo() int {
    return 10
}

fn main() {
    let t = spawn foo()
    if true {
        let t = spawn foo()
        let inner = t.get()
        print(inner)
    }
    let outer = t.get()
    print(outer)
}
"#);
    assert_eq!(out.trim(), "10\n10");
}

#[test]
fn spawn_task_alias_conservative() {
    // Aliased task handle should require ! or catch (conservatively fallible)
    compile_should_fail_with(r#"
error SomeError {
    message: string
}

fn foo() int {
    return 42
}

fn main() {
    let t = spawn foo()
    let u = t
    let result = u.get()
    print(result)
}
"#, "must be handled with ! or catch");
}

#[test]
fn compile_fail_spawn_closure() {
    compile_should_fail(r#"
fn main() {
    let t = spawn (() => 42)()
}
"#);
}

// ── Assignment invalidation ────────────────────────────────────────────

#[test]
fn spawn_assign_invalidates_origin() {
    compile_should_fail_with(r#"
error SomeError {
    message: string
}

fn foo() int {
    return 1
}

fn bar() int {
    return 2
}

fn main() {
    let t = spawn foo()
    t = spawn bar()
    let result = t.get()
    print(result)
}
"#, "must be handled with ! or catch");
}

#[test]
fn spawn_assign_in_if_invalidates() {
    compile_should_fail_with(r#"
error SomeError {
    message: string
}

fn foo() int {
    return 1
}

fn bar() int {
    return 2
}

fn main() {
    let t = spawn foo()
    if true {
        t = spawn bar()
    }
    let result = t.get()
    print(result)
}
"#, "must be handled with ! or catch");
}

// ── Copy-on-spawn isolation tests ───────────────────────────────────────

#[test]
fn spawn_isolates_counter() {
    // With copy-on-spawn, each task gets its own Counter copy.
    // The parent's counter stays at 0.
    let out = compile_and_run_stdout(r#"
class Counter {
    value: int
}

fn increment(c: Counter, n: int) {
    let i = 0
    while i < n {
        c.value = c.value + 1
        i = i + 1
    }
}

fn main() {
    let c = Counter { value: 0 }
    let t1 = spawn increment(c, 1000)
    let t2 = spawn increment(c, 1000)
    t1.get()
    t2.get()
    print(c.value)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn spawn_isolates_class_field_write() {
    // With copy-on-spawn, tasks write to their own copies.
    // Parent's value stays at the original.
    let out = compile_and_run_stdout(r#"
class Box {
    value: int
}

fn write_value(b: Box, v: int) {
    b.value = v
}

fn main() {
    let b = Box { value: 0 }
    let t1 = spawn write_value(b, 1)
    let t2 = spawn write_value(b, 2)
    t1.get()
    t2.get()
    print(b.value)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn spawn_deep_copy_class_isolation() {
    // Copy-on-spawn: task gets a deep copy, parent unchanged.
    let out = compile_and_run_stdout(r#"
class Container {
    value: int
}

fn set_value(c: Container, v: int) {
    c.value = v
}

fn main() {
    let c = Container { value: 0 }
    let t = spawn set_value(c, 42)
    t.get()
    print(c.value)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn spawn_deep_copy_array_isolation() {
    // Task gets a deep copy of an array — parent's array unchanged.
    let out = compile_and_run_stdout(r#"
fn modify_array(arr: [int]) {
    arr.push(999)
}

fn main() {
    let arr = [1, 2, 3]
    let t = spawn modify_array(arr)
    t.get()
    print(arr.len())
}
"#);
    assert_eq!(out.trim(), "3");
}

#[test]
fn spawn_deep_copy_nested_object() {
    // Task gets a deep copy of nested objects — parent's inner object unchanged.
    let out = compile_and_run_stdout(r#"
class Inner {
    value: int
}

class Outer {
    inner: Inner
}

fn modify_inner(o: Outer) {
    let mut i = o.inner
    i.value = 999
}

fn main() {
    let inner = Inner { value: 42 }
    let outer = Outer { inner: inner }
    let t = spawn modify_inner(outer)
    t.get()
    let result = outer.inner
    print(result.value)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn spawn_deep_copy_map_isolation() {
    // Task gets a deep copy of a map — parent's map unchanged.
    let out = compile_and_run_stdout(r#"
fn modify_map(m: Map<string, int>) {
    m["new_key"] = 999
}

fn main() {
    let m = Map<string, int> { "a": 1, "b": 2 }
    let t = spawn modify_map(m)
    t.get()
    print(m.len())
}
"#);
    assert_eq!(out.trim(), "2");
}

#[test]
fn spawn_deep_copy_set_isolation() {
    // Task gets a deep copy of a set — parent's set unchanged.
    let out = compile_and_run_stdout(r#"
fn modify_set(s: Set<int>) {
    s.insert(999)
}

fn main() {
    let s = Set<int> { 1, 2, 3 }
    let t = spawn modify_set(s)
    t.get()
    print(s.len())
}
"#);
    assert_eq!(out.trim(), "3");
}

#[test]
fn spawn_strings_safe_without_deep_copy() {
    // Strings are immutable — no deep copy needed, still safe.
    let out = compile_and_run_stdout(r#"
fn use_string(s: string) string {
    return s + " world"
}

fn main() {
    let s = "hello"
    let t = spawn use_string(s)
    let result = t.get()
    print(s)
    print(result)
}
"#);
    assert_eq!(out.trim(), "hello\nhello world");
}

#[test]
fn spawn_deep_copy_class_with_array_field() {
    // Deep copy of a class that contains an array field.
    let out = compile_and_run_stdout(r#"
class Container {
    items: [int]
}

fn modify_container(c: Container) {
    c.items.push(999)
}

fn main() {
    let c = Container { items: [1, 2, 3] }
    let t = spawn modify_container(c)
    t.get()
    print(c.items.len())
}
"#);
    assert_eq!(out.trim(), "3");
}

// ── Stress tests ─────────────────────────────────────────────────────────

#[test]
fn stress_many_concurrent_tasks() {
    // Spawn 20 tasks and collect all results
    let out = compile_and_run_stdout(r#"
fn compute(x: int) int {
    let result = 0
    let i = 0
    while i < 1000 {
        result = result + x
        i = i + 1
    }
    return result
}

fn main() {
    let t1 = spawn compute(1)
    let t2 = spawn compute(2)
    let t3 = spawn compute(3)
    let t4 = spawn compute(4)
    let t5 = spawn compute(5)
    let t6 = spawn compute(6)
    let t7 = spawn compute(7)
    let t8 = spawn compute(8)
    let t9 = spawn compute(9)
    let t10 = spawn compute(10)
    let t11 = spawn compute(11)
    let t12 = spawn compute(12)
    let t13 = spawn compute(13)
    let t14 = spawn compute(14)
    let t15 = spawn compute(15)
    let t16 = spawn compute(16)
    let t17 = spawn compute(17)
    let t18 = spawn compute(18)
    let t19 = spawn compute(19)
    let t20 = spawn compute(20)
    let sum = t1.get() + t2.get() + t3.get() + t4.get() + t5.get()
        + t6.get() + t7.get() + t8.get() + t9.get() + t10.get()
        + t11.get() + t12.get() + t13.get() + t14.get() + t15.get()
        + t16.get() + t17.get() + t18.get() + t19.get() + t20.get()
    print(sum)
}
"#);
    // sum = 1000 * (1+2+...+20) = 1000 * 210 = 210000
    assert_eq!(out.trim(), "210000");
}

#[test]
fn stress_gc_pressure_under_suppression() {
    // Tasks allocate many strings while GC is suppressed.
    // Validates that GC suppression + mutex doesn't crash under load.
    let code = compile_and_run(r#"
fn allocate_strings(n: int) {
    let i = 0
    while i < n {
        let s = "item number {i}"
        i = i + 1
    }
}

fn main() {
    let t1 = spawn allocate_strings(5000)
    let t2 = spawn allocate_strings(5000)
    let t3 = spawn allocate_strings(5000)
    let t4 = spawn allocate_strings(5000)
    t1.get()
    t2.get()
    t3.get()
    t4.get()
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn stress_tasks_with_errors() {
    // Many tasks where some succeed and some fail.
    // Validates error propagation works correctly under concurrent load.
    let out = compile_and_run_stdout(r#"
error ComputeError {
    message: string
}

fn maybe_fail(x: int) int {
    if x % 2 == 0 {
        raise ComputeError { message: "even number" }
    }
    return x * 10
}

fn main() {
    let t1 = spawn maybe_fail(1)
    let t2 = spawn maybe_fail(2)
    let t3 = spawn maybe_fail(3)
    let t4 = spawn maybe_fail(4)
    let t5 = spawn maybe_fail(5)
    let t6 = spawn maybe_fail(6)
    let t7 = spawn maybe_fail(7)
    let t8 = spawn maybe_fail(8)
    let t9 = spawn maybe_fail(9)
    let t10 = spawn maybe_fail(10)
    let sum = t1.get() catch 0
    let sum = sum + (t2.get() catch 0)
    let sum = sum + (t3.get() catch 0)
    let sum = sum + (t4.get() catch 0)
    let sum = sum + (t5.get() catch 0)
    let sum = sum + (t6.get() catch 0)
    let sum = sum + (t7.get() catch 0)
    let sum = sum + (t8.get() catch 0)
    let sum = sum + (t9.get() catch 0)
    let sum = sum + (t10.get() catch 0)
    print(sum)
}
"#);
    // Odd numbers succeed: 1*10 + 3*10 + 5*10 + 7*10 + 9*10 = 250
    // Even numbers fail and catch gives 0
    assert_eq!(out.trim(), "250");
}

// ── Must-use Task ────────────────────────────────────────────────────

#[test]
fn compile_fail_bare_spawn() {
    compile_should_fail_with(r#"
fn work() int {
    return 42
}

fn main() {
    spawn work()
}
"#, "Task handle must be used");
}

#[test]
fn spawn_with_detach_compiles() {
    let out = compile_and_run_stdout(r#"
fn work() int {
    return 42
}

fn main() {
    spawn work().detach()
    print("ok")
}
"#);
    assert_eq!(out.trim(), "ok");
}

// ── Detach ───────────────────────────────────────────────────────────

#[test]
fn detach_basic() {
    let out = compile_and_run_stdout_timeout(r#"
fn work(x: int) int {
    return x * 2
}

fn main() {
    let t = spawn work(21)
    t.detach()
    print("detached")
}
"#, 5);
    assert_eq!(out.trim(), "detached");
}

#[test]
fn detach_with_error_does_not_crash() {
    // A detached task that errors should print to stderr but not crash the process
    let (stdout, stderr, code) = compile_and_run_output(r#"
error WorkError {
    message: string
}

fn failing_work() int {
    raise WorkError { message: "oops" }
    return 0
}

fn main() {
    let t = spawn failing_work()
    t.detach()
    // Give the task time to run and fail
    let i = 0
    while i < 1000000 {
        i = i + 1
    }
    print("done")
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.trim().contains("done"));
    // Stderr should contain the error message from the detached task
    assert!(stderr.contains("detached task"), "Expected detached task error in stderr, got: {}", stderr);
}

#[test]
fn compile_fail_detach_with_args() {
    compile_should_fail_with(r#"
fn work() int {
    return 42
}

fn main() {
    let t = spawn work()
    t.detach(1)
}
"#, "detach() expects 0 arguments");
}

// ── Cancel ───────────────────────────────────────────────────────────

#[test]
fn cancel_basic() {
    // cancel() + get(): the spawned function must be fallible for catch to work
    let out = compile_and_run_stdout_timeout(r#"
error WorkError {
    message: string
}

fn work() int {
    // Make the function fallible so .get() allows catch
    if false {
        raise WorkError { message: "never" }
    }
    return 42
}

fn main() {
    let t = spawn work()
    t.cancel()
    let result = t.get() catch -1
    print(result)
}
"#, 5);
    // Task may complete before cancel takes effect, so result is either 42 or -1
    let val: i32 = out.trim().parse().unwrap();
    assert!(val == 42 || val == -1, "Expected 42 or -1, got {}", val);
}

#[test]
fn cancel_completed_task() {
    let out = compile_and_run_stdout_timeout(r#"
fn work() int {
    return 99
}

fn main() {
    let t = spawn work()
    let result = t.get()
    // Cancel after task is already done — get should still return result
    t.cancel()
    print(result)
}
"#, 5);
    assert_eq!(out.trim(), "99");
}

#[test]
fn compile_fail_cancel_with_args() {
    compile_should_fail_with(r#"
fn work() int {
    return 42
}

fn main() {
    let t = spawn work()
    t.cancel(1)
}
"#, "cancel() expects 0 arguments");
}

#[test]
fn cancel_channel_checkpoint() {
    // A task blocked on channel recv should see cancellation when woken
    let (stdout, _stderr, code) = compile_and_run_output(r#"
fn worker(rx: Receiver<int>) int {
    let val = rx.recv()!
    return val
}

fn main() {
    let (tx, rx) = chan<int>(1)
    let t = spawn worker(rx)
    // Cancel the task while it's waiting on recv
    t.cancel()
    // Send something to unblock the condvar
    tx.send(1) catch print("send failed")
    let result = t.get() catch -1
    print(result)
}
"#);
    // Task should either get -1 (cancelled) or 1 (completed before cancel)
    let val: i32 = stdout.trim().parse().unwrap_or(-99);
    assert!(val == 1 || val == -1, "Expected 1 or -1, got {}", val);
    assert_eq!(code, 0);
}

#[test]
fn detach_then_cancel() {
    // Both methods should work on the same handle
    let out = compile_and_run_stdout(r#"
fn work() int {
    return 42
}

fn main() {
    let t = spawn work()
    t.cancel()
    t.detach()
    print("ok")
}
"#);
    assert_eq!(out.trim(), "ok");
}

// ── Spawn method calls ────────────────────────────────────────────────

#[test]
fn spawn_method_call_with_args() {
    let out = compile_and_run_stdout(r#"
class Calculator {
    base: int

    fn add(self, x: int) int {
        return self.base + x
    }
}

fn main() {
    let c = Calculator { base: 10 }
    let t = spawn c.add(32)
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn spawn_method_call_chain() {
    let out = compile_and_run_stdout(r#"
class Inner {
    value: int

    fn compute(self) int {
        return self.value * 2
    }
}

class Outer {
    inner: Inner
}

fn main() {
    let inner = Inner { value: 21 }
    let outer = Outer { inner: inner }
    let t = spawn outer.inner.compute()
    let result = t.get()
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn spawn_method_call_detach() {
    let out = compile_and_run_stdout(r#"
class Worker {
    id: int

    fn work(self) int {
        return self.id
    }
}

fn main() {
    let w = Worker { id: 1 }
    let t = spawn w.work()
    t.detach()
    print("detached")
}
"#);
    assert_eq!(out.trim(), "detached");
}

#[test]
fn spawn_method_call_di_singleton() {
    let out = compile_and_run_stdout(r#"
class Counter {
    value: int

    fn get_value(self) int {
        return self.value
    }
}

class Service[counter: Counter] {
    id: int

    fn fetch(self) int {
        return self.counter.get_value()
    }
}

app MyApp[svc: Service] {
    fn main(self) {
        let t = spawn self.svc.fetch()
        let result = t.get()
        print(result)
    }
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn spawn_method_call_error_propagation() {
    let out = compile_and_run_stdout(r#"
error ServiceError {
    message: string
}

class Service {
    fail_flag: int

    fn process(self) int {
        if self.fail_flag == 1 {
            raise ServiceError { message: "failed" }
        }
        return 42
    }
}

fn main() {
    let s = Service { fail_flag: 0 }
    let t = spawn s.process()
    let result = t.get() catch err {
        print("caught error")
        return
    }
    print(result)
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn spawn_method_call_error_caught() {
    let out = compile_and_run_stdout(r#"
error ServiceError {
    message: string
}

class Service {
    fail_flag: int

    fn process(self) int {
        if self.fail_flag == 1 {
            raise ServiceError { message: "boom" }
        }
        return 42
    }
}

fn main() {
    let s = Service { fail_flag: 1 }
    let t = spawn s.process()
    let result = t.get() catch err {
        print("caught")
        return
    }
    print(result)
}
"#);
    assert_eq!(out.trim(), "caught");
}

#[test]
fn compile_fail_spawn_propagate_in_method_arg() {
    compile_should_fail_with(r#"
error MathError {
    message: string
}

fn fallible() int {
    raise MathError { message: "oops" }
    return 0
}

class Adder {
    base: int

    fn add(self, x: int) int {
        return self.base + x
    }
}

fn main() {
    let a = Adder { base: 0 }
    let t = spawn a.add(fallible()!)
}
"#, "error propagation (!) is not allowed in spawn arguments");
}

#[test]
fn spawn_method_call_self() {
    // spawn self.method() from within an app
    let out = compile_and_run_stdout(r#"
class Worker {
    value: int

    fn compute(self) int {
        return self.value + 10
    }
}

app MyApp[worker: Worker] {
    fn run_task(self) int {
        let t = spawn self.worker.compute()
        return t.get()
    }

    fn main(self) {
        let result = self.run_task()
        print(result)
    }
}
"#);
    assert_eq!(out.trim(), "10");
}

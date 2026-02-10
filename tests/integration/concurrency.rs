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
fn compile_fail_spawn_method_call() {
    compile_should_fail(r#"
class Foo {
    fn bar(self) int {
        return 42
    }
}

fn main() {
    let f = Foo {}
    let t = spawn f.bar()
}
"#);
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

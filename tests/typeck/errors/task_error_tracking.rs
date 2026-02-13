//! Task error tracking tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Basic task.get() fallibility
#[test]
fn task_get_no_handler() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() t.get()}"#, "unhandled error"); }
#[test]
fn task_get_in_assignment() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() let x=t.get()}"#, "unhandled error"); }
#[test]
fn task_get_in_binop() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() let x=t.get()+1}"#, "unhandled error"); }

// Task.get() when spawned function is fallible
#[test]
fn task_fallible_work_no_handler() { compile_should_fail_with(r#"error E{} fn work()int!{raise E{}} fn main(){let t=spawn work() t.get()}"#, "unhandled error"); }
#[test]
fn task_fallible_get_needs_propagate() { compile_should_fail_with(r#"error E{} fn work()int!{raise E{}} fn f()int{let t=spawn work() return t.get()} fn main(){}"#, "unhandled error"); }
#[test]
fn task_fallible_get_with_propagate() { compile_should_fail_with(r#"error E{} fn work()int!{raise E{}} fn f()int!{let t=spawn work() return t.get()!} fn main(){f()}"#, "unhandled error"); }

// Multiple tasks
#[test]
fn two_tasks_both_gets() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t1=spawn work() let t2=spawn work() t1.get() t2.get()}"#, "unhandled error"); }
#[test]
fn two_tasks_one_fallible() { compile_should_fail_with(r#"error E{} fn work1()int{return 42} fn work2()int!{raise E{}} fn main(){let t1=spawn work1() let t2=spawn work2() t1.get() t2.get()}"#, "unhandled error"); }

// Task get in control flow
#[test]
fn task_get_in_if() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() if true{t.get()}}"#, "unhandled error"); }
#[test]
fn task_get_in_while() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() while false{t.get()}}"#, "unhandled error"); }
#[test]
fn task_get_in_for() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() for i in 0..1{t.get()}}"#, "unhandled error"); }

// Task with generic return type
#[test]
fn generic_task_get() { compile_should_fail_with(r#"fn work<T>(x:T)T{return x} fn main(){let t=spawn work(42) t.get()}"#, "unhandled error"); }

// Task assigned to variable before get
#[test]
fn task_stored_then_get() { compile_should_fail_with(r#"fn work()int{return 42} fn main(){let t=spawn work() let x=t let y=x.get()}"#, "unhandled error"); }

// Task get in function call arg
#[test]
fn task_get_as_arg() { compile_should_fail_with(r#"fn work()int{return 42} fn consume(x:int){} fn main(){let t=spawn work() consume(t.get())}"#, "unhandled error"); }

// Task get in struct field
#[test]
fn task_get_in_struct_field() { compile_should_fail_with(r#"fn work()int{return 42} class C{x:int} fn main(){let t=spawn work() let c=C{x:t.get()}}"#, "unhandled error"); }

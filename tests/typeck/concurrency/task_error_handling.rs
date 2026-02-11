//! Task error handling tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Task get without error handling
#[test] fn task_get_no_error_handling() { compile_should_fail_with(r#"error E{} fn task()!int{raise E{}} fn main(){let t=spawn task() let x=t.get()}"#, ""); }

// Task error type mismatch
#[test] fn task_error_type_mismatch() { compile_should_fail_with(r#"error E1{} error E2{} fn task()!E1 int{raise E1{}} fn main(){let t=spawn task() let x=t.get() catch E2{}}"#, ""); }

// Task with multiple error types
#[test] fn task_multiple_errors() { compile_should_fail_with(r#"error E1{} error E2{} fn task()!int{raise E1{}} fn main(){let t=spawn task() let x=t.get() catch{}}"#, ""); }

// Spawn fallible function propagate in args
#[test] fn spawn_propagate_in_args() { compile_should_fail_with(r#"error E{} fn f()!int{raise E{}} fn task(x:int)int{return x} fn main(){spawn task(f()!)}"#, ""); }

// Spawn bare fallible call in args
#[test] fn spawn_bare_fallible_args() { compile_should_fail_with(r#"error E{} fn f()!int{raise E{}} fn task(x:int)int{return x} fn main(){spawn task(f())}"#, ""); }

// Task result type mismatch
#[test] fn task_result_type_mismatch() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let x:string=t.get()}"#, ""); }

// Task get on non-task
#[test] fn get_on_non_task() { compile_should_fail_with(r#"fn main(){let x=1 let y=x.get()}"#, ""); }

// Spawn non-function
#[test] fn spawn_non_function() { compile_should_fail_with(r#"fn main(){let x=1 spawn x}"#, ""); }

// Spawn with wrong args
#[test] fn spawn_wrong_args() { compile_should_fail_with(r#"fn task(x:int)int{return x} fn main(){spawn task("hi")}"#, ""); }

// Spawn with missing args
#[test] fn spawn_missing_args() { compile_should_fail_with(r#"fn task(x:int)int{return x} fn main(){spawn task()}"#, ""); }

// Spawn with extra args
#[test] fn spawn_extra_args() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){spawn task(1)}"#, ""); }

// Task propagate without fallible get
#[test] fn task_propagate_no_fallible() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let x=t.get()!}"#, ""); }

// Nested spawn error handling
#[test] fn nested_spawn_error() { compile_should_fail_with(r#"error E{} fn inner()!int{raise E{}} fn outer()int{let t=spawn inner() return t.get()} fn main(){spawn outer()}"#, ""); }

// Task error in loop
#[test] fn task_error_loop() { compile_should_fail_with(r#"error E{} fn task(x:int)!int{raise E{}} fn main(){for i in 0..10{let t=spawn task(i) let x=t.get()}}"#, ""); }

// Task error in match
#[test] fn task_error_match() { compile_should_fail_with(r#"error E{} enum Opt{Some{v:int}None} fn task()!int{raise E{}} fn main(){match Opt.Some{v:1}{Opt.Some{v}{let t=spawn task() let x=t.get()}Opt.None{}}}"#, ""); }

// Task with generic error
#[test] fn task_generic_error() { compile_should_fail_with(r#"error E<T>{val:T} fn task<T>()!E<T> int{raise E{val:1}} fn main(){let t=spawn task<int>() let x=t.get()}"#, ""); }

// Multiple tasks different errors
#[test] fn multi_task_diff_errors() { compile_should_fail_with(r#"error E1{} error E2{} fn task1()!E1 int{raise E1{}} fn task2()!E2 int{raise E2{}} fn main(){let t1=spawn task1() let t2=spawn task2() let x=t1.get() let y=t2.get()}"#, ""); }

// Task error through closure
#[test] fn task_error_closure() { compile_should_fail_with(r#"error E{} fn main(){let f=()=>spawn (()=>raise E{})() let t=f() let x=t.get()}"#, ""); }

// Task nullable result
#[test] fn task_nullable_result() { compile_should_fail_with(r#"fn task()int?{return none} fn main(){let t=spawn task() let x:int=t.get()}"#, ""); }

// Task error with catch in wrong scope
#[test] fn task_error_wrong_scope() { compile_should_fail_with(r#"error E{} fn task()!int{raise E{}} fn main(){let t=spawn task() if true{let x=t.get() catch{}}}"#, ""); }

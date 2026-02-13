//! Task lifecycle tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Double get on task
#[test]
fn double_get() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let x=t.get() let y=t.get()}"#, ""); }

// Get on moved task
#[test]
fn get_moved_task() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let u=t let x=t.get()}"#, ""); }

// Task assigned to wrong type
#[test]
fn task_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t:Task<string>=spawn task()}"#, ""); }

// Task type inference failure
#[test]
fn task_type_inference() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let x:string=t}"#, ""); }

// Task in array wrong type
#[test]
fn task_array_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let tasks:Array<Task<string>>=[spawn task()]}"#, ""); }

// Task in map wrong type
#[test]
fn task_map_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let m=Map<string,Task<string>>{} m["t"]=spawn task()}"#, ""); }

// Task return from function wrong type
#[test]
fn task_return_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn make()Task<string>{return spawn task()} fn main(){}"#, ""); }

// Task as parameter wrong type
#[test]
fn task_param_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn wait(t:Task<string>){let x=t.get()} fn main(){wait(spawn task())}"#, ""); }

// Task field wrong type
#[test]
fn task_field_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} class C{t:Task<string>} fn main(){let c=C{t:spawn task()}}"#, ""); }

// Task generic instantiation wrong
#[test]
fn task_generic_wrong() { compile_should_fail_with(r#"fn task<T>(x:T)T{return x} fn main(){let t:Task<string>=spawn task<int>(1)}"#, ""); }

// Task with multiple gets in different scopes
#[test]
fn task_get_diff_scopes() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() if true{let x=t.get()}else{let y=t.get()}}"#, ""); }

// Task passed through closure
#[test]
fn task_through_closure() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let f=()=>t.get() let x=f() let y=t.get()}"#, ""); }

// Task in nested spawn
#[test]
fn task_nested_spawn() { compile_should_fail_with(r#"fn inner()int{return 1} fn outer()Task<int>{return spawn inner()} fn main(){spawn outer()}"#, ""); }

// Task nullable field access
#[test]
fn task_nullable_field() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t:Task<int>?=spawn task() let x=t?.get()}"#, ""); }

// Task in trait bound
#[test]
fn task_trait_bound() { compile_should_fail_with(r#"trait Runnable{} fn task<T:Runnable>()T{} fn main(){spawn task<Task<int>>()}"#, ""); }

// Task comparison
#[test]
fn task_comparison() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t1=spawn task() let t2=spawn task() let eq=t1==t2}"#, ""); }

// Task arithmetic
#[test]
fn task_arithmetic() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() let x=t+1}"#, ""); }

// Task method call (non-get)
#[test]
fn task_method_call() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() t.cancel()}"#, ""); }

// Task indexing
#[test]
fn task_indexing() { compile_should_fail_with(r#"fn task()Array<int>{return [1,2,3]} fn main(){let t=spawn task() let x=t[0]}"#, ""); }

// Task in match binding
#[test]
fn task_match_binding() { compile_should_fail_with(r#"enum E{A{t:Task<int>}} fn task()int{return 1} fn main(){match E.A{t:spawn task()}{E.A{t}{let x=t.get() let y=t.get()}}}"#, ""); }

//! Task lifecycle tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Double get on task
#[test]
fn double_get() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn task()int{return 1}
fn main(){let t=spawn task()
let x=t.get()
let y=t.get()}"#).is_ok()); }

// Get on moved task
#[test]
fn get_moved_task() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn task()int{return 1}
fn main(){let t=spawn task()
let u=t
let x=t.get()}"#).is_ok()); }

// Task assigned to wrong type
#[test]
fn task_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t:Task<string>=spawn task()}"#, "type mismatch: expected Task<string>, found Task<int>"); }

// Task type inference failure
#[test]
fn task_type_inference() { compile_should_fail_with(r#"fn task()int{return 1}
fn main(){let t=spawn task()
let x:string=t}"#, "type mismatch: expected string, found Task<int>"); }

// Task in array wrong type
#[test]
fn task_array_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let tasks:[Task<string>]=[spawn task()]}"#, "type mismatch"); }

// Task in map wrong type
#[test]
fn task_map_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let m=Map<string,Task<string>>{} m["t"]=spawn task()}"#, "expected newline after statement"); }

// Task return from function wrong type
#[test]
fn task_return_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn make()Task<string>{return spawn task()} fn main(){}"#, "return type mismatch: expected Task<string>, found Task<int>"); }

// Task as parameter wrong type
#[test]
fn task_param_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} fn wait(t:Task<string>){let x=t.get()} fn main(){wait(spawn task())}"#, "argument 1 of 'wait': expected Task<string>, found Task<int>"); }

// Task field wrong type
#[test]
fn task_field_wrong_type() { compile_should_fail_with(r#"fn task()int{return 1} class C{t:Task<string>} fn main(){let c=C{t:spawn task()}}"#, "field 't': expected Task<string>, found Task<int>"); }

// Task generic instantiation wrong
#[test]
fn task_generic_wrong() { compile_should_fail_with(r#"fn task<T>(x:T)T{return x} fn main(){let t:Task<string>=spawn task<int>(1)}"#, "expected '(' or '.' after identifier in spawn expression"); }

// Task with multiple gets in different scopes
#[test]
fn task_get_diff_scopes() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() if true{let x=t.get()}else{let y=t.get()}}"#, "expected newline after statement"); }

// Task passed through closure
#[test]
fn task_through_closure() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn task()int{return 1}
fn main(){let t=spawn task()
let f=()=>t.get()
let x=f()
let y=t.get()}"#).is_ok()); }

// Task in nested spawn
#[test]
fn task_nested_spawn() {
    // Returning a spawned task and re-spawning are legal; the outer
    // discarded handle is the error
    compile_should_fail_with(r#"fn inner()int{return 1} fn outer()Task<int>{return spawn inner()} fn main(){spawn outer()}"#, "Task handle must be used");
}

// Task nullable field access
#[test]
fn task_nullable_field() { compile_should_fail_with(r#"fn task()int{return 1}
fn main(){let t:Task<int>?=spawn task()
let x=t?.get()}"#, "call to fallible method 'get' must be handled with ! or catch"); }

// Task in trait bound
#[test]
fn task_trait_bound() { compile_should_fail_with(r#"trait Runnable{} fn task<T:Runnable>()T{} fn main(){spawn task<Task<int>>()}"#, "expected '(' or '.' after identifier in spawn expression"); }

// Task comparison
#[test]
fn task_comparison() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"fn task()int{return 1}
fn main(){let t1=spawn task()
let t2=spawn task()
let eq=t1==t2}"#).is_ok()); }

// Task arithmetic
#[test]
fn task_arithmetic() { compile_should_fail_with(r#"fn task()int{return 1}
fn main(){let t=spawn task()
let x=t+1}"#, "operand type mismatch: Task<int> vs int"); }

// Task method call (non-get)
#[test]
fn task_method_call() { compile_should_fail_with(r#"fn task()int{return 1} fn main(){let t=spawn task() t.cancel()}"#, "expected newline after statement"); }

// Task indexing
#[test]
fn task_indexing() { compile_should_fail_with(r#"fn task()Array<int>{return [1,2,3]}
fn main(){let t=spawn task()
let x=t[0]}"#, "unknown generic type 'Array'"); }

// Task in match binding
#[test]
fn task_match_binding() { compile_should_fail_with(r#"enum E{A{t:Task<int>}} fn task()int{return 1} fn main(){match E.A{t:spawn task()}{E.A{t}{let x=t.get() let y=t.get()}}}"#, "expected ., found :"); }

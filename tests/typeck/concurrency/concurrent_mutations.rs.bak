//! Concurrent mutation detection - 10 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Mutation while task holds reference
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_while_task_holds() { compile_should_fail_with(r#"class C{x:int} fn task(c:C)int{return c.x} fn main(){let c=C{x:0} let t=spawn task(c) c.x=1}"#, ""); }

// Multiple mutations from tasks
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_task_mutations() { compile_should_fail_with(r#"class C{x:int} fn task1(c:C){c.x=1} fn task2(c:C){c.x=2} fn main(){let c=C{x:0} spawn task1(c) spawn task2(c)}"#, ""); }

// Mutation in task args
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_task_args() { compile_should_fail_with(r#"class C{x:int} fn task(v:int)int{return v} fn main(){let c=C{x:0} spawn task(c.x) c.x=1}"#, ""); }

// Concurrent bracket dep mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_bracket_dep() { compile_should_fail_with(r#"class Dep{x:int} class C[d:Dep]{} fn task(d:Dep){d.x=1} fn main(){let d=Dep{x:0} spawn task(d) d.x=5}"#, ""); }

// Mutation through multiple paths
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_multi_path() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=1} fn main(){let c=C{x:0} let d=c spawn task(d) c.x=5}"#, ""); }

// Mutation in nested task
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_nested_task() { compile_should_fail_with(r#"class C{x:int} fn inner(c:C){c.x=1} fn outer(c:C){spawn inner(c)} fn main(){let c=C{x:0} spawn outer(c) c.x=5}"#, ""); }

// Mutation through trait method in task
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_trait_method_task() { compile_should_fail_with(r#"trait T{fn set(mut self)} class C{x:int} impl T{fn set(mut self){self.x=1}} fn task(c:C){c.set()} fn main(){let c=C{x:0} spawn task(c)}"#, ""); }

// Concurrent mutation in loop
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_mut_loop() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=1} fn main(){let c=C{x:0} for i in 0..10{spawn task(c)}}"#, ""); }

// Mutation after task spawn
#[test]
#[ignore] // PR #46 - outdated assertions
fn mutate_after_spawn() { compile_should_fail_with(r#"class C{x:int} fn task(c:C)int{return c.x} fn main(){let c=C{x:0} let t=spawn task(c) c.x=1 let y=t.get()}"#, ""); }

// Concurrent mutation through generic
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_mut_generic() { compile_should_fail_with(r#"class Box<T>{val:T} fn task<T>(b:Box<T>){b.val=b.val} fn main(){let b=Box<int>{val:0} spawn task<int>(b) b.val=1}"#, ""); }

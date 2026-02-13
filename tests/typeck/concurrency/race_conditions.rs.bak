//! Race condition tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Concurrent field mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_field_mut() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=c.x+1} fn main(){let c=C{x:0} spawn task(c) c.x=5}"#, ""); }

// Concurrent array mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_array_mut() { compile_should_fail_with(r#"fn task(arr:Array<int>){arr[0]=1} fn main(){let arr=[0,0,0] spawn task(arr) arr[0]=5}"#, ""); }

// Concurrent map mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_map_mut() { compile_should_fail_with(r#"fn task(m:Map<string,int>){m["key"]=1} fn main(){let m=Map<string,int>{} spawn task(m) m["key"]=5}"#, ""); }

// Multiple tasks mutating same data
#[test]
#[ignore] // PR #46 - outdated assertions
fn multi_task_mut() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=c.x+1} fn main(){let c=C{x:0} spawn task(c) spawn task(c)}"#, ""); }

// Task mutates captured variable
#[test]
#[ignore] // PR #46 - outdated assertions
fn task_mutate_capture() { compile_should_fail_with(r#"fn main(){let x=0 spawn (()=>x=1)()}"#, ""); }

// Concurrent read-write
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_read_write() { compile_should_fail_with(r#"class C{x:int} fn reader(c:C)int{return c.x} fn writer(c:C){c.x=1} fn main(){let c=C{x:0} spawn reader(c) spawn writer(c)}"#, ""); }

// Nested concurrent mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn nested_concurrent_mut() { compile_should_fail_with(r#"class Inner{x:int} class Outer{i:Inner} fn task(o:Outer){o.i.x=1} fn main(){let o=Outer{i:Inner{x:0}} spawn task(o) o.i.x=5}"#, ""); }

// Task mutates global
#[test]
#[ignore] // PR #46 - outdated assertions
fn task_mutate_global() { compile_should_fail_with(r#"let global=0 fn task(){global=1} fn main(){spawn task() global=5}"#, ""); }

// Concurrent invariant violation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_invariant_violation() { compile_should_fail_with(r#"class C{x:int invariant self.x>=0} fn task(c:C){c.x=-1} fn main(){let c=C{x:0} spawn task(c)}"#, ""); }

// Concurrent method call mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_method_mut() { compile_should_fail_with(r#"class C{x:int} fn inc(mut self){self.x=self.x+1} fn main(){let c=C{x:0} spawn c.inc() c.inc()}"#, ""); }

// Task with mutable parameter
#[test]
#[ignore] // PR #46 - outdated assertions
fn task_mut_param() { compile_should_fail_with(r#"fn task(mut x:int){x=x+1} fn main(){spawn task(0)}"#, ""); }

// Concurrent channel send/receive
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_channel() { compile_should_fail_with(r#"fn task(s:Sender<int>){s.send(1)} fn main(){let ch=chan<int>() spawn task(ch.0) ch.0.send(2)}"#, ""); }

// Task accesses local after scope
#[test]
#[ignore] // PR #46 - outdated assertions
fn task_after_scope() { compile_should_fail_with(r#"fn task(x:int)int{return x} fn main(){if true{let x=1 spawn task(x)}}"#, ""); }

// Concurrent nullable mutation
#[test]
#[ignore] // PR #46 - outdated assertions
fn concurrent_nullable_mut() { compile_should_fail_with(r#"fn task(x:int?){if x?{let y=x?+1}} fn main(){let x:int?=1 spawn task(x) x=none}"#, ""); }

// Task mutates through trait
#[test]
#[ignore] // PR #46 - outdated assertions
fn task_trait_mut() { compile_should_fail_with(r#"trait T{fn update(mut self)} class C{x:int} impl T{fn update(mut self){self.x=1}} fn task(t:C){t.update()} fn main(){let c=C{x:0} spawn task(c)}"#, ""); }

//! Race condition tests - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Concurrent field mutation
#[test]
fn concurrent_field_mut() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=c.x+1} fn main(){let c=C{x:0} spawn task(c) c.x=5}"#, "expected newline after statement"); }

// Concurrent array mutation
#[test]
fn concurrent_array_mut() { compile_should_fail_with(r#"fn task(arr:Array<int>){arr[0]=1} fn main(){let arr=[0,0,0] spawn task(arr) arr[0]=5}"#, "expected newline after statement"); }

// Concurrent map mutation
#[test]
fn concurrent_map_mut() { compile_should_fail_with(r#"fn task(m:Map<string,int>){m["key"]=1} fn main(){let m=Map<string,int>{} spawn task(m) m["key"]=5}"#, "expected newline after statement"); }

// Multiple tasks mutating same data
#[test]
fn multi_task_mut() { compile_should_fail_with(r#"class C{x:int} fn task(c:C){c.x=c.x+1} fn main(){let c=C{x:0} spawn task(c) spawn task(c)}"#, "expected newline after statement"); }

// Task mutates captured variable
#[test]
fn task_mutate_capture() { compile_should_fail_with(r#"fn main(){let x=0 spawn (()=>x=1)()}"#, "expected newline after statement"); }

// Concurrent read-write
#[test]
fn concurrent_read_write() { compile_should_fail_with(r#"class C{x:int} fn reader(c:C)int{return c.x} fn writer(c:C){c.x=1} fn main(){let c=C{x:0} spawn reader(c) spawn writer(c)}"#, "expected newline after statement"); }

// Nested concurrent mutation
#[test]
fn nested_concurrent_mut() { compile_should_fail_with(r#"class Inner{x:int} class Outer{i:Inner} fn task(o:Outer){o.i.x=1} fn main(){let o=Outer{i:Inner{x:0}} spawn task(o) o.i.x=5}"#, "expected newline after statement"); }

// Task mutates global
#[test]
fn task_mutate_global() { compile_should_fail_with(r#"let global=0 fn task(){global=1} fn main(){spawn task() global=5}"#, "expected 'fn', 'class', 'trait', 'enum', 'error', 'app', 'stage', 'system', 'test', 'tests', 'extern fn', or 'extern rust', found let"); }

// Concurrent invariant violation
#[test]
fn concurrent_invariant_violation() { compile_should_fail_with(r#"class C{x:int
invariant self.x>=0}
fn task(c:C){c.x=-1}
fn main(){let c=C{x:0}
spawn task(c)}"#, "cannot assign to field of immutable variable 'c'; declare with 'let mut' to allow mutation"); }

// Concurrent method call mutation
#[test]
fn concurrent_method_mut() { compile_should_fail_with(r#"class C{x:int
fn inc(mut self){self.x=self.x+1}
}
fn main(){let c=C{x:0}
spawn c.inc() c.inc()}"#, "expected newline after statement"); }

// Task with mutable parameter
#[test]
fn task_mut_param() {
    // Spawning a fn with a mut param is legal (capture by value);
    // discarding the handle is the error
    compile_should_fail_with(r#"fn task(mut x:int){x=x+1} fn main(){spawn task(0)}"#, "Task handle must be used");
}

// Concurrent channel send/receive
#[test]
fn concurrent_channel() { compile_should_fail_with(r#"fn task(s:Sender<int>){s.send(1)} fn main(){let ch=chan<int>() spawn task(ch.0) ch.0.send(2)}"#, "expected newline after statement"); }

// Task accesses local after scope
#[test]
fn task_after_scope() { compile_should_fail_with(r#"fn task(x:int)int{return x} fn main(){if true{let x=1 spawn task(x)}}"#, "expected newline after statement"); }

// Concurrent nullable mutation
#[test]
fn concurrent_nullable_mut() { compile_should_fail_with(r#"fn task(x:int?){if x?{let y=x?+1}} fn main(){let x:int?=1 spawn task(x) x=none}"#, "expected newline after statement"); }

// Task mutates through trait
#[test]
fn task_trait_mut() { compile_should_fail_with(r#"trait T{fn update(mut self)}
class C impl T {x:int
fn update(mut self){self.x=1}}
fn task(t:C){t.update()}
fn main(){let c=C{x:0}
spawn task(c)}"#, "cannot call mutating method 'update' on immutable variable 't'; declare with 'let mut' to allow mutation"); }

//! Spawn expression validation tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Spawn method call
#[test]
fn spawn_method_call() { compile_should_fail_with(r#"class C{x:int
fn get(self)int{return self.x}
}
fn main(){let c=C{x:1}
spawn c.get()}"#, "Task handle must be used -- call .get(), .detach(), or assign to a variable"); }

// Spawn closure
#[test]
fn spawn_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 spawn f(1)}"#, "expected newline after statement"); }

// Spawn lambda directly
#[test]
fn spawn_lambda() { compile_should_fail_with(r#"fn main(){spawn ((x:int)=>x+1)(1)}"#, "expected identifier, found ("); }

// Spawn builtin function
#[test]
fn spawn_builtin() {
    // Spawning a builtin is legal; discarding the handle is not
    compile_should_fail_with(r#"fn main(){spawn print("hi")}"#, "Task handle must be used");
}

// Spawn void function
#[test]
fn spawn_void_func() {
    // Spawning a void fn is legal (see integration spawn_void_function);
    // discarding the handle is the error
    compile_should_fail_with(r#"fn task(){print("hi")} fn main(){spawn task()}"#, "Task handle must be used");
}

// Spawn constructor
#[test]
fn spawn_constructor() { compile_should_fail_with(r#"class C{x:int} fn main(){spawn C{x:1}}"#, "expected '(' or '.' after identifier in spawn expression"); }

// Spawn binary expression
#[test]
fn spawn_binop() { compile_should_fail_with(r#"fn main(){spawn 1+2}"#, "expected identifier, found 1"); }

// Spawn field access
#[test]
fn spawn_field_access() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} spawn c.x}"#, "expected newline after statement"); }

// Spawn array index
#[test]
fn spawn_array_index() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] spawn arr[0]}"#, "expected newline after statement"); }

// Spawn if expression
#[test]
fn spawn_if_expr() { compile_should_fail_with(r#"fn main(){spawn if true{1}else{2}}"#, "expected identifier, found if"); }

// Spawn match expression
#[test]
fn spawn_match() { compile_should_fail_with(r#"enum E{A B} fn main(){spawn match E.A{E.A{1}E.B{2}}}"#, "expected identifier, found match"); }

// Spawn string literal
#[test]
fn spawn_string_lit() { compile_should_fail_with(r#"fn main(){spawn "hello"}"#, "expected identifier, found \"hello\""); }

// Spawn in spawn args
#[test]
fn spawn_in_spawn_args() { compile_should_fail_with(r#"fn inner()int{return 1} fn outer(x:int)int{return x} fn main(){spawn outer(spawn inner())}"#, "argument 1 of 'outer': expected int, found Task<int>"); }

// Spawn generic function wrong type args
#[test]
fn spawn_generic_wrong_type() { compile_should_fail_with(r#"fn task<T>(x:T)T{return x} fn main(){spawn task<int>("hi")}"#, "expected '(' or '.' after identifier in spawn expression"); }

// Spawn with catch in args
#[test]
fn spawn_catch_in_args() { compile_should_fail_with(r#"error E{}
fn f()int{raise E{}}
fn task(x:int)int{return x}
fn main(){spawn task(f() catch{0})}"#, "unexpected token { in expression"); }

// Spawn recursive function
#[test]
fn spawn_recursive() {
    // Spawning a recursive fn is legal; discarding the handle is not
    compile_should_fail_with(r#"fn rec(n:int)int{if n==0{return 1}else{return rec(n-1)}} fn main(){spawn rec(5)}"#, "Task handle must be used");
}

// Spawn trait method
#[test]
fn spawn_trait_method() { compile_should_fail_with(r#"trait T{fn f(self)int}
class C impl T {x:int
fn f(self)int{return self.x}}
fn main(){let c=C{x:1}
spawn c.f()}"#, "Task handle must be used -- call .get(), .detach(), or assign to a variable"); }

// Spawn with nullable return
#[test]
fn spawn_nullable_return() { // Spawning a nullable-returning fn is legal; t.get() yields T? (audit: premise flipped)
    assert!(pluto::compile_to_object(r#"fn task()int?{return none} fn main(){let t=spawn task()}"#).is_ok()); }

// Spawn with error return unhandled
#[test]
fn spawn_error_return() { // The audit found the old should-fail source never parsed; the
    // repaired program is accepted — pin that
    assert!(pluto::compile_to_object(r#"error E{}
fn task()int{raise E{}}
fn main(){let t=spawn task()}"#).is_ok()); }

// Spawn array method
#[test]
fn spawn_array_method() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] spawn arr.len()}"#, "expected newline after statement"); }

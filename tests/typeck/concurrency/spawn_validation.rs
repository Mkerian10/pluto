//! Spawn expression validation tests - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Spawn method call
#[test] fn spawn_method_call() { compile_should_fail_with(r#"class C{x:int} fn get(self)int{return self.x} fn main(){let c=C{x:1} spawn c.get()}"#, ""); }

// Spawn closure
#[test] fn spawn_closure() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>x+1 spawn f(1)}"#, ""); }

// Spawn lambda directly
#[test] fn spawn_lambda() { compile_should_fail_with(r#"fn main(){spawn ((x:int)=>x+1)(1)}"#, ""); }

// Spawn builtin function
#[test] fn spawn_builtin() { compile_should_fail_with(r#"fn main(){spawn print("hi")}"#, ""); }

// Spawn void function
#[test] fn spawn_void_func() { compile_should_fail_with(r#"fn task(){print("hi")} fn main(){spawn task()}"#, ""); }

// Spawn constructor
#[test] fn spawn_constructor() { compile_should_fail_with(r#"class C{x:int} fn main(){spawn C{x:1}}"#, ""); }

// Spawn binary expression
#[test] fn spawn_binop() { compile_should_fail_with(r#"fn main(){spawn 1+2}"#, ""); }

// Spawn field access
#[test] fn spawn_field_access() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} spawn c.x}"#, ""); }

// Spawn array index
#[test] fn spawn_array_index() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] spawn arr[0]}"#, ""); }

// Spawn if expression
#[test] fn spawn_if_expr() { compile_should_fail_with(r#"fn main(){spawn if true{1}else{2}}"#, ""); }

// Spawn match expression
#[test] fn spawn_match() { compile_should_fail_with(r#"enum E{A B} fn main(){spawn match E.A{E.A{1}E.B{2}}}"#, ""); }

// Spawn string literal
#[test] fn spawn_string_lit() { compile_should_fail_with(r#"fn main(){spawn "hello"}"#, ""); }

// Spawn in spawn args
#[test] fn spawn_in_spawn_args() { compile_should_fail_with(r#"fn inner()int{return 1} fn outer(x:int)int{return x} fn main(){spawn outer(spawn inner())}"#, ""); }

// Spawn generic function wrong type args
#[test] fn spawn_generic_wrong_type() { compile_should_fail_with(r#"fn task<T>(x:T)T{return x} fn main(){spawn task<int>("hi")}"#, ""); }

// Spawn with catch in args
#[test] fn spawn_catch_in_args() { compile_should_fail_with(r#"error E{} fn f()!int{raise E{}} fn task(x:int)int{return x} fn main(){spawn task(f() catch{0})}"#, ""); }

// Spawn recursive function
#[test] fn spawn_recursive() { compile_should_fail_with(r#"fn rec(n:int)int{if n==0{return 1}else{return rec(n-1)}} fn main(){spawn rec(5)}"#, ""); }

// Spawn trait method
#[test] fn spawn_trait_method() { compile_should_fail_with(r#"trait T{fn f(self)int} class C{x:int} impl T{fn f(self)int{return self.x}} fn main(){let c=C{x:1} spawn c.f()}"#, ""); }

// Spawn with nullable return
#[test] fn spawn_nullable_return() { compile_should_fail_with(r#"fn task()int?{return none} fn main(){let t=spawn task()}"#, ""); }

// Spawn with error return unhandled
#[test] fn spawn_error_return() { compile_should_fail_with(r#"error E{} fn task()!int{raise E{}} fn main(){let t=spawn task()}"#, ""); }

// Spawn array method
#[test] fn spawn_array_method() { compile_should_fail_with(r#"fn main(){let arr=[1,2,3] spawn arr.len()}"#, ""); }

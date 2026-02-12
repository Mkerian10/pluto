//! Lifetime and scope lifetime errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Use after scope ends
#[test] fn use_after_scope() { compile_should_fail_with(r#"fn main(){if true{let x=1}let y=x}"#, "undefined"); }

// Return local reference (if supported)
#[test] fn return_local_ref() { compile_should_fail_with(r#"fn f()&int{let x=1 return &x} fn main(){}"#, ""); }

// Closure captures after scope
#[test] fn closure_after_scope() { compile_should_fail_with(r#"fn main(){let f if true{let x=1 f=()=>x}}"#, ""); }

// Use loop variable after loop
#[test] fn use_loop_var_after() { compile_should_fail_with(r#"fn main(){for i in 0..10{}let x=i}"#, "undefined"); }

// Access match binding outside
#[test] fn match_binding_outside() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){match E.A{x:1}{E.A{x}{}}let y=x}"#, "undefined"); }

// Nested scope lifetime
#[test] fn nested_scope_lifetime() { compile_should_fail_with(r#"fn main(){let x if true{if true{x=1}}let y=x}"#, ""); }

// Conditional initialization
#[test] fn conditional_init() { compile_should_fail_with(r#"fn main(){let x:int if true{x=1}let y=x}"#, ""); }

// Variable escapes scope
#[test] fn var_escapes_scope() { compile_should_fail_with(r#"fn main(){let x if true{let y=1 x=y}}"#, ""); }

// Temporary lifetime
#[test] fn temporary_lifetime() { compile_should_fail_with(r#"class C{x:int} fn f()C{return C{x:1}} fn main(){let x=f().x}"#, ""); }

// Closure captures temporary
#[test] fn closure_captures_temp() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){let g=()=>f()}"#, ""); }

// Reference to moved value (if supported)
#[test] fn ref_after_move() { compile_should_fail_with(r#"class C{x:int} fn main(){let c=C{x:1} let d=c let e=c}"#, ""); }

// Use in wrong scope level
#[test] fn wrong_scope_level() { compile_should_fail_with(r#"fn main(){{let x=1}let y=x}"#, "undefined"); }

// Variable lifetime in while
#[test] fn while_lifetime() { compile_should_fail_with(r#"fn main(){while true{let x=1}let y=x}"#, "undefined"); }

// Break carries value (not supported)
#[test] fn break_with_value() { compile_should_fail_with(r#"fn main(){while true{let x=1 break x}}"#, ""); }

// Lifetime across function boundary
#[test] fn cross_function_lifetime() { compile_should_fail_with(r#"fn f(){let x=1} fn g(){let y=x} fn main(){}"#, ""); }

// Static lifetime (if supported)
#[test] fn static_lifetime() { compile_should_fail_with(r#"static x:int=1 fn main(){let y=x}"#, ""); }

// Lifetime in error handling
#[test] fn lifetime_in_catch() { compile_should_fail_with(r#"error E{} fn f()!{raise E{}} fn main(){let x if true{f() catch{x=1}}}"#, ""); }

// Lifetime in spawn
#[test] fn lifetime_in_spawn() { compile_should_fail_with(r#"fn f()int{let x=1 return x} fn main(){spawn f()}"#, ""); }

// Global vs local lifetime
#[test] fn global_local_lifetime() { compile_should_fail_with(r#"let x=1 fn main(){let y=x}"#, ""); }

// Const lifetime
#[test] fn const_lifetime() { compile_should_fail_with(r#"const X:int=1 fn main(){let y=X}"#, ""); }

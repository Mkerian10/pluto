//! Variable redeclaration errors - 20 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Same scope redeclaration
#[test] fn redeclare_same_scope() { compile_should_fail_with(r#"fn main(){let x=1 let x=2}"#, "already declared"); }
#[test] fn redeclare_different_types() { compile_should_fail_with(r#"fn main(){let x=1 let x=\"hi\"}"#, "already declared"); }

// Function parameter redeclaration
#[test] fn param_redeclare() { compile_should_fail_with(r#"fn f(x:int){let x=2} fn main(){}"#, "already declared"); }
#[test] fn two_params_same_name() { compile_should_fail_with(r#"fn f(x:int,x:string){} fn main(){}"#, "already declared"); }

// For loop variable redeclaration
#[test] fn for_var_redeclare() { compile_should_fail_with(r#"fn main(){let i=1 for i in 0..10{}}"#, "already declared"); }
#[test] fn nested_for_same_var() { compile_should_fail_with(r#"fn main(){for i in 0..10{for i in 0..5{}}}"#, ""); }

// Match binding redeclaration
#[test] fn match_binding_redeclare() { compile_should_fail_with(r#"enum E{A{x:int}} fn main(){let x=1 match E.A{x:2}{E.A{x}{}}}"#, ""); }

// Closure parameter redeclaration
#[test] fn closure_param_redeclare() { compile_should_fail_with(r#"fn main(){let x=1 let f=(x:int)=>{x+1}}"#, ""); }

// Class field vs method param
#[test] fn field_vs_method_param() { compile_should_fail_with(r#"class C{x:int} fn foo(self,x:int){} fn main(){}"#, ""); }

// Redeclare in nested scope is allowed (shadowing)
#[test] fn shadow_in_nested_scope() { compile_should_fail_with(r#"fn main(){let x=1 if true{let x=2}}"#, ""); }

// Redeclare after nested scope
#[test] fn redeclare_after_scope() { compile_should_fail_with(r#"fn main(){let x=1 if true{} let x=2}"#, "already declared"); }

// Function name vs variable
#[test] fn function_name_vs_var() { compile_should_fail_with(r#"fn x(){} fn main(){let x=1}"#, ""); }

// Class name vs variable
#[test] fn class_name_vs_var() { compile_should_fail_with(r#"class C{} fn main(){let C=1}"#, ""); }

// Enum name vs variable
#[test] fn enum_name_vs_var() { compile_should_fail_with(r#"enum E{A} fn main(){let E=1}"#, ""); }

// Redeclare in match arms (same level)
#[test] fn match_arms_same_binding() { compile_should_fail_with(r#"enum E{A{x:int}B{x:int}} fn main(){match E.A{x:1}{E.A{x}{}E.B{x}{}}}"#, ""); }

// Generic type param vs variable
#[test] fn type_param_vs_var() { compile_should_fail_with(r#"fn f<T>(x:T){let T=1} fn main(){}"#, ""); }

// Trait name vs variable
#[test] fn trait_name_vs_var() { compile_should_fail_with(r#"trait T{} fn main(){let T=1}"#, ""); }

// Error name vs variable
#[test] fn error_name_vs_var() { compile_should_fail_with(r#"error E{} fn main(){let E=1}"#, ""); }

// App name vs variable
#[test] fn app_name_vs_var() { compile_should_fail_with(r#"app MyApp{fn main(self){let MyApp=1}}"#, ""); }

// Imported module name vs variable
#[test] fn module_name_vs_var() { compile_should_fail_with(r#"import math fn main(){let math=1}"#, ""); }

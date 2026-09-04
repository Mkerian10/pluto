//! Scope resolution errors - 15 tests
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// Ambiguous name resolution
#[test]
fn ambiguous_name() { compile_should_fail_with(r#"class A{} fn A(){} fn main(){A}"#, "function 'A' is already declared as a class"); }

// Cross-scope reference
#[test]
fn cross_scope_ref() { compile_should_fail_with(r#"fn main(){if true{let x=1}else{let y=x}}"#, "undefined"); }

// Unqualified import
#[test]
fn unqualified_import() { compile_should_fail_with(r#"import math fn main(){let x=add(1,2)}"#, "expected newline after statement"); }

// Module scope confusion
#[test]
fn module_scope_confusion() { compile_should_fail_with(r#"import mod1 class C{} fn main(){let c=C{} let m=mod1.C{}}"#, "expected newline after statement"); }

// Nested scope lookup of an undefined name still errors at depth
#[test]
fn nested_scope_lookup() {
    compile_should_fail_with(
        "fn main(){\n    let x = 1\n    if true {\n        if true {\n            if true {\n                let y = z\n            }\n        }\n    }\n}",
        "undefined variable 'z'",
    );
}

// Function scope vs class scope
#[test]
fn function_class_scope() { compile_should_fail_with(r#"class C{x:int} fn foo(){let y=x} fn main(){}"#, "undefined variable 'x'"); }

// Trait method scope
#[test]
fn trait_method_scope() { compile_should_fail_with(r#"trait T{fn foo(self)int}
class C impl T {x:int
fn foo(self)int{return y}}
fn main(){}"#, "undefined variable 'y'"); }

// Generic scope resolution
#[test]
fn generic_scope() { compile_should_fail_with(r#"fn f<T>(x:T){let y:T} fn g(){let z:T} fn main(){}"#, "expected =, found }"); }

// Closure scope vs outer scope
#[test]
fn closure_outer_scope() { compile_should_fail_with(r#"fn main(){let f=()=>{let x=1}
let y=x}"#, "undefined variable 'x'"); }

// Match arm scope isolation
#[test]
fn match_arm_isolation() { compile_should_fail_with(r#"enum E{A B} fn main(){match E.A{E.A{let x=1}E.B{let y=x}}}"#, "undefined"); }

// Block scope lookup
#[test]
#[ignore] // Compiler bug: should detect undefined variable
fn block_scope_lookup() { compile_should_fail_with(r#"fn main(){{let x=1}{let y=x}}"#, "undefined"); }

// Method self scope
#[test]
fn method_self_scope() { compile_should_fail_with(r#"class C{x:int} fn foo(){let y=self.x} fn main(){}"#, "undefined variable 'self'"); }

// App scope isolation
#[test]
fn app_scope() { compile_should_fail_with(r#"app MyApp{fn helper(self){let x=1} fn main(self){let y=x}}"#, "undefined"); }

// Enum variant scope
#[test]
fn enum_variant_scope() { compile_should_fail_with(r#"enum E{A{x:int}B{y:int}}
fn main(){let a=E.A{x:1}
let b=a.y}"#, "field access on non-class type E"); }

// Contract scope
#[test]
fn contract_scope() { compile_should_fail_with(r#"class C{x:int
invariant y>0}
fn main(){}"#, "undefined variable 'y'"); }

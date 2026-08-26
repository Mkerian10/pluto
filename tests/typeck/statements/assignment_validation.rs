//! Assignment validation - 25 tests
#[path = "../common.rs"]
mod common;
use common::{compile_and_run, compile_should_fail_with};

// Assign to undefined variable
#[test]
fn assign_undefined() { compile_should_fail_with(r#"fn main(){x=1}"#, "undefined"); }
#[test]
fn assign_undefined_field() { compile_should_fail_with(r#"class C{x:int}
fn main(){let mut c=C{x:1}
c.y=2}"#, "no field"); }

// Type mismatch in assignment
#[test]
fn assign_type_mismatch() { compile_should_fail_with(r#"fn main(){let mut x=1
x="hi"}"#, "type mismatch"); }
#[test]
fn assign_field_type_mismatch() { compile_should_fail_with(r#"class C{x:int}
fn main(){let mut c=C{x:1}
c.x="hi"}"#, "expected int, found string"); }

// Reassignment requires `let mut` (#282)
#[test]
fn variable_reassignment() {
    assert_eq!(compile_and_run("fn main(){\n    let mut x = 1\n    x = 2\n    print(x)\n}"), 0);
}

// Parameter reassignment requires `mut x: int` (#282)
#[test]
fn param_reassignment() {
    assert_eq!(compile_and_run("fn f(mut x: int){\n    x = 2\n    print(x)\n}\n\nfn main(){f(1)}"), 0);
}

// Loop variables are immutable (#282)
#[test]
fn for_var_reassignment() {
    compile_should_fail_with("fn main(){\n    for i in 0..3 {\n        i = 5\n    }\n}", "cannot assign to immutable variable");
}

// Assign to literal
#[test]
fn assign_to_literal() { compile_should_fail_with(r#"fn main(){1=2}"#, ""); }

// Assign to function call result
#[test]
fn assign_to_call() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){f()=2}"#, ""); }

// Assign to binary expression
#[test]
fn assign_to_binop() { compile_should_fail_with(r#"fn main(){let x=1 let y=2 (x+y)=3}"#, ""); }

// Array index assignment type mismatch
#[test]
fn array_index_assign_mismatch() { compile_should_fail_with(r#"fn main(){let mut arr=[1,2,3]
arr[0]="hi"}"#, "expected int, found string"); }

// Map value assignment type mismatch
#[test]
fn map_assign_mismatch() { compile_should_fail_with(r#"fn main(){let mut m=Map<string,int>{}
m["a"]="hi"}"#, "type mismatch"); }

// Assign to method call result
#[test]
fn assign_to_method_call() { compile_should_fail_with(r#"class C{} fn foo(self)int{return 1} fn main(){let c=C{} c.foo()=2}"#, ""); }

// Assign to enum variant
#[test]
fn assign_to_enum() { compile_should_fail_with(r#"enum E{A} fn main(){E.A=2}"#, ""); }

// Assign to trait object field
#[test]
fn assign_trait_object_field() { compile_should_fail_with(r#"trait T{} class C{x:int} impl T fn main(){let t:T=C{x:1}
t.x=2}"#, ""); }

// Assign to self in non-mut method
#[test]
fn assign_self_non_mut() { compile_should_fail_with(r#"class C{x:int} fn foo(self){self.x=2} fn main(){}"#, ""); }

// Assign to closure capture
#[test]
fn assign_capture() { compile_should_fail_with(r#"fn main(){let x=1 let f=()=>{x=2}}"#, ""); }

// Assign nullable to non-nullable
#[test]
fn assign_nullable_mismatch() { compile_should_fail_with(r#"fn main(){let x:int?=none
let y:int=x}"#, "type mismatch"); }

// Assign in expression position (not statement)
#[test]
fn assign_in_expr() { compile_should_fail_with(r#"fn main(){let x=1 let y=(x=2)}"#, ""); }

// Compound assignment on undefined
#[test]
fn compound_assign_undefined() { compile_should_fail_with(r#"fn main(){x+=1}"#, ""); }

// Array element assign out of bounds aborts at runtime (not a typeck error)
#[test]
fn array_assign_bounds() {
    let code = compile_and_run(
        "fn main(){\n    let mut arr = [1, 2, 3]\n    arr[10] = 5\n}",
    );
    assert_ne!(code, 0, "out-of-bounds index assignment should abort at runtime");
}

// Assign to string index (strings are immutable)
#[test]
fn assign_string_index() { compile_should_fail_with(r#"fn main(){let s=\"hi\"s[0]=\"x\"}"#, ""); }

// Assign generic type mismatch
#[test]
fn assign_generic_mismatch() { compile_should_fail_with(r#"class Box<T>{value:T}
fn main(){let mut b=Box<int>{value:1}
b.value="hi"}"#, "expected int, found string"); }

// Assign to spawn result
#[test]
fn assign_spawn_result() { compile_should_fail_with(r#"fn f()int{return 1} fn main(){spawn f()=2}"#, ""); }

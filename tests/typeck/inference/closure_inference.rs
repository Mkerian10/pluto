//! Closure type inference tests.
//!
//! Untyped params (`(x) => x + 1`) resolve from the expected fn type at the
//! use site and error with "cannot infer" when no context provides one.
//! Return-type annotations (`() void => ...`) are checked against the body.
//! Error effects are always inferred — a `!` on the annotation is rejected.
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn closure_return_mismatch() { compile_should_fail_with(r#"fn main(){let f=(x:int)int=>"hi"}"#, "expected int, found string"); }

#[test]
fn closure_param_mismatch() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = (x: int) => x
    f("hi")
}
"#,
        "expected int, found string",
    );
}

#[test]
fn closure_capture_wrong_type() {
    compile_should_fail_with(
        r#"
fn main() {
    let s = "hi"
    let f = (x: int) => s + x
}
"#,
        "operand type mismatch",
    );
}

#[test]
fn closure_no_param_type() {
    // #165: untyped param with no context to infer from
    compile_should_fail_with(
        r#"
fn main() {
    let f = (x) => x + 1
}
"#,
        "cannot infer",
    );
}

#[test]
fn closure_no_return_type_ambiguous() { compile_should_fail_with(r#"fn main(){let f=(x:int)=>{if x>0{return 1}return "no"}}"#, "return type mismatch"); }

#[test]
fn closure_capture_undefined() { compile_should_fail_with(r#"fn main(){let f=()=>undefined}"#, "undefined"); }

#[test]
fn closure_wrong_arg_count() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = (x: int, y: int) => x + y
    f(1)
}
"#,
        "expects 2 arguments, got 1",
    );
}

#[test]
fn closure_return_void_with_value() {
    // #165: explicit void return annotation vs value-returning body
    compile_should_fail_with(
        r#"
fn main() {
    let f = () void => {
        return 42
    }
}
"#,
        "expected void, found int",
    );
}

#[test]
fn closure_in_binop() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = (x: int) => x + 1
    let g = f + 1
    print(g)
}
"#,
        "operand type mismatch",
    );
}

#[test]
fn closure_generic_param_unresolved() {
    // #165: generic call args are inferred without a concrete hint, so an
    // untyped closure param cannot resolve there
    compile_should_fail_with(
        r#"
fn apply<T>(f: fn(T) T, x: T) T {
    return f(x)
}

fn main() {
    apply((x) => x + 1, 42)
}
"#,
        "cannot infer",
    );
}

#[test]
fn closure_field_assign() { compile_should_fail_with(r#"class C{x:int} fn main(){let f=(c:C)=>{c.x="hi"}}"#, "expected int, found string"); }

#[test]
fn closure_fallible_annotation_rejected() {
    // #165: closure error effects are inferred, never annotated
    compile_should_fail_with(
        r#"
fn safe() int {
    return 1
}

fn main() {
    let f = () int! => {
        return safe()!
    }
}
"#,
        "closure error effects are inferred",
    );
}

#[test]
fn closure_propagate_on_infallible() {
    compile_should_fail_with(
        r#"
fn safe() int {
    return 1
}

fn main() {
    let f = () => safe()!
}
"#,
        "'!' applied to infallible function",
    );
}

#[test]
fn nested_closure_scope() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = () => {
        let x = 1
        let g = () => y
    }
}
"#,
        "undefined",
    );
}

#[test]
fn closure_mut_capture() { compile_should_fail_with(r#"fn main(){let x=1
let f=()=>{x=2}}"#, "cannot assign to immutable variable 'x'"); }

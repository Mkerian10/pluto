//! `as` cast removal tests
//!
//! The `as` cast was removed (rfc-number-types phase 3): every conversion is
//! now a named method. A cast of any kind is a parse error whose message
//! points at the replacement, so each of these programs fails to compile.

#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn cast_int_to_string() {
    compile_should_fail_with(r#"fn main() { let x = 42 as string }"#, "removed");
}

#[test]
fn cast_string_to_int() {
    compile_should_fail_with(r#"fn main() { let x = "42" as int }"#, "removed");
}

#[test]
fn cast_bool_to_string() {
    compile_should_fail_with(r#"fn main() { let x = true as string }"#, "removed");
}

#[test]
fn cast_array_to_int() {
    compile_should_fail_with(r#"fn main() { let x = [1,2,3] as int }"#, "removed");
}

#[test]
fn cast_class_to_int() {
    compile_should_fail_with(
        r#"class Point { x: int }
fn main() { let p = Point{x:1}
let x = p as int }"#,
        "removed",
    );
}

#[test]
fn cast_nullable_to_concrete() {
    compile_should_fail_with(r#"fn main() { let x: int? = 5
let y = x as int }"#, "removed");
}

#[test]
fn cast_map_to_array() {
    compile_should_fail_with(
        r#"fn main() { let m = Map<string,int>{}
let a = m as [int] }"#,
        "removed",
    );
}

#[test]
fn cast_closure_to_int() {
    compile_should_fail_with(
        r#"fn main() { let f = (x:int) => x+1
let n = f as int }"#,
        "removed",
    );
}

#[test]
fn cast_enum_to_int() {
    compile_should_fail_with(
        r#"enum Color{Red}
fn main() { let c = Color.Red
let x = c as int }"#,
        "removed",
    );
}

#[test]
fn cast_task_to_int() {
    compile_should_fail_with(
        r#"fn work()int{return 42}
fn main(){ let t=spawn work()
let x=t as int }"#,
        "removed",
    );
}

// Total: 10 tests

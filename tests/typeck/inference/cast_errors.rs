//! Cast-removal tests
//!
//! The `as` cast operator was removed (rfc-number-types phase 3). Every use
//! is now rejected at parse time with a fix-it pointing at the replacement
//! conversion method. These tests pin that the removal diagnostic fires,
//! including for former casts that never had a numeric meaning.

#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

#[test]
fn cast_int_to_string() {
    compile_should_fail_with(r#"fn main() { let x = 42 as string }"#, "was removed");
}

#[test]
fn cast_string_to_int() {
    compile_should_fail_with(r#"fn main() { let x = "42" as int }"#, "was removed");
}

#[test]
fn cast_bool_to_string() {
    compile_should_fail_with(r#"fn main() { let x = true as string }"#, "was removed");
}

#[test]
fn cast_array_to_int() {
    compile_should_fail_with(r#"fn main() { let x = [1,2,3] as int }"#, "was removed");
}

#[test]
fn cast_class_to_int() {
    compile_should_fail_with(
        r#"class Point { x: int }
fn main() { let p = Point{x:1}
let x = p as int }"#,
        "was removed",
    );
}

#[test]
fn cast_nullable_to_concrete() {
    compile_should_fail_with(r#"fn main() { let x: int? = 5
let y = x as int }"#, "was removed");
}

#[test]
fn cast_map_to_array() {
    compile_should_fail_with(
        r#"fn main() { let m = Map<string,int>{}
let a = m as [int] }"#,
        "was removed",
    );
}

#[test]
fn cast_closure_to_int() {
    compile_should_fail_with(
        r#"fn main() { let f = (x:int) => x+1
let n = f as int }"#,
        "was removed",
    );
}

#[test]
fn cast_enum_to_int() {
    compile_should_fail_with(
        r#"enum Color{Red}
fn main() { let c = Color.Red
let x = c as int }"#,
        "was removed",
    );
}

#[test]
fn cast_task_to_int() {
    compile_should_fail_with(
        r#"fn work()int{return 42}
fn main(){ let t=spawn work()
let x=t as int }"#,
        "was removed",
    );
}

// Total: 10 tests

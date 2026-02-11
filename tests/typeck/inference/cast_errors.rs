//! Type cast error tests
//!
//! Tests invalid type casting operations.
//! Pluto's casting rules (from src/typeck/infer.rs):
//! - Allowed: int↔float, int↔bool, int↔byte, float↔bool, float↔byte
//! - Forbidden: any cast involving string, class, array, etc.

mod common;
use common::compile_should_fail_with;

#[test]
fn cast_int_to_string() {
    compile_should_fail_with(r#"fn main() { let x = 42 as string }"#, "invalid cast");
}

#[test]
fn cast_string_to_int() {
    compile_should_fail_with(r#"fn main() { let x = "42" as int }"#, "invalid cast");
}

#[test]
fn cast_bool_to_string() {
    compile_should_fail_with(r#"fn main() { let x = true as string }"#, "invalid cast");
}

#[test]
fn cast_array_to_int() {
    compile_should_fail_with(r#"fn main() { let x = [1,2,3] as int }"#, "invalid cast");
}

#[test]
fn cast_class_to_int() {
    compile_should_fail_with(
        r#"class Point { x: int } fn main() { let p = Point{x:1} let x = p as int }"#,
        "invalid cast",
    );
}

#[test]
fn cast_nullable_to_concrete() {
    compile_should_fail_with(r#"fn main() { let x: int? = 5 let y = x as int }"#, "invalid cast");
}

#[test]
fn cast_map_to_array() {
    compile_should_fail_with(
        r#"fn main() { let m = Map<string,int>{} let a = m as [int] }"#,
        "invalid cast",
    );
}

#[test]
fn cast_closure_to_int() {
    compile_should_fail_with(
        r#"fn main() { let f = (x:int) => x+1 let n = f as int }"#,
        "invalid cast",
    );
}

#[test]
fn cast_enum_to_int() {
    compile_should_fail_with(
        r#"enum Color{Red} fn main() { let c = Color.Red let x = c as int }"#,
        "invalid cast",
    );
}

#[test]
fn cast_task_to_int() {
    compile_should_fail_with(
        r#"fn work()int{return 42} fn main(){ let t=spawn work() let x=t as int }"#,
        "invalid cast",
    );
}

// Total: 10 tests

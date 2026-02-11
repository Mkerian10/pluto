//! Field access error tests
//!
//! Tests invalid field access operations.
//! Pluto's field access rules (from src/typeck/infer.rs):
//! - Classes: class_instance.field_name → field_type
//! - Errors: error_instance.field_name → field_type
//! - Enums: variant.field_name → field_type (data-carrying variants)

mod common;
use common::compile_should_fail_with;

// ============================================================================
// FIELD ACCESS ON NON-CLASS TYPES
// ============================================================================

#[test]
fn field_access_on_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 42
            let y = x.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_float() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 3.14
            let y = x.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_bool() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = true
            let y = x.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = "hello"
            let v = s.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_array() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let v = arr.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_map() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, int> { "a": 1 }
            let v = m.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_set() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = Set<int> { 1, 2, 3 }
            let v = s.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_closure() {
    compile_should_fail_with(
        r#"
        fn main() {
            let f = (x: int) => x + 1
            let v = f.value
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_task() {
    compile_should_fail_with(
        r#"
        fn worker() int {
            return 42
        }

        fn main() {
            let t = spawn worker()
            let v = t.value
        }
        "#,
        "no field",
    );
}

// ============================================================================
// UNKNOWN FIELD ON CLASS
// ============================================================================

#[test]
fn unknown_field_on_class() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point { x: 1, y: 2 }
            let v = p.z
        }
        "#,
        "no field",
    );
}

#[test]
fn typo_in_field_name() {
    compile_should_fail_with(
        r#"
        class Person { name: string, age: int }

        fn main() {
            let p = Person { name: "Alice", age: 30 }
            let n = p.nam
        }
        "#,
        "no field",
    );
}

#[test]
fn case_mismatch_field() {
    compile_should_fail_with(
        r#"
        class Config { debug_mode: bool, dummy: int }

        fn main() {
            let c = Config { debug_mode: true, dummy: 0 }
            let d = c.Debug_Mode
        }
        "#,
        "no field",
    );
}

// ============================================================================
// FIELD ACCESS ON NULLABLE TYPE
// ============================================================================

#[test]
fn field_access_on_nullable_class() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p: Point? = Point { x: 1, y: 2 }
            let v = p.x
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_none_literal() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = none.value
        }
        "#,
        "no field",
    );
}

// ============================================================================
// FIELD ACCESS ON ENUM
// ============================================================================

#[test]
fn field_access_on_unit_enum_variant() {
    compile_should_fail_with(
        r#"
        enum Color {
            Red
            Green
            Blue
        }

        fn main() {
            let c = Color.Red
            let v = c.value
        }
        "#,
        "no field",
    );
}

#[test]
fn unknown_field_on_enum_data_variant() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Circle { radius: float }
            Rectangle { width: float, height: float }
        }

        fn main() {
            let s = Shape.Circle { radius: 5.0 }
            let v = s.diameter
        }
        "#,
        "no field",
    );
}

// ============================================================================
// FIELD ACCESS ON GENERIC TYPES
// ============================================================================

#[test]
fn field_access_on_generic_type_param() {
    compile_should_fail_with(
        r#"
        fn get_x<T>(obj: T) int {
            return obj.x
        }

        fn main() {
            let v = get_x(42)
        }
        "#,
        "no field",
    );
}

#[test]
fn unknown_field_on_generic_class() {
    compile_should_fail_with(
        r#"
        class Box<T> {
            value: T
        }

        fn main() {
            let b = Box<int> { value: 42 }
            let v = b.data
        }
        "#,
        "no field",
    );
}

// ============================================================================
// FIELD ACCESS ON TRAIT OBJECT
// ============================================================================

#[test]
fn field_access_on_trait_object() {
    compile_should_fail_with(
        r#"
        trait Printable {
            fn print(self)
        }

        class Point impl Printable {
            x: int

            fn print(self) {
                print(self.x)
            }
        }

        fn main() {
            let p: Printable = Point { x: 5 }
            let v = p.x
        }
        "#,
        "no field",
    );
}

// ============================================================================
// CHAINED FIELD ACCESS ERRORS
// ============================================================================

#[test]
fn chained_field_access_first_invalid() {
    compile_should_fail_with(
        r#"
        class Inner { value: int }
        class Outer { inner: Inner }

        fn main() {
            let o = Outer { inner: Inner { value: 42 } }
            let v = o.wrong.value
        }
        "#,
        "no field",
    );
}

#[test]
fn chained_field_access_second_invalid() {
    compile_should_fail_with(
        r#"
        class Inner { value: int }
        class Outer { inner: Inner }

        fn main() {
            let o = Outer { inner: Inner { value: 42 } }
            let v = o.inner.wrong
        }
        "#,
        "no field",
    );
}

#[test]
fn field_access_on_method_result_non_class() {
    compile_should_fail_with(
        r#"
        class Counter { count: int }

        fn main() {
            let c = Counter { count: 5 }
            let v = c.count.value
        }
        "#,
        "no field",
    );
}

// ============================================================================
// FIELD ASSIGNMENT ERRORS
// ============================================================================

#[test]
fn assign_to_unknown_field() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point { x: 1, y: 2 }
            p.z = 3
        }
        "#,
        "no field",
    );
}

#[test]
fn assign_wrong_type_to_field() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point { x: 1, y: 2 }
            p.x = "hello"
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn field_assign_on_non_class() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 42
            x.value = 10
        }
        "#,
        "no field",
    );
}

// ============================================================================
// BRACKET DEPS FIELD ACCESS (DI system)
// ============================================================================

#[test]
fn access_bracket_dep_as_regular_field() {
    compile_should_fail_with(
        r#"
        class Database { connection: string }
        class UserRepo[db: Database] {
            name: string
        }

        fn main() {
            let db = Database { connection: "localhost" }
            let repo = UserRepo { name: "users" }
            let c = repo.db
        }
        "#,
        "no field",
    );
}

// ============================================================================
// MODULE-QUALIFIED FIELD ACCESS
// ============================================================================

#[test]
fn field_access_with_module_prefix() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point { x: 1, y: 2 }
            let v = p.Point.x
        }
        "#,
        "no field",
    );
}

// Total tests in this file: 30

//! Index operation type error tests
//!
//! Tests invalid indexing operations on various types.
//! Pluto's indexing rules (from src/typeck/infer.rs):
//! - Arrays: [T][int] → T
//! - Maps: Map<K,V>[K] → V
//! - Strings: string[int] → string (single char)
//! - Bytes: bytes[int] → byte

#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// ============================================================================
// INDEX ON NON-INDEXABLE TYPES
// ============================================================================

#[test]
#[ignore]
fn index_int_literal() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 42[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_float_literal() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 3.14[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_bool() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = true[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_class() {
    compile_should_fail_with(
        r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point { x: 1, y: 2 }
            let v = p[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_enum() {
    compile_should_fail_with(
        r#"
        enum Color {
            Red
            Green
            Blue
        }

        fn main() {
            let c = Color.Red
            let v = c[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_closure() {
    compile_should_fail_with(
        r#"
        fn main() {
            let f = (x: int) => x + 1
            let v = f[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_task() {
    compile_should_fail_with(
        r#"
        fn worker() int {
            return 42
        }

        fn main() {
            let t = spawn worker()
            let v = t[0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_set() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = Set<int> { 1, 2, 3 }
            let v = s[0]
        }
        "#,
        "cannot index",
    );
}

// ============================================================================
// WRONG INDEX KEY TYPE
// ============================================================================

#[test]
#[ignore]
fn index_array_with_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let v = arr["0"]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_array_with_float() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let v = arr[0.5]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_array_with_bool() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let v = arr[true]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_string_with_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = "hello"
            let c = s["0"]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_string_with_float() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = "hello"
            let c = s[2.5]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_bytes_with_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let b = b"hello"
            let v = b["0"]
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn index_map_string_int_with_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, int> { "a": 1 }
            let v = m[0]
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn index_map_int_string_with_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<int, string> { 1: "a" }
            let v = m["key"]
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn index_map_with_wrong_type() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, int> { "a": 1 }
            let v = m[true]
        }
        "#,
        "type mismatch",
    );
}

// ============================================================================
// NULLABLE INDEX KEY
// ============================================================================

#[test]
#[ignore]
fn index_array_with_nullable_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let idx: int? = 0
            let v = arr[idx]
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn index_map_with_nullable_key() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, int> { "a": 1 }
            let key: string? = "a"
            let v = m[key]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_string_with_nullable_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = "hello"
            let idx: int? = 0
            let c = s[idx]
        }
        "#,
        "type mismatch",
    );
}

// ============================================================================
// GENERIC TYPE PARAMETER AS INDEX
// ============================================================================

#[test]
#[ignore]
fn index_array_with_generic_param() {
    compile_should_fail_with(
        r#"
        fn get<T>(arr: [int], idx: T) int {
            return arr[idx]
        }

        fn main() {
            let arr = [1, 2, 3]
            let v = get(arr, 0)
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_map_with_generic_param_wrong_bound() {
    compile_should_fail_with(
        r#"
        fn lookup<K>(m: Map<string, int>, key: K) int {
            return m[key]
        }

        fn main() {
            let m = Map<string, int> { "a": 1 }
            let v = lookup(m, "a")
        }
        "#,
        "type mismatch",
    );
}

// ============================================================================
// MULTI-DIMENSIONAL INDEX ERRORS
// ============================================================================

#[test]
#[ignore]
fn double_index_non_nested_array() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [1, 2, 3]
            let v = arr[0][0]
        }
        "#,
        "cannot index",
    );
}

#[test]
#[ignore]
fn index_array_element_of_wrong_type() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [[1, 2], [3, 4]]
            let v = arr[0]["x"]
        }
        "#,
        "type mismatch",
    );
}

// ============================================================================
// INDEX ASSIGNMENT TYPE ERRORS
// ============================================================================

#[test]
#[ignore]
fn assign_wrong_type_to_array_index() {
    compile_should_fail_with(
        r#"
        fn main() {
            let mut arr = [1, 2, 3]
            arr[0] = "hello"
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn assign_wrong_type_to_map_value() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, int> { "a": 1 }
            m["a"] = "value"
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn assign_to_string_index() {
    compile_should_fail_with(
        r#"
        fn main() {
            let s = "hello"
            s[0] = "H"
        }
        "#,
        "cannot assign",
    );
}

// ============================================================================
// COMPLEX NESTED INDEX ERRORS
// ============================================================================

#[test]
#[ignore]
fn index_map_of_arrays_with_wrong_inner_index() {
    compile_should_fail_with(
        r#"
        fn main() {
            let m = Map<string, [int]> { "nums": [1, 2, 3] }
            let v = m["nums"]["x"]
        }
        "#,
        "type mismatch",
    );
}

#[test]
fn index_array_of_maps_with_wrong_map_key() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr = [Map<string, int> { "a": 1 }]
            let v = arr[0][42]
        }
        "#,
        "type mismatch",
    );
}

#[test]
#[ignore]
fn index_nullable_array() {
    compile_should_fail_with(
        r#"
        fn main() {
            let arr: [int]? = [1, 2, 3]
            let v = arr[0]
        }
        "#,
        "cannot index",
    );
}

// Total tests in this file: 35

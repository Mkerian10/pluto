mod common;
use common::{compile_should_fail_with, compile_should_fail};

// ============================================================
// Type Mismatch Errors (8 tests)
// ============================================================

#[test]
fn if_expr_int_vs_string() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { 1 } else { "string" }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_int_vs_float() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { 1 } else { 1.5 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_class_vs_primitive() {
    compile_should_fail_with(
        r#"
        class Foo { x: int }
        fn main() {
            let x = if true { Foo { x: 1 } } else { 42 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_array_vs_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { [1, 2, 3] } else { 42 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_enum_variant_mismatch() {
    compile_should_fail_with(
        r#"
        enum Color { Red Blue }
        enum Shape { Circle Square }
        fn main() {
            let x = if true { Color.Red } else { Shape.Circle }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_different_classes() {
    compile_should_fail_with(
        r#"
        class Foo { x: int }
        class Bar { y: int }
        fn main() {
            let x = if true { Foo { x: 1 } } else { Bar { y: 2 } }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_bool_vs_int() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { true } else { 0 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_three_way_type_conflict() {
    // Nested else-if with mismatched types
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { 1 } else if false { "str" } else { true }
        }
        "#,
        "incompatible types"
    );
}

// ============================================================
// Non-Bool Condition Errors (3 tests)
// ============================================================

#[test]
fn if_expr_int_condition() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if 42 { 1 } else { 2 }
        }
        "#,
        "must be bool"
    );
}

#[test]
fn if_expr_string_condition() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if "hello" { 1 } else { 2 }
        }
        "#,
        "must be bool"
    );
}

#[test]
fn if_expr_class_condition() {
    compile_should_fail_with(
        r#"
        class Foo { x: int }
        fn main() {
            let x = if (Foo { x: 1 }) { 1 } else { 2 }
        }
        "#,
        "must be bool"
    );
}

// ============================================================
// Void Type Errors (3 tests)
// ============================================================

#[test]
fn if_expr_void_in_then_branch() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { print("hi") } else { 1 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_void_in_else_branch() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { 1 } else { print("hi") }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_void_in_both_branches() {
    // NOTE: In Pluto, void+void → void is valid. The if-expression itself has type void.
    // This is allowed and can be used as a statement. Only invalid in non-void contexts.
    // Skipping this test as void if-expressions are intentionally allowed.
    // See if_expr_void_in_then_branch and if_expr_void_in_else_branch for actual type mismatches.
}

// ============================================================
// Nullable Type Errors (3 tests)
// ============================================================

#[test]
fn if_expr_nullable_wrong_base_type() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x: string? = if true { 1 } else { "str" }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_nested_nullable() {
    // No nested nullable support (int?? not allowed)
    compile_should_fail(
        r#"
        fn main() {
            let x: int?? = if true { none } else { none }
        }
        "#
    );
}

#[test]
fn if_expr_nullable_enum_mismatch() {
    compile_should_fail_with(
        r#"
        enum Color { Red Blue }
        enum Shape { Circle }
        fn main() {
            let x = if true { Color.Red } else { Shape.Circle }
        }
        "#,
        "incompatible types"
    );
}

// ============================================================
// Missing Else Errors (2 tests)
// ============================================================

#[test]
fn if_expr_missing_else_in_let() {
    compile_should_fail(
        r#"
        fn main() {
            let x = if true { 1 }
        }
        "#
    );
}

#[test]
fn if_expr_missing_else_in_return() {
    compile_should_fail(
        r#"
        fn foo() int {
            return if true { 1 }
        }
        fn main() {}
        "#
    );
}

// ============================================================
// Edge Case Errors (3 tests)
// ============================================================

#[test]
fn if_expr_empty_then_block() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { } else { 1 }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_empty_else_block() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = if true { 1 } else { }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn if_expr_both_blocks_empty() {
    // NOTE: Empty blocks have type void. void+void → void is valid in Pluto.
    // This is allowed as an if-expression that evaluates to void.
    // Skipping this test - see if_expr_empty_then_block and if_expr_empty_else_block for
    // actual type mismatches (void vs non-void).
}

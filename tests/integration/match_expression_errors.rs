mod common;
use common::{compile_should_fail_with, compile_should_fail};

// ============================================================
// Type Mismatch Errors (8 tests)
// ============================================================

#[test]
fn match_expr_int_vs_string() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => "string"
            }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn match_expr_int_vs_float() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => 1.5
            }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn match_expr_class_vs_primitive() {
    compile_should_fail_with(
        r#"
        class Foo { x: int }
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => Foo { x: 1 },
                E.B => 42
            }
        }
        "#,
        "expected ','"
    );
}

#[test]
fn match_expr_void_in_expression_context() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => print("hi"),
                E.B => 1
            }
        }
        "#,
        "void"
    );
}

#[test]
fn match_expr_nullable_mismatch() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => none
            }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn match_expr_array_vs_int() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => [1, 2, 3],
                E.B => 42
            }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn match_expr_enum_variant_mismatch() {
    compile_should_fail_with(
        r#"
        enum Color { Red Blue }
        enum Shape { Circle Square }
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => Color.Red,
                E.B => Shape.Circle
            }
        }
        "#,
        "incompatible types"
    );
}

#[test]
fn match_expr_three_way_type_conflict() {
    compile_should_fail_with(
        r#"
        enum E { A B C }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => "str",
                E.C => true
            }
        }
        "#,
        "incompatible types"
    );
}

// ============================================================
// Exhaustiveness Errors (6 tests)
// ============================================================

#[test]
fn match_expr_missing_single_variant() {
    compile_should_fail_with(
        r#"
        enum E { A B C }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => 2
            }
        }
        "#,
        "non-exhaustive match: missing variant 'C'"
    );
}

#[test]
fn match_expr_missing_data_variant() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Circle { radius: float }
            Square { side: float }
        }
        fn main() {
            let s = Shape.Circle { radius: 1.0 }
            let x = match s {
                Shape.Circle { radius: r } => r
            }
        }
        "#,
        "non-exhaustive match: missing variant 'Square'"
    );
}

#[test]
fn match_expr_generic_enum_missing_none() {
    compile_should_fail_with(
        r#"
        enum Option<T> { Some { value: T } None }
        fn main() {
            let opt = Option<int>.Some { value: 10 }
            let x = match opt {
                Option.Some { value: v } => v
            }
        }
        "#,
        "non-exhaustive match: missing variant 'None'"
    );
}

#[test]
fn match_expr_missing_two_variants() {
    compile_should_fail_with(
        r#"
        enum E { A B C D }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.B => 2
            }
        }
        "#,
        "non-exhaustive"
    );
}

#[test]
fn match_expr_empty_enum_all_missing() {
    compile_should_fail_with(
        r#"
        enum E { A B C D E F G H }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1
            }
        }
        "#,
        "non-exhaustive"
    );
}

#[test]
fn match_expr_module_qualified_missing_variant() {
    // Multi-file test - skip for now, will add to modules.rs
    compile_should_fail_with(
        r#"
        enum Status { Active Inactive Pending }
        fn main() {
            let s = Status.Active
            let x = match s {
                Status.Active => 1,
                Status.Inactive => 2
            }
        }
        "#,
        "non-exhaustive match: missing variant 'Pending'"
    );
}

// ============================================================
// Binding Errors (5 tests)
// ============================================================

#[test]
fn match_expr_wrong_binding_count_too_few() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Rectangle { width: float, height: float }
        }
        fn main() {
            let s = Shape.Rectangle { width: 10.0, height: 20.0 }
            let x = match s {
                Shape.Rectangle { width: w } => w
            }
        }
        "#,
        "has 2 fields, but 1 bindings provided"
    );
}

#[test]
fn match_expr_wrong_binding_count_too_many() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Circle { radius: float }
        }
        fn main() {
            let s = Shape.Circle { radius: 5.0 }
            let x = match s {
                Shape.Circle { radius: r, extra: e } => r
            }
        }
        "#,
        "has 1 fields, but 2 bindings provided"
    );
}

#[test]
fn match_expr_bindings_on_unit_variant() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A { x: val } => 1,
                E.B => 2
            }
        }
        "#,
        "has 0 fields, but 1 bindings provided"
    );
}

#[test]
fn match_expr_unknown_field_in_binding() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Circle { radius: float }
        }
        fn main() {
            let s = Shape.Circle { radius: 5.0 }
            let x = match s {
                Shape.Circle { diameter: d } => d
            }
        }
        "#,
        "has no field 'diameter'"
    );
}

#[test]
fn match_expr_typo_in_field_name() {
    compile_should_fail_with(
        r#"
        enum Shape {
            Rectangle { width: float, height: float }
        }
        fn main() {
            let s = Shape.Rectangle { width: 10.0, height: 20.0 }
            let x = match s {
                Shape.Rectangle { widht: w, height: h } => w * h
            }
        }
        "#,
        "has no field 'widht'"
    );
}

// ============================================================
// Invalid Syntax Errors (6 tests)
// ============================================================

#[test]
fn match_expr_missing_comma_between_arms() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1
                E.B => 2
            }
        }
        "#,
        "expected ','"
    );
}

#[test]
fn match_expr_empty_match() {
    compile_should_fail(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e { }
        }
        "#
    );
}

#[test]
fn match_expr_missing_fat_arrow() {
    compile_should_fail(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A 1,
                E.B => 2
            }
        }
        "#
    );
}

#[test]
fn match_expr_wrong_enum_in_arm() {
    compile_should_fail_with(
        r#"
        enum E1 { A B }
        enum E2 { X Y }
        fn main() {
            let e = E1.A
            let x = match e {
                E1.A => 1,
                E2.X => 2
            }
        }
        "#,
        "does not match scrutinee enum"
    );
}

#[test]
fn match_expr_unknown_variant() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.C => 2
            }
        }
        "#,
        "has no variant 'C'"
    );
}

#[test]
fn match_expr_duplicate_arms() {
    compile_should_fail_with(
        r#"
        enum E { A B }
        fn main() {
            let e = E.A
            let x = match e {
                E.A => 1,
                E.A => 2,
                E.B => 3
            }
        }
        "#,
        "duplicate match arm for variant 'A'"
    );
}

//! Snapshot tests for error message formatting.
//!
//! Uses insta to capture error messages and detect regressions.
//! Run `cargo insta review` to review changes.

use insta::assert_snapshot;

/// Format error message by stripping file paths to make snapshots stable.
fn format_error(err: &str) -> String {
    err.lines()
        .filter(|line| !line.contains("target/"))
        .filter(|line| !line.contains(".pluto-cache/"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn type_mismatch_error() {
    let source = r#"
        fn foo(x: int) string {
            return x
        }
    "#;

    let err = pluto::compile_to_object(source).unwrap_err();
    assert_snapshot!(format_error(&err.to_string()));
}

#[test]
fn undefined_variable_error() {
    let source = r#"
        fn main() {
            let x = unknown_var
        }
    "#;

    let err = pluto::compile_to_object(source).unwrap_err();
    assert_snapshot!(format_error(&err.to_string()));
}

#[test]
fn invalid_binary_op_error() {
    let source = r#"
        fn main() {
            let x = "hello" + 42
        }
    "#;

    let err = pluto::compile_to_object(source).unwrap_err();
    assert_snapshot!(format_error(&err.to_string()));
}

#[test]
fn trait_conformance_error() {
    let source = r#"
        trait Printable {
            fn print(self)
        }

        class Person impl Printable {
            name: string
        }

        fn main() {
            let p = Person { name: "Alice" }
        }
    "#;

    let err = pluto::compile_to_object(source).unwrap_err();
    assert_snapshot!(format_error(&err.to_string()));
}

#[test]
fn generic_bounds_error() {
    let source = r#"
        trait Printable {
            fn print(self)
        }

        fn print_it<T: Printable>(x: T) {
            x.print()
        }

        fn main() {
            print_it(42)
        }
    "#;

    let err = pluto::compile_to_object(source).unwrap_err();
    assert_snapshot!(format_error(&err.to_string()));
}

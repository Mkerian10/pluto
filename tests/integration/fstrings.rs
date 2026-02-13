mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ===== Positive Tests =====

#[test]
fn fstring_basic() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let name = "Alice"
            print(f"Hello {name}")
        }"#,
    );
    assert_eq!(output.trim(), "Hello Alice");
}

#[test]
fn fstring_integer() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let x = 42
            print(f"Number: {x}")
        }"#,
    );
    assert_eq!(output.trim(), "Number: 42");
}

#[test]
fn fstring_expression() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let x = 10
            print(f"Result: {x + 5}")
        }"#,
    );
    assert_eq!(output.trim(), "Result: 15");
}

#[test]
fn fstring_brace_escaping() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let x = 42
            print(f"{{{x}}}")
        }"#,
    );
    assert_eq!(output.trim(), "{42}");
}

#[test]
fn regular_string_interpolation_still_works() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let name = "World"
            print("Hello {name}")
        }"#,
    );
    assert_eq!(output.trim(), "Hello World");
}

// ===== Error Tests =====

#[test]
fn unterminated_interpolation_at_end() {
    compile_should_fail_with(
        r#"fn main() {
            let x = 42
            let result = f"Value: {x"
        }"#,
        "unterminated interpolation expression",
    );
}

#[test]
fn unterminated_interpolation_in_middle() {
    compile_should_fail_with(
        r#"fn main() {
            let x = 42
            let result = f"Start {x end"
        }"#,
        "unterminated interpolation expression",
    );
}

#[test]
fn single_opening_brace() {
    compile_should_fail_with(
        r#"fn main() {
            let result = f"Just {"
        }"#,
        "unterminated interpolation expression",
    );
}

#[test]
fn empty_interpolation_braces() {
    compile_should_fail_with(
        r#"fn main() {
            let result = f"Empty {}"
        }"#,
        "unexpected end of file in expression",
    );
}

#[test]
fn whitespace_only_interpolation() {
    compile_should_fail_with(
        r#"fn main() {
            let result = f"Spaces {   }"
        }"#,
        "unexpected end of file in expression",
    );
}

#[test]
fn incomplete_expression_in_interpolation() {
    compile_should_fail_with(
        r#"fn main() {
            let x = 42
            let result = f"Bad {x +}"
        }"#,
        "unexpected end of file in expression",
    );
}

#[test]
fn unclosed_parenthesis_in_interpolation() {
    compile_should_fail_with(
        r#"fn main() {
            let x = 10
            let result = f"Parens {(x + 5}"
        }"#,
        "expected ), found end of file",
    );
}

#[test]
fn undefined_variable_in_interpolation() {
    compile_should_fail_with(
        r#"fn main() {
            let result = f"Unknown {undefined_var}"
        }"#,
        "undefined variable 'undefined_var'",
    );
}

#[test]
fn multiple_interpolations_one_unterminated() {
    compile_should_fail_with(
        r#"fn main() {
            let a = 1
            let b = 2
            let result = f"{a} and {b"
        }"#,
        "unterminated interpolation expression",
    );
}

#[test]
fn regular_string_unterminated_interpolation() {
    compile_should_fail_with(
        r#"fn main() {
            let x = 42
            let result = "Value: {x"
        }"#,
        "unterminated interpolation expression",
    );
}

// Note: {{{x} is actually valid - {{ escapes to { and then {x} is interpolation
// Result would be "{42"
// This is not an error case, so we removed this test

// ===== Escape Sequence Tests =====

#[test]
fn fstring_hex_escape() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            print(f"\x48ello")
        }"#,
    );
    assert_eq!(output.trim(), "Hello");
}

#[test]
fn fstring_unicode_escape() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            print(f"\u{1F680}")
        }"#,
    );
    assert_eq!(output.trim(), "\u{1F680}");
}

#[test]
fn fstring_unicode_escape_with_interpolation() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let name = "World"
            print(f"\u{41}{name}")
        }"#,
    );
    assert_eq!(output.trim(), "AWorld");
}

#[test]
fn fstring_null_escape() {
    let output = compile_and_run_stdout(
        r#"fn main() {
            let s = f"a\0b"
            print(s.len())
        }"#,
    );
    assert_eq!(output.trim(), "3");
}

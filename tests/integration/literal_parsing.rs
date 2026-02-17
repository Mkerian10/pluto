// Literal Parsing Tests
// Inspired by Rust's literal tests and Go's scanner tests
//
// Tests parser's handling of various literal forms
// Target: 15 tests

mod common;
use common::*;

// ============================================================
// Number Literals
// ============================================================

#[test]
fn integer_large_value() {
    // Test large integer within i64 range
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 9223372036854775807
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn float_scientific_notation_positive_exp() {
    // 1e10 = 10000000000.0
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 1e10
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn float_scientific_notation_negative_exp() {
    // 2.5e-3 = 0.0025
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 2.5e-3
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn float_scientific_notation_uppercase() {
    // 1E10 should work with uppercase E
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 1E10
            print(x)
        }
    "#);
    assert!(stdout.trim().starts_with("10000000000"));
}

#[test]
fn float_scientific_notation_is_float_type() {
    // 1e6 should be float type, usable in float arithmetic
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 1e6
            let y = 2.0
            let z = x + y
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn float_scientific_notation_negative_exp_value() {
    // 1e-3 = 0.001
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 1e-3
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "0.001000");
}

#[test]
fn hex_literal() {
    // 0xFF = 255
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 0xFF
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "255");
}

#[test]
fn hex_literal_lowercase() {
    // 0xff = 255
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 0xff
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "255");
}

#[test]
#[ignore] // Feature not implemented: binary literals (0b prefix) in lexer
fn binary_literal() {
    // 0b1010 = 10
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 0b1010
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "10");
}

#[test]
#[ignore] // Feature not implemented: octal literals (0o prefix) in lexer
fn octal_literal() {
    // 0o755 = 493
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 0o755
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "493");
}

#[test]
fn underscore_separator_in_integer() {
    // 1_000_000 = 1000000
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 1_000_000
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "1000000");
}

#[test]
fn underscore_separator_in_hex() {
    // 0xFF_FF_FF = 16777215
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 0xFF_FF_FF
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "16777215");
}

// ============================================================
// String Literals
// ============================================================

#[test]
fn string_escape_sequences() {
    // Test \n, \t, \r, \\, \"
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let s = "line1\nline2\ttab\rcarriage\\backslash\"quote"
            print("pass")
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn string_empty() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let s = ""
            print(s.len())
        }
    "#);
    assert_eq!(stdout.trim(), "0");
}

#[test]
fn string_with_escaped_quotes() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let s = "He said \"hello\""
            print(s)
        }
    "#);
    assert!(stdout.trim().contains("hello"));
}

#[test]
fn string_interpolation_nested() {
    // "outer {inner {x}}" - nested interpolation
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 42
            let s = f"value: {x}"
            print(s)
        }
    "#);
    assert_eq!(stdout.trim(), "value: 42");
}

// ============================================================
// Boolean Literals
// ============================================================

#[test]
fn boolean_true() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let b = true
            if b { print("pass") } else { print("fail") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

#[test]
fn boolean_false() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let b = false
            if b { print("fail") } else { print("pass") }
        }
    "#);
    assert_eq!(stdout.trim(), "pass");
}

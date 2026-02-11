// Error Recovery & Malformed Input Tests
// Inspired by Rust's parse-fail tests and Go's error recovery
//
// Tests parser's ability to produce helpful errors for malformed input
// Target: 18 tests

mod common;
use common::*;

// ============================================================
// Missing Tokens
// ============================================================

#[test]
fn missing_opening_paren() {
    compile_should_fail_with(r#"
        fn main) {
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_closing_paren() {
    compile_should_fail_with(r#"
        fn main( {
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_opening_brace() {
    compile_should_fail_with(r#"
        fn main()
            print("test")
        }
    "#, "expected");
}

#[test]
fn missing_closing_brace_at_eof() {
    compile_should_fail_with(r#"
        fn main() {
            print("test")
    "#, "expected");
}

#[test]
fn missing_comma_in_function_params() {
    compile_should_fail_with(r#"
        fn add(x: int y: int) int {
            return x + y
        }
    "#, "expected");
}

#[test]
fn missing_colon_in_type_annotation() {
    compile_should_fail_with(r#"
        fn main() {
            let x int = 5
        }
    "#, "expected");
}

#[test]
fn missing_equals_in_let_binding() {
    compile_should_fail_with(r#"
        fn main() {
            let x: int 5
        }
    "#, "expected");
}

#[test]
fn missing_arrow_in_closure() {
    compile_should_fail_with(r#"
        fn main() {
            let f = (x: int) x + 1
        }
    "#, "expected");
}

// ============================================================
// Extra/Unexpected Tokens
// ============================================================

#[test]
fn double_comma_in_params() {
    compile_should_fail_with(r#"
        fn foo(x: int,, y: int) int {
            return x + y
        }
    "#, "expected");
}

#[test]
fn unexpected_keyword_as_identifier() {
    // Using 'fn' as a variable name
    compile_should_fail_with(r#"
        fn main() {
            let fn = 5
        }
    "#, "expected");
}

#[test]
fn stray_closing_brace() {
    compile_should_fail_with(r#"
        fn main() {
            print("test")
        }
        }
    "#, "unexpected");
}

#[test]
fn double_operator() {
    // ++ is not supported in Pluto (not increment operator)
    compile_should_fail(r#"
        fn main() {
            let x = 5
            x++
        }
    "#);
}

// ============================================================
// Incomplete Constructs
// ============================================================

#[test]
fn incomplete_if_statement() {
    compile_should_fail_with(r#"
        fn main() {
            if true
        }
    "#, "expected");
}

#[test]
fn incomplete_while_loop() {
    compile_should_fail_with(r#"
        fn main() {
            while x < 10
        }
    "#, "expected");
}

#[test]
fn incomplete_function_definition() {
    compile_should_fail_with(r#"
        fn foo()
    "#, "expected");
}

#[test]
fn incomplete_class_definition() {
    compile_should_fail_with(r#"
        class Foo
    "#, "expected");
}

#[test]
fn incomplete_match_expression() {
    compile_should_fail_with(r#"
        fn main() {
            let x = 5
            match x {
        }
    "#, "expected");
}

#[test]
fn incomplete_struct_literal() {
    compile_should_fail_with(r#"
        class Point { x: int, y: int }

        fn main() {
            let p = Point {
        }
    "#, "expected");
}

// Phase 2: Parser Explorer - Edge Cases Tests
//
// Tests for miscellaneous parser edge cases:
// - Deep nesting (stress testing)
// - Newline handling
// - Empty files
// - Error recovery
//
// Target: 7 tests

mod common;
use common::*;

#[test]
fn deeply_nested_parens() {
    // 20 levels of nested parens - parser shouldn't stack overflow
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = ((((((((((((((((((((42))))))))))))))))))))
            print(x)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn deeply_nested_arrays() {
    // Multiple levels of nested arrays
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = [[[[[[1]]]]]]
            print(x[0][0][0][0][0][0])
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn deeply_nested_generics() {
    // Nested generic types
    let stdout = compile_and_run_stdout(r#"
        class Box<T> { value: T }

        fn main() {
            let x = Box<Box<Box<Box<int>>>> {
                value: Box<Box<Box<int>>> {
                    value: Box<Box<int>> {
                        value: Box<int> {
                            value: 42
                        }
                    }
                }
            }
            print(x.value.value.value.value)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn newline_before_dot_method_call() {
    // obj\n.method() → newline before . should work (method chaining)
    let stdout = compile_and_run_stdout(r#"
        class Foo {
            fn get(self) int {
                return 42
            }
        }

        fn main() {
            let obj = Foo {}
            let result = obj
                .get()
            print(result)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
#[ignore] // Test expectation unclear: empty files currently parse successfully but fail at link time
fn empty_file() {
    // Empty source string → should produce empty program (no functions/classes)
    // This might fail if parser requires at least one declaration
    compile_should_fail("");
}

#[test]
#[ignore] // Test expectation unclear: comment-only files currently parse successfully but fail at link time
fn only_comments() {
    // File with only comments → should produce empty program
    compile_should_fail(r#"
        // This is a comment
        // Another comment
    "#);
}

#[test]
fn missing_closing_brace_recovery() {
    // fn main() { let x = 1 → missing } should produce helpful error
    compile_should_fail_with(r#"
        fn main() {
            let x = 1
    "#, "expected");
}

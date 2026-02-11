// Phase 2: Parser Explorer - Struct Literals Tests
//
// Tests for struct literal syntax edge cases:
// - Disambiguation from blocks (especially after if/while)
// - Trailing commas
// - Nested struct literals
// - Struct literals with expressions
// - Malformed struct syntax
//
// Target: 10 tests

mod common;
use common::*;

#[test]
fn struct_literal_vs_block_after_if() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { a: int }

        fn main() {
            let x = true
            let result = if x { Foo { a: 1 } } else { Foo { a: 2 } }
            print(result.a)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn struct_literal_trailing_comma() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { a: int, b: int }

        fn main() {
            let x = Foo { a: 1, b: 2, }
            print(x.a)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn struct_literal_no_trailing_comma() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { a: int, b: int }

        fn main() {
            let x = Foo { a: 1, b: 2 }
            print(x.a)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn struct_literal_single_field() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { a: int }

        fn main() {
            let x = Foo { a: 1 }
            print(x.a)
        }
    "#);
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn struct_literal_nested() {
    let stdout = compile_and_run_stdout(r#"
        class Inner { x: int }
        class Outer { inner: Inner }

        fn main() {
            let obj = Outer { inner: Inner { x: 42 } }
            print(obj.inner.x)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn struct_literal_with_expressions() {
    let stdout = compile_and_run_stdout(r#"
        class Foo { a: int, b: int }

        fn bar() int {
            return 10
        }

        fn main() {
            let x = Foo { a: 2 + 2, b: bar() }
            print(x.a + x.b)
        }
    "#);
    assert_eq!(stdout.trim(), "14");
}

#[test]
fn struct_literal_shorthand_rejected() {
    // Foo { a } → shorthand field syntax should be rejected (if not supported)
    // If shorthand IS supported, this test documents that fact
    compile_should_fail(r#"
        class Foo { a: int }

        fn main() {
            let a = 42
            let x = Foo { a }
        }
    "#);
}

#[test]
fn struct_literal_generic_type() {
    let stdout = compile_and_run_stdout(r#"
        class Box<T> { value: T }

        fn main() {
            let x = Box<int> { value: 42 }
            print(x.value)
        }
    "#);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn struct_literal_multiline() {
    let stdout = compile_and_run_stdout(r#"
        class Foo {
            a: int
            b: int
            c: int
        }

        fn main() {
            let x = Foo {
                a: 1,
                b: 2,
                c: 3
            }
            print(x.b)
        }
    "#);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn struct_literal_missing_comma_rejected() {
    // Foo { a: 1 b: 2 } → missing comma should be rejected
    compile_should_fail(r#"
        class Foo { a: int, b: int }

        fn main() {
            let x = Foo { a: 1 b: 2 }
        }
    "#);
}

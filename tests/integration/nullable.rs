mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

// ── Positive tests ─────────────────────────────────────────────────────

#[test]
fn none_assigned_to_nullable_int() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let x: int? = none
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn value_assigned_to_nullable_int() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let x: int? = 42
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn return_none_from_function() {
    let out = compile_and_run_stdout(r#"
fn maybe_int() int? {
    return none
}

fn main() {
    let x = maybe_int()
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn return_value_from_nullable_function() {
    let out = compile_and_run_stdout(r#"
fn maybe_int() int? {
    return 42
}

fn unwrap(x: int?) int {
    return x?
}

fn main() int? {
    let x = maybe_int()
    let y = unwrap(x)
    print(y)
    return none
}
"#);
    assert_eq!(out.trim(), "42");
}

#[test]
fn question_mark_propagation() {
    let out = compile_and_run_stdout(r#"
fn get_value() int? {
    return 10
}

fn double_it() int? {
    let x = get_value()?
    return x * 2
}

fn main() {
    let result = double_it()
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn question_mark_propagates_none() {
    let out = compile_and_run_stdout(r#"
fn get_nothing() int? {
    return none
}

fn try_use() int? {
    let x = get_nothing()?
    print(999)
    return x * 2
}

fn main() {
    let result = try_use()
    print(0)
}
"#);
    // Should NOT print 999 because ? propagates none early
    assert_eq!(out.trim(), "0");
}

#[test]
fn nullable_string() {
    let out = compile_and_run_stdout(r#"
fn maybe_str() string? {
    return "hello"
}

fn use_it() string? {
    let s = maybe_str()?
    return s
}

fn main() string? {
    let s = use_it()?
    print(s)
    return none
}
"#);
    assert_eq!(out.trim(), "hello");
}

#[test]
fn nullable_string_none() {
    let out = compile_and_run_stdout(r#"
fn maybe_str() string? {
    return none
}

fn use_it() string? {
    let s = maybe_str()?
    return s
}

fn main() {
    let result = use_it()
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn nullable_float() {
    let out = compile_and_run_stdout(r#"
fn maybe_float() float? {
    return 3.14
}

fn use_it() float? {
    let f = maybe_float()?
    return f
}

fn main() float? {
    let f = use_it()?
    print(f)
    return none
}
"#);
    assert_eq!(out.trim(), "3.140000");
}

#[test]
fn nullable_bool() {
    let out = compile_and_run_stdout(r#"
fn maybe_bool() bool? {
    return true
}

fn use_it() bool? {
    let b = maybe_bool()?
    return b
}

fn main() bool? {
    let b = use_it()?
    print(b)
    return none
}
"#);
    assert_eq!(out.trim(), "true");
}

#[test]
fn nullable_class() {
    let out = compile_and_run_stdout(r#"
class Point {
    x: int
    y: int
}

fn maybe_point() Point? {
    return Point { x: 1, y: 2 }
}

fn use_it() int? {
    let p = maybe_point()?
    return p.x + p.y
}

fn main() int? {
    let val = use_it()?
    print(val)
    return none
}
"#);
    assert_eq!(out.trim(), "3");
}

#[test]
fn nullable_class_none() {
    let out = compile_and_run_stdout(r#"
class Point {
    x: int
    y: int
}

fn maybe_point() Point? {
    return none
}

fn use_it() int? {
    let p = maybe_point()?
    return p.x + p.y
}

fn main() {
    let result = use_it()
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn nullable_parameter() {
    let out = compile_and_run_stdout(r#"
fn unwrap_it(x: int?) int? {
    let val = x?
    return val + 1
}

fn main() int? {
    let a: int? = 10
    let b = unwrap_it(a)?
    print(b)
    return none
}
"#);
    assert_eq!(out.trim(), "11");
}

#[test]
fn nullable_with_conditional_return() {
    let out = compile_and_run_stdout(r#"
fn find_positive(x: int) int? {
    if x > 0 {
        return x
    }
    return none
}

fn main() {
    let a = find_positive(5)
    let b = find_positive(-3)
    print(0)
}
"#);
    assert_eq!(out.trim(), "0");
}

#[test]
fn chained_nullable_calls() {
    let out = compile_and_run_stdout(r#"
fn step1() int? {
    return 10
}

fn step2(x: int) int? {
    return x + 5
}

fn pipeline() int? {
    let a = step1()?
    let b = step2(a)?
    return b
}

fn main() int? {
    let result = pipeline()?
    print(result)
    return none
}
"#);
    assert_eq!(out.trim(), "15");
}

#[test]
fn chained_nullable_none_short_circuits() {
    let out = compile_and_run_stdout(r#"
fn step1() int? {
    return none
}

fn step2(x: int) int? {
    print(999)
    return x + 5
}

fn pipeline() int? {
    let a = step1()?
    let b = step2(a)?
    return b
}

fn main() {
    let result = pipeline()
    print(0)
}
"#);
    // step2 should never be called, so 999 should not print
    assert_eq!(out.trim(), "0");
}

// ── Negative tests (compile failures) ──────────────────────────────────

#[test]
fn error_nested_nullable() {
    // Parser sees int? as a type, then second ? triggers a parse error
    compile_should_fail_with(r#"
fn main() {
    let x: int?? = none
}
"#, "expected =, found ?");
}

#[test]
fn error_void_nullable() {
    compile_should_fail_with(r#"
fn foo() void? {
    return none
}

fn main() {
    foo()
}
"#, "void");
}

#[test]
fn error_question_on_non_nullable() {
    compile_should_fail_with(r#"
fn get_int() int {
    return 42
}

fn main() int? {
    let x = get_int()?
    return none
}
"#, "non-nullable");
}

#[test]
fn question_mark_in_void_function() {
    // ? in a void function acts as a guard: bail early if null
    let out = compile_and_run_stdout(r#"
fn process(line: string?) {
    let value = line?
    print(value)
}

fn main() {
    process("hello")
    process(none)
    print("done")
}
"#);
    assert_eq!(out.trim(), "hello\ndone");
}

#[test]
fn question_mark_in_void_method() {
    let out = compile_and_run_stdout(r#"
class Processor {
    tag: string

    fn process(self, line: string?) {
        let value = line?
        print("{self.tag}: {value}")
    }
}

fn main() {
    let p = Processor { tag: "P" }
    p.process("hello")
    p.process(none)
    print("done")
}
"#);
    assert_eq!(out.trim(), "P: hello\ndone");
}

// ============================================================
// If-Expression Integration Tests
// ============================================================

#[test]
fn if_expr_nullable_widening_t_to_t_nullable() {
    // int widened to int? when assigned to nullable variable
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let x: int? = if true { 10 } else { 5 }
            print(x?)
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn if_expr_none_literal_in_branches() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let x: int? = if false { none } else { none }
            if x == none {
                print("none")
            } else {
                print("some")
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "none");
}

#[test]
fn if_expr_null_propagate_in_branch() {
    let out = compile_and_run_stdout(
        r#"
        fn main() {
            let opt: int? = 10
            let x = if true { opt? } else { 0 }
            print(x)
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn if_expr_returning_nullable() {
    let out = compile_and_run_stdout(
        r#"
        fn maybe_value(flag: bool) int? {
            if flag {
                return 42
            } else {
                return none
            }
        }
        fn main() {
            let x = maybe_value(true)
            print(x?)
        }
        "#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn if_expr_nullable_class() {
    let out = compile_and_run_stdout(
        r#"
        class Foo { x: int }
        fn main() {
            let f: Foo? = if true { Foo { x: 10 } } else { Foo { x: 20 } }
            if f != none {
                print(f?.x)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}


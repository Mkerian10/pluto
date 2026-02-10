mod common;
use common::{compile_and_run, compile_batch_stdout, compile_should_fail, compile_should_fail_with, plutoc};
use std::collections::HashMap;
use std::sync::OnceLock;

// ============================================================
// Batched stdout tests — compiled + run as a single binary
// ============================================================

static BATCH: OnceLock<HashMap<String, String>> = OnceLock::new();

fn batch() -> &'static HashMap<String, String> {
    BATCH.get_or_init(|| {
        compile_batch_stdout(&[
            (
                "print_int",
                "fn main() {\n    print(42)\n}",
            ),
            (
                "print_int_expression",
                "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    print(add(1, 2))\n}",
            ),
            (
                "print_float",
                "fn main() {\n    print(3.14)\n}",
            ),
            (
                "print_bool",
                "fn main() {\n    print(true)\n    print(false)\n}",
            ),
            (
                "print_string",
                "fn main() {\n    print(\"hello world\")\n}",
            ),
            (
                "print_multiple",
                "fn main() {\n    print(1)\n    print(2)\n    print(3)\n}",
            ),
            (
                "print_in_loop",
                "fn main() {\n    let i = 0\n    while i < 3 {\n        print(i)\n        i = i + 1\n    }\n}",
            ),
            (
                "void_return",
                "fn early(x: int) {\n    if x > 0 {\n        print(1)\n        return\n    }\n    print(2)\n}\n\nfn main() {\n    early(5)\n    early(-1)\n}",
            ),
            (
                "multiple_return_paths",
                "fn classify(x: int) string {\n    if x > 0 {\n        return \"positive\"\n    }\n    if x < 0 {\n        return \"negative\"\n    }\n    return \"zero\"\n}\n\nfn main() {\n    print(classify(5))\n    print(classify(-3))\n    print(classify(0))\n}",
            ),
            (
                "comments_ignored",
                "// this is a comment\nfn main() {\n    // another comment\n    let x = 42 // inline comment\n    print(x)\n}",
            ),
            (
                "parenthesized_expressions",
                "fn main() {\n    print((2 + 3) * 4)\n    print(2 + 3 * 4)\n}",
            ),
            (
                "recursive_function",
                "fn factorial(n: int) int {\n    if n <= 1 {\n        return 1\n    }\n    return n * factorial(n - 1)\n}\n\nfn main() {\n    print(factorial(5))\n}",
            ),
            (
                "variable_reassignment",
                "fn main() {\n    let x = 1\n    print(x)\n    x = 2\n    print(x)\n    x = x + 10\n    print(x)\n}",
            ),
            (
                "extern_fn_call_print_int",
                "extern fn __pluto_print_int(value: int)\n\nfn main() {\n    __pluto_print_int(42)\n}",
            ),
            (
                "extern_fn_with_return",
                "extern fn __pluto_string_len(s: string) int\n\nfn main() {\n    let s = \"hello\"\n    let n = __pluto_string_len(s)\n    print(n)\n}",
            ),
            (
                "time_ns_returns_positive",
                "fn main() {\n    let t = time_ns()\n    if t > 0 {\n        print(\"ok\")\n    }\n}",
            ),
            (
                "time_ns_elapsed",
                "fn main() {\n    let start = time_ns()\n    let i = 0\n    while i < 1000000 {\n        i = i + 1\n    }\n    let elapsed = time_ns() - start\n    if elapsed > 0 {\n        print(\"ok\")\n    }\n}",
            ),
            (
                "underscore_int_literal",
                "fn main() {\n    let x = 1_000_000\n    print(x)\n}",
            ),
            (
                "underscore_int_literal_small",
                "fn main() {\n    let x = 1_0\n    print(x)\n}",
            ),
            (
                "underscore_float_literal",
                "fn main() {\n    let x = 1_000.50\n    print(x)\n}",
            ),
            (
                "underscore_float_both_sides",
                "fn main() {\n    let x = 1_000.000_5\n    print(x)\n}",
            ),
            (
                "underscore_arithmetic",
                "fn main() {\n    let x = 1_000 + 2_000\n    print(x)\n}",
            ),
            (
                "multiline_function_call",
                "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        1,\n        2\n    )\n    print(x)\n}",
            ),
            (
                "multiline_function_call_trailing_comma",
                "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        1,\n        2,\n    )\n    print(x)\n}",
            ),
            (
                "multiline_method_call",
                "class Calc {\n    val: int\n\n    fn add(self, x: int) int {\n        return self.val + x\n    }\n}\nfn main() {\n    let c = Calc { val: 10 }\n    let r = c.add(\n        5\n    )\n    print(r)\n}",
            ),
            (
                "multiline_array_literal",
                "fn main() {\n    let arr = [\n        1,\n        2,\n        3\n    ]\n    print(arr.len())\n}",
            ),
            (
                "multiline_nested_call",
                "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        add(\n            1,\n            2\n        ),\n        add(\n            3,\n            4\n        )\n    )\n    print(x)\n}",
            ),
        ])
    })
}

#[test]
fn print_int() {
    assert_eq!(batch()["print_int"], "42\n");
}

#[test]
fn print_int_expression() {
    assert_eq!(batch()["print_int_expression"], "3\n");
}

#[test]
fn print_float() {
    assert_eq!(batch()["print_float"], "3.140000\n");
}

#[test]
fn print_bool() {
    assert_eq!(batch()["print_bool"], "true\nfalse\n");
}

#[test]
fn print_string() {
    assert_eq!(batch()["print_string"], "hello world\n");
}

#[test]
fn print_multiple() {
    assert_eq!(batch()["print_multiple"], "1\n2\n3\n");
}

#[test]
fn print_in_loop() {
    assert_eq!(batch()["print_in_loop"], "0\n1\n2\n");
}

#[test]
fn void_return() {
    assert_eq!(batch()["void_return"], "1\n2\n");
}

#[test]
fn multiple_return_paths() {
    assert_eq!(batch()["multiple_return_paths"], "positive\nnegative\nzero\n");
}

#[test]
fn comments_ignored() {
    assert_eq!(batch()["comments_ignored"], "42\n");
}

#[test]
fn parenthesized_expressions() {
    assert_eq!(batch()["parenthesized_expressions"], "20\n14\n");
}

#[test]
fn recursive_function() {
    assert_eq!(batch()["recursive_function"], "120\n");
}

#[test]
fn variable_reassignment() {
    assert_eq!(batch()["variable_reassignment"], "1\n2\n12\n");
}

#[test]
fn extern_fn_call_print_int() {
    assert_eq!(batch()["extern_fn_call_print_int"], "42\n");
}

#[test]
fn extern_fn_with_return() {
    assert_eq!(batch()["extern_fn_with_return"], "5\n");
}

#[test]
fn time_ns_returns_positive() {
    assert_eq!(batch()["time_ns_returns_positive"], "ok\n");
}

#[test]
fn time_ns_elapsed() {
    assert_eq!(batch()["time_ns_elapsed"], "ok\n");
}

#[test]
fn underscore_int_literal() {
    assert_eq!(batch()["underscore_int_literal"], "1000000\n");
}

#[test]
fn underscore_int_literal_small() {
    assert_eq!(batch()["underscore_int_literal_small"], "10\n");
}

#[test]
fn underscore_float_literal() {
    assert_eq!(batch()["underscore_float_literal"], "1000.500000\n");
}

#[test]
fn underscore_float_both_sides() {
    assert_eq!(batch()["underscore_float_both_sides"], "1000.000500\n");
}

#[test]
fn underscore_arithmetic() {
    assert_eq!(batch()["underscore_arithmetic"], "3000\n");
}

#[test]
fn multiline_function_call() {
    assert_eq!(batch()["multiline_function_call"], "3\n");
}

#[test]
fn multiline_function_call_trailing_comma() {
    assert_eq!(batch()["multiline_function_call_trailing_comma"], "3\n");
}

#[test]
fn multiline_method_call() {
    assert_eq!(batch()["multiline_method_call"], "15\n");
}

#[test]
fn multiline_array_literal() {
    assert_eq!(batch()["multiline_array_literal"], "3\n");
}

#[test]
fn multiline_nested_call() {
    assert_eq!(batch()["multiline_nested_call"], "10\n");
}

// ============================================================
// Non-batched tests — exit code checks
// ============================================================

#[test]
fn empty_main() {
    let code = compile_and_run("fn main() { }");
    assert_eq!(code, 0);
}

#[test]
fn main_with_let() {
    let code = compile_and_run("fn main() {\n    let x = 42\n}");
    assert_eq!(code, 0);
}

#[test]
fn function_call() {
    let code = compile_and_run(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1, 2)\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn nested_function_calls() {
    let code = compile_and_run(
        "fn double(x: int) int {\n    return x * 2\n}\n\nfn add_one(x: int) int {\n    return x + 1\n}\n\nfn main() {\n    let result = add_one(double(5))\n}",
    );
    assert_eq!(code, 0);
}

// ============================================================
// Non-batched tests — compile-fail checks
// ============================================================

#[test]
fn type_error_rejected() {
    compile_should_fail("fn main() {\n    let x: int = true\n}");
}

#[test]
fn undefined_variable_rejected() {
    compile_should_fail("fn main() {\n    let x = y\n}");
}

#[test]
fn undefined_function_rejected() {
    compile_should_fail("fn main() {\n    let x = foo(1)\n}");
}

#[test]
fn print_wrong_arg_count() {
    compile_should_fail("fn main() {\n    print(1, 2)\n}");
}

#[test]
fn print_no_args() {
    compile_should_fail("fn main() {\n    print()\n}");
}

#[test]
fn wrong_arg_count_rejected() {
    compile_should_fail(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    let x = add(1)\n}",
    );
}

#[test]
fn return_type_mismatch_rejected() {
    compile_should_fail(
        "fn foo() int {\n    return true\n}\n\nfn main() {\n    foo()\n}",
    );
}

#[test]
fn arg_type_mismatch_rejected() {
    compile_should_fail(
        "fn foo(x: int) int {\n    return x\n}\n\nfn main() {\n    foo(\"hello\")\n}",
    );
}

#[test]
fn assign_type_mismatch_rejected() {
    compile_should_fail(
        "fn main() {\n    let x = 42\n    x = true\n}",
    );
}

#[test]
fn extern_fn_class_param_rejected() {
    compile_should_fail(
        "class Foo {\n    x: int\n}\n\nextern fn bad(f: Foo)\n\nfn main() {\n}",
    );
}

#[test]
fn extern_fn_duplicate_name_rejected() {
    compile_should_fail(
        "extern fn foo(x: int)\n\nfn foo(x: int) {\n}\n\nfn main() {\n}",
    );
}

// ============================================================
// CLI smoke tests — exercise the actual plutoc binary subprocess
// ============================================================

#[test]
fn cli_compile_and_run() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.pluto");
    let bin = dir.path().join("test_bin");
    std::fs::write(&src, "fn main() {\n    print(42)\n}").unwrap();
    let output = plutoc().arg("compile").arg(&src).arg("-o").arg(&bin).output().unwrap();
    assert!(output.status.success(), "CLI compile failed: {}", String::from_utf8_lossy(&output.stderr));
    let run_output = std::process::Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&run_output.stdout), "42\n");
}

#[test]
fn cli_compile_error_formatting() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.pluto");
    let bin = dir.path().join("test_bin");
    std::fs::write(&src, "fn main() {\n    let x: int = \"hello\"\n}").unwrap();
    let output = plutoc().arg("compile").arg(&src).arg("-o").arg(&bin).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"), "Expected CLI error format, got: {}", stderr);
}

#[test]
fn cli_run_subcommand() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.pluto");
    std::fs::write(&src, "fn main() {\n    print(99)\n}").unwrap();
    let output = plutoc().arg("run").arg(&src).output().unwrap();
    assert!(output.status.success(), "CLI run failed: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "99\n");
}

// ============================================================
// Robustness: duplicate/overflow/malformed input rejection
// ============================================================

#[test]
fn duplicate_function_name_rejected() {
    compile_should_fail_with(
        "fn foo() int {\n    return 1\n}\n\nfn foo() int {\n    return 2\n}\n\nfn main() {\n    print(foo())\n}",
        "already defined",
    );
}

#[test]
fn integer_overflow_literal_error() {
    compile_should_fail_with(
        "fn main() {\n    let x = 99999999999999999999\n}",
        "out of range",
    );
}

#[test]
fn empty_string_interpolation_error() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello {}\"\n}",
        "empty expression in string interpolation",
    );
}

#[test]
fn deep_nesting_rejected() {
    let mut src = String::from("fn main() {\n    let x = ");
    for _ in 0..60 {
        src.push('(');
    }
    src.push('1');
    for _ in 0..60 {
        src.push(')');
    }
    src.push_str("\n}");
    compile_should_fail_with(&src, "nesting too deep");
}

mod common;
use common::{compile_and_run, compile_and_run_stdout, compile_should_fail, plutoc};

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
fn print_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(42)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn print_int_expression() {
    let out = compile_and_run_stdout(
        "fn add(a: int, b: int) int {\n    return a + b\n}\n\nfn main() {\n    print(add(1, 2))\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn print_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(3.14)\n}");
    assert_eq!(out, "3.140000\n");
}

#[test]
fn print_bool() {
    let out = compile_and_run_stdout("fn main() {\n    print(true)\n    print(false)\n}");
    assert_eq!(out, "true\nfalse\n");
}

#[test]
fn print_string() {
    let out = compile_and_run_stdout("fn main() {\n    print(\"hello world\")\n}");
    assert_eq!(out, "hello world\n");
}

#[test]
fn print_multiple() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(1)\n    print(2)\n    print(3)\n}",
    );
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn print_in_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while i < 3 {\n        print(i)\n        i = i + 1\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n");
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
fn void_return() {
    let out = compile_and_run_stdout(
        "fn early(x: int) {\n    if x > 0 {\n        print(1)\n        return\n    }\n    print(2)\n}\n\nfn main() {\n    early(5)\n    early(-1)\n}",
    );
    assert_eq!(out, "1\n2\n");
}

#[test]
fn multiple_return_paths() {
    let out = compile_and_run_stdout(
        "fn classify(x: int) string {\n    if x > 0 {\n        return \"positive\"\n    }\n    if x < 0 {\n        return \"negative\"\n    }\n    return \"zero\"\n}\n\nfn main() {\n    print(classify(5))\n    print(classify(-3))\n    print(classify(0))\n}",
    );
    assert_eq!(out, "positive\nnegative\nzero\n");
}

#[test]
fn comments_ignored() {
    let out = compile_and_run_stdout(
        "// this is a comment\nfn main() {\n    // another comment\n    let x = 42 // inline comment\n    print(x)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn parenthesized_expressions() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print((2 + 3) * 4)\n    print(2 + 3 * 4)\n}",
    );
    assert_eq!(out, "20\n14\n");
}

#[test]
fn recursive_function() {
    let out = compile_and_run_stdout(
        "fn factorial(n: int) int {\n    if n <= 1 {\n        return 1\n    }\n    return n * factorial(n - 1)\n}\n\nfn main() {\n    print(factorial(5))\n}",
    );
    assert_eq!(out, "120\n");
}

#[test]
fn variable_reassignment() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1\n    print(x)\n    x = 2\n    print(x)\n    x = x + 10\n    print(x)\n}",
    );
    assert_eq!(out, "1\n2\n12\n");
}

#[test]
fn extern_fn_call_print_int() {
    let out = compile_and_run_stdout(
        "extern fn __pluto_print_int(value: int)\n\nfn main() {\n    __pluto_print_int(42)\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn extern_fn_with_return() {
    let out = compile_and_run_stdout(
        "extern fn __pluto_string_len(s: string) int\n\nfn main() {\n    let s = \"hello\"\n    let n = __pluto_string_len(s)\n    print(n)\n}",
    );
    assert_eq!(out, "5\n");
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

#[test]
fn time_ns_returns_positive() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let t = time_ns()\n    if t > 0 {\n        print(\"ok\")\n    }\n}",
    );
    assert_eq!(out, "ok\n");
}

#[test]
fn time_ns_elapsed() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let start = time_ns()\n    let i = 0\n    while i < 1000000 {\n        i = i + 1\n    }\n    let elapsed = time_ns() - start\n    if elapsed > 0 {\n        print(\"ok\")\n    }\n}",
    );
    assert_eq!(out, "ok\n");
}

// === Underscore in numeric literals ===

#[test]
fn underscore_int_literal() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1_000_000\n    print(x)\n}",
    );
    assert_eq!(out, "1000000\n");
}

#[test]
fn underscore_int_literal_small() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1_0\n    print(x)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn underscore_float_literal() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1_000.50\n    print(x)\n}",
    );
    assert_eq!(out, "1000.500000\n");
}

#[test]
fn underscore_float_both_sides() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1_000.000_5\n    print(x)\n}",
    );
    assert_eq!(out, "1000.000500\n");
}

#[test]
fn underscore_arithmetic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1_000 + 2_000\n    print(x)\n}",
    );
    assert_eq!(out, "3000\n");
}

// === Multi-line function calls ===

#[test]
fn multiline_function_call() {
    let out = compile_and_run_stdout(
        "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        1,\n        2\n    )\n    print(x)\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn multiline_function_call_trailing_comma() {
    let out = compile_and_run_stdout(
        "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        1,\n        2,\n    )\n    print(x)\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn multiline_method_call() {
    let out = compile_and_run_stdout(
        "class Calc {\n    val: int\n\n    fn add(self, x: int) int {\n        return self.val + x\n    }\n}\nfn main() {\n    let c = Calc { val: 10 }\n    let r = c.add(\n        5\n    )\n    print(r)\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn multiline_array_literal() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let arr = [\n        1,\n        2,\n        3\n    ]\n    print(arr.len())\n}",
    );
    assert_eq!(out, "3\n");
}

#[test]
fn multiline_nested_call() {
    let out = compile_and_run_stdout(
        "fn add(a: int, b: int) int {\n    return a + b\n}\nfn main() {\n    let x = add(\n        add(\n            1,\n            2\n        ),\n        add(\n            3,\n            4\n        )\n    )\n    print(x)\n}",
    );
    assert_eq!(out, "10\n");
}

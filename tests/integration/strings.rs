mod common;
use common::{compile_and_run_stdout, compile_should_fail, compile_should_fail_with};

#[test]
fn string_concatenation() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello \" + \"world\"\n    print(s)\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"hello\".len())\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn string_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"foo\" == \"foo\")\n    print(\"foo\" == \"bar\")\n    print(\"foo\" != \"bar\")\n    print(\"foo\" != \"foo\")\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

#[test]
fn string_let_binding_and_print() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"hello world\"\n    print(s)\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_as_function_param() {
    let out = compile_and_run_stdout(
        "fn greet(name: string) string {\n    return \"hello \" + name\n}\n\nfn main() {\n    print(greet(\"world\"))\n}",
    );
    assert_eq!(out, "hello world\n");
}

#[test]
fn string_function_return() {
    let out = compile_and_run_stdout(
        "fn get_msg() string {\n    return \"hi\"\n}\n\nfn main() {\n    print(get_msg())\n}",
    );
    assert_eq!(out, "hi\n");
}

#[test]
fn string_in_struct_field() {
    let out = compile_and_run_stdout(
        "class Person {\n    name: string\n    age: int\n}\n\nfn main() {\n    let p = Person { name: \"alice\", age: 30 }\n    print(p.name)\n    print(p.age)\n}",
    );
    assert_eq!(out, "alice\n30\n");
}

#[test]
fn string_concat_len() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"ab\" + \"cde\"\n    print(s.len())\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn string_empty() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"\".len())\n}",
    );
    assert_eq!(out, "0\n");
}

#[test]
fn string_concat_not_int() {
    compile_should_fail("fn main() {\n    let s = \"hello\" + 42\n}");
}

#[test]
fn string_concat_chain() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let s = \"a\" + \"b\" + \"c\"\n    print(s)\n}",
    );
    assert_eq!(out, "abc\n");
}

// String interpolation

#[test]
fn string_interp_basic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(\"hello {name}\")\n}",
    );
    assert_eq!(out, "hello alice\n");
}

#[test]
fn string_interp_int() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 42\n    print(\"x is {x}\")\n}",
    );
    assert_eq!(out, "x is 42\n");
}

#[test]
fn string_interp_float() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let pi = 3.14\n    print(\"pi is {pi}\")\n}",
    );
    assert_eq!(out, "pi is 3.140000\n");
}

#[test]
fn string_interp_bool() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let flag = true\n    print(\"flag is {flag}\")\n}",
    );
    assert_eq!(out, "flag is true\n");
}

#[test]
fn string_interp_expr() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(\"sum is {a + b}\")\n}",
    );
    assert_eq!(out, "sum is 3\n");
}

#[test]
fn string_interp_multiple() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = 1\n    let b = 2\n    print(\"{a} + {b} = {a + b}\")\n}",
    );
    assert_eq!(out, "1 + 2 = 3\n");
}

#[test]
fn string_interp_no_interp() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"plain string\")\n}",
    );
    assert_eq!(out, "plain string\n");
}

#[test]
fn string_interp_escaped_braces() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(\"use {{braces}}\")\n}",
    );
    assert_eq!(out, "use {braces}\n");
}

#[test]
fn string_interp_concat() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let name = \"alice\"\n    print(\"hi {name}\" + \"!\")\n}",
    );
    assert_eq!(out, "hi alice!\n");
}

#[test]
fn string_interp_class_rejected() {
    compile_should_fail_with(
        "class Foo {\n    x: int\n}\n\nfn main() {\n    let p = Foo { x: 1 }\n    let s = \"value is {p}\"\n}",
        "cannot interpolate",
    );
}

#[test]
fn string_interp_trailing_tokens_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let a = 1\n    let s = \"{a b}\"\n}",
        "unexpected tokens",
    );
}

#[test]
fn string_interp_unterminated_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let name = \"alice\"\n    let s = \"hello {name\"\n}",
        "unterminated",
    );
}

#[test]
fn string_interp_stray_close_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let s = \"hello }\"\n}",
        "unexpected '}'",
    );
}

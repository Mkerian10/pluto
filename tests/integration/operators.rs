mod common;
use common::compile_and_run_stdout;

#[test]
fn arithmetic_operations() {
    let code = common::compile_and_run(
        "fn main() {\n    let a = 10\n    let b = 3\n    let sum = a + b\n    let diff = a - b\n    let prod = a * b\n    let quot = a / b\n    let rem = a % b\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn boolean_operations() {
    let code = common::compile_and_run(
        "fn main() {\n    let a = true\n    let b = false\n    let c = 1 < 2\n    let d = 3 == 3\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn arithmetic_add_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 + 3)\n}");
    assert_eq!(out, "13\n");
}

#[test]
fn arithmetic_sub_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 - 3)\n}");
    assert_eq!(out, "7\n");
}

#[test]
fn arithmetic_mul_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 * 3)\n}");
    assert_eq!(out, "30\n");
}

#[test]
fn arithmetic_div_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 / 3)\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn arithmetic_mod_output() {
    let out = compile_and_run_stdout("fn main() {\n    print(10 % 3)\n}");
    assert_eq!(out, "1\n");
}

#[test]
fn float_arithmetic() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(1.5 + 2.5)\n    print(5.0 - 1.5)\n    print(2.0 * 3.0)\n    print(7.0 / 2.0)\n}",
    );
    assert_eq!(out, "4.000000\n3.500000\n6.000000\n3.500000\n");
}

#[test]
fn comparison_greater_than() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(5 > 3)\n    print(3 > 5)\n    print(3 > 3)\n}",
    );
    assert_eq!(out, "true\nfalse\nfalse\n");
}

#[test]
fn comparison_less_than_eq() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(3 <= 5)\n    print(5 <= 5)\n    print(6 <= 5)\n}",
    );
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn comparison_greater_than_eq() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(5 >= 3)\n    print(5 >= 5)\n    print(4 >= 5)\n}",
    );
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn int_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(42 == 42)\n    print(42 == 43)\n    print(42 != 43)\n    print(42 != 42)\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

#[test]
fn logical_and() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true && true)\n    print(true && false)\n    print(false && true)\n    print(false && false)\n}",
    );
    assert_eq!(out, "true\nfalse\nfalse\nfalse\n");
}

#[test]
fn logical_or() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true || true)\n    print(true || false)\n    print(false || true)\n    print(false || false)\n}",
    );
    assert_eq!(out, "true\ntrue\ntrue\nfalse\n");
}

#[test]
fn unary_negation() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    print(-x)\n    print(-10)\n}",
    );
    assert_eq!(out, "-5\n-10\n");
}

#[test]
fn unary_not() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(!true)\n    print(!false)\n}",
    );
    assert_eq!(out, "false\ntrue\n");
}

#[test]
fn bool_equality() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true == true)\n    print(true == false)\n    print(false != true)\n}",
    );
    assert_eq!(out, "true\nfalse\ntrue\n");
}

mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

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

// ── Bitwise operators ─────────────────────────────────────────────────────────

#[test]
fn bitwise_and() {
    let out = compile_and_run_stdout("fn main() {\n    print(255 & 15)\n}");
    assert_eq!(out, "15\n");
}

#[test]
fn bitwise_or() {
    let out = compile_and_run_stdout("fn main() {\n    print(12 | 10)\n}");
    assert_eq!(out, "14\n");
}

#[test]
fn bitwise_xor() {
    let out = compile_and_run_stdout("fn main() {\n    print(255 ^ 170)\n}");
    assert_eq!(out, "85\n");
}

#[test]
fn bitwise_shl() {
    let out = compile_and_run_stdout("fn main() {\n    print(1 << 4)\n}");
    assert_eq!(out, "16\n");
}

#[test]
fn bitwise_shr() {
    let out = compile_and_run_stdout("fn main() {\n    print(16 >> 2)\n}");
    assert_eq!(out, "4\n");
}

#[test]
fn bitwise_not() {
    // ~0 == -1 in two's complement
    let out = compile_and_run_stdout("fn main() {\n    print(~0)\n}");
    assert_eq!(out, "-1\n");
}

#[test]
fn bitwise_not_value() {
    // ~255 with 64-bit int
    let out = compile_and_run_stdout("fn main() {\n    print(~255)\n}");
    assert_eq!(out, "-256\n");
}

#[test]
fn bitwise_combined() {
    // (12 | 10) & 14 == 14 & 14 == 14
    let out = compile_and_run_stdout("fn main() {\n    print((12 | 10) & 14)\n}");
    assert_eq!(out, "14\n");
}

#[test]
fn bitwise_precedence_or_xor_and() {
    // a & b has higher precedence than a | b and a ^ b
    // 3 | 5 & 6 => 3 | (5 & 6) = 3 | 4 = 7
    let out = compile_and_run_stdout("fn main() {\n    print(3 | 5 & 6)\n}");
    assert_eq!(out, "7\n");
}

#[test]
fn bitwise_shift_precedence() {
    // shift binds tighter than comparison:  1 << 4 > 10  =>  (1 << 4) > 10  =>  16 > 10  =>  true
    let out = compile_and_run_stdout("fn main() {\n    print(1 << 4 > 10)\n}");
    assert_eq!(out, "true\n");
}

#[test]
fn bitwise_double_not() {
    let out = compile_and_run_stdout("fn main() {\n    print(~~42)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn bitwise_not_with_or() {
    let out = compile_and_run_stdout("fn main() {\n    print(~(3 | 4))\n}");
    assert_eq!(out, "-8\n");
}

#[test]
fn bitwise_on_float_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = 1.0 & 2.0\n}",
        "bitwise operators require int",
    );
}

#[test]
fn bitwise_not_on_bool_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = ~true\n}",
        "cannot apply '~'",
    );
}

// ── compound assignment tests ──

#[test]
fn plus_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 10\n    x += 5\n    print(x)\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn minus_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 10\n    x -= 3\n    print(x)\n}",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn star_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 4\n    x *= 3\n    print(x)\n}",
    );
    assert_eq!(out, "12\n");
}

#[test]
fn slash_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 20\n    x /= 4\n    print(x)\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn percent_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 17\n    x %= 5\n    print(x)\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn compound_assign_in_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let sum = 0\n    for i in 1..=10 {\n        sum += i\n    }\n    print(sum)\n}",
    );
    assert_eq!(out, "55\n");
}

#[test]
fn compound_assign_float() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 1.5\n    x += 2.5\n    print(x)\n}",
    );
    assert_eq!(out, "4.000000\n");
}

#[test]
fn compound_assign_field() {
    let out = compile_and_run_stdout(
        "class Counter {\n    value: int\n}\n\nfn main() {\n    let mut c = Counter { value: 0 }\n    c.value += 10\n    print(c.value)\n}",
    );
    assert_eq!(out, "10\n");
}

#[test]
fn compound_assign_index() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let a = [1, 2, 3]\n    a[1] += 10\n    print(a[1])\n}",
    );
    assert_eq!(out, "12\n");
}

// ── increment / decrement tests ──

#[test]
fn increment() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    x++\n    print(x)\n}",
    );
    assert_eq!(out, "6\n");
}

#[test]
fn decrement() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 5\n    x--\n    print(x)\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn increment_in_while_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 0\n    while i < 5 {\n        print(i)\n        i++\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n4\n");
}

#[test]
fn decrement_countdown() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let i = 3\n    while i > 0 {\n        print(i)\n        i--\n    }\n}",
    );
    assert_eq!(out, "3\n2\n1\n");
}

#[test]
fn increment_field() {
    let out = compile_and_run_stdout(
        "class Counter {\n    value: int\n}\n\nfn main() {\n    let mut c = Counter { value: 0 }\n    c.value++\n    c.value++\n    c.value++\n    print(c.value)\n}",
    );
    assert_eq!(out, "3\n");
}

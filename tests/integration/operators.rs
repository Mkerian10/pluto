mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn arithmetic_operations() {
    let code = common::compile_and_run(
        "fn main() {\n    let mut a = 10\n    let mut b = 3\n    let mut sum = a + b\n    let mut diff = a - b\n    let mut prod = a * b\n    let mut quot = a / b\n    let mut rem = a % b\n}",
    );
    assert_eq!(code, 0);
}

#[test]
fn boolean_operations() {
    let code = common::compile_and_run(
        "fn main() {\n    let mut a = true\n    let mut b = false\n    let mut c = 1 < 2\n    let mut d = 3 == 3\n}",
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
    assert_eq!(out, "4\n3.5\n6\n3.5\n");
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
        "fn main() {\n    let mut x = 5\n    print(-x)\n    print(-10)\n}",
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
        "fn main() {\n    let mut x = 1.0 & 2.0\n}",
        "bitwise operators require int",
    );
}

#[test]
fn bitwise_not_on_bool_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let mut x = ~true\n}",
        "cannot apply '~'",
    );
}

// ── compound assignment tests ──

#[test]
fn plus_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 10\n    x += 5\n    print(x)\n}",
    );
    assert_eq!(out, "15\n");
}

#[test]
fn minus_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 10\n    x -= 3\n    print(x)\n}",
    );
    assert_eq!(out, "7\n");
}

#[test]
fn star_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 4\n    x *= 3\n    print(x)\n}",
    );
    assert_eq!(out, "12\n");
}

#[test]
fn slash_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 20\n    x /= 4\n    print(x)\n}",
    );
    assert_eq!(out, "5\n");
}

#[test]
fn percent_equals() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 17\n    x %= 5\n    print(x)\n}",
    );
    assert_eq!(out, "2\n");
}

#[test]
fn compound_assign_in_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut sum = 0\n    for i in 1..=10 {\n        sum += i\n    }\n    print(sum)\n}",
    );
    assert_eq!(out, "55\n");
}

#[test]
fn compound_assign_float() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 1.5\n    x += 2.5\n    print(x)\n}",
    );
    assert_eq!(out, "4\n");
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
        "fn main() {\n    let mut a = [1, 2, 3]\n    a[1] += 10\n    print(a[1])\n}",
    );
    assert_eq!(out, "12\n");
}

// ── increment / decrement tests ──

#[test]
fn increment() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 5\n    x++\n    print(x)\n}",
    );
    assert_eq!(out, "6\n");
}

#[test]
fn decrement() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut x = 5\n    x--\n    print(x)\n}",
    );
    assert_eq!(out, "4\n");
}

#[test]
fn increment_in_while_loop() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut i = 0\n    while i < 5 {\n        print(i)\n        i++\n    }\n}",
    );
    assert_eq!(out, "0\n1\n2\n3\n4\n");
}

#[test]
fn decrement_countdown() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut i = 3\n    while i > 0 {\n        print(i)\n        i--\n    }\n}",
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

// Issue #388: && and || must not evaluate their right operand when the left
// one already decides the result.

#[test]
fn and_guard_protects_right_operand() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let xs = [10, 20]
    let j = 0 - 1
    if j >= 0 && xs[j] > 5 {
        print("entered")
    } else {
        print("guarded")
    }
}
"#);
    assert_eq!(out, "guarded\n");
}

#[test]
fn or_guard_protects_right_operand() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let xs = [10, 20]
    let j = 5
    if j >= xs.len() || xs[j] > 5 {
        print("guarded")
    }
}
"#);
    assert_eq!(out, "guarded\n");
}

#[test]
fn while_guard_insertion_sort() {
    let out = compile_and_run_stdout(r#"
fn main() {
    let mut xs = [5, 2, 9, 1]
    let mut i = 1
    while i < xs.len() {
        let v = xs[i]
        let mut j = i - 1
        while j >= 0 && xs[j] > v {
            xs[j + 1] = xs[j]
            j = j - 1
        }
        xs[j + 1] = v
        i = i + 1
    }
    print(f"{xs[0]} {xs[1]} {xs[2]} {xs[3]}")
}
"#);
    assert_eq!(out, "1 2 5 9\n");
}

#[test]
fn logical_ops_evaluate_right_operand_only_when_needed() {
    let out = compile_and_run_stdout(r#"
fn side(tag: string, v: bool) bool {
    print(tag)
    return v
}

fn main() {
    let a = side("A", false) && side("B", true)
    let b = side("C", true) || side("D", true)
    let c = side("E", true) && side("F", false) || side("G", true)
    let d = side("H", false) || side("I", false)
    print(f"{a} {b} {c} {d}")
}
"#);
    assert_eq!(out, "A\nC\nE\nF\nG\nH\nI\nfalse true true false\n");
}

#[test]
fn and_skips_fallible_right_operand() {
    let out = compile_and_run_stdout(r#"
error Bad {}

fn check(x: int) bool {
    print(f"check {x}")
    if x < 0 {
        raise Bad {}
    }
    return x > 1
}

fn guarded(x: int) bool {
    return x >= 0 && check(x)!
}

fn main() {
    let r = guarded(0 - 1) catch false
    print(r)
    let r2 = guarded(3) catch false
    print(r2)
}
"#);
    assert_eq!(out, "false\ncheck 3\ntrue\n");
}

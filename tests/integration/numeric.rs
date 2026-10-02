mod common;
use common::{
    compile_and_run_output, compile_and_run_stdout, compile_should_fail_with,
    compile_test_and_run,
};

// ── Type casting (as) ─────────────────────────────────────────────────────────

#[test]
fn cast_int_to_float() {
    let out = compile_and_run_stdout("fn main() {\n    let x = 42 as float\n    print(x)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn cast_float_to_int() {
    let out = compile_and_run_stdout("fn main() {\n    let x = 3.14 as int\n    print(x)\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn cast_float_to_int_truncates() {
    let out = compile_and_run_stdout("fn main() {\n    print(3.99 as int)\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn cast_negative_float_to_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(-2.7 as int)\n}");
    assert_eq!(out, "-2\n");
}

#[test]
fn cast_int_to_bool_nonzero() {
    let out = compile_and_run_stdout("fn main() {\n    print(1 as bool)\n    print(42 as bool)\n    print(-1 as bool)\n}");
    assert_eq!(out, "true\ntrue\ntrue\n");
}

#[test]
fn cast_int_to_bool_zero() {
    let out = compile_and_run_stdout("fn main() {\n    print(0 as bool)\n}");
    assert_eq!(out, "false\n");
}

#[test]
fn cast_bool_to_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(true as int)\n    print(false as int)\n}");
    assert_eq!(out, "1\n0\n");
}

#[test]
fn cast_chained() {
    // int -> float -> int round-trips
    let out = compile_and_run_stdout("fn main() {\n    let x = 42 as float as int\n    print(x)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn cast_in_expression() {
    // 1 + 2 as float should parse as 1 + (2 as float) since 'as' is postfix
    // But int + float is a type error, so this should fail
    compile_should_fail_with(
        "fn main() {\n    let x = 1 + 2 as float\n}",
        "type mismatch",
    );
}

#[test]
fn cast_invalid_string_to_int() {
    compile_should_fail_with(
        "fn main() {\n    let x = \"hello\" as int\n}",
        "cannot cast",
    );
}

#[test]
fn cast_invalid_bool_to_float() {
    compile_should_fail_with(
        "fn main() {\n    let x = true as float\n}",
        "cannot cast",
    );
}

// ── Math builtins ─────────────────────────────────────────────────────────────

#[test]
fn math_abs_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(abs(-5))\n    print(abs(3))\n    print(abs(0))\n}");
    assert_eq!(out, "5\n3\n0\n");
}

#[test]
fn math_abs_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(abs(-2.5))\n    print(abs(3.7))\n}");
    assert_eq!(out, "2.5\n3.7\n");
}

#[test]
fn math_min_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(min(3, 7))\n    print(min(10, 2))\n    print(min(5, 5))\n}");
    assert_eq!(out, "3\n2\n5\n");
}

#[test]
fn math_min_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(min(3.5, 7.2))\n}");
    assert_eq!(out, "3.5\n");
}

#[test]
fn math_max_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(max(3, 7))\n    print(max(10, 2))\n}");
    assert_eq!(out, "7\n10\n");
}

#[test]
fn math_max_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(max(1.5, 2.5))\n}");
    assert_eq!(out, "2.5\n");
}

#[test]
fn math_pow_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(pow(2, 10) catch 0)\n    print(pow(3, 3) catch 0)\n    print(pow(5, 0) catch 0)\n}");
    assert_eq!(out, "1024\n27\n1\n");
}

#[test]
fn math_pow_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(pow(2.0, 3.0))\n}");
    assert_eq!(out, "8\n");
}

#[test]
fn math_sqrt() {
    let out = compile_and_run_stdout("fn main() {\n    print(sqrt(4.0))\n    print(sqrt(9.0))\n}");
    assert_eq!(out, "2\n3\n");
}

#[test]
fn math_floor() {
    let out = compile_and_run_stdout("fn main() {\n    print(floor(3.7))\n    print(floor(3.0))\n    print(floor(-1.5))\n}");
    assert_eq!(out, "3\n3\n-2\n");
}

#[test]
fn math_ceil() {
    let out = compile_and_run_stdout("fn main() {\n    print(ceil(3.2))\n    print(ceil(3.0))\n    print(ceil(-1.5))\n}");
    assert_eq!(out, "4\n3\n-1\n");
}

#[test]
fn math_round() {
    let out = compile_and_run_stdout("fn main() {\n    print(round(3.4))\n    print(round(3.5))\n    print(round(-1.6))\n}");
    assert_eq!(out, "3\n4\n-2\n");
}

#[test]
fn math_sin_cos() {
    let out = compile_and_run_stdout("fn main() {\n    print(sin(0.0))\n    print(cos(0.0))\n}");
    assert_eq!(out, "0\n1\n");
}

#[test]
fn math_tan() {
    let out = compile_and_run_stdout("fn main() {\n    print(tan(0.0))\n}");
    assert_eq!(out, "0\n");
}

#[test]
fn math_log() {
    let out = compile_and_run_stdout("fn main() {\n    print(log(1.0))\n}");
    assert_eq!(out, "0\n");
}

// ── Arity checks ──────────────────────────────────────────────────────────────

#[test]
fn math_abs_wrong_arity() {
    compile_should_fail_with(
        "fn main() {\n    print(abs(1, 2))\n}",
        "expects 1 argument",
    );
}

#[test]
fn math_min_wrong_arity() {
    compile_should_fail_with(
        "fn main() {\n    print(min(1))\n}",
        "expects 2 arguments",
    );
}

#[test]
fn math_sqrt_wrong_type() {
    compile_should_fail_with(
        "fn main() {\n    print(sqrt(4))\n}",
        "float",
    );
}

// ── Builtin name collision ────────────────────────────────────────────────────

#[test]
fn builtin_shadow_rejected() {
    compile_should_fail_with(
        "fn abs(x: int) int {\n    return x\n}\n\nfn main() {\n    print(abs(-5))\n}",
        "shadow builtin",
    );
}

// ── pow(int, int) error handling ──────────────────────────────────────────────

#[test]
fn pow_int_negative_exp_catch_shorthand() {
    let out = compile_and_run_stdout("fn main() {\n    let r = pow(2, -1) catch 0\n    print(r)\n}");
    assert_eq!(out, "0\n");
}

#[test]
fn pow_int_negative_exp_propagate() {
    let out = compile_and_run_stdout(
        "fn compute() int {\n    let r = pow(2, -1)!\n    return r\n}\n\nfn main() {\n    let x = compute() catch -1\n    print(x)\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn pow_int_positive_exp_no_error() {
    let out = compile_and_run_stdout("fn main() {\n    let r = pow(2, 8) catch 0\n    print(r)\n}");
    assert_eq!(out, "256\n");
}

#[test]
fn pow_int_without_error_handling_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let r = pow(2, 3)\n}",
        "must be handled",
    );
}

#[test]
fn pow_float_no_error_handling_needed() {
    let out = compile_and_run_stdout("fn main() {\n    print(pow(2.0, -1.0))\n}");
    assert_eq!(out, "0.5\n");
}

#[test]
fn pow_int_catch_wildcard() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let r = pow(2, -1) catch err { 99 }\n    print(r)\n}",
    );
    assert_eq!(out, "99\n");
}

// ── Deterministic float formatting (#130) ────────────────────────────────────
// Floats print as the shortest decimal string that round-trips to the same
// double: fixed notation for decimal exponents -4..15, scientific outside,
// canonical inf/-inf/nan. Platform-independent by construction.

#[test]
fn float_formatting_shortest_roundtrip() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(5.5)
    print(7.0)
    print(10.0)
    print(3000000.0)
    print(0.1 + 0.2)
    print(1.0 / 3.0)
    print(0.0001)
    print(0.0)
    print(-0.0)
}
"#,
    );
    assert_eq!(
        out.trim(),
        "5.5\n7\n10\n3000000\n0.30000000000000004\n0.3333333333333333\n0.0001\n0\n-0"
    );
}

#[test]
fn float_formatting_scientific_extremes() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(100000000000000000000.0)
    print(0.00001)
    print(1000000000000000.0)
    print(10000000000000000.0)
    print(2.5e-10)
    print(1.7976931348623157e308)
    print(5e-324)
}
"#,
    );
    assert_eq!(
        out.trim(),
        "1e+20\n1e-05\n1000000000000000\n1e+16\n2.5e-10\n1.7976931348623157e+308\n5e-324"
    );
}

#[test]
fn float_formatting_special_values() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(1.0 / 0.0)
    print(-1.0 / 0.0)
    print(0.0 / 0.0)
    let inf = 1.0 / 0.0
    print(inf - inf)
    print(f"got {0.0 / 0.0} and {2.5}")
}
"#,
    );
    assert_eq!(out.trim(), "inf\n-inf\nnan\nnan\ngot nan and 2.5");
}

// ── Integer overflow defects (issue #416: conditions raise, defects trap) ────
//
// Signed i64 overflow on + - *, unary negation, and MIN / -1 — like
// division/modulo by zero — is a defect: the process prints
// "pluto: defect: ..." to stderr and aborts. Defects never become typed
// errors and never enter error inference.

fn assert_defect(source: &str, expected_stderr: &str) {
    let (_stdout, stderr, code) = compile_and_run_output(source);
    assert_ne!(code, 0, "expected defect abort, got exit 0; stderr: {stderr}");
    assert!(
        stderr.contains(expected_stderr),
        "expected stderr containing '{expected_stderr}', got: {stderr}"
    );
}

#[test]
fn add_overflow_traps() {
    assert_defect(
        r#"
fn main() {
    let a = 9223372036854775807
    let b = a + 1
    print(b)
}
"#,
        "pluto: defect: integer overflow in '+': 9223372036854775807 + 1",
    );
}

#[test]
fn sub_overflow_traps() {
    assert_defect(
        r#"
fn main() {
    let a = -9223372036854775808
    let b = a - 1
    print(b)
}
"#,
        "pluto: defect: integer overflow in '-': -9223372036854775808 - 1",
    );
}

#[test]
fn mul_overflow_traps() {
    assert_defect(
        r#"
fn main() {
    let a = -9223372036854775808
    let b = a * -1
    print(b)
}
"#,
        "pluto: defect: integer overflow in '*': -9223372036854775808 * -1",
    );
}

#[test]
fn min_div_neg_one_traps() {
    assert_defect(
        r#"
fn main() {
    let a = -9223372036854775808
    let d = -1
    let b = a / d
    print(b)
}
"#,
        "pluto: defect: integer overflow in '/': -9223372036854775808 / -1",
    );
}

#[test]
fn div_by_zero_traps() {
    assert_defect(
        r#"
fn zero() int {
    return 0
}

fn main() {
    let b = 7 / zero()
    print(b)
}
"#,
        "pluto: defect: division by zero: 7 / 0",
    );
}

#[test]
fn mod_by_zero_traps() {
    assert_defect(
        r#"
fn zero() int {
    return 0
}

fn main() {
    let b = 7 % zero()
    print(b)
}
"#,
        "pluto: defect: modulo by zero: 7 % 0",
    );
}

#[test]
fn unary_neg_min_traps() {
    assert_defect(
        r#"
fn main() {
    let m = -9223372036854775808
    let x = -m
    print(x)
}
"#,
        "pluto: defect: integer overflow in unary '-': -(-9223372036854775808)",
    );
}

#[test]
fn compound_add_overflow_traps() {
    assert_defect(
        r#"
fn main() {
    let mut x = 9223372036854775807
    x += 1
    print(x)
}
"#,
        "pluto: defect: integer overflow in '+': 9223372036854775807 + 1",
    );
}

#[test]
fn pow_overflow_traps() {
    assert_defect(
        r#"
fn main() {
    print(pow(2, 64) catch 0)
}
"#,
        "pluto: defect: integer overflow in pow(): pow(2, 64)",
    );
}

// The i64::MIN literal: the lexer accepts i64::MAX + 1 after unary minus and
// the parser folds the sign into the literal — no runtime negation, no trap.
#[test]
fn min_literal_is_not_a_defect() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(-9223372036854775808)
    let m = -9223372036854775808
    print(m + 1)
}
"#,
    );
    assert_eq!(out.trim(), "-9223372036854775808\n-9223372036854775807");
}

// MIN % -1 is mathematically 0 (representable) — defined, not a defect.
#[test]
fn min_mod_neg_one_is_zero() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a = -9223372036854775808
    let d = -1
    print(a % d)
}
"#,
    );
    assert_eq!(out.trim(), "0");
}

// Boundary arithmetic that stays in range must not trap.
#[test]
fn boundary_arithmetic_in_range_is_fine() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let hi = 9223372036854775807
    let lo = -9223372036854775808
    print(hi + 0)
    print(lo + 1)
    print(hi - 1 + 1)
    print(lo / -2)
    print(hi * -1)
}
"#,
    );
    assert_eq!(
        out.trim(),
        "9223372036854775807\n-9223372036854775807\n9223372036854775807\n4611686018427387904\n-9223372036854775807"
    );
}

// ── wrapping_* builtins: the visible escape hatch for modular arithmetic ─────

#[test]
fn wrapping_builtins_round_trip() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let hi = 9223372036854775807
    let lo = -9223372036854775808
    print(wrapping_add(hi, 1))
    print(wrapping_sub(lo, 1))
    print(wrapping_mul(lo, -1))
    print(wrapping_add(wrapping_add(hi, 1), -1))
    print(wrapping_add(2, 3))
    print(wrapping_sub(10, 4))
    print(wrapping_mul(6, 7))
}
"#,
    );
    assert_eq!(
        out.trim(),
        "-9223372036854775808\n9223372036854775807\n-9223372036854775808\n9223372036854775807\n5\n6\n42"
    );
}

#[test]
fn wrapping_builtin_wrong_arity_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = wrapping_add(1)\n    print(x)\n}",
        "wrapping_add() expects 2 arguments",
    );
}

#[test]
fn wrapping_builtin_wrong_type_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = wrapping_add(1.0, 2.0)\n    print(x)\n}",
        "wrapping_add() expects int arguments",
    );
}

// A defect inside a `test` block fails that test: the binary aborts with the
// defect message mid-run (same path as the other runtime aborts under the
// deterministic runner) and the pluto CLI reports the non-zero exit.
#[test]
fn defect_in_test_block_fails_that_test() {
    let (stdout, stderr, code) = compile_test_and_run(
        r#"
fn big() int {
    return 9223372036854775807
}

test "passes first" {
    expect(1).to_equal(1)
}

test "overflows" {
    let x = big() + 1
    expect(x).to_equal(0)
}
"#,
    );
    assert_ne!(code, 0, "defect in a test must fail the run; stdout: {stdout}");
    assert!(stdout.contains("passes first ... ok"), "stdout: {stdout}");
    assert!(
        stderr.contains("pluto: defect: integer overflow in '+': 9223372036854775807 + 1"),
        "stderr: {stderr}"
    );
}

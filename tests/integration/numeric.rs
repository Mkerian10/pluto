mod common;
use common::{
    compile_and_run_output, compile_and_run_stdout, compile_should_fail_with,
    compile_test_and_run,
};

// ── Conversion methods (`as` removed, rfc-number-types phase 3) ────────────────

#[test]
fn convert_int_to_float() {
    let out = compile_and_run_stdout("fn main() {\n    let x = (42).to_float()\n    print(x)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn convert_float_to_int() {
    // A float literal proves in range, so `.to_int()` narrows to int.
    let out = compile_and_run_stdout("fn main() {\n    let x = (3.14).to_int()\n    print(x)\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn convert_float_to_int_truncates() {
    let out = compile_and_run_stdout("fn main() {\n    print((3.99).to_int())\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn convert_negative_float_to_int() {
    // Negation of a literal is not itself a literal, so the result stays
    // `int?`; a fallback supplies the value where a proof is absent.
    let out = compile_and_run_stdout("fn main() {\n    print((-2.7).to_int() ?? 0)\n}");
    assert_eq!(out, "-2\n");
}

#[test]
fn convert_int_to_bool_nonzero() {
    let out = compile_and_run_stdout("fn main() {\n    print(1 != 0)\n    print(42 != 0)\n    print(-1 != 0)\n}");
    assert_eq!(out, "true\ntrue\ntrue\n");
}

#[test]
fn convert_int_to_bool_zero() {
    let out = compile_and_run_stdout("fn main() {\n    print(0 != 0)\n}");
    assert_eq!(out, "false\n");
}

#[test]
fn convert_bool_to_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(true.to_int())\n    print(false.to_int())\n}");
    assert_eq!(out, "1\n0\n");
}

#[test]
fn convert_chained() {
    // int -> float -> int round-trips; the float is not a literal, so the
    // final `.to_int()` is `int?` and a fallback makes the value total.
    let out = compile_and_run_stdout("fn main() {\n    let x = (42).to_float().to_int() ?? -1\n    print(x)\n}");
    assert_eq!(out, "42\n");
}

#[test]
fn cast_as_operator_removed() {
    // `as` no longer parses; the diagnostic points at the replacement method.
    compile_should_fail_with(
        "fn main() {\n    let x = 1 + 2 as float\n}",
        "was removed",
    );
}

#[test]
fn cast_string_to_int_removed() {
    compile_should_fail_with(
        "fn main() {\n    let x = \"hello\" as int\n}",
        "was removed",
    );
}

#[test]
fn cast_bool_to_float_removed() {
    compile_should_fail_with(
        "fn main() {\n    let x = true as float\n}",
        "was removed",
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

// ── Shift amounts (issue #441) ───────────────────────────────────────────────
//
// A shift amount outside 0..63 is a defect: constant amounts are a compile
// error, non-constant amounts trap at runtime. Bits shifted out of the value
// are NOT a defect (Rust's model) — `<<` never reports overflow.

#[test]
fn shift_constant_amount_64_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = 1 << 64\n    print(x)\n}",
        "shift amount 64 is out of range 0..63",
    );
}

#[test]
fn shift_constant_amount_negative_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = 1 << -1\n    print(x)\n}",
        "shift amount -1 is out of range 0..63",
    );
}

#[test]
fn shr_constant_amount_out_of_range_rejected() {
    compile_should_fail_with(
        "fn main() {\n    let x = 1024 >> 100\n    print(x)\n}",
        "shift amount 100 is out of range 0..63",
    );
}

#[test]
fn shift_variable_amount_64_traps() {
    assert_defect(
        r#"
fn amount() int {
    return 64
}

fn main() {
    print(1 << amount())
}
"#,
        "pluto: defect: shift amount 64 out of range 0..63",
    );
}

#[test]
fn shift_variable_amount_negative_traps() {
    assert_defect(
        r#"
fn main() {
    let n = -1
    print(1 << n)
}
"#,
        "pluto: defect: shift amount -1 out of range 0..63",
    );
}

#[test]
fn shr_variable_amount_70_traps() {
    assert_defect(
        r#"
fn main() {
    let n = 70
    let x = 1024 >> n
    print(x)
}
"#,
        "pluto: defect: shift amount 70 out of range 0..63",
    );
}

#[test]
fn shift_rotate_shape_amount_traps() {
    // The `x >> n | x << (32 - n)` rotate shape with n = 40: the right
    // shift is fine, the left shift amount is 32 - 40 = -8.
    assert_defect(
        r#"
fn rot(x: int, n: int) int {
    return x >> n | x << (32 - n)
}

fn main() {
    print(rot(5, 40))
}
"#,
        "pluto: defect: shift amount -8 out of range 0..63",
    );
}

#[test]
fn shift_boundary_amounts_are_fine() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(5 << 0)
    print(5 >> 0)
    print(1 << 63)
    print(-1 >> 63)
    let z = 0
    let t = 63
    print(7 << z)
    print(1 << t)
    print(-9223372036854775808 >> t)
}
"#,
    );
    assert_eq!(
        out.trim(),
        "5\n5\n-9223372036854775808\n-1\n7\n-9223372036854775808\n-1"
    );
}

#[test]
fn shifting_bits_out_is_not_a_defect() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(255 << 60)
    print(3 << 63)
    let n = 62
    print(7 << n)
    print(-1 << 63)
}
"#,
    );
    assert_eq!(
        out.trim(),
        "-1152921504606846976\n-9223372036854775808\n-4611686018427387904\n-9223372036854775808"
    );
}

#[test]
fn shift_fact_proven_amount_runs() {
    // The guard proves 0 <= k <= 63, so the range check is elided; the
    // result must still be correct.
    let out = compile_and_run_stdout(
        r#"
fn amount() int {
    return 10
}

fn main() {
    let k = amount()
    let mut v = 0
    if k >= 0 && k < 64 {
        v = 1 << k
    }
    print(v)
}
"#,
    );
    assert_eq!(out.trim(), "1024");
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

// Pluto `%` is truncated (C-style) remainder — codegen lowers int `%` to
// Cranelift `srem`, so the result's sign follows the DIVIDEND. The fact
// engine's `x % c` interval rule (facts.rs `expr_bounds`) is derived from
// exactly this: for constant c > 0 the result lies in [-(c-1), c-1], and in
// [0, c-1] only when the dividend is provably non-negative. If this test
// ever changes, that rule is wrong.
#[test]
fn mod_truncated_sign_follows_dividend() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a = -7
    let b = 7
    print(a % 3)
    print(b % 3)
    print(a % -3)
    print(b % -3)
}
"#,
    );
    assert_eq!(out.trim(), "-1\n1\n-1\n1");
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

// ── rfc-number-types §2: conversion methods + proof narrowing ─────────────────

#[test]
fn byte_to_int_total() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let b: byte = (200).to_byte()\n    print(b.to_int())\n}",
    );
    assert_eq!(out, "200\n");
}

#[test]
fn bool_to_int_total() {
    let out = compile_and_run_stdout(
        "fn main() {\n    print(true.to_int())\n    print(false.to_int())\n}",
    );
    assert_eq!(out, "1\n0\n");
}

#[test]
fn low_byte_truncates() {
    // 300 & 0xFF == 44; low_byte is deliberate truncation (total).
    let out = compile_and_run_stdout("fn main() {\n    print((300).low_byte().to_int())\n}");
    assert_eq!(out, "44\n");
}

#[test]
fn to_byte_literal_folds() {
    // An in-range integer literal narrows `byte?` to `byte` at compile time.
    let out = compile_and_run_stdout(
        "fn main() {\n    let b: byte = (61).to_byte()\n    print(b.to_int())\n}",
    );
    assert_eq!(out, "61\n");
}

#[test]
fn to_byte_narrows_under_guard() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let x = 500\n    if x >= 0 && x <= 255 {\n        let b: byte = x.to_byte()\n        print(b.to_int())\n    } else {\n        print(0 - 1)\n    }\n}",
    );
    assert_eq!(out, "-1\n");
}

#[test]
fn to_byte_narrows_after_assert() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let z = 42\n    assert z >= 0 && z <= 255\n    let b: byte = z.to_byte()\n    print(b.to_int())\n}",
    );
    assert_eq!(out, "42\n");
}

#[test]
fn to_byte_narrows_under_mask() {
    // `x & 0xFF` lies in 0..255 (facts.rs mask interval rule).
    let out = compile_and_run_stdout(
        "fn main() {\n    let w = 1000\n    let b: byte = (w & 0xFF).to_byte()\n    print(b.to_int())\n}",
    );
    assert_eq!(out, "232\n");
}

#[test]
fn to_byte_narrows_in_masked_loop() {
    // `i & 0xFF` lies in 0..255 by the structural mask rule, so the
    // conversion narrows even inside a loop (where flow facts are dropped).
    let out = compile_and_run_stdout(
        "fn main() {\n    let mut dst = \"\".to_bytes()\n    let mut i = 65\n    while i < 68 {\n        dst.push((i & 0xFF).to_byte())\n        i = i + 1\n    }\n    print(dst.to_string())\n}",
    );
    assert_eq!(out, "ABC\n");
}

#[test]
fn float_to_int_literal_folds() {
    let out = compile_and_run_stdout("fn main() {\n    print((3.7).to_int())\n}");
    assert_eq!(out, "3\n");
}

#[test]
fn float_to_int_none_for_nan_inf_overflow() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let nan = 0.0 / 0.0\n    print(nan.to_int() ?? (0 - 1))\n    let big = 1.0e30\n    print(big.to_int() ?? (0 - 2))\n    let neg = -1.0e30\n    print(neg.to_int() ?? (0 - 3))\n}",
    );
    assert_eq!(out, "-1\n-2\n-3\n");
}

#[test]
fn float_to_int_runtime_value_truncates_toward_zero() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let f = read_f()\n    print(f.to_int() ?? 0)\n}\nfn read_f() float {\n    return 0.0 - 9.9\n}",
    );
    assert_eq!(out, "-9\n");
}

// ── Diagnostics: still-nullable conversion used where T is required ───────────

#[test]
fn to_byte_unproven_stays_nullable_and_diagnoses() {
    // No facts about x: the conversion stays `byte?` and the diagnostic
    // points at the conversion.
    compile_should_fail_with(
        "fn main() {\n    let x = read_i()\n    let b: byte = x.to_byte()\n    print(b.to_int())\n}\nfn read_i() int {\n    return 5\n}",
        "`x.to_byte()` may be none",
    );
}

#[test]
fn to_byte_partial_bound_diagnostic_states_known_fact() {
    compile_should_fail_with(
        "fn main() {\n    let x = read_i()\n    if x >= 0 {\n        let b: byte = x.to_byte()\n        print(b.to_int())\n    }\n}\nfn read_i() int {\n    return 5\n}",
        "known >= 0 but has no upper bound",
    );
}

#[test]
fn to_byte_diagnostic_suggests_guard_or_assert() {
    compile_should_fail_with(
        "fn main() {\n    let x = read_i()\n    let b: byte = x.to_byte()\n    print(b.to_int())\n}\nfn read_i() int {\n    return 5\n}",
        "add a guard",
    );
}

#[test]
fn float_to_int_unproven_stays_nullable() {
    compile_should_fail_with(
        "fn main() {\n    let f = read_f()\n    let n: int = f.to_int()\n    print(n)\n}\nfn read_f() float {\n    return 1.5\n}",
        "may be none",
    );
}

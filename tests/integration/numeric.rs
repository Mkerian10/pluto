mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Type casting (as) ─────────────────────────────────────────────────────────

#[test]
fn cast_int_to_float() {
    let out = compile_and_run_stdout("fn main() {\n    let x = 42 as float\n    print(x)\n}");
    assert_eq!(out, "42.000000\n");
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
    assert_eq!(out, "2.500000\n3.700000\n");
}

#[test]
fn math_min_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(min(3, 7))\n    print(min(10, 2))\n    print(min(5, 5))\n}");
    assert_eq!(out, "3\n2\n5\n");
}

#[test]
fn math_min_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(min(3.5, 7.2))\n}");
    assert_eq!(out, "3.500000\n");
}

#[test]
fn math_max_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(max(3, 7))\n    print(max(10, 2))\n}");
    assert_eq!(out, "7\n10\n");
}

#[test]
fn math_max_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(max(1.5, 2.5))\n}");
    assert_eq!(out, "2.500000\n");
}

#[test]
fn math_pow_int() {
    let out = compile_and_run_stdout("fn main() {\n    print(pow(2, 10) catch 0)\n    print(pow(3, 3) catch 0)\n    print(pow(5, 0) catch 0)\n}");
    assert_eq!(out, "1024\n27\n1\n");
}

#[test]
fn math_pow_float() {
    let out = compile_and_run_stdout("fn main() {\n    print(pow(2.0, 3.0))\n}");
    assert_eq!(out, "8.000000\n");
}

#[test]
fn math_sqrt() {
    let out = compile_and_run_stdout("fn main() {\n    print(sqrt(4.0))\n    print(sqrt(9.0))\n}");
    assert_eq!(out, "2.000000\n3.000000\n");
}

#[test]
fn math_floor() {
    let out = compile_and_run_stdout("fn main() {\n    print(floor(3.7))\n    print(floor(3.0))\n    print(floor(-1.5))\n}");
    assert_eq!(out, "3.000000\n3.000000\n-2.000000\n");
}

#[test]
fn math_ceil() {
    let out = compile_and_run_stdout("fn main() {\n    print(ceil(3.2))\n    print(ceil(3.0))\n    print(ceil(-1.5))\n}");
    assert_eq!(out, "4.000000\n3.000000\n-1.000000\n");
}

#[test]
fn math_round() {
    let out = compile_and_run_stdout("fn main() {\n    print(round(3.4))\n    print(round(3.5))\n    print(round(-1.6))\n}");
    assert_eq!(out, "3.000000\n4.000000\n-2.000000\n");
}

#[test]
fn math_sin_cos() {
    let out = compile_and_run_stdout("fn main() {\n    print(sin(0.0))\n    print(cos(0.0))\n}");
    assert_eq!(out, "0.000000\n1.000000\n");
}

#[test]
fn math_tan() {
    let out = compile_and_run_stdout("fn main() {\n    print(tan(0.0))\n}");
    assert_eq!(out, "0.000000\n");
}

#[test]
fn math_log() {
    let out = compile_and_run_stdout("fn main() {\n    print(log(1.0))\n}");
    assert_eq!(out, "0.000000\n");
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
    assert_eq!(out, "0.500000\n");
}

#[test]
fn pow_int_catch_wildcard() {
    let out = compile_and_run_stdout(
        "fn main() {\n    let r = pow(2, -1) catch err { 99 }\n    print(r)\n}",
    );
    assert_eq!(out, "99\n");
}

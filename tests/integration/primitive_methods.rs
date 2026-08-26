//! Methods on primitive types (#128).
//!
//! int: to_string, to_float, abs
//! float: to_string, to_int, abs, sqrt, floor, ceil, round
//! bool: to_string
//!
//! Math methods call the same runtime functions as the free builtins, so
//! `x.abs()` and `abs(x)` agree exactly. Method calls work on literals too:
//! the lexer only rejects `1.2.3` (dot followed by another number), not
//! `1.5.to_string()`.
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn int_methods() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let x = -42
    print(x.abs())
    print(x.to_string() + "!")
    print(x.to_float() / 4.0)
}
"#,
    );
    assert_eq!(out.trim(), "42\n-42!\n-10.5");
}

#[test]
fn float_methods() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let f = 2.25
    print(f.sqrt())
    print(f.to_string())
    print(3.7.floor())
    print(3.2.ceil())
    print(2.5.round())
    print(9.9.to_int())
    print((-3.7).abs())
}
"#,
    );
    assert_eq!(out.trim(), "1.5\n2.25\n3\n4\n3\n9\n3.7");
}

#[test]
fn bool_methods() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(true.to_string())
    print(false.to_string() + "!")
}
"#,
    );
    assert_eq!(out.trim(), "true\nfalse!");
}

#[test]
fn methods_on_literals() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print(42.to_string() + "?")
    print(1.5.to_string() + ".")
}
"#,
    );
    assert_eq!(out.trim(), "42?\n1.5.");
}

#[test]
fn method_chaining() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print((-16.0).abs().sqrt().to_string())
    let x = -42
    print(x.abs().to_float().sqrt().floor())
}
"#,
    );
    assert_eq!(out.trim(), "4\n6");
}

#[test]
fn methods_in_fstrings() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let x = -7
    print(f"n={x.abs().to_string()} f={2.25.sqrt()}")
}
"#,
    );
    assert_eq!(out.trim(), "n=7 f=1.5");
}

#[test]
fn method_agrees_with_free_builtin() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let v = -2.5
    print(v.abs() == abs(v))
    print(2.5.round() == round(2.5))
    print(6.25.sqrt() == sqrt(6.25))
}
"#,
    );
    assert_eq!(out.trim(), "true\ntrue\ntrue");
}

// ── Rejections ───────────────────────────────────────────────────────────────

#[test]
fn unknown_int_method_rejected() {
    compile_should_fail_with(
        "fn main(){\n    print(5.foo())\n}",
        "int has no method 'foo'",
    );
}

#[test]
fn unknown_float_method_rejected() {
    compile_should_fail_with(
        "fn main(){\n    print(2.5.nope())\n}",
        "float has no method 'nope'",
    );
}

#[test]
fn unknown_bool_method_rejected() {
    compile_should_fail_with(
        "fn main(){\n    print(true.nope())\n}",
        "bool has no method 'nope'",
    );
}

#[test]
fn primitive_method_arity_enforced() {
    compile_should_fail_with(
        "fn main(){\n    let x = 5\n    print(x.abs(1))\n}",
        "abs() expects 0 arguments",
    );
}

#[test]
fn multiple_decimal_points_still_rejected() {
    compile_should_fail_with(
        "fn main(){\n    print(1.2.3)\n}",
        "multiple decimal points",
    );
}

#[test]
fn nullable_receiver_requires_narrowing() {
    compile_should_fail_with(
        "fn main(){\n    let x: int? = 5\n    print(x.abs())\n}",
        "",
    );
}

#[test]
fn narrowed_nullable_receiver_works() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let x: int? = -5
    if x != none {
        print(x.abs())
    }
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

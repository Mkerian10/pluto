//! Named-function references as first-class values.
//!
//! A bare identifier that resolves to a (non-generic) top-level function is a
//! value of fn type: it can be bound, passed to higher-order functions,
//! returned, and stored. Implementation: typeck records the reference sites,
//! and closure lifting eta-expands them into captureless wrapper closures.
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

#[test]
fn fn_ref_bound_and_called() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int {
    return x * 2
}

fn main() {
    let f = double
    print(f(21))
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn fn_ref_passed_to_higher_order_function() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int {
    return x * 2
}

fn triple(x: int) int {
    return x * 3
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    print(apply(double, 5))
    print(apply(triple, 5))
}
"#,
    );
    assert_eq!(out.trim(), "10\n15");
}

#[test]
fn fn_ref_passed_to_generic_higher_order_function() {
    let out = compile_and_run_stdout(
        r#"
fn shout(s: string) string {
    return s + "!"
}

fn apply<T>(f: fn(T) T, x: T) T {
    return f(x)
}

fn main() {
    print(apply(shout, "hey"))
}
"#,
    );
    assert_eq!(out.trim(), "hey!");
}

#[test]
fn fn_ref_returned_from_function() {
    let out = compile_and_run_stdout(
        r#"
fn inc(x: int) int {
    return x + 1
}

fn make_op() fn(int) int {
    return inc
}

fn main() {
    let op = make_op()
    print(op(41))
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn fn_refs_in_array() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int {
    return x * 2
}

fn triple(x: int) int {
    return x * 3
}

fn main() {
    let ops = [double, triple]
    let mut total = 0
    for op in ops {
        total = total + op(10)
    }
    print(total)
}
"#,
    );
    assert_eq!(out.trim(), "50");
}

#[test]
fn fn_ref_multiple_params_and_void() {
    let out = compile_and_run_stdout(
        r#"
fn add3(a: int, b: int, c: int) int {
    return a + b + c
}

fn announce(x: int) {
    print(x)
}

fn main() {
    let f = add3
    let g = announce
    g(f(1, 2, 3))
}
"#,
    );
    assert_eq!(out.trim(), "6");
}

// ── Error handling through references ────────────────────────────────────────

#[test]
fn fallible_fn_ref_alias_call_with_catch() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn main() {
    let f = risky
    let a = f(2) catch -1
    print(a)
    let b = f(9) catch err: E { err.code }
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "2\n9");
}

#[test]
fn unhandled_fallible_fn_ref_alias_call_rejected() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn main() {
    let f = risky
    let a = f(2)
    print(a)
}
"#,
        "must be handled with ! or catch",
    );
}

#[test]
fn catch_on_infallible_fn_ref_alias_rejected() {
    compile_should_fail_with(
        r#"
fn safe(x: int) int {
    return x
}

fn main() {
    let f = safe
    let a = f(2) catch -1
    print(a)
}
"#,
        "catch applied to infallible",
    );
}

#[test]
fn escaping_fallible_fn_ref_keeps_definer_fallible() {
    // Passing a fallible function reference away means it may be called where
    // the analysis can't see; the passer conservatively absorbs its error set.
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn run_it(x: int) int {
    return apply(risky, x)
}

fn main() {
    let ok = run_it(1) catch -1
    print(ok)
    let bad = run_it(7) catch -1
    print(bad)
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

// ── Rejections ───────────────────────────────────────────────────────────────

#[test]
fn generic_fn_as_value_rejected_with_guidance() {
    compile_should_fail_with(
        r#"
fn id<T>(x: T) T {
    return x
}

fn main() {
    let f = id
    print(f(1))
}
"#,
        "generic function 'id' cannot be used as a value",
    );
}

#[test]
fn generic_fn_as_argument_rejected_with_guidance() {
    compile_should_fail_with(
        r#"
fn id<T>(x: T) T {
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    print(apply(id, 5))
}
"#,
        "generic function 'id' cannot be used as a value",
    );
}

#[test]
fn closure_wrapper_workaround_for_generic() {
    let out = compile_and_run_stdout(
        r#"
fn id<T>(x: T) T {
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    print(apply((x: int) => id(x), 5))
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn unknown_identifier_still_undefined_variable() {
    compile_should_fail_with(
        r#"
fn main() {
    let f = nonexistent
    print(f)
}
"#,
        "undefined variable 'nonexistent'",
    );
}

#[test]
fn param_shadowing_fn_name_is_the_variable() {
    // Params may share a function's name (only `let` is barred from
    // shadowing); the identifier must resolve to the parameter.
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int {
    return x * 2
}

fn use_it(double: int) int {
    return double + 1
}

fn main() {
    print(use_it(10))
}
"#,
    );
    assert_eq!(out.trim(), "11");
}

#[test]
fn compose_named_functions() {
    let out = compile_and_run_stdout(
        r#"
fn double(x: int) int {
    return x * 2
}

fn square(x: int) int {
    return x * x
}

fn compose(f: fn(int) int, g: fn(int) int) fn(int) int {
    return (x: int) => g(f(x))
}

fn main() {
    let double_then_square = compose(double, square)
    print(double_then_square(3))
}
"#,
    );
    assert_eq!(out.trim(), "36");
}

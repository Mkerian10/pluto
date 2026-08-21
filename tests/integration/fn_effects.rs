//! Fallibility in function types (docs/design/rfc-fn-effects.md).
//!
//! `fn(int) int` is an infallible contract — fallible values are rejected at
//! the boundary. `fn(int) int!` accepts fallible values; calls through such a
//! value must be handled, and errors flow to whoever calls it.
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── Boundary rejections ──────────────────────────────────────────────────────

#[test]
fn fallible_fn_ref_into_infallible_param() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    print(apply(risky, 2))
}
"#,
        "cannot pass fallible function 'risky' where an infallible function type is expected",
    );
}

#[test]
fn fallible_closure_into_infallible_param() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    print(apply((n: int) => {
        raise E { code: n }
        return n
    }, 2))
}
"#,
        "cannot pass fallible closure where an infallible function type is expected",
    );
}

#[test]
fn fallible_value_into_infallible_let_annotation() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn main() {
    let f: fn(int) int = risky
    print(f(1))
}
"#,
        "cannot pass fallible function 'risky'",
    );
}

#[test]
fn fallible_alias_into_infallible_param() {
    // Provenance flows through variable bindings.
    compile_should_fail_with(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    let g = risky
    print(apply(g, 2))
}
"#,
        "where an infallible function type is expected",
    );
}

// ── Fallible contracts ───────────────────────────────────────────────────────

#[test]
fn fallible_contract_end_to_end() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn apply(f: fn(int) int!, x: int) int {
    return f(x)!
}

fn main() {
    let ok = apply(risky, 1) catch -1
    print(ok)
    let bad = apply(risky, 9) catch -1
    print(bad)
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

#[test]
fn infallible_value_satisfies_fallible_contract() {
    // Subsumption: a value that never raises is fine where failure is allowed.
    let out = compile_and_run_stdout(
        r#"
fn apply(f: fn(int) int!, x: int) int {
    return f(x)!
}

fn main() {
    print(apply((n: int) => n + 1, 10) catch -1)
}
"#,
    );
    assert_eq!(out.trim(), "11");
}

#[test]
fn unhandled_call_through_fallible_value_rejected() {
    compile_should_fail_with(
        r#"
fn apply(f: fn(int) int!, x: int) int {
    return f(x)
}

fn main() {
    print(apply((x: int) => x, 1))
}
"#,
        "call through fallible function value 'f' must be handled with ! or catch",
    );
}

#[test]
fn typed_catch_through_opaque_fallible_value_needs_wildcard() {
    compile_should_fail_with(
        r#"
error E { code: int }

fn apply(f: fn(int) int!, x: int) int {
    let r = f(x) catch err: E { -5 }
    return r
}

fn main() {
    print(apply((n: int) => n, 3))
}
"#,
        "typed catch cannot prove coverage",
    );
}

#[test]
fn wildcard_catch_through_fallible_value_works() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn apply(f: fn(int) int!, x: int) int {
    let r = f(x) catch -5
    return r
}

fn main() {
    print(apply(risky, 3))
}
"#,
    );
    assert_eq!(out.trim(), "-5");
}

#[test]
fn errors_propagate_through_fallible_contract_chain() {
    // apply propagates the value's error; its inferred set widens, so its own
    // call sites are enforced.
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn apply(f: fn(int) int!, x: int) int {
    return f(x)!
}

fn chain(x: int) int {
    return apply(risky, x)!
}

fn main() {
    let a = chain(2) catch -1
    print(a)
    let b = chain(8) catch -1
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "2\n-1");
}

// ── Handling inside the passer ───────────────────────────────────────────────

#[test]
fn wrapper_closure_satisfies_infallible_contract() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    raise E { code: x }
    return x
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    let wrapped = (x: int) => risky(x) catch -9
    print(apply(wrapped, 4))
}
"#,
    );
    assert_eq!(out.trim(), "-9");
}

#[test]
fn passer_not_marked_fallible_for_fallible_contract_flow() {
    // Flowing a fallible value into a fallible-typed slot is the receiver's
    // declared responsibility — the passer stays infallible.
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn apply(g: fn(int) int!, x: int) int {
    return g(x)!
}

fn definer(x: int) int {
    let f = (n: int) => {
        if n > 3 {
            raise E { code: n }
        }
        return n
    }
    return apply(f, x) catch -1
}

fn main() {
    print(definer(1))
    print(definer(9))
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

// ── Fallible fn types in other positions ─────────────────────────────────────

#[test]
fn fallible_fn_type_as_return_type() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn pick() fn(int) int! {
    return risky
}

fn main() {
    let f = pick()
    let a = f(1)!
    print(a)
    let b = f(9) catch -1
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

#[test]
fn void_fallible_fn_type() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn shout(x: int) {
    if x > 3 {
        raise E { code: x }
    }
    print(x)
}

fn run(f: fn(int)!, x: int) {
    f(x) catch err {
        print(-1)
    }
}

fn main() {
    run(shout, 1)
    run(shout, 9)
}
"#,
    );
    assert_eq!(out.trim(), "1\n-1");
}

#[test]
fn generic_hof_with_fallible_contract() {
    let out = compile_and_run_stdout(
        r#"
error E { code: int }

fn risky(x: int) int {
    if x > 3 {
        raise E { code: x }
    }
    return x
}

fn apply<T>(f: fn(T) T!, x: T) T {
    return f(x)!
}

fn main() {
    let a = apply(risky, 2) catch -1
    print(a)
    let b = apply(risky, 8) catch -1
    print(b)
}
"#,
    );
    assert_eq!(out.trim(), "2\n-1");
}

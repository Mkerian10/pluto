//! Nullable ergonomics: the `??` coalescing operator and flow narrowing.
//!
//! `a ?? b` — a if non-none, else b; the result is non-nullable when b is,
//! and stays nullable for chaining when b is nullable. Lowest-precedence
//! infix operator, right-associative.
//!
//! Narrowing: `if x != none { ... }` proves x non-none in the then branch
//! (`x == none` proves it in the else branch), and a guard whose none-path
//! never falls through (`if x == none { return }`) proves it for the rest of
//! the block. Redundant `?` on a narrowed variable stays legal.
mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

// ── ?? operator ──────────────────────────────────────────────────────────────

#[test]
fn coalesce_value_types() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a: int? = 42
    let b: int? = none
    print(a ?? -1)
    print(b ?? -1)
    let f: float? = none
    print(f ?? 2.5)
    let t: bool? = true
    print(t ?? false)
}
"#,
    );
    assert_eq!(out.trim(), "42\n-1\n2.5\ntrue");
}

#[test]
fn coalesce_heap_types() {
    let out = compile_and_run_stdout(
        r#"
class Point {
    x: int
}

fn main() {
    let s: string? = "hi"
    let t: string? = none
    print(s ?? "gone")
    print(t ?? "gone")
    let p: Point? = none
    let q = p ?? Point { x: 7 }
    print(q.x)
}
"#,
    );
    assert_eq!(out.trim(), "hi\ngone\n7");
}

#[test]
fn coalesce_chains_right_associative() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a: int? = none
    let b: int? = none
    let c: int? = 3
    print(a ?? b ?? c ?? 9)
    print(a ?? b ?? 9)
}
"#,
    );
    assert_eq!(out.trim(), "3\n9");
}

#[test]
fn coalesce_with_to_int() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    print("12".to_int() ?? 0)
    print("oops".to_int() ?? 0)
}
"#,
    );
    assert_eq!(out.trim(), "12\n0");
}

#[test]
fn coalesce_precedence_is_lowest() {
    // `a ?? b + 1` parses as `a ?? (b + 1)`
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a: int? = none
    let b = 4
    print(a ?? b + 1)
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn coalesce_on_non_nullable_rejected() {
    compile_should_fail_with(
        r#"
fn main() {
    let x = 5
    print(x ?? 1)
}
"#,
        "'??' applied to non-nullable",
    );
}

#[test]
fn coalesce_fallback_type_mismatch_rejected() {
    compile_should_fail_with(
        r#"
fn main() {
    let x: int? = 5
    print(x ?? "nope")
}
"#,
        "'??' fallback type mismatch",
    );
}

#[test]
fn coalesce_lazy_fallback() {
    // The fallback only evaluates when the left side is none.
    let out = compile_and_run_stdout(
        r#"
fn loud() int {
    print("evaluated")
    return -1
}

fn main() {
    let a: int? = 1
    print(a ?? loud())
    let b: int? = none
    print(b ?? loud())
}
"#,
    );
    assert_eq!(out.trim(), "1\nevaluated\n-1");
}

// ── Flow narrowing ───────────────────────────────────────────────────────────

#[test]
fn narrow_in_then_branch() {
    let out = compile_and_run_stdout(
        r#"
fn describe(x: int?) string {
    if x != none {
        let doubled = x + x
        return f"value {doubled}"
    }
    return "nothing"
}

fn main() {
    print(describe(5))
    print(describe(none))
}
"#,
    );
    assert_eq!(out.trim(), "value 10\nnothing");
}

#[test]
fn narrow_in_else_branch() {
    let out = compile_and_run_stdout(
        r#"
fn bump(x: int?) int {
    if x == none {
        return 0
    } else {
        return x + 1
    }
}

fn main() {
    print(bump(9))
    print(bump(none))
}
"#,
    );
    assert_eq!(out.trim(), "10\n0");
}

#[test]
fn guard_idiom_narrows_rest_of_block() {
    let out = compile_and_run_stdout(
        r#"
fn parse_or_flag(text: string) int {
    let n = text.to_int()
    if n == none {
        return -1
    }
    return n * 2
}

fn main() {
    print(parse_or_flag("21"))
    print(parse_or_flag("oops"))
}
"#,
    );
    assert_eq!(out.trim(), "42\n-1");
}

#[test]
fn inverted_guard_narrows_after_terminating_else() {
    let out = compile_and_run_stdout(
        r#"
fn take(x: int?) int {
    if x != none {
        // fallthrough with x narrowed below
    } else {
        return -1
    }
    return x + 100
}

fn main() {
    print(take(1))
    print(take(none))
}
"#,
    );
    assert_eq!(out.trim(), "101\n-1");
}

#[test]
fn narrow_heap_type_method_call() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let s: string? = "hey"
    if s != none {
        print(s.len())
    }
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn narrowing_invalidated_by_reassignment() {
    // After assigning a possibly-none value the variable is nullable again.
    compile_should_fail_with(
        r#"
fn main() {
    let mut x: int? = 5
    if x != none {
        x = none
        print(x + 1)
    }
}
"#,
        "operand type mismatch: int? vs int",
    );
}

#[test]
fn redundant_question_on_narrowed_still_works() {
    // The pre-narrowing idiom (check then `?`) keeps compiling.
    let out = compile_and_run_stdout(
        r#"
class Foo {
    x: int
}

fn main() {
    let f: Foo? = Foo { x: 10 }
    if f != none {
        print(f?.x)
        print(f.x)
    }
}
"#,
    );
    assert_eq!(out.trim(), "10\n10");
}

#[test]
fn no_narrowing_without_check() {
    // Outside a null check the variable stays nullable.
    compile_should_fail_with(
        r#"
fn main() {
    let x: int? = 5
    print(x + 1)
}
"#,
        "operand type mismatch: int? vs int",
    );
}

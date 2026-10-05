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

// ── Narrowed value types in non-I64 slots (#447) ─────────────────────────────
//
// Nullable value types are boxed; a flow-narrowed read unwraps the box. The
// unwrap's dead none-branch must still produce a value of the enclosing
// function's Cranelift return type — float (F64) and bool/byte (I8) used to
// fail verification where int (I64) happened to type-check.

#[test]
fn narrowed_float_guard_raise_then_return() {
    // Exact repro from #447.
    let out = compile_and_run_stdout(
        r#"
error E {}

fn f(s: string) float {
    let v = s.to_float()
    if v == none {
        raise E {}
    }
    return v
}

fn main() {
    print(f("1.5") catch 0.0)
    print(f("nope") catch 0.0)
}
"#,
    );
    assert_eq!(out.trim(), "1.5\n0");
}

#[test]
fn narrowed_float_guard_return_then_return() {
    let out = compile_and_run_stdout(
        r#"
fn f(s: string) float {
    let v = s.to_float()
    if v == none {
        return -1.0
    }
    return v
}

fn main() {
    print(f("2.25"))
    print(f("nope"))
}
"#,
    );
    assert_eq!(out.trim(), "2.25\n-1");
}

#[test]
fn narrowed_float_then_branch_use() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let v: float? = 3.5
    if v != none {
        print(v)
    }
}
"#,
    );
    assert_eq!(out.trim(), "3.5");
}

#[test]
fn narrowed_float_arithmetic_argument_and_field() {
    let out = compile_and_run_stdout(
        r#"
class Holder {
    val: float
}

fn twice(x: float) float {
    return x * 2.0
}

fn main() {
    let v: float? = 1.5
    if v != none {
        print(v + 1.0)
        print(twice(v))
        let h = Holder { val: v }
        print(h.val)
    }
}
"#,
    );
    assert_eq!(out.trim(), "2.5\n3\n1.5");
}

#[test]
fn narrowed_byte_guard_raise_and_uses() {
    let out = compile_and_run_stdout(
        r#"
error E {}

class Holder {
    val: byte
}

fn pick(b: byte?) byte {
    if b == none {
        raise E {}
    }
    return b
}

fn main() {
    let v: byte? = 42 as byte
    print(pick(v) catch 0 as byte)
    print(pick(none) catch 7 as byte)
    if v != none {
        let h = Holder { val: v }
        print(h.val)
    }
}
"#,
    );
    assert_eq!(out.trim(), "42\n7\n42");
}

#[test]
fn narrowed_bool_guard_raise_and_uses() {
    let out = compile_and_run_stdout(
        r#"
error E {}

class Holder {
    val: bool
}

fn flip(x: bool) bool {
    return !x
}

fn pick(b: bool?) bool {
    if b == none {
        raise E {}
    }
    return b
}

fn main() {
    let v: bool? = true
    print(pick(v) catch false)
    print(pick(none) catch false)
    if v != none {
        print(flip(v))
        let h = Holder { val: v }
        print(h.val)
    }
}
"#,
    );
    assert_eq!(out.trim(), "true\nfalse\nfalse\ntrue");
}

#[test]
fn narrowed_int_guard_raise_control_case() {
    // Control: the shape that already worked (I64 return slot).
    let out = compile_and_run_stdout(
        r#"
error E {}

fn f(s: string) int {
    let v = s.to_int()
    if v == none {
        raise E {}
    }
    return v
}

fn main() {
    print(f("15") catch 0)
    print(f("nope") catch 0)
}
"#,
    );
    assert_eq!(out.trim(), "15\n0");
}

// ── Narrowing through && / || (short-circuit, #450) ──────────────────────────

#[test]
fn narrow_in_rhs_of_and() {
    // `v != none` on the left of `&&` narrows v in the right operand: the
    // rhs only evaluates when the lhs is true.
    let out = compile_and_run_stdout(
        r#"
fn describe(v: int?) string {
    if v != none && v > 3 {
        return "big"
    }
    return "small or missing"
}

fn main() {
    print(describe(7))
    print(describe(1))
    print(describe(none))
}
"#,
    );
    assert_eq!(out.trim(), "big\nsmall or missing\nsmall or missing");
}

#[test]
fn narrow_in_rhs_of_or() {
    // De Morgan: the rhs of `||` only evaluates when the lhs is false, so
    // `v == none` on the left proves v non-none on the right.
    let out = compile_and_run_stdout(
        r#"
fn flag(v: int?) string {
    if v == none || v > 3 {
        return "missing or big"
    }
    return "small"
}

fn main() {
    print(flag(none))
    print(flag(7))
    print(flag(1))
}
"#,
    );
    assert_eq!(out.trim(), "missing or big\nmissing or big\nsmall");
}

#[test]
fn narrow_chained_conjuncts() {
    // Left-associative chain: both earlier conjuncts narrow in the last.
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let a: int? = 2
    let b: int? = 3
    if a != none && b != none && a + b > 4 {
        print(a + b)
    }
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn then_branch_keeps_all_conjunct_narrowings() {
    let out = compile_and_run_stdout(
        r#"
fn add(a: int?, b: int?) int {
    if a != none && b != none {
        return a + b
    }
    return -1
}

fn main() {
    print(add(20, 22))
    print(add(20, none))
    print(add(none, 22))
}
"#,
    );
    assert_eq!(out.trim(), "42\n-1\n-1");
}

#[test]
fn else_branch_narrows_all_disjuncts() {
    // `a == none || b == none` false means BOTH are non-none in the else.
    let out = compile_and_run_stdout(
        r#"
fn add(a: int?, b: int?) int {
    if a == none || b == none {
        return -1
    } else {
        return a + b
    }
}

fn main() {
    print(add(20, 22))
    print(add(none, 22))
}
"#,
    );
    assert_eq!(out.trim(), "42\n-1");
}

#[test]
fn disjunctive_guard_narrows_rest_of_block() {
    // The common early-exit idiom with two nullables at once.
    let out = compile_and_run_stdout(
        r#"
fn add(a: int?, b: int?) int {
    if a == none || b == none {
        return -1
    }
    return a + b
}

fn main() {
    print(add(20, 22))
    print(add(20, none))
}
"#,
    );
    assert_eq!(out.trim(), "42\n-1");
}

#[test]
fn narrow_in_while_condition_and_body() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let mut c: int? = 3
    while c != none && c > 0 {
        print(c)
        if c == 1 {
            c = none
        } else {
            c = c - 1
        }
    }
    print("done")
}
"#,
    );
    assert_eq!(out.trim(), "3\n2\n1\ndone");
}

#[test]
fn narrow_through_negated_none_check() {
    // `!(v == none)` is `v != none` — `!` flips the sense.
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let v: int? = 5
    if !(v == none) && v > 3 {
        print("big")
    }
}
"#,
    );
    assert_eq!(out.trim(), "big");
}

#[test]
fn narrow_in_and_float_and_string() {
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let f: float? = 2.5
    if f != none && f > 1.5 {
        print(f + 0.5)
    }
    let s: string? = "hey"
    if s != none && s.len() > 2 {
        print(s.len())
    }
}
"#,
    );
    assert_eq!(out.trim(), "3\n3");
}

#[test]
fn redundant_question_on_rhs_narrowed_still_works() {
    // The pre-narrowing idiom keeps compiling inside the rhs of `&&`.
    let out = compile_and_run_stdout(
        r#"
class Foo {
    x: int
}

fn main() {
    let f: Foo? = Foo { x: 10 }
    if f != none && f?.x > 3 {
        print(f.x)
    }
}
"#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn int_fact_guard_shape_with_index() {
    // The facts-engine twin of the nullable shape: a bound on the left of
    // `&&` guarding an index on the right (issue #450's facts note).
    let out = compile_and_run_stdout(
        r#"
fn main() {
    let xs = [10, 20, 30]
    let j = 1
    if j >= 0 && xs[j] > 15 {
        print("yes")
    }
}
"#,
    );
    assert_eq!(out.trim(), "yes");
}

#[test]
fn no_narrowing_in_lhs_of_and() {
    // Order matters: the FIRST operand evaluates unconditionally, so the
    // check on the right narrows nothing on the left.
    compile_should_fail_with(
        r#"
fn main() {
    let v: int? = 7
    if v > 3 && v != none {
        print("no")
    }
}
"#,
        "cannot compare int? with int",
    );
}

#[test]
fn and_narrows_only_the_checked_variable() {
    compile_should_fail_with(
        r#"
fn main() {
    let v: int? = 7
    let w: int? = 1
    if v != none && w > 3 {
        print("no")
    }
}
"#,
        "cannot compare int? with int",
    );
}

#[test]
fn or_with_none_check_true_sense_narrows_nothing() {
    // `v != none || v > 3`: the rhs runs when the check FAILED — v may be
    // none there, so no narrowing.
    compile_should_fail_with(
        r#"
fn main() {
    let v: int? = 7
    if v != none || v > 3 {
        print("no")
    }
}
"#,
        "cannot compare int? with int",
    );
}

#[test]
fn and_condition_false_does_not_narrow_else_branch() {
    // `v != none && w > 0` can be false with v non-none, so the else
    // branch must not treat v as narrowed.
    compile_should_fail_with(
        r#"
fn main() {
    let v: int? = 7
    let w = 1
    if v != none && w > 0 {
        print("yes")
    } else {
        print(v + 1)
    }
}
"#,
        "operand type mismatch: int? vs int",
    );
}

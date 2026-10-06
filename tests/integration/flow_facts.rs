//! Flow-fact engine (verification RFC phase 1): degenerate-condition
//! warnings from integer comparison facts, and the conservatism rules
//! (kills, loops, closures). See src/typeck/facts.rs.

/// Compile source and return warning messages.
fn warnings_for(source: &str) -> Vec<String> {
    match pluto::compile_to_object_with_warnings(source) {
        Ok((_obj, warnings)) => warnings.iter().map(|w| w.msg.clone()).collect(),
        Err(e) => panic!("Compilation failed unexpectedly: {e}"),
    }
}

fn assert_no_warnings(source: &str) {
    let w = warnings_for(source);
    assert!(w.is_empty(), "expected no warnings, got: {w:?}");
}

fn assert_single_warning(source: &str, expected: &str) {
    let w = warnings_for(source);
    assert_eq!(w.len(), 1, "expected exactly one warning, got: {w:?}");
    assert!(
        w[0].contains(expected),
        "expected warning containing '{expected}', got: {w:?}"
    );
}

// ── Always-false / always-true from branch facts ─────────────────────────────

#[test]
fn nested_condition_always_false() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x <= 3 {
        if x > 5 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(1))
}
"#,
        "condition is always false",
    );
}

#[test]
fn nested_condition_always_true() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x > 10 {
        if x > 5 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(1))
}
"#,
        "condition is always true",
    );
}

#[test]
fn else_branch_gets_negated_fact() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x > 5 {
        return 1
    } else {
        if x > 5 {
            print(x)
        }
    }
    return 3
}

fn main() {
    print(f(1))
}
"#,
        "condition is always false",
    );
}

#[test]
fn equality_fact_proves_comparison() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x == 7 {
        if x > 3 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(7))
}
"#,
        "condition is always true",
    );
}

#[test]
fn self_comparison_always_false() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x < x {
        return 1
    }
    return 0
}

fn main() {
    print(f(3))
}
"#,
        "condition is always false",
    );
}

// ── Guards whose failure path terminates ─────────────────────────────────────

#[test]
fn guard_with_return_narrows_rest_of_block() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x > 10 {
        return 0
    }
    if x <= 10 {
        return 1
    }
    return 2
}

fn main() {
    print(f(5))
}
"#,
        "condition is always true",
    );
}

#[test]
fn guard_with_raise_narrows_rest_of_block() {
    assert_single_warning(
        r#"
error TooBig {
    msg: string
}

fn f(x: int) int {
    if x > 100 {
        raise TooBig { msg: "too big" }
    }
    if x <= 100 {
        return 1
    }
    return 0
}

fn main() {
    print(f(5) catch 0)
}
"#,
        "condition is always true",
    );
}

#[test]
fn or_guard_narrows_via_de_morgan() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x < 0 || x > 10 {
        return 0
    }
    if x >= 0 {
        return 1
    }
    return 2
}

fn main() {
    print(f(5))
}
"#,
        "condition is always true",
    );
}

#[test]
fn and_decomposition_gives_both_facts() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x > 0 && x < 10 {
        if x > 20 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(5))
}
"#,
        "condition is always false",
    );
}

#[test]
fn and_guard_before_index_in_rhs_compiles_clean() {
    // #450 facts note: a bound on the left of `&&` guarding a use on the
    // right (`j >= 0 && xs[j] > v`). Type-checks and produces no
    // degenerate-condition warnings. (The arith-fit/shift elision scan
    // stays conservative inside a single `&&` expression — runtime checks
    // remain; this guards against regressions in the checked behavior.)
    assert_no_warnings(
        r#"
fn f(xs: [int], j: int, v: int) int {
    if j >= 0 && xs[j] > v {
        return 1
    }
    return 0
}

fn main() {
    print(f([10, 20, 30], 1, 15))
}
"#,
    );
}

// ── Variable-variable relations ──────────────────────────────────────────────

#[test]
fn relation_fact_refutes_reverse_comparison() {
    assert_single_warning(
        r#"
fn f(amt: int, balance: int) int {
    if amt <= balance {
        if amt > balance {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(1, 2))
}
"#,
        "condition is always false",
    );
}

// ── Linear arithmetic (constant coefficients) ────────────────────────────────

#[test]
fn affine_offset_condition_decided() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x <= 5 {
        if x + 1 > 7 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(1))
}
"#,
        "condition is always false",
    );
}

// ── Kills: reassignment ──────────────────────────────────────────────────────

#[test]
fn reassignment_kills_fact() {
    assert_no_warnings(
        r#"
fn f(mut x: int) int {
    if x > 10 {
        return 0
    }
    x = 50
    if x <= 10 {
        return 1
    }
    return 2
}

fn main() {
    print(f(5))
}
"#,
    );
}

#[test]
fn surviving_branch_kill_is_not_resurrected() {
    // then_facts (x > 5) would flow past the if because the else terminates,
    // but the surviving branch reassigned x — the guard fact must stay dead.
    assert_no_warnings(
        r#"
fn f(mut x: int) int {
    if x > 5 {
        x = 0
    } else {
        return 0
    }
    if x <= 5 {
        return 1
    }
    return 2
}

fn main() {
    print(f(9))
}
"#,
    );
}

// ── Kills: method calls and field facts ──────────────────────────────────────

#[test]
fn field_fact_decides_condition() {
    assert_single_warning(
        r#"
class Point {
    x: int
}

fn main() {
    let p = Point { x: 1 }
    if p.x > 5 {
        if p.x <= 5 {
            print(0)
        }
    }
    print(p.x)
}
"#,
        "condition is always false",
    );
}

#[test]
fn method_call_kills_field_fact() {
    assert_no_warnings(
        r#"
class Account {
    balance: int

    fn drain(mut self) {
        self.balance = 0
    }
}

fn seed() int {
    return 5
}

fn main() {
    // Opaque initializer: a literal would make the guard below degenerate
    // (construction facts are transparent).
    let mut a = Account { balance: seed() }
    if a.balance <= 10 {
        a.drain()
        if a.balance > 10 {
            print(1)
        }
    }
    print(a.balance)
}
"#,
    );
}

#[test]
fn field_assignment_kills_field_fact() {
    assert_no_warnings(
        r#"
class Point {
    x: int
}

fn seed() int {
    return 1
}

fn main() {
    let mut p = Point { x: seed() }
    if p.x <= 5 {
        p.x = 100
        if p.x > 5 {
            print(1)
        }
    }
    print(p.x)
}
"#,
    );
}

// ── Conservatism: loops ──────────────────────────────────────────────────────

#[test]
fn loop_entry_drops_facts() {
    // x is never reassigned, so the fact would actually survive — but phase 1
    // is deliberately conservative and drops all facts at loop entry.
    assert_no_warnings(
        r#"
fn f(x: int) int {
    if x > 10 {
        return 0
    }
    let mut i = 0
    while i < 3 {
        i = i + 1
        if x <= 10 {
            print(x)
        }
    }
    return x
}

fn main() {
    print(f(5))
}
"#,
    );
}

#[test]
fn guard_inside_loop_body_still_narrows() {
    // Facts established inside the body hold for the rest of that iteration:
    // a continue-guard behaves like a return-guard.
    assert_single_warning(
        r#"
fn f(n: int) int {
    let mut total = 0
    for i in 0..n {
        if i > 2 {
            continue
        }
        if i <= 2 {
            total = total + i
        }
    }
    return total
}

fn main() {
    print(f(5))
}
"#,
        "condition is always true",
    );
}

// ── Conservatism: Unknown stays silent ───────────────────────────────────────

#[test]
fn unrelated_condition_no_warning() {
    assert_no_warnings(
        r#"
fn f(x: int, y: int) int {
    if x > y {
        return 1
    }
    return 0
}

fn main() {
    print(f(1, 2))
}
"#,
    );
}

#[test]
fn undecided_bound_no_warning() {
    assert_no_warnings(
        r#"
fn f(x: int) int {
    if x <= 10 {
        if x > 5 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(7))
}
"#,
    );
}

#[test]
fn literal_true_condition_no_warning() {
    // Constant scaffolding (`if true`) is never warned about — the engine
    // only speaks when a tracked variable is involved.
    assert_no_warnings(
        r#"
fn main() {
    if true {
        print(1)
    }
}
"#,
    );
}

#[test]
fn condition_with_call_no_facts_no_warning() {
    // A call in the condition could mutate state: no facts are extracted
    // and nothing is decided.
    assert_no_warnings(
        r#"
fn limit() int {
    return 10
}

fn f(x: int) int {
    if x > limit() {
        return 0
    }
    if x <= 10 {
        return 1
    }
    return 2
}

fn main() {
    print(f(5))
}
"#,
    );
}

// ── Conservatism: closures ───────────────────────────────────────────────────

#[test]
fn outer_field_facts_do_not_leak_into_closure() {
    // The closure runs later; outer field facts must not decide conditions
    // inside its body.
    assert_no_warnings(
        r#"
class Point {
    x: int
}

fn seed() int {
    return 1
}

fn main() {
    let p = Point { x: seed() }
    if p.x > 5 {
        let f = () => {
            if p.x <= 5 {
                print(0)
            }
            return 1
        }
        print(f())
    }
    print(p.x)
}
"#,
    );
}

// ── Nullable narrowing unregressed ───────────────────────────────────────────

#[test]
fn nullable_guard_then_int_facts_compose() {
    // The none-guard narrows x to int; comparison facts then apply to the
    // narrowed variable without disturbing the nullable machinery.
    assert_no_warnings(
        r#"
fn f(x: int?) int {
    if x == none {
        return 0
    }
    if x > 5 {
        return 1
    }
    return 2
}

fn main() {
    print(f(7))
}
"#,
    );
}

#[test]
fn narrowed_nullable_gets_comparison_facts() {
    assert_single_warning(
        r#"
fn f(x: int?) int {
    if x == none {
        return 0
    }
    if x > 10 {
        return 1
    }
    if x <= 10 {
        return 2
    }
    return 3
}

fn main() {
    print(f(7))
}
"#,
        "condition is always true",
    );
}

// ── Warnings never block compilation ─────────────────────────────────────────

#[test]
fn degenerate_condition_still_compiles_and_runs() {
    let result = pluto::compile_to_object_with_warnings(
        r#"
fn f(x: int) int {
    if x <= 3 {
        if x > 5 {
            return 1
        }
    }
    return 0
}

fn main() {
    print(f(1))
}
"#,
    );
    let (obj, warnings) = result.expect("compilation should succeed despite warnings");
    assert!(!obj.is_empty(), "object bytes should not be empty");
    assert_eq!(warnings.len(), 1, "expected exactly one warning: {warnings:?}");
}

// ── Chained comparisons (#451) ───────────────────────────────────────────────

#[test]
fn chain_decomposition_gives_both_facts() {
    // `0 < x < 10` narrows like `x > 0 && x < 10`: both pair facts hold
    // in the then-branch.
    assert_single_warning(
        r#"
fn f(x: int) int {
    if 0 < x < 10 {
        if x > 20 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(5))
}
"#,
        "condition is always false",
    );
}

#[test]
fn chain_bounds_guard_narrows_index() {
    // The issue's motivating guard: `0 <= i < xs.len()` establishes both
    // `i >= 0` and the `i < xs.len()` relation, so the inner re-check is
    // decided.
    assert_single_warning(
        r#"
fn f(xs: [int], i: int) int {
    if 0 <= i < xs.len() {
        if i >= 0 {
            return xs[i]
        }
        return -2
    }
    return -1
}

fn main() {
    print(f([10, 20, 30], 1))
}
"#,
        "condition is always true",
    );
}

#[test]
fn chain_condition_decided_by_facts() {
    // eval_condition understands a chain as the conjunction of its pairs:
    // under x == 5, `0 <= x < 10` is proven.
    assert_single_warning(
        r#"
fn f(x: int) int {
    if x == 5 {
        if 0 <= x < 10 {
            return 1
        }
        return 2
    }
    return 3
}

fn main() {
    print(f(5))
}
"#,
        "condition is always true",
    );
}

// ── Length terms (`xs.len()`) ────────────────────────────────────────────────

#[test]
fn len_comparison_narrows() {
    // i < xs.len() establishes a relation; the affine form xs.len() - 1
    // participates, so i <= xs.len() - 1 is decided.
    assert_single_warning(
        r#"
fn f(xs: [int], i: int) int {
    if i < xs.len() {
        if i <= xs.len() - 1 {
            return 1
        }
        return 2
    }
    return 0
}

fn main() {
    print(f([1, 2], 0))
}
"#,
        "condition is always true",
    );
}

#[test]
fn len_automatic_nonnegative_is_always_true() {
    assert_single_warning(
        r#"
fn f(xs: [int]) int {
    if xs.len() >= 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f([1]))
}
"#,
        "condition is always true",
    );
}

#[test]
fn len_negative_is_always_false() {
    assert_single_warning(
        r#"
fn f(s: string) int {
    if s.len() < 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f("abc"))
}
"#,
        "condition is always false",
    );
}

#[test]
fn map_and_set_len_terms_work() {
    assert_single_warning(
        r#"
fn f(m: Map<string, int>) int {
    if m.len() >= 0 {
        return 1
    }
    return 0
}

fn main() {
    let m = Map<string, int> { "a": 1 }
    print(f(m))
}
"#,
        "condition is always true",
    );
}

#[test]
fn mut_method_call_kills_len_fact() {
    // push between the two checks may change the length: no warning.
    assert_no_warnings(
        r#"
fn f(mut xs: [int], i: int) int {
    if i < xs.len() {
        xs.push(9)
        if i < xs.len() {
            return 1
        }
        return 2
    }
    return 0
}

fn main() {
    print(f([1, 2], 0))
}
"#,
    );
}

#[test]
fn reassignment_kills_len_fact() {
    assert_no_warnings(
        r#"
fn f(i: int) int {
    let mut xs = [1, 2, 3, 4, 5]
    if 3 < xs.len() {
        xs = [1]
        if 3 < xs.len() {
            return 1
        }
        return 2
    }
    return 0
}

fn main() {
    print(f(0))
}
"#,
    );
}

#[test]
fn unrelated_call_kills_len_fact() {
    // A call may reach the array through an alias and mutate it: the
    // conservative kill drops the length fact.
    assert_no_warnings(
        r#"
fn g() int {
    return 1
}

fn f(xs: [int]) int {
    if 3 < xs.len() {
        let v = g()
        if 3 < xs.len() {
            return v
        }
        return 2
    }
    return 0
}

fn main() {
    print(f([1, 2, 3, 4, 5]))
}
"#,
    );
}

#[test]
fn pure_reads_do_not_kill_len_fact() {
    // Indexing and len() itself are not calls: the fact survives.
    assert_single_warning(
        r#"
fn f(xs: [int]) int {
    if 3 < xs.len() {
        let v = xs[0]
        if 3 < xs.len() {
            return v
        }
        return 2
    }
    return 0
}

fn main() {
    print(f([1, 2, 3, 4, 5]))
}
"#,
        "condition is always true",
    );
}

#[test]
fn len_binding_transfers_nonnegative_fact() {
    assert_single_warning(
        r#"
fn f(s: string) int {
    let n = s.len()
    if n >= 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f("abc"))
}
"#,
        "condition is always true",
    );
}

#[test]
fn len_binding_equality_relates_to_term() {
    assert_single_warning(
        r#"
fn f(s: string) int {
    let n = s.len()
    if n == s.len() {
        return 1
    }
    return 0
}

fn main() {
    print(f("abc"))
}
"#,
        "condition is always true",
    );
}

#[test]
fn len_binding_fact_killed_by_reassignment() {
    assert_no_warnings(
        r#"
fn f(s: string) int {
    let mut n = s.len()
    n = 0 - 1
    if n >= 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f("abc"))
}
"#,
    );
}

#[test]
fn class_len_method_is_not_a_length_term() {
    // A user-defined `len` method is an ordinary call: no automatic facts,
    // and it stays effectful (no extraction from the condition).
    assert_no_warnings(
        r#"
class Weird {
    n: int

    fn len(self) int {
        return self.n
    }
}

fn f(w: Weird) int {
    if w.len() >= 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f(Weird { n: 0 - 5 }))
}
"#,
    );
}

// ── Field paths mixed with length terms ──────────────────────────────────────

#[test]
fn field_path_relates_to_len_term() {
    assert_single_warning(
        r#"
class Grant {
    token: int
}

fn f(g: Grant, xs: [int]) int {
    if g.token < xs.len() {
        if g.token <= xs.len() - 1 {
            return 1
        }
        return 2
    }
    return 0
}

fn main() {
    print(f(Grant { token: 0 }, [1, 2]))
}
"#,
        "condition is always true",
    );
}

// ── Phase 4.5 precision: purity-aware kills + construction transparency ─────

#[test]
fn construction_facts_make_guard_degenerate() {
    // Construction is transparent: the literal's exact value decides the
    // guard, and the engine says so.
    assert_single_warning(
        r#"
class Point {
    x: int
}

fn main() {
    let p = Point { x: 1 }
    if p.x <= 5 {
        print(1)
    }
    print(p.x)
}
"#,
        "condition is always true",
    );
}

#[test]
fn print_does_not_kill_field_fact() {
    // print() provably cannot reach the object (its argument is an int
    // value), so the guard fact survives it and still decides the inner
    // condition.
    assert_single_warning(
        r#"
class Point {
    x: int
}

fn seed() int {
    return 1
}

fn main() {
    let p = Point { x: seed() }
    if p.x <= 5 {
        print(p.x)
        if p.x <= 5 {
            print(2)
        }
    }
}
"#,
        "condition is always true",
    );
}

#[test]
fn reach_free_fn_does_not_kill_field_fact() {
    // A free function whose declared params cannot reach any class value
    // cannot invalidate field facts.
    assert_single_warning(
        r#"
class Point {
    x: int
}

fn seed() int {
    return 1
}

fn noop(v: int) int {
    return v
}

fn main() {
    let p = Point { x: seed() }
    if p.x <= 5 {
        let y = noop(3)
        if p.x <= 5 {
            print(y)
        }
    }
}
"#,
        "condition is always true",
    );
}

#[test]
fn class_taking_fn_still_kills_field_fact() {
    // A free function whose declared parameter is class-typed may hold an
    // alias — the kill still fires (the conservative negative).
    assert_no_warnings(
        r#"
class Point {
    x: int
}

fn seed() int {
    return 1
}

fn poke(q: Point) int {
    return q.x
}

fn main() {
    let p = Point { x: seed() }
    if p.x <= 5 {
        let y = poke(p)
        if p.x <= 5 {
            print(y)
        }
    }
}
"#,
    );
}

#[test]
fn reach_free_fn_still_kills_len_fact() {
    // A reach-free user function may still mutate a collection through an
    // alias in principle (Collections severity) — length terms die.
    assert_no_warnings(
        r#"
fn noop() int {
    return 0
}

fn f(xs: [int]) int {
    if 3 < xs.len() {
        let v = noop()
        if 3 < xs.len() {
            return v
        }
        return 2
    }
    return 0
}

fn main() {
    print(f([1, 2, 3, 4, 5]))
}
"#,
    );
}

#[test]
fn bytes_len_fact_narrows() {
    // bytes len() is a first-class fact term: under b.len() >= 4, the nested
    // b.len() >= 2 is decided.
    assert_single_warning(
        r#"
fn f(b: bytes) int {
    if b.len() >= 4 {
        if b.len() >= 2 {
            return 1
        }
        return 2
    }
    return 0
}

fn main() {
    let mut b = bytes_new()
    b.push(1 as byte)
    print(f(b))
}
"#,
        "condition is always true",
    );
}

// ── While guard facts (#416 phase 2) ─────────────────────────────────────────
//
// The loop guard holds at every body entry (it was just evaluated true), so
// its facts are assumed inside the body — after loop havoc, and killed by
// body reassignments as usual.

#[test]
fn while_guard_fact_decides_body_condition() {
    assert_single_warning(
        r#"
fn main() {
    let mut i = 0
    while i < 10 {
        if i < 20 {
            print(i)
        }
        i = i + 1
    }
}
"#,
        "condition is always true",
    );
}

#[test]
fn while_guard_fact_killed_by_body_reassignment() {
    // The reassignment kills the guard fact before the condition, so no
    // degenerate-condition verdict is possible.
    assert_no_warnings(
        r#"
fn main() {
    let mut i = 0
    while i < 10 {
        i = i + 15
        if i < 20 {
            print(i)
        }
    }
}
"#,
    );
}

#[test]
fn while_guard_impure_condition_contributes_no_facts() {
    assert_no_warnings(
        r#"
class Box {
    v: int

    fn below(mut self, n: int) bool {
        self.v = self.v + 0
        return self.v < n
    }
}

fn main() {
    let mut b = Box { v: 0 }
    while b.below(10) {
        if b.v < 20 {
            print(b.v)
        }
        b.v = b.v + 1
    }
}
"#,
    );
}

// ── Assert as a fact source ──────────────────────────────────────────────────
// `assert <cond>` aborts the process when false, so the condition holds for
// the rest of the enclosing block — the terminating-guard rule, with the
// same extraction path and impure-call exclusion.

#[test]
fn assert_fact_proves_later_condition() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    assert x >= 5
    if x >= 5 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7))
}
"#,
        "condition is always true",
    );
}

#[test]
fn assert_conjunction_decomposes() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    assert x >= 0 && x < 64
    if x > 100 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7))
}
"#,
        "condition is always false",
    );
}

#[test]
fn assert_equality_decides_comparison() {
    assert_single_warning(
        r#"
fn f(x: int) int {
    assert x == 7
    if x > 3 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7))
}
"#,
        "condition is always true",
    );
}

#[test]
fn assert_containing_call_contributes_nothing() {
    // The call could mutate state between evaluation and use — the whole
    // condition is excluded, including its pure conjunct.
    assert_no_warnings(
        r#"
fn limit() int {
    return 64
}

fn f(x: int) int {
    assert x >= 0 && x < limit()
    if x >= 0 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7))
}
"#,
    );
}

#[test]
fn assert_fact_killed_by_reassignment() {
    assert_no_warnings(
        r#"
fn f(n: int) int {
    let mut x = n
    assert x >= 5
    x = n
    if x >= 5 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7))
}
"#,
    );
}

#[test]
fn assert_fact_pops_with_enclosing_branch() {
    assert_no_warnings(
        r#"
fn f(x: int, n: int) int {
    if n > 0 {
        assert x >= 5
    }
    if x >= 5 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7, 1))
}
"#,
    );
}

#[test]
fn assert_fact_dropped_at_loop_entry() {
    // Loop havoc applies to assert facts like any other: the fact does not
    // survive into the body (phase 1 conservatism).
    assert_no_warnings(
        r#"
fn f(x: int) int {
    assert x >= 5
    let mut i = 0
    while i < 3 {
        if x >= 5 {
            i = i + 1
        }
        i = i + 1
    }
    return i
}

fn main() {
    print(f(7))
}
"#,
    );
}

// ── Byte widening: `let x = b as int` starts bounded 0..255 ─────────────────

#[test]
fn byte_cast_binding_carries_byte_range() {
    assert_single_warning(
        r#"
fn f(b: byte) int {
    let x = b as int
    if x < 256 {
        return 1
    }
    return 0
}

fn main() {
    print(f(7 as byte))
}
"#,
        "condition is always true",
    );
}

#[test]
fn int_cast_binding_of_nonbyte_carries_nothing() {
    // `bool as int` is NOT a byte widening — no [0, 255] fact may appear
    // (it happens to be 0..1, but only the byte rule is implemented, and a
    // wrong-source fact would be a soundness hole for future rules).
    assert_no_warnings(
        r#"
fn f(flag: bool) int {
    let x = flag as int
    if x < 256 {
        return 1
    }
    return 0
}

fn main() {
    print(f(true))
}
"#,
    );
}

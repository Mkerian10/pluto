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

fn main() {
    let mut a = Account { balance: 5 }
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

fn main() {
    let mut p = Point { x: 1 }
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

fn main() {
    let p = Point { x: 1 }
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

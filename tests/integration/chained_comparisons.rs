// Chained comparisons (#451, spec/src/expressions.md "Comparison Chaining"):
// `a < b < c` means `a < b && b < c` with `b` evaluated exactly once, and
// later operands/comparisons short-circuited when an earlier link fails.
// Chaining groups a *syntactic* run of comparison operators from one
// precedence level (equality `==`/`!=`, or relational `<`/`<=`/`>`/`>=`);
// a parenthesized comparison is an ordinary operand and never re-chains.

mod common;
use common::*;

// ============================================================
// The spec's own examples
// ============================================================

#[test]
fn spec_example_range_check() {
    // 0 < x <= 100  ≡  0 < x && x <= 100
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            print(0 < x <= 100)
            let y = 0
            print(0 < y <= 100)
            let z = 101
            print(0 < z <= 100)
            let w = 100
            print(0 < w <= 100)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse\nfalse\ntrue");
}

#[test]
fn spec_example_equality_chain() {
    // a == b == c  ≡  a == b && b == c
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 1
            let b = 1
            let c = 1
            print(a == b == c)
            print(a == b == 2)
            print(a == 7 == 7)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse\nfalse");
}

#[test]
fn spec_example_mixed_directions() {
    // x >= 0 < y  ≡  x >= 0 && 0 < y (unusual but valid)
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 5
            let y = 3
            print(x >= 0 < y)
            let z = 0
            print(x >= 0 < z)
            print(-1 >= 0 < y)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse\nfalse");
}

// ============================================================
// Evaluation order: middle operand evaluated once, short-circuit
// ============================================================

#[test]
fn middle_operand_evaluated_once() {
    // `b` is a call with a print side effect: it must appear exactly once
    // even though it participates in two comparisons.
    let stdout = compile_and_run_stdout(r#"
        fn side(x: int) int {
            print("eval")
            return x
        }

        fn main() {
            print(1 < side(2) < 3)
        }
    "#);
    assert_eq!(stdout.trim(), "eval\ntrue");
}

#[test]
fn short_circuit_skips_later_operands() {
    // First link fails (1 < 0 is false): side(99) must not run.
    let stdout = compile_and_run_stdout(r#"
        fn side(x: int) int {
            print("evaluated")
            return x
        }

        fn main() {
            print(1 < 0 < side(99))
        }
    "#);
    assert_eq!(stdout.trim(), "false");
}

#[test]
fn short_circuit_runs_operands_before_failure_point() {
    // Operands evaluate left to right: a, b, compare; only if true, c.
    // Here the first link fails after side("a") and side("b") ran.
    let stdout = compile_and_run_stdout(r#"
        fn side(tag: string, x: int) int {
            print(tag)
            return x
        }

        fn main() {
            print(side("a", 9) < side("b", 2) < side("c", 5))
        }
    "#);
    assert_eq!(stdout.trim(), "a\nb\nfalse");
}

#[test]
fn four_element_chain() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 1
            let b = 2
            let c = 3
            print(a <= b < c <= 3)
            print(a <= b < c <= 2)
            print(a <= a < c <= 3)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse\ntrue");
}

#[test]
fn four_element_chain_evaluates_each_operand_once_in_order() {
    let stdout = compile_and_run_stdout(r#"
        fn side(tag: string, x: int) int {
            print(tag)
            return x
        }

        fn main() {
            print(side("a", 1) < side("b", 2) < side("c", 3) < side("d", 4))
        }
    "#);
    assert_eq!(stdout.trim(), "a\nb\nc\nd\ntrue");
}

// ============================================================
// Operand types
// ============================================================

#[test]
fn float_chain() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let x = 2.5
            print(1.0 < x <= 2.5)
            print(1.0 < x < 2.5)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse");
}

#[test]
fn byte_chain_compares_unsigned() {
    // Bytes are unsigned: 0xFF is 255, not -1, so it sits at the top.
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let lo = (0x01).to_byte()
            let mid = (0x7F).to_byte()
            let hi = (0xFF).to_byte()
            print(lo < mid < hi)
            print(hi > mid > lo)
            print(lo < hi < mid)
        }
    "#);
    assert_eq!(stdout.trim(), "true\ntrue\nfalse");
}

#[test]
fn string_equality_chain() {
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = "x"
            let b = "x"
            print(a == b == "x")
            print(a == b == "y")
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse");
}

// ============================================================
// `>` chains vs `>>` (the generics-driven two-token lexing)
// ============================================================

#[test]
fn gt_chain_does_not_collide_with_shr() {
    // `>>` is lexed as two adjacent `>` tokens (for nested generics);
    // separated `>` tokens chain, adjacent ones stay a right shift.
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 3
            let b = 2
            let c = 1
            print(a > b > c)
            print(c > b > a)
            print(a >> c)
            print(16 >> 2 > 3)
        }
    "#);
    // 16 >> 2 = 4 > 3 → true (shift binds tighter than comparison)
    assert_eq!(stdout.trim(), "true\nfalse\n1\ntrue");
}

#[test]
fn nested_generics_still_parse_after_chaining() {
    let stdout = compile_and_run_stdout(r#"
        class Box<T> {
            value: T

            fn get(self) T {
                return self.value
            }
        }

        fn main() {
            let inner = Box<int> { value: 7 }
            let outer = Box<Box<int>> { value: inner }
            let x = outer.get().get()
            print(0 < x < 10)
        }
    "#);
    assert_eq!(stdout.trim(), "true");
}

// ============================================================
// Grouping: parens opt out, mixed levels do not chain
// ============================================================

#[test]
fn parenthesized_comparison_is_an_ordinary_operand() {
    // (a == b) == c compares a bool result with a bool — no chaining
    // through parens.
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let a = 1
            let b = 2
            let c = false
            print((a == b) == c)
            print((a == b) == true)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse");
}

#[test]
fn equality_and_relational_do_not_cross_chain() {
    // `flag == x < y` keeps the precedence-table reading
    // `flag == (x < y)` (equality binds looser than relational).
    let stdout = compile_and_run_stdout(r#"
        fn main() {
            let flag = true
            let x = 1
            let y = 2
            print(flag == x < y)
            print(flag == y < x)
        }
    "#);
    assert_eq!(stdout.trim(), "true\nfalse");
}

// ============================================================
// Flow facts: a chain guard narrows like the equivalent conjunction
// ============================================================

#[test]
fn chain_bounds_guard_indexes_safely() {
    let stdout = compile_and_run_stdout(r#"
        fn get(xs: [int], i: int) int {
            if 0 <= i < xs.len() {
                return xs[i]
            }
            return -1
        }

        fn main() {
            let xs = [10, 20, 30]
            print(get(xs, 1))
            print(get(xs, -1))
            print(get(xs, 3))
        }
    "#);
    assert_eq!(stdout.trim(), "20\n-1\n-1");
}

// ============================================================
// Negative: neighbor pairs must be comparable under existing rules
// ============================================================

#[test]
fn chain_with_incomparable_neighbors_rejected() {
    compile_should_fail_with(
        r#"
        fn main() {
            let x = 1
            print(0 < x < 2.5)
        }
    "#,
        "cannot compare int with float",
    );
}

#[test]
fn chain_comparison_on_strings_rejected() {
    compile_should_fail_with(
        r#"
        fn main() {
            print("a" < "b" < "c")
        }
    "#,
        "comparison not supported for type string",
    );
}

#[test]
fn equality_chain_type_mismatch_rejected() {
    compile_should_fail_with(
        r#"
        fn main() {
            let a = 1
            print(a == a == "one")
        }
    "#,
        "cannot compare int with string",
    );
}

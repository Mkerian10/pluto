// Error-set shrinking (verification RFC phase 3, docs/design/rfc-verification.md):
// a `raise` proven unreachable at a call site by the caller's flow facts
// removes the variant from that site's *required-handling* set. The callee's
// canonical inferred error set is unchanged — handling a provably-impossible
// error stays legal.

mod common;
use common::{compile_and_run_stdout, compile_should_fail_with};

/// The RFC's motivating example: a guard that refutes the callee's raise
/// condition removes the handling obligation at that site.
#[test]
fn guarded_method_call_needs_no_handling() {
    let out = compile_and_run_stdout(
        r#"
        error Insufficient {
            needed: int
        }

        class Account {
            balance: int

            fn withdraw(mut self, amt: int)
            requires amt > 0
            {
                if amt > self.balance {
                    raise Insufficient { needed: amt }
                }
                self.balance = self.balance - amt
            }
        }

        fn main() {
            let mut account = Account { balance: 100 }
            let amt = 30
            if amt <= account.balance {
                account.withdraw(amt)
                print(account.balance)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "70");
}

/// The same call without the guard still requires handling.
#[test]
fn unguarded_method_call_still_requires_handling() {
    compile_should_fail_with(
        r#"
        error Insufficient {
            needed: int
        }

        class Account {
            balance: int

            fn withdraw(mut self, amt: int) {
                if amt > self.balance {
                    raise Insufficient { needed: amt }
                }
                self.balance = self.balance - amt
            }
        }

        fn main() {
            let mut account = Account { balance: 100 }
            account.withdraw(30)
        }
        "#,
        "call to fallible method 'withdraw' must be handled",
    );
}

/// An interleaved mut call between the guard and the call kills the
/// receiver's field facts: the site requires handling again.
#[test]
fn interleaved_mut_call_invalidates_guard() {
    compile_should_fail_with(
        r#"
        error Insufficient {
            needed: int
        }

        class Account {
            balance: int

            fn withdraw(mut self, amt: int) {
                if amt > self.balance {
                    raise Insufficient { needed: amt }
                }
                self.balance = self.balance - amt
            }
        }

        fn main() {
            let mut account = Account { balance: 100 }
            let amt = 30
            if amt <= account.balance {
                account.withdraw(90) catch err {
                    print(0)
                }
                account.withdraw(amt)
            }
        }
        "#,
        "call to fallible method 'withdraw' must be handled",
    );
}

/// Shrinking works for free functions, from guard facts on the argument.
#[test]
fn guarded_free_function_call_needs_no_handling() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn main() {
            let a = 21
            if a >= 0 {
                print(double_checked(a))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "42");
}

/// A constant argument that refutes the guard shrinks the site too.
#[test]
fn constant_argument_refutes_guard() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn main() {
            print(double_checked(5))
        }
        "#,
    );
    assert_eq!(out.trim(), "10");
}

/// The early-return guard idiom establishes the fact for the rest of the
/// block, so the later call needs no handling.
#[test]
fn early_return_guard_shrinks_later_call() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn run(a: int) {
            if a < 0 {
                return
            }
            print(double_checked(a))
        }

        fn main() {
            run(7)
        }
        "#,
    );
    assert_eq!(out.trim(), "14");
}

/// Reassigning the guarded variable between guard and call kills its facts.
#[test]
fn reassignment_invalidates_guard() {
    compile_should_fail_with(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn main() {
            let mut a = 5
            if a >= 0 {
                a = a - 10
                print(double_checked(a))
            }
        }
        "#,
        "call to fallible function 'double_checked' must be handled",
    );
}

/// Loop entry drops all facts: a guarded fact does not survive into the body.
#[test]
fn loop_entry_invalidates_guard() {
    compile_should_fail_with(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn main() {
            let a = 5
            if a >= 0 {
                for i in 0..3 {
                    print(double_checked(a))
                }
            }
        }
        "#,
        "call to fallible function 'double_checked' must be handled",
    );
}

/// A variant with several raise sites only shrinks when every site's guard
/// is refuted.
#[test]
fn multi_raise_site_needs_all_guards_refuted() {
    // Only the lower bound is established: the x > 100 site is not refuted.
    compile_should_fail_with(
        r#"
        error OutOfRange {}

        fn clamp_checked(x: int) int {
            if x < 0 {
                raise OutOfRange {}
            }
            if x > 100 {
                raise OutOfRange {}
            }
            return x
        }

        fn main() {
            let a = 50
            if a >= 0 {
                print(clamp_checked(a))
            }
        }
        "#,
        "call to fallible function 'clamp_checked' must be handled",
    );
}

/// With both raise sites refuted, the variant shrinks.
#[test]
fn multi_raise_site_shrinks_when_all_refuted() {
    let out = compile_and_run_stdout(
        r#"
        error OutOfRange {}

        fn clamp_checked(x: int) int {
            if x < 0 {
                raise OutOfRange {}
            }
            if x > 100 {
                raise OutOfRange {}
            }
            return x
        }

        fn main() {
            let a = 50
            if a >= 0 && a <= 100 {
                print(clamp_checked(a))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "50");
}

/// An unguarded raise site poisons its variant: no amount of caller facts
/// shrinks it.
#[test]
fn unguarded_raise_site_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Bad {}

        fn always_bad_eventually(x: int) int {
            if x < 0 {
                raise Bad {}
            }
            if x == 0 {
                return 0
            }
            raise Bad {}
        }

        fn main() {
            let a = 5
            if a >= 0 {
                print(always_bad_eventually(a))
            }
        }
        "#,
        "call to fallible function 'always_bad_eventually' must be handled",
    );
}

/// A variant that (also) arrives via propagation (`!` from a transitive
/// callee) never shrinks, even when the direct raise's guard is refuted.
#[test]
fn propagated_variant_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Bad {}

        fn inner() {
            raise Bad {}
        }

        fn outer(x: int) int {
            if x < 0 {
                raise Bad {}
            }
            inner()!
            return x
        }

        fn main() {
            let a = 5
            if a >= 0 {
                print(outer(a))
            }
        }
        "#,
        "call to fallible function 'outer' must be handled",
    );
}

/// Handling a provably-impossible error stays legal: `catch` (shorthand,
/// wildcard, typed) and `!` on a shrunk-to-empty site still compile.
#[test]
fn redundant_handling_on_shrunk_site_still_compiles() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn fallible_wrapper(a: int) int {
            if a < 0 {
                return 0
            }
            return double_checked(a)!
        }

        fn main() {
            let a = 5
            if a >= 0 {
                let w = double_checked(a) catch -1
                let x = double_checked(a) catch err {
                    -1
                }
                let y = double_checked(a) catch err: Negative {
                    -1
                }
                print(w + x + y)
            }
            print(fallible_wrapper(a) catch -1)
        }
        "#,
    );
    assert_eq!(out.trim(), "30\n10");
}

/// Typed-catch coverage uses the shrunk set: a handler for the remaining
/// variant suffices once the guarded variant is proven impossible here.
#[test]
fn typed_catch_coverage_uses_shrunk_set() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}
        error Odd {}

        fn half_checked(x: int, strict: bool) int {
            if x < 0 {
                raise Negative {}
            }
            if strict {
                raise Odd {}
            }
            return x / 2
        }

        fn main() {
            let a = 8
            if a >= 0 {
                let r = half_checked(a, false) catch err: Odd {
                    -1
                }
                print(r)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "4");
}

/// Without the guard, the same typed catch fails coverage — the baseline the
/// previous test narrows from.
#[test]
fn typed_catch_coverage_full_set_without_guard() {
    compile_should_fail_with(
        r#"
        error Negative {}
        error Odd {}

        fn half_checked(x: int, strict: bool) int {
            if x < 0 {
                raise Negative {}
            }
            if strict {
                raise Odd {}
            }
            return x / 2
        }

        fn main() {
            let a = 8
            let r = half_checked(a, false) catch err: Odd {
                -1
            }
            print(r)
        }
        "#,
        "can raise 'Negative', which no catch handler covers",
    );
}

/// Parameter-only guards shrink calls on entities too (entity methods
/// serialize; the argument's entry value is the caller's local fact).
#[test]
fn entity_method_shrinks_on_param_guard() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        object Counter {
            n: int

            fn add(mut self, d: int) {
                if d < 0 {
                    raise Negative {}
                }
                self.n = self.n + d
            }

            fn get(self) int {
                return self.n
            }
        }

        fn main() {
            let mut c = Counter { n: 0 }
            let d = 5
            if d >= 0 {
                c.add(d)
            }
            print(c.get())
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

/// Entity field guards never shrink: entity fields can change concurrently,
/// so no flow fact about them is ever tracked.
#[test]
fn entity_field_guard_does_not_shrink() {
    compile_should_fail_with(
        r#"
        error Insufficient {}

        object Vault {
            balance: int

            fn withdraw(mut self, amt: int) {
                if amt > self.balance {
                    raise Insufficient {}
                }
                self.balance = self.balance - amt
            }
        }

        fn main() {
            let mut v = Vault { balance: 100 }
            let amt = 30
            if amt <= v.balance {
                v.withdraw(amt)
            }
        }
        "#,
        "call to fallible method 'withdraw' must be handled",
    );
}

/// Calls through closure variables never shrink (dynamic target).
#[test]
fn closure_call_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Negative {}

        fn main() {
            let f = (x: int) int => {
                if x < 0 {
                    raise Negative {}
                }
                return x * 2
            }
            let a = 5
            if a >= 0 {
                print(f(a))
            }
        }
        "#,
        "call to fallible closure 'f' must be handled",
    );
}

/// Calls through fallible fn-typed values never shrink (opaque target).
#[test]
fn fn_value_call_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Negative {}

        fn checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x
        }

        fn apply(f: fn(int) int!, a: int) int {
            if a >= 0 {
                return f(a)
            }
            return 0
        }

        fn main() {
            print(apply(checked, 5) catch -1)
        }
        "#,
        "call through fallible function value 'f' must be handled",
    );
}

/// Calls through trait objects never shrink (dynamic dispatch).
#[test]
fn trait_dynamic_call_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Negative {}

        trait Checker {
            fn check(self, x: int) int
        }

        class Strict impl Checker {
            dummy: int

            fn check(self, x: int) int {
                if x < 0 {
                    raise Negative {}
                }
                return x
            }
        }

        fn run(c: Checker, a: int) int {
            if a >= 0 {
                return c.check(a)
            }
            return 0
        }

        fn main() {
            let s = Strict { dummy: 0 }
            print(run(s, 5) catch -1)
        }
        "#,
        "call to fallible method 'check' must be handled",
    );
}

/// A guard established by `requires` on the caller shrinks its call sites.
#[test]
fn requires_clause_shrinks_call() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn double_checked(x: int) int {
            if x < 0 {
                raise Negative {}
            }
            return x * 2
        }

        fn run(a: int) int
        requires a >= 0
        {
            return double_checked(a)
        }

        fn main() {
            print(run(6))
        }
        "#,
    );
    assert_eq!(out.trim(), "12");
}

/// Relational guards (between a parameter and a receiver field) shrink via
/// the relation fact, mirroring the RFC example with non-constant values.
#[test]
fn relational_guard_shrinks() {
    let out = compile_and_run_stdout(
        r#"
        error Insufficient {}

        class Account {
            balance: int

            fn withdraw(mut self, amt: int) {
                if amt > self.balance {
                    raise Insufficient {}
                }
                self.balance = self.balance - amt
            }
        }

        fn transfer(mut acct: Account, amt: int) int {
            if amt > acct.balance {
                return 0
            }
            acct.withdraw(amt)
            return acct.balance
        }

        fn main() {
            let a = Account { balance: 100 }
            print(transfer(a, 60))
        }
        "#,
    );
    assert_eq!(out.trim(), "40");
}

/// Raises nested inside loops are never summarized: the callee's guard may
/// be evaluated after arbitrary mutation across iterations.
#[test]
fn callee_raise_inside_loop_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Negative {}

        fn scan(x: int) int {
            for i in 0..3 {
                if x < 0 {
                    raise Negative {}
                }
            }
            return x
        }

        fn main() {
            let a = 5
            if a >= 0 {
                print(scan(a))
            }
        }
        "#,
        "call to fallible function 'scan' must be handled",
    );
}

/// A callee that mutates a guard-mentioned field before the guard runs never
/// yields a summary for that raise (entry-value staleness).
#[test]
fn callee_mutation_before_guard_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Insufficient {}

        class Account {
            balance: int

            fn fee_then_withdraw(mut self, amt: int) {
                self.balance = self.balance - 1
                if amt > self.balance {
                    raise Insufficient {}
                }
                self.balance = self.balance - amt
            }
        }

        fn main() {
            let mut account = Account { balance: 100 }
            let amt = 30
            if amt <= account.balance {
                account.fee_then_withdraw(amt)
            }
        }
        "#,
        "call to fallible method 'fee_then_withdraw' must be handled",
    );
}

// ── Callee guards over `param.len()` (fact-fragment extension) ───────────────

/// A callee guard over a collection parameter's length is summarizable,
/// and a caller fact over the actual's length term refutes it.
#[test]
fn param_len_guard_shrinks_with_caller_len_fact() {
    let out = compile_and_run_stdout(
        r#"
        error Empty {}

        fn head(xs: [int]) int {
            if xs.len() == 0 {
                raise Empty {}
            }
            return xs[0]
        }

        fn main() {
            let xs = [7, 8]
            if xs.len() > 0 {
                print(head(xs))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "7");
}

/// Without the caller guard the site still requires handling.
#[test]
fn param_len_guard_without_caller_fact_requires_handling() {
    compile_should_fail_with(
        r#"
        error Empty {}

        fn head(xs: [int]) int {
            if xs.len() == 0 {
                raise Empty {}
            }
            return xs[0]
        }

        fn main() {
            let xs = [7, 8]
            print(head(xs))
        }
        "#,
        "must be handled",
    );
}

/// A mutating method call between the guard and the call kills the
/// caller's length fact: handling is required again.
#[test]
fn mut_call_between_len_guard_and_call_restores_handling() {
    compile_should_fail_with(
        r#"
        error Empty {}

        fn head(xs: [int]) int {
            if xs.len() == 0 {
                raise Empty {}
            }
            return xs[0]
        }

        fn main() {
            let mut xs = [7, 8]
            if xs.len() > 0 {
                xs.pop()
                print(head(xs))
            }
        }
        "#,
        "must be handled",
    );
}

/// Reassigning the collection between guard and call also kills the fact.
#[test]
fn reassignment_between_len_guard_and_call_restores_handling() {
    compile_should_fail_with(
        r#"
        error Empty {}

        fn head(xs: [int]) int {
            if xs.len() == 0 {
                raise Empty {}
            }
            return xs[0]
        }

        fn main() {
            let mut xs = [7, 8]
            if xs.len() > 0 {
                xs = []
                print(head(xs))
            }
        }
        "#,
        "must be handled",
    );
}

/// A string-length guard works the same way (the automatic >= 0 bound
/// refutes a negative-length guard with no caller fact at all).
#[test]
fn len_auto_nonneg_refutes_impossible_guard() {
    let out = compile_and_run_stdout(
        r#"
        error Impossible {}

        fn check(s: string) int {
            if s.len() < 0 {
                raise Impossible {}
            }
            return s.len()
        }

        fn main() {
            let s = "abc"
            print(check(s))
        }
        "#,
    );
    assert_eq!(out.trim(), "3");
}

// ── Callee guards over `param.field` (fact-fragment extension) ───────────────

/// A callee guard over a one-level field path of a class parameter is
/// summarizable; the caller's fact about the actual's field refutes it.
#[test]
fn param_field_guard_shrinks_with_caller_fact() {
    let out = compile_and_run_stdout(
        r#"
        error BadToken {}

        class Grant {
            token: int
        }

        fn redeem(g: Grant) int {
            if g.token <= 0 {
                raise BadToken {}
            }
            return g.token
        }

        fn main() {
            let g = Grant { token: 5 }
            if g.token > 0 {
                print(redeem(g))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

/// Without the caller's field fact the site still requires handling.
#[test]
fn param_field_guard_without_caller_fact_requires_handling() {
    compile_should_fail_with(
        r#"
        error BadToken {}

        class Grant {
            token: int
        }

        fn redeem(g: Grant) int {
            if g.token <= 0 {
                raise BadToken {}
            }
            return g.token
        }

        fn main() {
            let g = Grant { token: 5 }
            print(redeem(g))
        }
        "#,
        "must be handled",
    );
}

/// An interleaved call between the field guard and the call kills the
/// caller's field fact (an alias may have mutated the object).
#[test]
fn interleaved_call_kills_param_field_guard() {
    compile_should_fail_with(
        r#"
        error BadToken {}

        class Grant {
            token: int
        }

        fn noop() int {
            return 0
        }

        fn redeem(g: Grant) int {
            if g.token <= 0 {
                raise BadToken {}
            }
            return g.token
        }

        fn main() {
            let g = Grant { token: 5 }
            if g.token > 0 {
                let x = noop()
                print(redeem(g) + x)
            }
        }
        "#,
        "must be handled",
    );
}

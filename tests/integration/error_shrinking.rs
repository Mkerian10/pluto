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

        fn seed() int {
            return 100
        }

        fn main() {
            // Opaque initializer: a literal would grant the exact
            // construction fact (balance == 100) and shrink the call.
            let mut account = Account { balance: seed() }
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

/// A loop-resident raise whose guard is loop-invariant and entry-stable
/// summarizes: the loop never disturbs `x`, so the guard means the same
/// thing on every iteration and the caller's fact refutes it.
#[test]
fn loop_invariant_guard_shrinks() {
    let out = compile_and_run_stdout(
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
    );
    assert_eq!(out.trim(), "5");
}

/// A loop body that writes a guard-mentioned field invalidates the guard
/// for every iteration (an earlier iteration may have run the write before
/// a later iteration's raise), so the variant never shrinks.
#[test]
fn loop_mutated_field_guard_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Overflow {}

        class Counter {
            c: int

            fn bump(mut self) {
                for i in 0..3 {
                    if self.c > 10 {
                        raise Overflow {}
                    }
                    self.c = self.c + 1
                }
            }
        }

        fn main() {
            let mut k = Counter { c: 0 }
            if k.c <= 10 {
                k.bump()
            }
        }
        "#,
        "call to fallible method 'bump' must be handled",
    );
}

/// A call anywhere in the loop body taints every loop guard (an alias may
/// mutate reachable state between iterations), so the variant never shrinks.
#[test]
fn call_in_loop_body_poisons_guard() {
    compile_should_fail_with(
        r#"
        error Big {}

        fn helper() int {
            return 1
        }

        fn scan(x: int) int {
            for i in 0..3 {
                let h = helper()
                if x > 100 {
                    raise Big {}
                }
            }
            return x
        }

        fn main() {
            let a = 5
            if a <= 100 {
                print(scan(a))
            }
        }
        "#,
        "call to fallible function 'scan' must be handled",
    );
}

/// Per-raise-site granularity across regions: a variant with an
/// entry-guarded site AND a loop-resident loop-invariant site drops when
/// the caller refutes both sites.
#[test]
fn entry_site_plus_loop_site_both_refuted() {
    let out = compile_and_run_stdout(
        r#"
        error OutOfRange {}

        fn check(x: int) int {
            if x < 0 {
                raise OutOfRange {}
            }
            for i in 0..3 {
                if x > 100 {
                    raise OutOfRange {}
                }
            }
            return x
        }

        fn main() {
            let a = 5
            if a >= 0 && a <= 100 {
                print(check(a))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

/// A while loop whose body only mutates a local counter leaves a
/// param-only guard usable.
#[test]
fn while_loop_param_guard_shrinks() {
    let out = compile_and_run_stdout(
        r#"
        error Negative {}

        fn count(n: int) int {
            let mut i = 0
            while i < 3 {
                if n < 0 {
                    raise Negative {}
                }
                i = i + 1
            }
            return n
        }

        fn main() {
            let a = 4
            if a >= 0 {
                print(count(a))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "4");
}

/// An unguarded raise of the same variant inside a match arm still poisons
/// the variant: refuting the entry-guarded site is not enough.
#[test]
fn match_arm_raise_still_poisons_variant() {
    compile_should_fail_with(
        r#"
        error Bad {}

        enum Mode {
            Fast
            Slow
        }

        fn run(m: Mode, x: int) int {
            if x < 0 {
                raise Bad {}
            }
            match m {
                Mode.Fast {
                    raise Bad {}
                }
                Mode.Slow {
                    print(0)
                }
            }
            return x
        }

        fn main() {
            let a = 5
            if a >= 0 {
                print(run(Mode.Slow, a))
            }
        }
        "#,
        "call to fallible function 'run' must be handled",
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

        fn seed() int {
            return 5
        }

        fn main() {
            // Opaque initializer: a literal would grant the exact
            // construction fact (token == 5) and shrink the call.
            let g = Grant { token: seed() }
            print(redeem(g))
        }
        "#,
        "must be handled",
    );
}

/// An interleaved call that may reach the guarded class (its declared
/// parameter is class-typed, so it may hold an alias) kills the caller's
/// field fact.
#[test]
fn interleaved_call_kills_param_field_guard() {
    compile_should_fail_with(
        r#"
        error BadToken {}

        class Grant {
            token: int
        }

        fn poke(h: Grant) int {
            return h.token
        }

        fn redeem(g: Grant) int {
            if g.token <= 0 {
                raise BadToken {}
            }
            return g.token
        }

        fn main() {
            let g = Grant { token: 5 }
            let h = Grant { token: 7 }
            if g.token > 0 {
                let x = poke(h)
                print(redeem(g) + x)
            }
        }
        "#,
        "must be handled",
    );
}

/// The purity-aware flip side: an interleaved call that provably cannot
/// reach the guarded class (declared params are reach-free) no longer
/// kills the guard fact — the shrink survives.
#[test]
fn reach_free_interleaved_call_keeps_param_field_guard() {
    let out = compile_and_run_stdout(
        r#"
        error BadToken {}

        class Grant {
            token: int
        }

        fn noop() int {
            return 0
        }

        fn read_token() int {
            return 5
        }

        fn redeem(g: Grant) int {
            if g.token <= 0 {
                raise BadToken {}
            }
            return g.token
        }

        fn main() {
            let mut g = Grant { token: 0 }
            g.token = read_token()
            if g.token > 0 {
                let x = noop()
                print(redeem(g) + x)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "5");
}

// ─────────────────────────────────────────────────────────────────────────────
// Construction facts (phase 4.5 precision)
// ─────────────────────────────────────────────────────────────────────────────

/// Construction is fully transparent: `balance == 100` flows to the
/// caller's fact env, refuting the raise condition with no guard at all.
#[test]
fn construction_facts_enable_shrinking() {
    let out = compile_and_run_stdout(
        r#"
        error Insufficient {}

        class Account {
            balance: int

            fn try_withdraw(mut self, amount: int) int {
                if amount > self.balance {
                    raise Insufficient {}
                }
                self.balance = self.balance - amount
                return self.balance
            }
        }

        fn main() {
            let mut acc = Account { balance: 100 }
            print(acc.try_withdraw(50))
        }
        "#,
    );
    assert_eq!(out.trim(), "50");
}

/// Initializers that are caller locals transfer as relation facts
/// (`b.cap == base`, with `base` a local no callee can change) — enough to
/// refute a raise condition phrased against the same local.
#[test]
fn construction_facts_relate_to_locals() {
    let out = compile_and_run_stdout(
        r#"
        error Empty {}

        class Buffer {
            cap: int

            fn take(mut self, n: int) int {
                if n > self.cap {
                    raise Empty {}
                }
                self.cap = self.cap - n
                return self.cap
            }
        }

        fn main() {
            let base = 60
            let mut b = Buffer { cap: base }
            print(b.take(base))
        }
        "#,
    );
    assert_eq!(out.trim(), "0");
}

/// An opaque initializer (a call) grants nothing — handling required.
#[test]
fn opaque_construction_grants_no_facts() {
    compile_should_fail_with(
        r#"
        error Insufficient {}

        fn seed() int {
            return 100
        }

        class Account {
            balance: int

            fn try_withdraw(mut self, amount: int) int {
                if amount > self.balance {
                    raise Insufficient {}
                }
                self.balance = self.balance - amount
                return self.balance
            }
        }

        fn main() {
            let mut acc = Account { balance: seed() }
            print(acc.try_withdraw(50))
        }
        "#,
        "must be handled",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Generic callees (template summaries)
// ─────────────────────────────────────────────────────────────────────────────

/// A generic free function's guard over a type-param-independent parameter
/// shrinks at a concrete call site, exactly like concrete code.
#[test]
fn generic_free_fn_shrinks() {
    let out = compile_and_run_stdout(
        r#"
        error BadIndex {}

        fn nth<T>(xs: [T], i: int, fallback: T) T {
            if i < 0 {
                raise BadIndex {}
            }
            return fallback
        }

        fn main() {
            let i = 2
            if i >= 0 {
                print(nth([1, 2, 3], i, 0))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "0");
}

/// Without the caller guard the generic call still requires handling.
#[test]
fn generic_free_fn_unrefuted_requires_handling() {
    compile_should_fail_with(
        r#"
        error BadIndex {}

        fn nth<T>(xs: [T], i: int, fallback: T) T {
            if i < 0 {
                raise BadIndex {}
            }
            return fallback
        }

        fn main() {
            let i = 2
            print(nth([1, 2, 3], i, 0))
        }
        "#,
        "call to fallible function 'nth' must be handled",
    );
}

/// The typestate pattern (census category a): a state-generic class whose
/// state param is phantom and whose guard is over a plain int field. The
/// caller's fact on the receiver's field refutes the raise condition — the
/// `where`-constrained transition call needs no handling.
#[test]
fn typestate_method_shrinks() {
    let out = compile_and_run_stdout(
        r#"
        error Degraded {
            code: int
        }

        class Held {
            tag: int
        }

        class Lease<S> {
            id: int
            epoch: int

            fn renew(self) Lease<Held> where S == Held {
                if self.epoch > 2 {
                    raise Degraded { code: self.epoch }
                }
                return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
            }
        }

        fn main() {
            let h = Lease<Held> { id: 7, epoch: 1 }
            if h.epoch <= 2 {
                let h2 = h.renew()
                print(h2.epoch)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "2");
}

/// A plain (unconstrained) generic class method shrinks on guards over
/// type-param-independent fields and params; both variants refuted here.
#[test]
fn generic_method_shrinks() {
    let out = compile_and_run_stdout(
        r#"
        error TooSmall {}
        error TooBig {}

        class Box<T> {
            v: T
            n: int

            fn take(self, amt: int) int {
                if amt < 0 {
                    raise TooSmall {}
                }
                if amt > self.n {
                    raise TooBig {}
                }
                return self.n - amt
            }
        }

        fn main() {
            let b = Box<int> { v: 1, n: 10 }
            let amt = 3
            if amt >= 0 && amt <= b.n {
                print(b.take(amt))
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "7");
}

/// A guard mentioning a type-param-dependent leaf (here a `[T]` parameter's
/// length) never yields a summary: the fragment vocabulary does not survive
/// instantiation, so the call still requires handling.
#[test]
fn skolem_dependent_guard_never_shrinks() {
    compile_should_fail_with(
        r#"
        error TooLong {}

        fn cap<T>(xs: [T]) int {
            if xs.len() > 10 {
                raise TooLong {}
            }
            return xs.len()
        }

        fn main() {
            let xs = [1, 2, 3]
            if xs.len() <= 10 {
                print(cap(xs))
            }
        }
        "#,
        "call to fallible function 'cap' must be handled",
    );
}

/// Typed-catch coverage on a generic method call uses the shrunk set: the
/// refuted variant needs no handler; covering the remaining one suffices.
#[test]
fn generic_typed_catch_coverage_narrows() {
    let out = compile_and_run_stdout(
        r#"
        error TooSmall {}
        error TooBig {}

        class Box<T> {
            v: T
            n: int

            fn take(self, amt: int) int {
                if amt < 0 {
                    raise TooSmall {}
                }
                if amt > self.n {
                    raise TooBig {}
                }
                return self.n - amt
            }
        }

        fn main() {
            let b = Box<int> { v: 1, n: 10 }
            let amt = 3
            if amt >= 0 {
                let r = b.take(amt) catch e: TooBig {
                    print(-1)
                    return
                }
                print(r)
            }
        }
        "#,
    );
    assert_eq!(out.trim(), "7");
}

/// Handling a provably-impossible error on a generic call stays legal: the
/// `!`-applied-to-infallible check keeps using the full inferred set.
#[test]
fn propagate_on_shrunk_generic_call_stays_legal() {
    let out = compile_and_run_stdout(
        r#"
        error Degraded {
            code: int
        }

        class Held {
            tag: int
        }

        class Lease<S> {
            id: int
            epoch: int

            fn renew(self) Lease<Held> where S == Held {
                if self.epoch > 2 {
                    raise Degraded { code: self.epoch }
                }
                return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
            }
        }

        fn use_lease(h: Lease<Held>) int {
            if h.epoch <= 2 {
                let h2 = h.renew()!
                return h2.epoch
            }
            return 0
        }

        fn main() {
            let h = Lease<Held> { id: 7, epoch: 1 }
            print(use_lease(h) catch -1)
        }
        "#,
    );
    assert_eq!(out.trim(), "2");
}

/// A generic callee whose caller does not refute the guard still requires
/// handling (method form).
#[test]
fn typestate_method_unrefuted_requires_handling() {
    compile_should_fail_with(
        r#"
        error Degraded {
            code: int
        }

        class Held {
            tag: int
        }

        class Lease<S> {
            id: int
            epoch: int

            fn renew(self) Lease<Held> where S == Held {
                if self.epoch > 2 {
                    raise Degraded { code: self.epoch }
                }
                return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
            }
        }

        fn main() {
            let h = Lease<Held> { id: 7, epoch: 1 }
            let h2 = h.renew()
            print(h2.epoch)
        }
        "#,
        "call to fallible method 'renew' must be handled",
    );
}

/// A variant that can also arrive through a `!` propagation edge inside the
/// generic template body never shrinks, even when the guarded direct raise
/// is refuted (propagation filtering uses the template's own edge set, not
/// the instance->template bridge artifact).
#[test]
fn generic_method_propagated_variant_never_shrinks() {
    compile_should_fail_with(
        r#"
        error Boom {}

        fn helper(n: int) int {
            if n > 1000 {
                raise Boom {}
            }
            return n
        }

        class Box<T> {
            v: T
            n: int

            fn poke(self, amt: int) int {
                if amt < 0 {
                    raise Boom {}
                }
                return helper(self.n)!
            }
        }

        fn main() {
            let b = Box<int> { v: 1, n: 2 }
            let amt = 3
            if amt >= 0 {
                print(b.poke(amt))
            }
        }
        "#,
        "call to fallible method 'poke' must be handled",
    );
}

# Contracts

Most languages punt on correctness. You write code, you write tests, and you hope the gap between what you intended and what you shipped is small. The available tools are underwhelming: assertions are ad hoc and stripped in release builds, property-based testing runs outside the program, and Eiffel-style contracts — where they exist at all — are runtime checks that crash *after* the bug has happened.

Pluto's contract system takes the opposite position: **contracts are specifications the compiler proves, not guards that crash**. A class invariant is not checked at runtime — it is discharged at compile time, at every site that could break it, or the program does not compile.

Three primitives:

| Primitive | Meaning | When checked |
|-----------|---------|--------------|
| `invariant` | property of a class that always holds | **compile time** — proven at every construction and write site |
| `requires` | precondition the caller must meet | runtime, at function entry (and assumed by the prover inside the body) |
| `assert` | explicit runtime check | runtime — and establishes the fact for the prover afterward |

There is no `ensures`. Postconditions are expressed by invariants and return types (more below).

## Invariants are compile-time proofs

Declare an invariant inside a class body:

```
class Account {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
    {
        self.balance = self.balance + amount
    }

    fn withdraw(mut self, amount: int)
        requires amount > 0
        requires self.balance >= amount
    {
        self.balance = self.balance - amount
    }
}

fn main() {
    let mut account = Account { balance: 100 }
    account.deposit(50)
    account.withdraw(30)
    print(account.balance)    // 120
}
```

This compiles — and that is the whole story at runtime. There is no invariant check in the generated code, anywhere. The compiler proved, at compile time, that no execution of this program can make `balance` negative:

- The construction `Account { balance: 100 }` substitutes the initializer into the invariant: `100 >= 0`. Proven.
- Inside `deposit`, the write is tracked symbolically: `balance = old(balance) + amount`. The entry facts are the invariant on `self` (`old(balance) >= 0`) and the `requires` (`amount > 0`). The sum is nonnegative. Proven.
- Inside `withdraw`, `balance = old(balance) - amount` with entry fact `old(balance) >= amount`. Proven.

The invariant may be broken *temporarily* between writes inside a `mut self` method; the proof obligations sit at the boundaries — method exits, raises, any statement containing a call (something might observe the object), loop entry and body end, branch joins. Foreign writes (`obj.field = v` from outside the class's own methods) are proven immediately; external code gets no temporary-violation window.

## A rejected write site

Delete the guard and the compiler rejects the program at the write site, not at 3 a.m. in production:

```
class Account {
    balance: int

    invariant self.balance >= 0

    fn drain(mut self, amt: int) {
        self.balance = self.balance - amt    // COMPILE ERROR
    }
}
```

```
error: cannot prove invariant 'self.balance >= 0' of class 'Account' holds
at the end of this method in method 'drain': at this point self.balance =
-amt + old(self.balance). The invariant may be broken temporarily between
writes, but must be re-established at every boundary (method exits, raises,
calls, loops, branch joins). Establish the missing bound before this point
with a guard, a 'requires' clause, or an 'assert'
(e.g. 'if amt <= self.balance { ... }')
```

The diagnostic names the invariant, the site, the symbolic state the prover reached, and the three ways to fix it. Any of them works:

```
fn drain(mut self, amt: int) {
    if amt >= 0 && amt <= self.balance {
        self.balance = self.balance - amt    // proven from the guard
    }
}
```

Construction sites carry the same obligation. An initializer the prover cannot bound is rejected:

```
class Account {
    balance: int

    invariant self.balance >= 0
}

fn make(b: int) Account {
    return Account { balance: b }    // COMPILE ERROR: b is unbounded
}
```

```
error: cannot prove invariant 'self.balance >= 0' of class 'Account' for
this construction: the field initializers are not statically known to
satisfy it. Establish the needed facts before constructing — a guard
('if x >= 0 { ... }'), an 'assert', or a 'requires' clause on the
enclosing function — or simplify the initializers to expressions the
prover can bound
```

```
fn make(b: int) Account {
    assert b >= 0                    // runtime check; prover fact afterward
    return Account { balance: b }    // proven
}
```

There is no fallback mode. An invariant the compiler cannot discharge is a compile error — never a runtime residual, never undefined behavior. Unprovability costs you a guard or a `requires` clause, not a crash path.

## The provable fragment

Because unprovable means rejected, invariants are restricted to what the flow-fact engine can decide — no SMT solver, no timeouts, no proofs that sometimes work:

**Allowed:** `&&`, `||`, `!` over integer comparisons (`==`, `!=`, `<`, `>`, `<=`, `>=`), where each side is linear arithmetic (`+`, `-`, `*` by a constant) over int literals and direct int fields of `self` — `self.balance >= 0`, `self.lo <= self.hi`, `self.applied <= self.epoch`.

**Rejected at declaration:** float, string, or boolean properties; `.len()` and any method call; indexing; nested field access (`self.child.value`); non-linear arithmetic, division, modulo; invariants on generic classes.

```
class User {
    name: string

    invariant self.name.len() > 0    // COMPILE ERROR
}
```

```
error: invariant 'self.name.len() > 0' is outside the provable fragment:
'.len()' — collection and method facts are not provable. Invariants are
compile-time proof obligations; they must be built from &&, ||, ! over
integer comparisons of linear arithmetic over the class's own int fields
(e.g. 'self.balance >= 0', 'self.lo <= self.hi'). Properties outside this
fragment cannot be statically discharged and are rejected
```

The fragment grows deliberately, one proof shape at a time. Predictability beats power: a prover that sometimes times out teaches people to distrust it.

## What the prover knows

At any program point, the facts available for discharge are:

- **`requires` clauses** of the enclosing function — checked at runtime on entry, assumed by the prover inside the body.
- **Invariants of class-typed parameters** — any `Account` you receive already satisfies its invariant.
- **`if` guards** — flow facts, the same machinery that narrows nullables. Inside `if amt <= self.balance { ... }` the prover knows `amt <= balance`.
- **`assert` statements** — after an `assert` passes, the condition is a fact.

The same flow-fact engine also flags conditions it can prove degenerate — `warning: condition is always false` on a branch that contradicts facts already established — so dead guards surface instead of silently papering over logic errors.

## DI construction: the zero-state proof

Instances synthesized by dependency injection never pass through a struct literal — they are allocated zero-initialized and only their injected dep fields are wired. So a DI-constructed class may only carry invariants its zero state satisfies: `invariant self.count >= 0` proves for a DI singleton; `invariant self.count > 0` is a compile error. A class whose invariant needs nonzero initial state belongs outside startup wiring — give it a `scoped` lifecycle and seed it in a scope block, where the seed's struct literal carries the ordinary construction proof.

## `requires`: runtime at entry, fact inside

`requires` semantics are unchanged from the runtime-contracts era: clauses are evaluated at function entry, and a violation is a hard abort —

```
fn safe_divide(a: int, b: int) int
    requires b != 0
{
    return a / b
}

fn main() {
    print(safe_divide(10, 0))    // aborts
}
```

```
requires violation in safe_divide: b != 0
```

A violated precondition means the *caller's* logic is wrong — not "the network is down," which is what typed errors are for. Aborting beats continuing with corrupted assumptions, and contracts are never stripped from production builds.

Inside the body, `requires` clauses are entry facts for the prover — the `withdraw` example above is provable precisely because of its `requires`. Static discharge of `requires` at call sites (proving the caller meets them, eliminating the entry check) is planned; the proof engine it needs is the one already shipping for invariants.

Trait methods can carry `requires`, inherited and enforced for every implementation. Implementations cannot add their own — demanding more than the trait promised would break substitutability, and the compiler rejects it:

```
error: method 'process' on class 'MyProcessor' cannot add 'requires'
clauses: it implements trait 'Processor' and adding preconditions would
violate the Liskov Substitution Principle
```

## `assert`: the escape hatch

`assert` is for facts the prover cannot establish — typically values from outside the program:

```
fn divide(a: int, b: int) int
    requires b != 0
{
    return a / b
}

fn main() {
    let input = 7            // imagine: read from stdin
    assert input != 0        // runtime check
    print(divide(10, input)) // prover accepts: the assert established it
}
```

It generates a runtime check, aborts on failure, and feeds the condition to the prover as a fact. Use it at trust boundaries; prefer guards and `requires` everywhere else.

## Why no `ensures`

Pluto rejects `ensures` at parse time, by design:

```
error: Syntax error: 'ensures' clauses are not supported: Pluto has no
postconditions by design; express guarantees with class invariants or
return types
```

With whole-program compilation, postconditions are redundant: what a function guarantees about state is an invariant (proven at its write sites); what it guarantees about its result is a return type — construct a `PositiveBalance` and its invariant *is* the postcondition, proven at the construction site. A separate runtime postcondition mechanism would re-introduce exactly the crash-after-the-fact checking the proof system exists to eliminate.

## The one runtime validation point: the wire

Inside a compiled program, invariants need no checks — every write path was proven. But data *entering* the program over a placement or serve boundary was not produced by your proven write paths. It is testimony, not proof.

So wire decode is the single place invariant validation runs at runtime: decoding a value of an invariant-carrying class re-checks the invariants and raises the standard wire error on violation — a typed error, mandatory to handle like any other boundary failure. A peer running skewed code, or a hand-crafted payload, cannot inject a `balance: -5` into a program whose compiler proved `balance >= 0` everywhere.

One trust boundary, one check, typed and catchable. Everywhere else: proofs.

> **The pattern in the wild.** You already know this bug — the web ships it. A TLS certificate is a lease: evidence with a validity window. Web revocation is famously broken precisely because it is poll-based — browsers act on beliefs that outlive their warrants, and when the revocation check doesn't answer, they soft-fail and trust the stale belief anyway. OCSP stapling is the fix with the right shape: carry fresh evidence *with the effect* instead of hoping a cached belief still holds. Certificate Transparency logs are monotone append-only facts — safe to cache and gossip because nothing is ever retracted. And the CA's issuance validation is the authority's fence: judged by the party that owns the namespace, at the point of effect. Pluto's wire check above is the same move at program scale — testimony validated where it enters, proofs everywhere inside — and [Verified Distribution](../vision/verified-distribution.md) is the chapter-length version of "here is that bug as a type error."

## Proofs have consequences

Facts do not just gate writes — they flow into error inference. A guard that makes a callee's `raise` impossible removes the handling obligation at that call site entirely. See [error-set shrinking](errors.md#proofs-shrink-error-sets) in the Error Handling chapter.

## Comparison

| Feature | Pluto | Eiffel | D | Rust |
|---------|-------|--------|---|------|
| Class invariants | **compile-time proof** (strict) | runtime check | runtime check | none |
| Preconditions | `requires`, runtime at entry + prover fact | `require`, runtime | `in`, runtime | `debug_assert!` (stripped in release) |
| Postconditions | none — invariants + return types | `ensure`, runtime | `out`, runtime | none |
| Provable fragment | enforced (int linear arithmetic) | unrestricted | unrestricted | n/a |
| Active in production | always (what remains at runtime) | configurable | configurable | stripped |
| Unprovable invariant | compile error | n/a (not proven) | n/a | n/a |

Eiffel asked the right question in the 1980s and answered it with runtime checks. Pluto's answer is proofs: the invariant you declare is the invariant the compiler discharges — and the diagnostic you get when it can't is the specification of exactly what your code failed to establish.

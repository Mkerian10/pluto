# Contracts

Most languages punt on correctness. You write code, you write tests, and you hope the gap between what you intended and what you shipped is small. The available tools are underwhelming: assertions are ad hoc and stripped in release builds, property-based testing runs outside the program, and Eiffel-style contracts — where they exist at all — are runtime checks that crash *after* the bug has happened.

Pluto's contract system takes the opposite position: **contracts are specifications the compiler proves, not guards that crash**. A class invariant is not checked at runtime — it is discharged at compile time, at every site that could break it, or the program does not compile.

Four primitives:

| Primitive | Meaning | When checked |
|-----------|---------|--------------|
| `invariant` | property of a class that always holds — including *two-state* relations against `old()` | **compile time** — proven at every construction and write site |
| `ensures` | postcondition relating a method's exit state to its entry state via `old()` | **compile time** — proven at every normal exit, assumed by callers |
| `guarded_by` | every write to a field is dominated by a validity check | **compile time** — proven at every write site |
| `requires` | precondition the caller must meet | runtime, at function entry (and assumed by the prover inside the body) |

Plus `assert`, the explicit runtime check — which also establishes its condition as a fact for the prover afterward.

There is no *runtime* `ensures`. Postconditions exist only in proof form (more below).

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

## `ensures` and `old()`: two-state proofs

An invariant says what is *always* true. An `ensures` clause says what a method *did*: it relates the receiver's exit state to its entry state, with `old(expr)` naming the value `expr` had when the method was entered.

```
class Account {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) + amount
    {
        self.balance = self.balance + amount
    }
}

fn main() {
    let mut account = Account { balance: 100 }
    account.deposit(50)
    print(account.balance)    // 150
}
```

Like invariants, this is a compile-time obligation, discharged by the same symbolic machinery — the one that was already tracking `balance = old(balance) + amount` through the body. The clause must be proven at every normal exit of the method; a body that does not establish the declared relation is rejected with the symbolic state the prover reached:

```
class Counter {
    n: int

    fn bump(mut self) ensures self.n == old(self.n) + 2 {
        self.n = self.n + 1    // COMPILE ERROR
    }
}
```

```
error: ensures clause 'self.n == old(self.n) + 2' of method 'bump' of class
'Counter' is violated at the end of this method: at this point
self.n = old(self.n) + 1
```

Raise paths are exempt — a method that raises did not do what it promised, and the error contract governs those edges instead.

**Callers assume the relation.** This is the half that pays rent. Before `ensures`, a method call was opaque to the prover: afterward, all it knew about the receiver was its invariant. With a declared `ensures`, the caller keeps the exact two-state relation across the call — and that fact feeds every downstream proof:

```
class Pos {
    v: int

    invariant self.v >= 1
}

class Counter {
    n: int

    invariant self.n >= 0

    fn bump(mut self) ensures self.n == old(self.n) + 1 {
        self.n = self.n + 1
    }
}

fn make(mut c: Counter) Pos {
    c.bump()
    return Pos { v: c.n }    // proven: n == old(n) + 1 and old(n) >= 0, so n >= 1
}

fn main() {
    let mut c = Counter { n: 5 }
    let p = make(c)
    print(p.v)    // 6
}
```

Delete the `c.bump()` call and the construction stops compiling — `n >= 0` alone cannot prove `v >= 1`. The assumption really flows from the callee's contract, not from wishful thinking. Sibling calls compose the same way: a `double_bump` that calls `bump()` twice proves `ensures self.n == old(self.n) + 2` from the parts.

Two honest limits. `ensures` lives on class and object methods only — a free function has no receiver state to relate. And a call to anything *other* than a sibling method with its own declared `ensures` conservatively severs exact two-state knowledge mid-body (the callee might reach the receiver through an alias); the prover re-anchors to invariant-level facts after it.

## Two-state invariants: monotonicity without a keyword

`old()` is also legal in an `invariant` — turning it into a relation that every state change must respect:

```
object Epoch {
    e: int

    invariant self.e >= old(self.e)

    fn advance(mut self) {
        self.e = self.e + 1    // proven: e + 1 >= e
    }

    fn now(self) int {
        return self.e
    }
}
```

`self.e >= old(self.e)` is monotonicity — the epoch never decreases — stated with no new keyword. The obligation sites are the same ones single-state invariants already carry: every `mut self` method exit is checked against the method's entry state, and every foreign write against the pre-write state. A method that could ever move the field backward is a compile error:

```
fn reset(mut self) {
    self.e = 0    // COMPILE ERROR
}
```

```
error: cannot prove invariant 'self.e >= old(self.e)' of class 'Epoch' holds
at the end of this method in method 'reset': at this point self.e = 0. The
invariant may be broken temporarily between writes, but must be re-established
at every boundary (method exits, raises, calls, loops, branch joins).
Establish the missing bound before this point with a guard, a 'requires'
clause, or an 'assert' (e.g. 'if amt <= self.balance { ... }')
```

Construction has no pre-state, so two-state clauses impose nothing there — any initial value is legal; the relation binds every change *after* birth. This is the proof shape behind epochs, versions, sequence numbers, and high-water marks: a fencing design is only sound if a minted token can never become current again, and that sentence is now a clause the compiler checks rather than a comment reviewers hope stays true.

## `guarded_by`: every write provably fenced

The third proof shape is about control flow rather than arithmetic. A field can declare that no write to it may ever escape a validity check:

```
error StaleGrant {
    token: int
    epoch: int
}

class WriteGrant {
    token: int
}

object Store {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    fn grant_write(mut self) WriteGrant {
        self.epoch = self.epoch + 1
        return WriteGrant { token: self.epoch }
    }

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise StaleGrant { token: grant.token, epoch: self.epoch }
        }
        self.data = d.to_bytes()    // proven: dominated by the fence
    }

    fn read(self) string {
        return self.data.to_string()
    }
}
```

The clause reads: every write to `self.data` must be **dominated** by a conditional the prover can show implies `g.token == self.epoch` for some in-scope `WriteGrant`. In `apply`, the raise-guard is exactly that conditional — on the fall-through path, `grant.token == self.epoch` is a fact, and it survives to the write. This is the *fencing* pattern: `grant_write` advances the epoch, silently invalidating every older grant, and the fence rejects stale writers at the point of effect with a typed error.

Remove the fence — or move the write above it, or put a call between them that could disturb the compared fields — and the program stops compiling:

```
fn apply(mut self, grant: WriteGrant, d: string) {
    self.data = d.to_bytes()    // COMPILE ERROR: no fence dominates this write
}
```

```
error: cannot prove guard 'data' guarded_by (g: WriteGrant) g.token ==
self.epoch of class 'Store' at this write: no dominating check implies the
predicate for any in-scope 'WriteGrant' value (tried 'grant'). Every write to
a guarded field must be dominated by a conditional the prover can carry to
the write — e.g. a check of 'grant' whose failing path raises or returns,
placed before the write — and the facts it establishes must survive to the
write site (they are invalidated by calls that may run user code, loop
boundaries, and writes to the compared fields)
```

The details that make this sound: a guarded field may only be written through `self`, inside its class's own methods — the write-set is closed, so the proof covers every write in the program. On an object, method serialization closes the check-then-act window (nothing interleaves between fence and write); on a value class the clause is sound because values do not share. And the binder must be a concrete value class — evidence you hold, not an entity whose fields can change under you.

Together, a two-state invariant and a `guarded_by` turn a fencing protocol's safety theorem into compiled fact: *the epoch only increases, and no write lands without a currently-valid grant*. The repository's `examples/blob` is the full protocol — a lockless single-writer blob store whose safety claim is no longer "by inspection" anywhere; see [Verified Distribution](../vision/verified-distribution.md) for why that theorem is the load-bearing one.

## The provable fragment

Because unprovable means rejected, every proof-form clause is restricted to what the flow-fact engine can decide — no SMT solver, no timeouts, no proofs that sometimes work:

**Allowed in `invariant`:** `&&`, `||`, `!` over integer comparisons (`==`, `!=`, `<`, `>`, `<=`, `>=`), where each side is linear arithmetic (`+`, `-`, `*` by a constant) over int literals and direct int fields of `self` — `self.balance >= 0`, `self.lo <= self.hi`, `self.applied <= self.epoch` — plus `old(...)` of those fields for two-state relations.

**Allowed in `ensures`:** the same shape, over the class's own int fields, the method's int parameters, and `old(...)` of those — `self.balance == old(self.balance) - amt`. (`.len()` terms are excluded from `ensures` for now: every collection mutation is an opaque call that severs the entry relation, so no `len()` postcondition could be proven today.)

**Allowed in `guarded_by`:** the same comparison shape, whose leaves are the class's own int fields (`self.epoch`) and one-level int fields of the binder (`g.token`).

**Rejected at declaration:** float, string, or boolean properties; `.len()` and any method call; indexing; nested field access (`self.child.value`); non-linear arithmetic, division, modulo; any of these clauses on generic classes. `ensures` additionally rejects free functions — there is no receiver state to relate.

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
- **`ensures` of called methods** — after `c.bump()`, the declared two-state relation on `c` is a fact (see above).

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

## Why no *runtime* `ensures`

`ensures` in Pluto is a proof, never a check. The runtime postcondition of Eiffel and D — evaluate the clause on exit, crash if false — remains rejected by design: it is exactly the crash-after-the-fact checking the proof system exists to eliminate. There is no mode in which an `ensures` compiles into generated code; an `ensures` the compiler cannot discharge is a compile error, same as an invariant.

The division of labor: what a method guarantees about *how state changed* is an `ensures` (proven at its exits, assumed by callers); what a function guarantees about *its result* is a return type — construct a `PositiveBalance` and its invariant *is* the postcondition, proven at the construction site.

## The one runtime validation point: the wire

Inside a compiled program, invariants need no checks — every write path was proven. But data *entering* the program over a placement or serve boundary was not produced by your proven write paths. It is testimony, not proof.

So wire decode is the single place invariant validation runs at runtime: decoding a value of an invariant-carrying class re-checks the invariants and raises the standard wire error on violation — a typed error, mandatory to handle like any other boundary failure. A peer running skewed code, or a hand-crafted payload, cannot inject a `balance: -5` into a program whose compiler proved `balance >= 0` everywhere.

One trust boundary, one check, typed and catchable. Everywhere else: proofs.

> **The pattern in the wild.** You already know this bug — the web ships it. A TLS certificate is a lease: evidence with a validity window. Web revocation is famously broken precisely because it is poll-based — browsers act on beliefs that outlive their warrants, and when the revocation check doesn't answer, they soft-fail and trust the stale belief anyway. OCSP stapling is the fix with the right shape: carry fresh evidence *with the effect* instead of hoping a cached belief still holds. Certificate Transparency logs are monotone append-only facts — safe to cache and gossip because nothing is ever retracted. And the CA's issuance validation is the authority's fence: judged by the party that owns the namespace, at the point of effect. Pluto's wire check above is the same move at program scale — testimony validated where it enters, proofs everywhere inside — and [Verified Distribution](../vision/verified-distribution.md) is the chapter-length version of "here is that bug as a type error."

## Proofs have consequences

Facts do not just gate writes — they flow into error inference. A guard that makes a callee's `raise` impossible removes the handling obligation at that call site entirely — and so does an `ensures`: after `c.bump()` with `ensures self.n == old(self.n) + 1`, a callee that raises only when `n < 1` needs no handler. See [error-set shrinking](errors.md#proofs-shrink-error-sets) in the Error Handling chapter.

## Comparison

| Feature | Pluto | Eiffel | D | Rust |
|---------|-------|--------|---|------|
| Class invariants | **compile-time proof** (strict), incl. two-state via `old()` | runtime check | runtime check | none |
| Preconditions | `requires`, runtime at entry + prover fact | `require`, runtime | `in`, runtime | `debug_assert!` (stripped in release) |
| Postconditions | `ensures` + `old()`, **compile-time proof**, assumed by callers | `ensure`, runtime | `out`, runtime | none |
| Write fencing | `guarded_by`, **compile-time dominance proof** | none | none | none |
| Provable fragment | enforced (int linear arithmetic) | unrestricted | unrestricted | n/a |
| Active in production | always (what remains at runtime) | configurable | configurable | stripped |
| Unprovable invariant | compile error | n/a (not proven) | n/a | n/a |

Eiffel asked the right question in the 1980s and answered it with runtime checks. Pluto's answer is proofs: the invariant you declare is the invariant the compiler discharges — and the diagnostic you get when it can't is the specification of exactly what your code failed to establish.

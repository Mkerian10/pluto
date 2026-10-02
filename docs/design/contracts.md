# Contracts

## Overview

Pluto provides a contract system for **proving correctness at compile-time**. Contracts are not defensive runtime checks — they are specifications that the compiler verifies statically using whole-program analysis.

The guiding principle:

> **Contracts are specifications, not guards.** The goal is to prove violations cannot happen, not to crash when they do.

## Philosophy

Traditional approaches to correctness use defensive programming: add runtime checks, catch violations, handle errors. Pluto takes the opposite approach: **prove at compile-time that violations are impossible**.

With whole-program compilation, the compiler sees:
- All code paths
- All function calls
- All concurrent operations (`spawn`)
- All data flows

This enables static verification that most languages can't achieve. Contracts guide the verifier by specifying what must be true.

## The Four Primitives

Pluto's contract system has exactly four primitives:

| Primitive | Purpose | When Checked |
|-----------|---------|--------------|
| **`invariant`** | Class properties that always hold — single-state, or two-state with `old()` | **Compile-time proof (strict — shipped).** Runtime validation only at wire decode boundaries (single-state clauses only) |
| **`requires`** | Preconditions — what caller must prove | Compile-time at proven direct call sites (check elided); runtime at entry for every other site |
| **`ensures`** | Postconditions — the proof form only (rfc-properties.md) | **Compile-time proof at every normal exit (strict — shipped).** Never a runtime check |
| **`assert`** | Explicit runtime check | Always runtime (and establishes the fact for the prover) |

**Why these four?**
- `invariant` expresses "always true" properties of data
- `requires` expresses "must be true to call" preconditions
- `ensures` expresses what a method's exit state guarantees relative to its entry state
- `assert` bridges gaps when static proof isn't possible

**What about runtime postconditions?** Eliminated, permanently — and that
rejection is unchanged. The `ensures` that exists today is a *compile-time
proof obligation* (docs/design/rfc-properties.md, atom 1), discharged by the
same symbolic machinery as invariants and never emitted into the binary. The
runtime-checked form ("evaluate the clause when the function returns, abort
on failure") remains rejected by design: with whole-program compilation,
anything a runtime postcondition could catch, the prover either discharges
statically or rejects at compile time.

---

## 1. Invariants

**Class properties that always hold.**

### Syntax

```pluto
class Account {
    balance: int
    invariant self.balance >= 0
}
```

Multiple invariants can be declared. All must hold (logical AND).

### Semantics

An invariant is a property that is **always true** for instances of the class:
- After construction
- Whenever the object is observable during a `mut self` method (method exits —
  returns, fall-through, raises — and at any call, which might observe the
  object through the callee or an alias). *Between* writes inside a `mut self`
  method the invariant may be temporarily broken; only boundaries matter.
- Everywhere else (can be assumed by the compiler)

Any code holding a reference to an `Account` can **assume** `balance >= 0` without checking. This is hugely powerful for optimization and verification — and the prover uses it: parameter invariants and `requires` clauses are entry facts for every proof.

### Implementation: Static Discharge (strict — SHIPPED, decided 2026-09-30)

Invariants are **compile-time proof obligations**. Every site that could break
one must be statically proven not to, or compilation fails. There is no gradual
fallback to runtime checks. See `src/typeck/discharge.rs` and
[rfc-verification.md](rfc-verification.md) phase 2.

**Obligation sites:**
- **Construction** — each struct literal substitutes its field initializers
  into the invariant and evaluates them against the flow facts in scope
  (`if x >= 0 { Account { balance: x } }` proves; an unbounded initializer is a
  compile error).
- **DI construction** — instances synthesized by DI wiring (startup singletons,
  per-injection transients, scope-block auto-created instances) never pass
  through a struct literal: they are allocated zero-initialized and only their
  injected (class-typed) dep fields are wired. Their invariants must hold for
  the all-int-fields-are-zero state (`invariant self.count >= 0` proves;
  `invariant self.count > 0` on a DI-wired class is a compile error). Seeded
  scope instances are ordinary struct literals and carry the construction
  obligation above instead; scoped classes with data fields are never
  zero-constructed (the captive-dependency check forces a seed).
- **Foreign writes** (`obj.field = v` outside the class's own `mut self`
  methods) — proven immediately after the write; external code gets no
  temporary-violation window.
- **`mut self` method bodies** — writes are symbolic strong updates (each int
  field's value is tracked as a linear form over its entry value and locals),
  and the invariant is proven at every boundary: exits, raises,
  `break`/`continue`/`yield`, statements containing calls (conservative
  reentrancy answer), loop entry/body-end, and branch joins. Subtract-then-add
  proves because the symbolic forms cancel.

**What the prover knows:** `requires` clauses (runtime-checked at entry),
invariants of class-typed parameters, `if` guards, and `assert` statements
(the escape hatch — after an `assert` passes, the prover assumes it).

**Violation behavior:** a compile error naming the invariant, the site, the
symbolic state, and the fix:
```
cannot prove invariant 'self.balance >= 0' of class 'Account' holds at the end
of this method in method 'drain': at this point self.balance =
old(self.balance) - amt. ... Establish the missing bound before this point with
a guard, a 'requires' clause, or an 'assert'
```

**Boundary validation (the only runtime check):** data decoded from the wire is
testimony, not proof. `__unmarshal_T` for an invariant-carrying class re-checks
the invariants and raises `wire.WireError` on violation. Inside the compilation
unit, no runtime invariant checks exist.

### Provable Fragment

Because unprovable means rejected, invariants are restricted to what the flow-
fact engine can decide:

**Allowed:**
- `&&`, `||`, `!` over integer comparisons (`==`, `!=`, `<`, `>`, `<=`, `>=`)
- Sides are linear arithmetic (`+`, `-`, `*` by a constant) over int literals
  and **direct int fields of `self`** (`self.balance`, `self.lo + 1`)

**Rejected at declaration** ("invariant is outside the provable fragment"):
- Float, string, or boolean properties
- `.len()` and any method call, indexing, nested field access
  (`self.child.value`)
- Non-linear arithmetic (`self.x * self.y`), division, modulo
- On a generic class: any field whose declared type involves a type
  parameter (the contract must hold for every instantiation, so its
  vocabulary must be independent of the parameters — see Generics below)

This keeps verification decidable without an SMT solver. The fragment grows
deliberately (e.g. `x != const` facts were added so `invariant self.x != 0`
proves through `if v != 0` guards); float/string/collection domains are out of
scope.

### Rules

- Invariants apply to classes and objects (not enums, traits, modules)
- Multiple invariants are conjoined (all must hold)
- Invariants must be in the provable fragment (see above); on generic classes
  it is further restricted to param-independent vocabulary (see Generics)
- A write to a field no invariant mentions carries no obligation

---

## 2. Requires

**Preconditions — what the caller must prove.**

### Syntax

```pluto
fn transfer(mut from: Account, mut to: Account, amount: int)
requires from.balance >= amount
requires amount > 0
{
    from.balance = from.balance - amount
    to.balance = to.balance + amount
    // Compiler verifies: both Account invariants still hold
}
```

Multiple `requires` clauses can be declared. All must hold (logical AND).

### Semantics

A `requires` clause creates a **proof obligation** for the caller:
- The compiler must prove it's true at every call site
- OR the caller must use `assert` to establish it at runtime
- Once proven/asserted, the callee can assume it's true (no check needed inside the function)

### Current Implementation (Phase 2 + Phase 6 slice 1)

**Status:** Runtime checks at function entry, with **static call-site
discharge shipped** (2026-10-01, `src/typeck/requires.rs`): a proven direct
call site skips the entry check.

**Baseline behavior (every unproven site):**
- `requires` expressions evaluated at function entry
- If any returns false, program aborts (hard abort, like invariants)

**Violation:**
```
requires violation in transfer: from.balance >= amount
```

**Static discharge (proven sites):** at a *direct* call — a named
non-generic free function, or a method on a class/object-typed receiver
(the same scope as error-set shrinking slice 1) — the caller's live flow
facts are evaluated against every requires clause of the callee, with the
actual arguments and the receiver path substituted in (`p` ↦ the actual's
affine form, `self.f` ↦ `recv.f`, one-level `p.field` and `p.len()` leaves
supported). If **every** clause is Proven, codegen routes the call to an
unchecked twin of the callee body (`<name>$nochk`), eliding the entry
check for that site only:

```pluto
fn caller(mut a: Account, mut b: Account, amount: int) {
    if a.balance >= amount && amount > 0 {
        transfer(a, b, amount)   // proven: no runtime requires check here
    }
    transfer(a, b, amount)       // unproven: checked entry, aborts on violation
}
```

**Scope and soundness rules (conservative by construction):**
- All-or-nothing: a partially proven conjunction keeps the check.
- Facts must be live *at the call*: an interleaved call drops field facts
  (aliases may mutate), loop headers (`while` conditions, `for` iterables)
  never prove, generic callers never record (skolem-checked once).
- Generic callees never elide (their call names are rewritten per
  instantiation at monomorphization); the entry check stays.
- Calls through fn-typed values, closures, trait objects (vtables hold the
  checked entry), spawn, `at` placement, and serve/RPC entries always hit
  the checked entry — wire data is testimony, not proof.
- Entity (object) receivers prove parameter clauses only (`self.field`
  facts are never tracked for concurrently-mutating entities).
- Trait-propagated requires are part of the proof obligation: the site
  proves the trait clauses plus the method's own, or keeps the check.

**Unprovable sites are not errors.** The strict proof-or-reject form of
phase 6 ("cannot prove requires clause" as a compile error) remains future
work; today the ladder's runtime rung simply stays in place. `assert` still
establishes facts the prover uses, so an `assert amount <= 100` before a
call can turn an unproven site into a proven one.

### Rules

- `requires` applies to functions and methods (including trait methods)
- Can reference parameters, but not local variables
- Must be in the decidable fragment
- Cannot reference `self.field` in trait method contracts (traits don't know implementor fields)

---

## 3. Assert

**Explicit runtime check when static proof isn't possible.**

### Syntax

```pluto
fn caller(x: int) {
    assert x > 0       // Runtime check
    process(x)         // Compiler knows x > 0 after assert
}
```

### Semantics

`assert` is the escape hatch when the compiler can't prove something statically but you know it's true:
- Generates a runtime check
- If the check fails, program aborts (hard abort)
- After the `assert`, the compiler can assume the condition holds

### When to Use

**Use `assert` when:**
- Input comes from outside the program (user input, network, files)
- The compiler's proof system isn't sophisticated enough
- You need to establish a fact for a `requires` clause

**Example:**
```pluto
fn divide(a: int, b: int) int
requires b != 0
{
    return a / b
}

fn main() {
    let input = read_int()  // From stdin
    assert input != 0       // Can't prove statically, need runtime check
    let result = divide(10, input)  // Now compiler accepts it
}
```

### vs. Requires

| | `requires` | `assert` |
|---|-----------|----------|
| **Checked by** | Caller | At the assert itself |
| **When checked** | Compile-time (future) or function entry (current) | Always runtime |
| **Failure** | Compile error (future) or hard abort (current) | Hard abort |
| **Use case** | "Caller must prove this" | "I can't prove this, but it's true" |

---

## `ensures` — The Proof Form

**The runtime form stays dead; the proof form shipped** (rfc-properties.md
phases 1–2).

An `ensures` clause on a class/object method is a *two-state postcondition*:
a relation between the method's exit state and its entry state, with
`old(expr)` denoting the entry value. It is a compile-time proof obligation
at every **normal** exit — returns and reachable fall-through; raise paths
owe nothing (the error contract governs those edges):

```pluto
class Counter {
    value: int

    fn increment(mut self) ensures self.value == old(self.value) + 1 {
        self.value = self.value + 1
    }
}
```

- **Strict:** an exit the prover cannot discharge is a compile error (same
  policy as invariants). The relation may be broken *between* writes — only
  exits matter.
- **Fragment:** `&&`/`||`/`!` over integer comparisons of linear arithmetic
  over the class's own int fields, the method's int parameters, and
  `old(...)` of those. Out-of-fragment clauses are rejected at declaration.
- **Callers assume the relation.** After a direct call, the caller's fact
  state gains the instantiated ensures (receiver and arguments substituted) —
  feeding invariant proofs, flow-fact narrowing, and error-set shrinking
  downstream. Inside a method, a call to a sibling method with ensures keeps
  the symbolic proof state exact, which is what lets a method's own ensures
  see through such calls.
- **Context:** class/object methods only (a receiver is the state being
  related). Free functions, app/stage/trait methods, and generic classes
  reject it with a diagnostic.
- **Never at runtime:** codegen only ever extracts `requires` clauses; an
  `ensures` never reaches the binary.

`invariant` clauses may also use `old()` — a **two-state invariant** — making
monotonicity a checked claim with no new keyword
(`invariant self.epoch >= old(self.epoch)`). Obligation sites are the same as
single-state invariants where a pre-state exists: every `mut self` method
boundary (relative to the method's entry) and every foreign write (relative
to the pre-write state). Construction, DI synthesis, and wire-decode have no
pre-state: two-state clauses are code-path-only obligations there, and decode
validates single-state clauses only.

**For simple "always true" properties, still prefer invariants:**
```pluto
class Counter {
    value: int
    invariant self.value >= 0
}

fn increment(mut self) {
    self.value = self.value + 1
    // Compiler verifies: invariant still holds
}
```

**For return values, use types + invariants:**
```pluto
class PositiveFloat {
    value: float
    invariant self.value > 0.0
}

fn sqrt(x: float) PositiveFloat
requires x >= 0.0
{
    // Compiler verifies: returned PositiveFloat satisfies invariant
}
```

**Simpler, cleaner, no redundancy.**

---

## Implementation Status

| Feature | Status | Phase |
|---------|--------|-------|
| **`invariant` (static discharge)** | ✅ Implemented (strict) | Verification RFC phase 2 |
| **`requires` (runtime)** | ✅ Implemented | Phase 2 |
| **`ensures` (runtime)** | ❌ Removed — permanently | Phase 4 |
| **`ensures` (proof form, `old()`)** | ✅ Implemented (strict) | rfc-properties phase 1 |
| **Two-state invariants (`old()`)** | ✅ Implemented (strict) | rfc-properties phase 2 |
| **Trait contracts** | ✅ Implemented | Phase 3 |
| **`assert`** | ✅ Implemented (runtime check + prover fact) | Phase 4 |
| **Static `requires` discharge at call sites** | ✅ Implemented (slice 1: proven direct sites elide the entry check) | Phase 6 |

### Phase 1: Invariants — Done (upgraded to static discharge)

Originally runtime-checked; now statically discharged (see the invariant
section above). What remains from the runtime era:

- `invariant` keyword and syntax
- Syntactic decidable-fragment validator (`src/contracts.rs`), tightened by the
  provable-fragment validator (`src/typeck/discharge.rs`)
- Decode-boundary validation in generated marshalers (`src/marshal.rs`)

**Key files:** `src/contracts.rs`, `src/typeck/discharge.rs`, `src/typeck/facts.rs`, `src/marshal.rs`

### Phase 2: Requires (Runtime) — Done

Runtime-checked preconditions. **Note:** This phase also implemented `ensures`, which will be removed.

**Delivered:**
- `requires` keyword and syntax
- Runtime checks at function entry
- Hard abort on violation
- Works on functions, methods, trait methods
- 25 integration tests

**To remove:**
- `ensures` keyword and enforcement (redundant)
- `old()` expression support (only used with ensures)
- `result` keyword (only used with ensures)

**Key files:** `src/contracts.rs`, `src/typeck/register.rs`, `src/codegen/lower.rs`, `runtime/builtins.c`

### Phase 3: Trait Contracts — Done

Contracts on trait methods, enforced on implementations.

**Delivered:**
- `requires` on trait methods
- Liskov checking (impls cannot add `requires`)
- Multi-trait collision guard
- Runtime enforcement
- 13 integration tests

**Note:** Trait methods also support `ensures`, which will be removed in Phase 4.

### Phase 4: Remove `ensures` + Add `assert`

**Scope:**
- Remove `ensures` keyword from lexer/parser/AST
- Remove `ensures` type-checking and codegen
- Remove `old()` expression support
- Remove `result` keyword
- Add `assert` keyword and syntax
- Add `assert` runtime enforcement (hard abort on failure)
- Update constraint tracking to include assertions
- Update tests to use `assert` instead of defensive checks

**Estimated complexity:** Low. Removal of ensures is straightforward. `assert` is simple (just runtime check + assumption).

### Phase 5: Concurrency Safety

**Scope:**
- Detect concurrent mutations via `spawn`
- Prove operations are safe (disjoint data, read-only, or provably atomic)
- Compile error if safety cannot be proven
- Integration with invariants (prove they hold despite concurrent operations)

**Estimated complexity:** High. Requires dataflow analysis across tasks, reasoning about interleavings.

### Phase 6: Static Verification of `requires`

**Slice 1 shipped** (`src/typeck/requires.rs` + codegen unchecked twins):
proven direct call sites elide the entry-time runtime check; see "Current
Implementation" above for the exact scope. Invariant discharge — originally
part of this phase — shipped earlier as verification RFC phase 2 (strict
mode), providing the constraint tracking (flow facts) this slice reuses.

**Remaining:**
- Strict obligation propagation (unprovable site ⇒ compile error) — a
  policy decision, not an engine gap
- Generic callees (per-instantiation twins or template-level proofs)
- Facts that survive call boundaries / loop headers (would unlock the
  stdlib/json `pos <= src_len` cross-method case)
- Proof-coverage reporting in `pluto analyze`

---

## Interaction with Concurrency

With `spawn`, contracts become even more powerful:

**Example: Concurrent operations must maintain invariants**
```pluto
class Counter {
    value: int
    invariant self.value >= 0
}

fn increment(mut c: Counter) {
    c.value = c.value + 1
}

fn concurrent_example(c: Counter) {
    let t1 = spawn increment(c)
    let t2 = spawn increment(c)
    t1.get()
    t2.get()
    // Spawn deep-copies values, so each task's copy is proven at its own
    // write sites; synchronized singletons serialize mut methods. The
    // static proof covers each write path; interleaving-specific analysis
    // is Phase 5.
}
```

**What the compiler needs to prove (Phase 5):**
- No data races (conflicting mutations)
- Invariants maintained despite interleaving
- Operations are atomic where needed

**What makes operations provably safe:**
- **Disjoint data:** Tasks operate on different memory regions
- **Read-only:** No mutations, safe to share
- **Provably atomic:** Invariants hold after each atomic step

---

## Interaction with Other Features

### Error Handling

Contracts and errors are complementary:
- **Invariant violations are bugs (hard abort)**, not recoverable errors
- **`requires` violations at boundaries may raise typed errors** (future)
- **`assert` always aborts** — not catchable with `catch`

### Dependency Injection

A DI-constructed instance starts in the zero state (non-dep fields are
zero-initialized; there is no struct literal to prove anything stronger), so a
class that DI wiring constructs — a startup singleton, a transient, or a
scope-auto-created class — may only carry invariants the zero state satisfies.
A class whose invariant requires nonzero initial state must be taken out of DI
wiring: give it a `scoped` lifecycle and seed it in a scope block, where the
seed's struct literal carries the ordinary construction proof.

Contracts on DI-injected dependencies are verified through concrete types:

```pluto
class OrderService[gateway: PaymentGateway] {
    fn process(mut self, order: Order) ! PayError
    requires order.total > 0
    {
        // Compiler sees gateway.charge requires amount > 0
        // Must prove order.total > 0 (satisfied by our requires)
        self.gateway.charge(order.total)!
    }
}
```

### Generics

Generic classes and objects carry invariants and ensures under one rule:
the contract's **vocabulary must be independent of the type parameters** —
int fields and int method parameters whose declared type mentions no
parameter, and `old(...)` of those. A clause naming a param-typed field or
parameter is rejected at declaration with a dedicated diagnostic (the same
restriction that makes generic raise-summaries sound).

```pluto
class Box<T> {
    value: T
    count: int
    invariant self.count >= 0          // OK: count's type is int, no T
    // invariant self.value >= 0       // rejected: value's type involves T
}
```

Because the vocabulary cannot mention the parameters, the proof is
**template-level**: the obligations on the template's method bodies are
discharged once, under skolem substitution (the same pass that type-checks
generic bodies), and that one proof covers every instantiation.
Monomorphized copies do not re-prove. Per-instantiation obligations work
exactly like concrete classes: each construction site (`Box<int>{...}`),
foreign write, and DI synthesis of an instantiation is proven against the
template's clauses, and wire decode of a monomorphized instantiation
re-checks its single-state invariants at the trust boundary.

Typestate interaction: `where S == State`-gated methods exist only on the
instantiations their constraint names, and obligations apply to whichever
methods an instantiation has. A transition method *constructs* a new value
(often a different instantiation), and construction has no pre-state —
single-state invariants apply to the new value; two-state clauses do not
relate across the transition (relating the pre- and post-transition values
is future work, not a two-state invariant).

---

## Examples

### Example 1: Bank Transfer

```pluto
class Account {
    balance: int
    invariant self.balance >= 0
}

fn transfer(mut from: Account, mut to: Account, amount: int)
requires from.balance >= amount
requires amount > 0
{
    from.balance = from.balance - amount
    to.balance = to.balance + amount
    // Compiler verifies both invariants still hold:
    // - from.balance >= 0 (because from.balance >= amount ∧ amount > 0)
    // - to.balance >= 0 (because to.balance was >= 0 and we added positive amount)
}

fn caller(a: Account, b: Account, amount: int) {
    assert a.balance >= amount  // Runtime check
    assert amount > 0           // Runtime check
    transfer(a, b, amount)      // Compiler accepts: asserts established requires
}
```

### Example 2: Bounded Counter

```pluto
class BoundedCounter {
    value: int
    max: int
    invariant self.value >= 0
    invariant self.value <= self.max
    invariant self.max > 0
}

fn increment(mut self)
requires self.value < self.max  // Can't increment if at max
{
    self.value = self.value + 1
    // Compiler verifies: all invariants still hold
}

fn caller(mut c: BoundedCounter) {
    if c.value < c.max {  // Establish the requires
        c.increment()     // Compiler knows requires holds from if-guard
    }
}
```

### Example 3: Runtime Input

```pluto
fn divide(a: int, b: int) int
requires b != 0
{
    return a / b
}

fn main() {
    let numerator = read_int()
    let denominator = read_int()

    // Can't prove denominator != 0 statically (external input)
    assert denominator != 0  // Explicit runtime check

    let result = divide(numerator, denominator)  // Now OK
    print(result)
}
```

---

## Open Questions

- [ ] **Proof sophistication:** How powerful should Phase 6's verifier be? Simple (constant prop, ranges) or SMT solver?
- [ ] **Loop invariants:** Should we support invariants on loops for proving complex properties?
- [ ] **Quantifiers:** Should we add bounded quantifiers (`forall item in self.items: item.price > 0`)?
- [ ] **Mutation tracking:** When should we narrow invariant checks to `mut self` only?
- [ ] **External boundaries:** How should contracts interact with `extern` functions?
- [ ] **Contract testing:** Should there be a `@test` mode with extra assertions?
- [ ] **Gradual adoption:** Should contracts be opt-in per module or always enforced?
- [ ] **Concurrency primitives:** What abstractions help prove concurrent safety (atomics, locks, channels)?

---

## Contracts and Wire Evolution

Type-level contract clauses are part of a type's **wire interface**
(rfc-properties.md phase 5.5, `codegen::interface_hash`): the invariants,
`guarded_by` clauses, and `satisfies` instantiations of every type that
crosses a service boundary — the served class/entity itself and every
value class transitively reachable through its dispatchable signatures —
fold into the RPC interface hash alongside the method signatures.

The evolution rule: **changing a contract clause on a boundary-crossing
type is a breaking change**, exactly like changing a signature. Downstream
proofs assume the clauses (a consumer that decoded a `Receipt` under
`invariant self.amount >= 0` may have discharged its own obligations from
that fact), so a consumer compiled against the old contract must be
refused — the version-skew rejection at the boundary fires before
dispatch, classifying the failure as definite. A consumer pairs by
mirroring the clauses in its interface declaration; type-level clauses
are vacuously dischargeable on an interface stub (no constructions, no
writes), so mirroring is always possible and keeps the consumer's local
proofs honest.

Method-level clauses (`requires` / `ensures` / fn-level `provides`) are
deliberately **excluded** from the hash for now, pinned by test: a stub
cannot honestly mirror an `ensures` — it would have to implement it. When
a declaration-level mirroring story exists, they enter by the same rule.

## Distributed Contracts (Future)

We've punted the rest of distributed contracts for now, focusing on
**within-program** correctness. Idempotency for safe retries has since
landed through the property system (`std.verify.idempotent`, the
dedup-guard CHECKED discharge — rfc-properties.md phase 5.5), not as a
contract primitive. The remaining vision includes:

- **Effect tracking:** `effects: FileWrite(path)`, `effects: RPC(endpoint)`
- **Causality:** `causality: after payment_charged(order_id)` for ordering
- **Protocol contracts:** State machines for channel/RPC interactions

These will be addressed after the core concurrent contract system is proven.

---

## Summary

Pluto's contract system is simple and powerful:

**Three primitives:**
1. `invariant` — what's always true (classes only)
2. `requires` — what must be proven to call (functions/methods)
3. `assert` — explicit runtime check when static proof fails

**Philosophy:**
- Contracts are specifications, not defensive checks
- Prove correctness at compile-time via whole-program analysis
- Runtime checks only when static proof isn't possible (external inputs)

**Current status:**
- `invariant`: statically discharged, strict mode (verification RFC phase 2);
  runtime validation only at wire decode boundaries
- `requires`: runtime-enforced at entry (Phases 2-3 done), with proven
  direct call sites statically discharged — the entry check is elided per
  site (Phase 6 slice 1)
- `assert`: runtime check that also feeds the prover (Phase 4 done)
- `ensures`: runtime form removed permanently; proof form shipped
  (two-state postconditions with `old()`, strict discharge, caller-side
  assumption — rfc-properties.md phases 1–2)

**Next steps:**
- Strict `requires` obligation propagation + generic callees (Phase 6 remainder)
- Concurrency safety (Phase 5)

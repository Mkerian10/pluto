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

## The Three Primitives

Pluto's contract system has exactly three primitives:

| Primitive | Purpose | When Checked |
|-----------|---------|--------------|
| **`invariant`** | Class properties that always hold | Compile-time proof (future); runtime after `mut self` methods (current) |
| **`requires`** | Preconditions — what caller must prove | Compile-time at call sites (future); runtime at entry (current) |
| **`assert`** | Explicit runtime check | Always runtime |

**Why just three?**
- `invariant` expresses "always true" properties of data
- `requires` expresses "must be true to call" preconditions
- `assert` bridges gaps when static proof isn't possible

**What about postconditions (`ensures`)?** Eliminated — they're redundant. With whole-program compilation:
- Return types + invariants express what a function guarantees
- Callers can see implementations
- Invariants are checked after mutations
- No need for a separate postcondition mechanism

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
- After every method with `mut self`
- Everywhere else (can be assumed by the compiler)

Any code holding a reference to an `Account` can **assume** `balance >= 0` without checking. This is hugely powerful for optimization and verification.

### Current Implementation (Phase 1)

**Status:** Runtime checks after construction and all method calls.

**When checked:**
- After every struct literal (`Account { balance: 100 }`)
- After every method call (conservative — will narrow to `mut self` only in Phase 6)

**Violation behavior:** Hard abort — prints diagnostic to stderr and exits:
```
invariant violation on Account: self.balance >= 0
```

This is intentional. Invariant violations mean the program is in an invalid state that should never occur. They represent bugs, not recoverable errors.

### Future Implementation (Phase 6)

**Static verification:**
- Prove invariants hold after construction (from field values)
- Prove `mut self` methods preserve invariants (from requires/ensures... wait, we removed ensures!)
- Actually, prove methods preserve invariants by analyzing the method body
- Eliminate runtime checks where proven

**Runtime checks remain only:**
- Where compiler can't prove it holds
- At external boundaries (deserialization, etc.)

### Decidable Fragment

Invariant expressions are restricted to a decidable subset so the compiler can always evaluate them:

**Allowed:**
- Field access: `self.balance`, `self.items`
- Comparisons: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Logical: `&&`, `||`, `!`
- Literals: `0`, `3.14`, `true`
- `.len()` method (only allowed method call)

**Rejected:**
- Function calls
- Method calls (except `.len()`)
- Array/map indexing
- String literals or interpolation
- Closures, casts, spawn, error handling

This keeps verification decidable without an SMT solver.

### Rules

- Invariants apply to classes only (not enums, traits, modules)
- Multiple invariants are conjoined (all must hold)
- Generic classes get invariants after monomorphization
- Invariants cannot reference methods (only fields and `.len()`)

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

### Current Implementation (Phase 2)

**Status:** Runtime checks at function entry.

**Behavior:**
- `requires` expressions evaluated at function entry
- If any returns false, program aborts (hard abort, like invariants)

**Violation:**
```
requires violation in transfer: from.balance >= amount
```

### Future Implementation (Phase 6)

**Static verification via obligation propagation:**

When function `A` calls function `B`:
1. `A` must **prove** `B`'s `requires` clauses hold at the call site
2. If the compiler can prove it (from control flow, known values, etc.), no runtime check
3. If the compiler cannot prove it, compile error: "cannot prove requires clause"

**Example (provable):**
```pluto
fn caller() {
    let a = Account { balance: 100 }
    transfer(a, b, 50)  // Compiler proves: 100 >= 50 ✓
}
```

**Example (not provable, needs assert):**
```pluto
fn caller(amount: int) {
    let a = Account { balance: 100 }
    transfer(a, b, amount)  // Compile error: can't prove 100 >= amount
}
```

**Example (with assert):**
```pluto
fn caller(amount: int) {
    assert amount <= 100  // Explicit runtime check
    let a = Account { balance: 100 }
    transfer(a, b, amount)  // Compiler accepts: assert established amount <= 100
}
```

### Proof Strategy (Phase 6)

The compiler uses lightweight abstract interpretation:
1. Track known constraints at each program point (from `requires`, `if` guards, `assert`)
2. At each call site, check if callee's `requires` are entailed by current constraints
3. If not provable, compile error

Not a full SMT solver — handles linear arithmetic, comparisons, field access. Complex expressions that can't be proven require explicit `assert` or refactoring.

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

## Why No `ensures`?

**Postconditions are redundant with invariants + whole-program compilation.**

Traditional contracts have `ensures` to specify what a function guarantees:
```pluto
fn increment(mut c: Counter)
ensures c.value == old(c.value) + 1  // Awkward!
```

But in Pluto:
1. **Invariants cover state:** Classes have invariants that must hold after `mut self` methods
2. **Whole-program compilation:** Callers can see implementations
3. **No need for `old()`:** We don't need to compare pre/post values — invariants express what's always true

**Instead of `ensures`, use invariants:**
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
| **`invariant` (runtime)** | ✅ Implemented | Phase 1 |
| **`requires` (runtime)** | ✅ Implemented | Phase 2 |
| **`ensures` (runtime)** | ✅ Implemented, **will be removed** | Phase 2 |
| **Trait contracts** | ✅ Implemented | Phase 3 |
| **`assert`** | ⬜ Not yet implemented | Phase 4 |
| **Static verification** | ⬜ Not started | Phase 6 |

### Phase 1: Invariants (Runtime) — Done

Runtime-checked class invariants.

**Delivered:**
- `invariant` keyword and syntax
- Decidable fragment validator
- Runtime checks after construction and method calls
- Hard abort on violation
- 29 integration tests

**Key files:** `src/contracts.rs`, `src/typeck/register.rs`, `src/codegen/lower.rs`, `runtime/builtins.c`

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

### Phase 6: Static Verification

**Scope:**
- Prove `requires` clauses at call sites (obligation propagation)
- Prove invariants hold after construction and `mut self` methods
- Eliminate runtime checks where proven
- Abstract interpretation for constraint tracking
- Good error messages when proof fails

**Dependencies:** Phase 4 (simplified contract model), Phase 5 (concurrency safety)

**Estimated complexity:** High. This is the capstone — full static verifier.

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
    // Compiler must prove: invariant holds despite interleaving
    // Current: runtime checks after each increment
    // Future: static proof or compile error
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

Invariants work with generics after monomorphization:

```pluto
class Box<T> {
    value: T
    count: int
    invariant self.count >= 0
}

// After monomorphization: Box__int, Box__string, etc.
// Each gets the invariant check
```

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

## Distributed Contracts (Future)

We've punted distributed contracts for now, focusing on **within-program** correctness. But the vision includes:

- **Effect tracking:** `effects: FileWrite(path)`, `effects: RPC(endpoint)`
- **Idempotency:** `@idempotent(key = order_id)` for safe retries
- **Causality:** `causality: after payment_charged(order_id)` for ordering
- **Protocol contracts:** State machines for channel/RPC interactions

These will be addressed after the core concurrent contract system is proven and the RPC layer is implemented.

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
- Runtime enforcement of `invariant` and `requires` (Phases 1-3 done)
- Static verification coming (Phase 6)
- Concurrent safety coming (Phase 5)

**Next steps:**
- Remove `ensures` (redundant)
- Add `assert` (Phase 4)
- Build static verifier (Phase 6)

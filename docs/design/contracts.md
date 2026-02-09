# Contracts

## Overview

Pluto provides a contract system for verifying correctness in distributed systems. Contracts are **compile-time first**: the compiler proves what it can statically, inserts runtime checks only where static proof is impossible (external boundaries), and rejects programs that violate contract rules.

The guiding principle:

> Semantic correctness is guaranteed for the verified subset under declared external assumptions.

## Contract Types

Pluto v1 defines five contract types, ordered from data to behavior to communication:

| Contract | Applies to | Verified |
|----------|-----------|----------|
| **Invariants** | Classes, data types | Compile-time + runtime at boundaries |
| **Pre/post conditions** | Functions, methods | Compile-time (obligation propagation) |
| **Protocol contracts** | Channels, RPC | Compile-time (state machine checking) |
| **Failure semantics** | Functions, methods | Compile-time (effect compatibility) |
| **Interface guarantees** | Traits | Compile-time (obligation propagation) |

---

## 1. Invariants

Invariants are properties of data types that must always hold. Declared with `invariant` inside a class body.

### Syntax

```
class Order {
    id: string
    items: [Item]
    total: float

    invariant self.total >= 0.0
    invariant self.items.len() > 0
}
```

### When Checked

| Point | How |
|-------|-----|
| **Construction** | Compiler verifies the invariant holds after every constructor / struct literal |
| **Mutation exit** | After any method with `mut self` returns, invariants are re-verified |
| **Cross-pod ingress** | Runtime check on deserialization — data arriving from another pod is validated |
| **Cross-pod egress** | Compiler proves invariant holds before serialization (static); runtime fallback if proof fails |

### Rules

- Invariant expressions must be in the **decidable fragment**: quantifier-free, over `int`, `float`, `bool`, `string.len()`, array `.len()`, and field access. No function calls (except `.len()`).
- Multiple invariants on a class are conjoined (all must hold).
- Invariants are inherited by the type — any code that constructs or mutates an `Order` must satisfy them.

### Decidable Fragment

The constraint language is intentionally restricted so the compiler can always decide validity:

```
constraint := expr CMP expr          // comparison
            | constraint && constraint  // conjunction
            | constraint || constraint  // disjunction
            | !constraint              // negation
            | (constraint)             // grouping

expr := field_access                 // self.total, self.items.len()
      | int_literal | float_literal | bool_literal
      | expr ARITH expr             // +, -, *, /, %
```

Quantifiers (`forall`, `exists`), arbitrary function calls, and heap-dependent expressions are excluded. This keeps verification decidable without an SMT solver.

---

## 2. Pre/Post Conditions

Functions and methods can declare `requires` (preconditions) and `ensures` (postconditions) that the compiler propagates through the call graph.

### Syntax

```
fn withdraw(mut self, amount: float) float
    requires amount > 0.0
    requires self.balance >= amount
    ensures self.balance == old(self.balance) - amount
{
    self.balance = self.balance - amount
    return self.balance
}
```

### `old()` Expressions

`old(expr)` captures the value of `expr` at function entry. Only valid in `ensures` clauses. The compiler snapshots the referenced values before the function body executes.

```
ensures self.balance == old(self.balance) - amount
ensures result >= 0.0
```

`result` refers to the function's return value in `ensures` clauses.

### Obligation Propagation

When function `A` calls function `B`:

1. `A` must **prove** `B`'s `requires` clauses hold at the call site.
2. `A` may **assume** `B`'s `ensures` clauses hold after the call returns.

The compiler checks this transitively through the entire call graph. If `A` cannot prove `B`'s precondition, it is a compile error. The programmer must either:
- Add a matching `requires` to `A` (pushing the obligation to `A`'s callers), or
- Add a guard (e.g., an `if` check) before the call that makes the precondition provably true.

### Proof Strategy

The compiler uses a lightweight abstract interpretation pass:

1. Track known constraints at each program point (from `requires`, `if` guards, `let` bindings).
2. At each call site, check whether the callee's `requires` are entailed by the current constraint set.
3. After a call, add the callee's `ensures` to the constraint set.
4. At function exit, verify the function's own `ensures` hold.

This is not a full SMT solver — it handles linear arithmetic over `int`/`float`/`bool` and simple field access. Complex expressions that cannot be proven statically produce a compile error asking the programmer to add an explicit guard.

---

## 3. Protocol Contracts

Protocol contracts define the allowed interaction patterns on channels and RPC connections. They are typed state machines that the compiler verifies at compile time.

### Syntax

```
protocol OrderFlow {
    state Created
    state Validated
    state Charged
    state Completed
    state Cancelled

    Created -> Validated: validate()
    Created -> Cancelled: cancel()
    Validated -> Charged: charge()
    Validated -> Cancelled: cancel()
    Charged -> Completed: fulfill()
    Charged -> Cancelled: refund()
}
```

### Ordering Modes

For channels and RPC endpoints, the programmer declares ordering semantics:

```
@serial_by(order.customer_id)
fn process_order(self, order: Order) ! OrderError

@unordered
fn log_event(self, event: Event)
```

| Mode | Meaning |
|------|---------|
| `@serial_by(key)` | Messages with the same key are processed in order. Different keys may be parallel. |
| `@unordered` | No ordering guarantees. Enables maximum parallelism. |

Default (no annotation): **serial** — all messages processed in order.

### State Machine Verification

The compiler tracks protocol state through the program:

1. When a channel or RPC connection is created with a protocol annotation, the compiler assigns it the initial state.
2. Each method call on the connection must correspond to a valid transition from the current state.
3. Calling a method not allowed in the current state is a compile error.
4. At the end of a scope, the compiler checks whether the protocol reached a terminal state (if one is designated).

---

## 4. Failure Semantics

Failure semantics contracts declare how functions behave under retries, reordering, and delivery guarantees. These are critical for distributed correctness — they let the compiler enforce that retried or redelivered operations are safe by construction.

### Annotations

```
@idempotent(key = order_id)
@retryable(on = [Timeout, Unavailable], max = 3)
fn handle_order(mut self, cmd: PlaceOrder) Order ! OrderError {
    // ...
}

@commutative
fn increment_counter(mut self, amount: int) {
    self.count = self.count + amount
}
```

| Annotation | Meaning |
|-----------|---------|
| `@idempotent(key = expr)` | Calling this function multiple times with the same key produces the same effect as calling it once. The runtime deduplicates using the key. |
| `@retryable(on = [E1, E2], max = n)` | On errors `E1` or `E2`, the runtime may retry up to `n` times. The function body must be safe to retry. |
| `@commutative` | The function's effect is order-independent. Multiple calls can be reordered without changing the outcome. |
| `@delivery(mode)` | Declares delivery semantics for message handlers. Modes: `at_least_once`, `at_most_once`, `exactly_once`. |

### Compile-Time Rules

These are the core enforcement rules that make failure semantics contracts more than annotations — they are **compiler-enforced constraints**:

**Rule 1: Retryable requires idempotent or commutative callees.**
A `@retryable` function cannot call side-effecting code unless that code is `@idempotent` or `@commutative`. The compiler walks the call graph from every `@retryable` function and verifies each callee that has side effects (mutation, I/O, channel sends) carries a compatible annotation.

```
@retryable(on = [Timeout], max = 3)
fn process(mut self, order: Order) ! Timeout {
    self.db.save(order)!        // ERROR: db.save is side-effecting but not @idempotent
    self.counter.increment(1)   // OK: if increment is @commutative
}
```

**Rule 2: `at_least_once` delivery requires idempotent handler.**
If a channel or message handler is declared `@delivery(at_least_once)`, the consumer function must be `@idempotent(key = ...)`. This ensures that redelivered messages produce no duplicate effects.

```
@delivery(at_least_once)
channel orders: chan<PlaceOrder>

// Consumer must be idempotent:
@idempotent(key = cmd.order_id)
fn handle(mut self, cmd: PlaceOrder) ! OrderError {
    // ...
}
```

**Rule 3: Idempotency key must be stable and present.**
The `key` expression in `@idempotent(key = expr)` must be:
- A field of a function parameter (not a local variable or computed value)
- Of a hashable type (`int`, `string`, `bool`, or enum)

If the key expression references a field that doesn't exist or isn't of a hashable type, it is a compile error.

```
@idempotent(key = order.id)     // OK: field of parameter, string type
fn handle(mut self, order: Order) { ... }

@idempotent(key = compute_key()) // ERROR: key must be a parameter field, not a function call
fn handle(mut self, order: Order) { ... }
```

**Rule 4: Escape hatches downgrade guarantees.**
`extern` functions and raw thread operations are opaque to the contract checker. If a `@retryable` function calls `extern` code, the compiler emits a warning (not an error) that the retryability guarantee depends on an unverified assumption. The programmer can silence this with an explicit `@assume` annotation:

```
@retryable(on = [Timeout], max = 3)
fn send_email(self, to: string, body: string) ! Timeout {
    @assume(idempotent)
    extern fn smtp_send(to: string, body: string) ! Timeout
    smtp_send(to, body)!
}
```

### Side Effect Tracking

The compiler determines whether a function has side effects by analyzing:

1. **`mut self`** — modifies instance state
2. **Channel sends** — observable side effect
3. **`extern` calls** — assumed side-effecting unless annotated otherwise
4. **Transitive effects** — if `f` calls `g` and `g` has side effects, `f` has side effects

Pure functions (no `mut self`, no channel sends, no extern calls, no transitive effects) are trivially safe in `@retryable` contexts.

---

## 5. Interface Guarantees

Traits can specify contracts on their methods. Any class implementing the trait must satisfy these contracts.

### Syntax

```
trait PaymentGateway {
    fn charge(self, req: ChargeReq) ChargeRes ! PayError
        requires req.amount > 0
        ensures result.status != Status.Unknown

    @idempotent(key = req.transaction_id)
    fn refund(self, req: RefundReq) RefundRes ! PayError
        requires req.amount > 0
}
```

### Rules

- Trait contracts apply to all implementations. An `impl` block for a class must satisfy every `requires`/`ensures` and failure semantics annotation declared on the trait.
- Implementation methods may **strengthen** postconditions (promise more) but may not **weaken** preconditions (demand more than the trait declares). This follows the Liskov Substitution Principle.
- Failure semantics annotations (`@idempotent`, `@retryable`, etc.) on trait methods are **requirements**, not suggestions — implementations must carry compatible annotations.

---

## Verification Pipeline

The contract checker runs as a compiler pass after type checking and before codegen:

```
lex → parse → modules → flatten → prelude → ambient → typeck → CONTRACTS → monomorphize → closures → codegen → link
```

### Pass Structure

| Sub-pass | What it does |
|----------|-------------|
| **Collect** | Gather all contract declarations (invariants, pre/post, protocols, failure semantics, interface guarantees) |
| **Validate** | Check that contract expressions are in the decidable fragment |
| **Propagate** | Build obligation graph — which callers must prove which callees' preconditions |
| **Check** | Walk each function body, tracking known constraints, verifying obligations at call sites |
| **Effect check** | Walk `@retryable` functions verifying callee compatibility (Rule 1-4) |
| **Protocol check** | Track state machine transitions, verify valid sequences |
| **Emit** | Generate runtime check stubs for cross-pod boundary validation |

### What Is Proven vs. Checked at Runtime

| Aspect | Static (compile-time) | Dynamic (runtime) |
|--------|----------------------|-------------------|
| Invariants | After construction and `mut self` methods | Cross-pod ingress (deserialization) |
| Pre/post conditions | All call sites in the program | External inputs (if contracts reference them) |
| Protocol states | All transitions in the program | External protocol violations |
| Failure semantics | Effect compatibility, key validity | Actual deduplication, retry execution |
| Interface guarantees | All implementations | N/A (fully static) |

The boundary between static and dynamic follows a clear rule: **anything within the program graph is proven statically; anything crossing the program boundary gets a runtime check.**

---

## Interaction with Other Features

### Error Handling

Contracts and error handling are complementary:

- `requires` violations at external boundaries raise errors (not panics)
- `@retryable` interacts with the error system — retried errors are caught and retried automatically, non-retryable errors propagate normally
- Invariant violations at cross-pod ingress raise typed errors

### Dependency Injection

Contracts on DI-injected dependencies are checked through the concrete type:

```
class OrderService[gateway: PaymentGateway] {
    fn process(mut self, order: Order) ! PayError {
        // Compiler knows gateway.charge() requires req.amount > 0
        // Must prove order.total > 0 here
        self.gateway.charge(ChargeReq { amount: order.total })!
    }
}
```

### Concurrency

- `@commutative` operations are safe to parallelize — the compiler can use this for optimization
- `@idempotent` operations are safe to retry after task failure
- Protocol contracts extend naturally to channel communication

---

## Open Questions

- [ ] **Contract inheritance on generic types** — how do invariants interact with generics? Does `Box<T>` inherit T's invariants?
- [ ] **Quantifiers** — should a future version support bounded quantifiers (`forall item in self.items: item.price > 0`)?
- [ ] **Contract testing** — should there be a `@test` mode that inserts runtime assertions for all contracts (for debugging)?
- [ ] **`old()` implementation** — what values can `old()` capture? Deep clone for heap types? Shallow for primitives?
- [ ] **Protocol composition** — can protocols be composed or extended?
- [ ] **`@assume` scope** — should `@assume` apply to a single call, a block, or an entire function?
- [ ] **Gradual adoption** — should contracts be opt-in per module, or always enforced?

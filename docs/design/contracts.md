# Contracts

## Overview

Pluto provides a contract system for verifying correctness in distributed systems. Contracts are **compile-time first**: the compiler proves what it can statically, inserts runtime checks only where static proof is impossible (external boundaries), and rejects programs that violate contract rules.

The guiding principle:

> Semantic correctness is guaranteed for the verified subset under declared external assumptions.

## Implementation Status

| Contract | Status | Phase |
|----------|--------|-------|
| **Invariants** | Implemented (runtime checks) | Phase 1 |
| **Pre/post conditions** | Implemented (runtime checks) | Phase 2 |
| **Interface guarantees** | Implemented (runtime checks) | Phase 3 |
| **Failure semantics** | Not started | Phase 4 |
| **Protocol contracts** | Not started | Phase 5 |
| **Static verifier** | Not started | Phase 6 |

See [Implementation Phases](#implementation-phases) for the full roadmap.

## Contract Types

Pluto v1 defines five contract types, ordered from data to behavior to communication:

| Contract | Applies to | Verified |
|----------|-----------|----------|
| **Invariants** | Classes, data types | Runtime (Phase 1); compile-time + runtime at boundaries (future) |
| **Pre/post conditions** | Functions, methods | Compile-time (obligation propagation) |
| **Protocol contracts** | Channels, RPC | Compile-time (state machine checking) |
| **Failure semantics** | Functions, methods | Compile-time (effect compatibility) |
| **Interface guarantees** | Traits | Compile-time (obligation propagation) |

---

## 1. Invariants

> **Status: Implemented (Phase 1)** — runtime checks after construction and method calls.

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

**Current behavior (Phase 1):**

| Point | How |
|-------|-----|
| **Construction** | Runtime check after every struct literal |
| **After every method call** | Runtime check after any method with `self` returns |

Invariants are currently checked after *all* method calls, not just mutating ones. Pluto does not yet track `mut self` at the parser/typeck level, so the conservative approach is to check after every call. This will be narrowed to `mut self`-only methods once mutability tracking is added.

**Future behavior (with static verifier):**

| Point | How |
|-------|-----|
| **Construction** | Static proof where possible; runtime fallback |
| **Mutation exit** | After any method with `mut self` returns; static proof eliminates redundant checks |
| **Cross-pod ingress** | Runtime check on deserialization |
| **Cross-pod egress** | Static proof before serialization; runtime fallback |

### Violation Behavior

Invariant violations are **hard aborts** — the program prints a diagnostic to stderr and exits with a non-zero code:

```
invariant violation on Order: self.total >= 0.0
```

This is intentional: an invariant violation means the program is in an invalid state that should never occur. Making violations catchable would encourage defensive programming around bugs rather than fixing them. Cross-pod boundary violations may use typed errors in the future, but within a single program, violations are always fatal.

### Rules

- Invariant expressions must be in the **decidable fragment** (see below).
- Multiple invariants on a class are conjoined (all must hold).
- Invariants are inherited by the type — any code that constructs or mutates an `Order` must satisfy them.
- Invariants on generic classes work after monomorphization — checks use concrete types.

### Decidable Fragment

The constraint language is intentionally restricted so the compiler can always decide validity:

**Allowed expressions:**

| Expression | Example |
|-----------|---------|
| Field access | `self.balance`, `self.name` |
| Comparisons | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Logical operators | `&&`, `||` |
| Unary operators | `!` (negation), `-` (negate) |
| `.len()` method | `self.items.len()` — the **only** allowed method call |
| Int/float/bool literals | `0`, `3.14`, `true` |

**Rejected expressions (compile error):**

| Expression | Why |
|-----------|-----|
| Function calls | `self.validate()` — side effects, non-termination |
| Method calls (except `.len()`) | `self.name.contains("x")` — not in decidable fragment |
| Index expressions | `self.items[0]` — requires bounds reasoning |
| String literals | `"hello"` — string comparison not supported in contracts |
| String interpolation | `"{self.x}"` — not a constraint |
| Array/map/set literals | `[1, 2, 3]` — not a constraint |
| Closures | `(x: int) => x > 0` — higher-order, not decidable |
| Type casts | `self.x as float` — implicit coercion, complicates reasoning |
| Spawn | Not a constraint |
| Catch/Propagate | Error handling is not a constraint |

Quantifiers (`forall`, `exists`), arbitrary function calls, and heap-dependent expressions are excluded. This keeps verification decidable without an SMT solver.

### Compiler Pipeline

Invariant processing spans two compiler passes:

1. **During typeck (class registration):** Each invariant expression is type-checked in a scope with `self` bound to the class type. The expression must evaluate to `bool`.
2. **After typeck (`validate_contracts`):** The decidable fragment validator walks each invariant expression AST and rejects anything outside the allowed subset.
3. **During codegen:** After struct literal construction and after every method call on a class with invariants, the compiler emits code that evaluates each invariant expression and calls `__pluto_invariant_violation` if any returns false.

---

## 2. Pre/Post Conditions

> **Status: Implemented (Phase 2)** — runtime checks at function entry (requires) and exit (ensures).

Functions and methods can declare `requires` (preconditions) and `ensures` (postconditions). Currently enforced via runtime checks; static obligation propagation is planned for Phase 6.

### Syntax

```
fn withdraw(self, amount: int) int
    requires amount > 0
    requires self.balance >= amount
    ensures self.balance == old(self.balance) - amount
{
    self.balance = self.balance - amount
    return self.balance
}
```

Contracts appear between the return type and the opening `{`, one per line. Multiple `requires` and `ensures` clauses are allowed.

### Current Behavior

In the current release, `requires` and `ensures` clauses are:
- **Parsed** by the parser (syntax is stable)
- **Type-checked** as bool expressions in the function's parameter scope
- **Fragment-validated** against the decidable fragment (with `old()` allowed in ensures)
- **Runtime-enforced** — `requires` checked at function entry, `ensures` checked at function exit

Violations are hard aborts (same as invariants): the program prints a diagnostic to stderr and exits with a non-zero code.

```
requires violation in withdraw: self.balance >= amount
ensures violation in deposit: self.balance == old(self.balance) + amount
```

### `old()` Expressions

`old(expr)` captures the value of `expr` at function entry. Only valid in `ensures` clauses. The compiler snapshots the referenced values before the function body executes. Only expressions in the decidable fragment are allowed inside `old()`.

```
ensures self.balance == old(self.balance) - amount
ensures result >= 0
```

`result` refers to the function's return value in `ensures` clauses. It has the function's return type and is only valid in `ensures` (using `result` in `requires` is a compile error). For void functions, `result` is not available.

### Implementation

**Runtime enforcement (current):**
- `requires` expressions are evaluated at function entry. If any returns false, the program aborts with a diagnostic.
- `ensures` expressions are evaluated at function exit (all return paths). If any returns false, the program aborts.
- `old()` values are computed once at function entry and stored as snapshots. They are referenced during ensures evaluation.
- `result` is bound to the return value and available during ensures evaluation.

**Ensures block pattern:** The compiler creates a single ensures block that all return paths jump to, avoiding code duplication. The return value is passed as a block parameter:
```
return expr → ensures_block(val) → exit_block(val) → actual return
```

### Obligation Propagation (Phase 6)

When function `A` calls function `B`:

1. `A` must **prove** `B`'s `requires` clauses hold at the call site.
2. `A` may **assume** `B`'s `ensures` clauses hold after the call returns.

The compiler checks this transitively through the entire call graph. If `A` cannot prove `B`'s precondition, it is a compile error. The programmer must either:
- Add a matching `requires` to `A` (pushing the obligation to `A`'s callers), or
- Add a guard (e.g., an `if` check) before the call that makes the precondition provably true.

### Proof Strategy (Phase 6)

The compiler uses a lightweight abstract interpretation pass:

1. Track known constraints at each program point (from `requires`, `if` guards, `let` bindings).
2. At each call site, check whether the callee's `requires` are entailed by the current constraint set.
3. After a call, add the callee's `ensures` to the constraint set.
4. At function exit, verify the function's own `ensures` hold.

This is not a full SMT solver — it handles linear arithmetic over `int`/`float`/`bool` and simple field access. Complex expressions that cannot be proven statically produce a compile error asking the programmer to add an explicit guard.

---

## 3. Protocol Contracts

> **Status: Not started (Phase 5 target)**

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

> **Status: Not started (Phase 4 target)**

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

> **Status: Implemented (Phase 3)** — runtime checks on all implementing class methods.

Traits can specify contracts on their methods. Any class implementing the trait must satisfy these contracts. Enforcement is **runtime**: requires/ensures are checked at method entry/exit on every implementation, whether called directly or via dynamic dispatch (trait-typed parameter).

### Syntax

```
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
        ensures result >= 0
}

class MyValidator impl Validator {
    // Trait requires/ensures are enforced at runtime on this method.
    // Class methods CANNOT add requires (Liskov violation).
    // Class methods CAN add ensures (strengthening postconditions is safe).
    fn validate(self, x: int) int
        ensures result > x
    {
        return x * 2
    }
}
```

### Rules

- **Trait contracts apply to all implementations.** Every `requires`/`ensures` on a trait method is runtime-enforced on all implementing class methods, including default methods (both inherited and overridden).
- **Liskov Substitution Principle:** Implementation methods **must not** add `requires` clauses. A trait method with no `requires` effectively has `requires true`; adding any `requires` in the class would weaken the precondition, breaking substitutability. This is enforced at compile time — any `requires` on a trait impl method is a compile error, regardless of whether the trait method itself declares contracts.
- **Strengthening postconditions is allowed:** Implementation methods **may** add `ensures` clauses (promising more than the trait requires is Liskov-safe).
- **Multi-trait collision guard:** If a class implements multiple traits that define the same method name, and at least one trait has contracts on that method, the compiler rejects it at compile time. This prevents ambiguous contract merging.
- **Trait contracts can reference parameters and `result`**, but **not** `self.field` (since a trait doesn't know its implementors' field layout). Attempting to access `self.field` in a trait contract is a type error.

---

## Verification Pipeline

The contract checker runs as part of the compiler pipeline. After Phase 2, it spans typeck, validation, and codegen:

**Current pipeline:**
```
lex → parse → modules → flatten → prelude → ambient → typeck* → validate_contracts → monomorphize → closures → codegen* → link
                                                         ↑                                                        ↑
                                              invariant + requires/ensures                             invariant, requires, ensures
                                              expressions type-checked here                            runtime checks emitted here
```

**Future pipeline (with full verifier):**
```
lex → parse → modules → flatten → prelude → ambient → typeck → CONTRACTS → monomorphize → closures → codegen → link
                                                                    ↑
                                                         collect, validate, propagate,
                                                         check, effect check, protocol check, emit
```

### Future Pass Structure

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
| Invariants | After construction and `mut self` methods (future) | After construction and all method calls (current); cross-pod ingress (future) |
| Pre/post conditions | All call sites in the program | External inputs (if contracts reference them) |
| Protocol states | All transitions in the program | External protocol violations |
| Failure semantics | Effect compatibility, key validity | Actual deduplication, retry execution |
| Interface guarantees | All implementations | N/A (fully static) |

The boundary between static and dynamic follows a clear rule: **anything within the program graph is proven statically; anything crossing the program boundary gets a runtime check.**

---

## Interaction with Other Features

### Error Handling

Contracts and error handling are complementary:

- Invariant violations within a program are **hard aborts** (not errors) — they represent bugs, not expected failure modes
- `requires` violations at external boundaries will raise typed errors in the future
- `@retryable` interacts with the error system — retried errors are caught and retried automatically, non-retryable errors propagate normally

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

## Implementation Phases

### Phase 1: Class Invariants (Done)

Runtime-checked class invariants. The foundation — exercises the full pipeline (lex → parse → typeck → codegen) and establishes the decidable fragment validator that all future contract types reuse.

**Delivered:**
- `invariant` keyword and syntax in class bodies
- Decidable fragment validator (`src/contracts.rs`)
- Type-checking of invariant expressions during class registration
- Runtime checks after struct literal construction and after every method call
- `__pluto_invariant_violation` hard abort with class name and expression
- `requires`/`ensures` syntax parsed on functions/methods (forward-compatible, not enforced)
- 29 integration tests

**Key files:** `src/contracts.rs`, `src/lexer/token.rs`, `src/parser/ast.rs`, `src/parser/mod.rs`, `src/typeck/register.rs`, `src/codegen/lower.rs`, `runtime/builtins.c`

### Phase 2: Pre/Post Condition Runtime Enforcement (Done)

Runtime enforcement of `requires`/`ensures` contracts. Functions and methods with contracts get runtime checks at entry and exit.

**Delivered:**
- Type-checking of `requires`/`ensures` expressions in function parameter scope
- `old(expr)` support in ensures — snapshots values at function entry
- `result` keyword in ensures — refers to return value
- Decidable fragment validation extended for `old()` (ensures only)
- Runtime `requires` checks at function entry (hard abort on violation)
- Runtime `ensures` checks at function exit, all return paths (hard abort on violation)
- `__pluto_requires_violation` and `__pluto_ensures_violation` runtime functions
- Ensures block pattern: single block for all return paths, return value as block param
- Works on: free functions, class methods, app methods, trait default methods
- 25 integration tests (54 total contract tests)

**Key files:** `src/contracts.rs`, `src/typeck/register.rs`, `src/typeck/infer.rs`, `src/codegen/lower.rs`, `src/codegen/mod.rs`, `runtime/builtins.c`

### Phase 3: Interface Guarantees (Implemented)

Contracts on trait methods, runtime-enforced on all implementations.

**What was implemented:**
- `requires`/`ensures` parsing on trait method declarations (both abstract and default methods)
- Type-checking of trait method contracts (parameters and `result` in scope, `self.field` rejected)
- Trait contracts stored in `TraitInfo.method_contracts` and propagated to implementing class methods in codegen
- **Liskov checking:** class methods implementing a trait MUST NOT add `requires` clauses (compile error). Classes CAN add `ensures` (strengthening postconditions is Liskov-safe). This applies unconditionally — even when the trait has no contracts.
- **Multi-trait collision guard:** if a class implements two traits that define the same method name and at least one has contracts, it's a compile error
- Runtime enforcement via the same requires/ensures check mechanism from Phase 2
- Contracts work through dynamic dispatch (trait-typed parameters)
- 13 integration tests (67 total contract tests)

**Key files:** `src/parser/mod.rs` (parse_trait_method), `src/typeck/env.rs` (TraitInfo.method_contracts), `src/typeck/register.rs` (trait registration, type-checking, Liskov checking, multi-trait guard), `src/codegen/mod.rs` (contract propagation)

### Phase 4: Failure Semantics

Annotations for distributed safety: `@idempotent`, `@retryable`, `@commutative`, `@delivery`.

**Scope:**
- Annotation syntax parsing (`@` annotations on functions)
- Side effect tracking (mut self, channel sends, extern calls, transitive)
- Rule 1: retryable requires idempotent/commutative callees
- Rule 2: at_least_once delivery requires idempotent handler
- Rule 3: idempotency key validation (stable, hashable, parameter field)
- Rule 4: escape hatch warnings for extern/raw thread code, `@assume` silencing
- Runtime deduplication for `@idempotent` (key-based)
- Runtime retry loop for `@retryable`

**Dependencies:** Phase 2 (for `@assume` interaction with the verifier). Could partially start in parallel — annotation parsing and side effect tracking are independent.

**Estimated complexity:** High. Side effect analysis is a whole-program pass. Runtime deduplication requires a key-value store. Retry logic interacts with the error system.

### Phase 5: Protocol Contracts

State machine contracts for channels and RPC.

**Scope:**
- `protocol` declaration syntax
- State and transition parsing
- Ordering mode annotations (`@serial_by`, `@unordered`)
- Compile-time state tracking through program flow
- Invalid transition detection
- Terminal state verification at scope exit

**Dependencies:** Channels (done), cross-pod RPC (not started). Protocol contracts are most useful once cross-pod communication exists. Can be prototyped on channels alone.

**Estimated complexity:** High. State tracking through control flow (branches create state splits), handling of state in loops, interaction with concurrency (spawned tasks may advance protocol state).

### Phase 6: Static Verifier for Invariants

Replace runtime invariant checks with compile-time proofs where possible. Runtime checks remain only where the compiler can't prove the invariant holds.

**Scope:**
- Abstract interpretation for invariant expressions (extends Phase 2's constraint tracker)
- Prove invariants hold after construction from known field values
- Prove invariants preserved by methods from requires/ensures
- Eliminate redundant runtime checks where proof succeeds
- Narrow method checks to `mut self`-only (requires mutability tracking in parser/typeck)
- Cross-pod boundary runtime checks (ingress validation, egress proof)

**Dependencies:** Phase 2 (constraint tracking infrastructure). Also depends on `mut self` tracking being added to the parser/typeck.

**Estimated complexity:** Medium. Reuses Phase 2's abstract interpretation. The main new work is connecting invariant expressions to the constraint tracker and deciding when to elide runtime checks.

---

## Open Questions

- [ ] **Contract inheritance on generic types** — how do invariants interact with generics? Does `Box<T>` inherit T's invariants?
- [ ] **Quantifiers** — should a future version support bounded quantifiers (`forall item in self.items: item.price > 0`)?
- [ ] **Contract testing** — should there be a `@test` mode that inserts runtime assertions for all contracts (for debugging)?
- [x] **`old()` implementation** — implemented in Phase 2. Captures by value (Cranelift Variable snapshot at function entry). For heap types (strings, arrays), captures the pointer — shallow snapshot. Deep clone for heap types is a future consideration.
- [ ] **Protocol composition** — can protocols be composed or extended?
- [ ] **`@assume` scope** — should `@assume` apply to a single call, a block, or an entire function?
- [ ] **Gradual adoption** — should contracts be opt-in per module, or always enforced?
- [ ] **`mut self` tracking** — when should parser/typeck be updated to distinguish `self` from `mut self`? This affects invariant check narrowing (Phase 6) and side effect tracking (Phase 4).

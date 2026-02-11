# Contracts

Most languages punt on correctness. You write code, you write tests, and you hope the gap between what you intended and what you shipped is small. When it is not, you find out in production.

The tools available are underwhelming. **Assertions** (`assert`, `debug_assert!`, Java's `assert`) are ad hoc -- scattered through code with no semantic relationship to the functions they guard, stripped in release builds, and invisible to callers. **Property-based testing** (QuickCheck, Hypothesis) is powerful but post-hoc: it runs _outside_ the program, cannot express relationships between pre-state and post-state, and never executes in production. **Eiffel** got contracts right in the 1980s -- invariants, preconditions, postconditions, the Liskov Substitution Principle baked into the type system -- but the language never broke out of its niche. **Rust** has no contract system at all; `debug_assert!` is the best you get, and it vanishes in release mode.

Pluto builds contracts into the language. Not as annotations the compiler ignores. Not as macros that desugar to panics. As first-class constructs that the compiler type-checks, validates for decidability, and enforces at runtime on every deployment.

## Class Invariants

An invariant is a property of a class that must hold after construction and after every method call. Declare it with the `invariant` keyword inside the class body:

```
class Account {
    balance: int

    invariant self.balance >= 0
}
```

If you construct an `Account` with `balance: -1`, the program aborts. If a method leaves `balance` negative, the program aborts. There is no way to catch this -- it is not an error, it is a bug.

Multiple invariants are supported. Each must independently hold:

```
class BoundedCounter {
    count: int

    invariant self.count >= 0
    invariant self.count <= 1000

    fn increment(mut self) {
        self.count = self.count + 1
    }
}
```

After every call to `increment`, both invariants are checked. If `count` reaches 1001, the program terminates with a diagnostic on stderr:

```
invariant violation on BoundedCounter: self.count <= 1000
```

### The Decidable Fragment

Contract expressions are restricted to a **decidable fragment** -- a subset of Pluto expressions that the compiler can always evaluate without side effects or non-termination:

| Allowed | Example |
|---------|---------|
| Field access | `self.balance`, `self.items` |
| Comparisons | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Logical operators | `&&`, `\|\|`, `!` |
| `.len()` | `self.items.len() > 0` |
| Numeric and boolean literals | `0`, `3.14`, `true` |

Everything else is a compile error: no function calls, no indexing, no closures, no casts, no string literals. This is intentional. Contract expressions must be pure, total, and cheaply evaluable. The fragment is designed so a future static verifier can prove contracts at compile time without an SMT solver.

```
class Bad {
    name: string

    // Compile error: method call '.contains()' is not allowed
    // in contract expressions
    invariant self.name.contains("x")
}
```

## Preconditions and Postconditions

Functions and methods can declare `requires` (preconditions) and `ensures` (postconditions). They appear between the return type and the opening brace:

```
fn safe_divide(a: int, b: int) int
    requires b != 0
    ensures result * b == a
{
    return a / b
}
```

`requires` is checked before the body executes. `ensures` is checked after -- on every return path. Violations are hard aborts, just like invariants:

```
requires violation in safe_divide: b != 0
ensures violation in safe_divide: result * b == a
```

### `result` and `old()`

Two special names are available in `ensures` clauses:

- **`result`** refers to the function's return value. Its type matches the function's return type.
- **`old(expr)`** captures the value of `expr` at function entry, before the body executes.

Together, they let you express relationships between the pre-state and post-state of a method:

```
class Wallet {
    balance: int

    invariant self.balance >= 0

    fn deposit(mut self, amount: int)
        requires amount > 0
        ensures self.balance == old(self.balance) + amount
    {
        self.balance = self.balance + amount
    }

    fn withdraw(mut self, amount: int)
        requires amount > 0
        requires self.balance >= amount
        ensures self.balance == old(self.balance) - amount
    {
        self.balance = self.balance - amount
    }
}
```

This class has three layers of protection:

1. The **invariant** guarantees the balance is never negative -- not after construction, not after any method call.
2. The **requires** on `withdraw` guarantees callers never request more than the balance.
3. The **ensures** on both methods guarantee the implementation actually does what it claims.

If someone writes a `withdraw` that forgets to subtract the amount, the ensures clause catches it. If someone calls `withdraw` with a negative amount, the requires clause catches it. If a bug somehow drives the balance below zero, the invariant catches it.

`old()` is only valid in `ensures` clauses. Using it in `requires` is a compile error. `result` is only valid in `ensures` and is unavailable for void functions.

### Combining All Three

Invariants, requires, and ensures compose naturally. On a method call, the execution order is:

1. Evaluate `requires` clauses (abort if any fail)
2. Snapshot `old()` values
3. Execute the method body
4. Evaluate `ensures` clauses, with `result` and `old()` available (abort if any fail)
5. Check class invariants (abort if any fail)

```
class Stack {
    size: int

    invariant self.size >= 0

    fn push(mut self) int
        ensures result == old(self.size)
        ensures self.size == old(self.size) + 1
    {
        let old_size = self.size
        self.size = self.size + 1
        return old_size
    }
}
```

## Interface Guarantees

Traits can declare contracts on their methods. Every implementing class inherits those contracts -- they are enforced at runtime on every call, whether through the concrete type or through dynamic dispatch:

```
trait Validator {
    fn validate(self, x: int) int
        requires x > 0
        ensures result >= 0
}

class StrictValidator impl Validator {
    id: int

    fn validate(self, x: int) int {
        return x * 2
    }
}
```

`StrictValidator.validate` will abort if called with `x <= 0`, even though the class itself declares no contracts. The trait's contracts flow through.

### Liskov Substitution

Pluto enforces the Liskov Substitution Principle at compile time:

- **Implementations cannot add `requires`.** A trait defines the contract callers rely on. An implementation that demands _more_ from callers breaks substitutability. This is a compile error, unconditionally -- even if the trait has no contracts at all.
- **Implementations can add `ensures`.** Promising _more_ than the trait requires is always safe. Callers get a stronger guarantee.

```
trait Processor {
    fn process(self, x: int) int
        requires x > 0
}

class MyProcessor impl Processor {
    id: int

    // Compile error: cannot add requires to trait impl method
    // (violates Liskov Substitution Principle)
    fn process(self, x: int) int
        requires x > 10
    {
        return x
    }
}
```

```
class BetterProcessor impl Processor {
    id: int

    // OK: adding ensures strengthens the postcondition
    fn process(self, x: int) int
        ensures result > 0
    {
        return x * 2
    }
}
```

If a class implements two traits that both define a method with the same name, and either trait has contracts on that method, the compiler rejects it. Ambiguous contract merging is not allowed.

## Why Hard Aborts

Invariant, requires, and ensures violations are not catchable errors. They terminate the program. This is a deliberate design choice.

A violated contract means the program's logic is wrong. Not "the network is down" or "the file doesn't exist" -- those are expected failures handled by Pluto's error system. A contract violation means the programmer made a false assumption. Catching it and continuing would mean running with corrupted state, which is how silent data loss happens.

Every deployment runs with contracts active. There is no release mode that strips them. Every staging environment, every production instance, every canary deploy is checking your contracts. This is the correct default: if your contracts are too expensive to run in production, they are testing the wrong thing.

## What Comes Next (Designed, Not Yet Implemented)

The contract system has a roadmap beyond what ships today.

**Protocol contracts** define valid sequences of operations as state machines. An order must be validated before it is charged, charged before it is fulfilled. The compiler would track state transitions at compile time and reject invalid sequences.

**Failure semantics** annotations (`@idempotent`, `@retryable`, `@commutative`) declare how functions behave under retry and redelivery. The compiler would verify that retryable functions only call idempotent or commutative operations -- critical for distributed systems where at-least-once delivery is the norm.

**Static verification** would let the compiler _prove_ contracts at compile time where possible, eliminating runtime checks that can be statically decided. Runtime checks would remain only at program boundaries -- deserialized data, external inputs, cross-service calls.

## Comparison

| Feature | Pluto | Eiffel | D | Rust | Java |
|---------|-------|--------|---|------|------|
| Class invariants | Yes, runtime-enforced | Yes, runtime-enforced | Yes, via `invariant` blocks | No | No |
| Preconditions | `requires`, runtime-enforced | `require`, runtime-enforced | `in` contracts | No (`debug_assert!` only) | No (`assert` disabled by default) |
| Postconditions | `ensures` with `result` and `old()` | `ensure` with `Result` and `old` | `out` contracts | No | No |
| Decidable fragment | Yes, compiler-enforced | No restriction | No restriction | N/A | N/A |
| Liskov enforcement | Compile-time (no requires on impl) | Runtime | No | N/A | N/A |
| Active in production | Always | Configurable | Configurable | `debug_assert!` stripped | Disabled by default |
| Violation behavior | Hard abort | Exception | `AssertError` | Panic (debug only) | `AssertionError` |

Pluto's contracts are closest in spirit to Eiffel's Design by Contract. The key differences: Pluto restricts contract expressions to a decidable fragment (enabling future static verification), enforces Liskov at compile time rather than runtime, and never strips contracts from production builds.

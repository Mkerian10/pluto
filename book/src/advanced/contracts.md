# Contracts

Contracts are Pluto's system for verifying correctness. You declare properties that must hold -- invariants on data, preconditions on functions -- and the compiler enforces them. The goal is to catch bugs at compile time, with runtime checks only at external boundaries.

## Class Invariants

An invariant is a condition on a class's fields that must always be true. Declare it with `invariant` inside the class body:

```
class BankAccount {
    name: string
    balance: float

    invariant self.balance >= 0.0

    fn deposit(self, amount: float) {
        self.balance = self.balance + amount
    }

    fn withdraw(self, amount: float) bool {
        if amount > self.balance {
            return false
        }
        self.balance = self.balance - amount
        return true
    }
}
```

The invariant `self.balance >= 0.0` is checked automatically:

- **After construction** -- when you write `BankAccount { name: "Alice", balance: 100.0 }`, the compiler verifies the invariant holds
- **After every method call** -- when `deposit` or `withdraw` returns, the invariant is re-checked

If an invariant is violated, the program exits with an error message identifying the class and the failed condition.

## Multiple Invariants

A class can have any number of invariants. All must hold simultaneously:

```
class BoundedCounter {
    value: int

    invariant self.value >= 0
    invariant self.value <= 1000

    fn increment(self) {
        if self.value < 1000 {
            self.value = self.value + 1
        }
    }

    fn get(self) int {
        return self.value
    }
}
```

Here, `value` is constrained to `[0, 1000]`. The `increment` method guards against overflow, so the invariant always holds.

## Invariant Expressions

Invariants support comparisons, arithmetic, logical operators, and `.len()` on arrays and strings:

```
class NonEmptyList {
    items: [int]

    invariant self.items.len() > 0

    fn first(self) int {
        return self.items[0]
    }
}

class Rectangle {
    width: int
    height: int

    invariant self.width > 0 && self.height > 0

    fn area(self) int {
        return self.width * self.height
    }
}
```

This restricted set of expressions -- called the **decidable fragment** -- is intentional. By keeping invariants simple, the compiler can always verify them without needing an SMT solver.

### What's Allowed

| Expression | Example |
|-----------|---------|
| Comparisons | `self.x > 0`, `self.a == self.b` |
| Arithmetic | `self.x + self.y > 10` |
| Logical operators | `self.x > 0 && self.y > 0` |
| Negation | `!self.is_empty` |
| `.len()` | `self.items.len() > 0` |
| Literals | `0`, `3.14`, `true` |
| Field access | `self.balance`, `self.name` |

### What's Not Allowed

Function calls (other than `.len()`), closures, string literals, array literals, and index expressions cannot appear in invariants. If you need complex validation logic, put it in a method and call it explicitly.

## Invariant Violations

When an invariant is violated at runtime, the program prints a diagnostic and exits:

```
class Positive {
    value: int

    invariant self.value > 0
}

fn main() {
    let p = Positive { value: 0 }    // Runtime error!
}
```

This prints:
```
invariant violation on Positive: self.value > 0
```

The same check runs after method calls:

```
class Counter {
    value: int

    invariant self.value >= 0

    fn decrement(self) {
        self.value = self.value - 1   // Could violate invariant!
    }
}

fn main() {
    let c = Counter { value: 0 }
    c.decrement()    // Runtime error: invariant violation
}
```

## Preconditions (requires)

Functions and methods can declare preconditions with `requires`. These state what must be true when the function is called:

```
fn safe_divide(a: float, b: float) float
    requires b != 0.0
{
    return a / b
}
```

Methods can reference `self` in preconditions:

```
class Account {
    balance: float

    fn withdraw(self, amount: float) float
        requires amount > 0.0
        requires amount <= self.balance
    {
        self.balance = self.balance - amount
        return self.balance
    }
}
```

Multiple `requires` clauses are allowed -- all must hold.

> **Note:** In the current release, `requires` clauses are parsed and validated but not yet enforced. The static obligation propagation system (where the compiler proves callers satisfy callees' preconditions) is planned for a future release. Writing `requires` now is forward-compatible -- your contracts will be enforced once the verifier lands.

## Postconditions (ensures)

`ensures` declares what must be true when a function returns:

```
fn abs(x: int) int
    ensures result >= 0
{
    if x < 0 {
        return 0 - x
    }
    return x
}
```

`result` refers to the function's return value. `old(expr)` captures a value at function entry:

```
fn increment(self) int
    ensures self.value == old(self.value) + 1
    ensures result == self.value
{
    self.value = self.value + 1
    return self.value
}
```

> **Note:** Like `requires`, `ensures` clauses are parsed and validated but not yet enforced. They are included in the syntax now so you can start writing contracts before the verifier is complete.

## The Decidable Fragment

All contract expressions -- invariants, requires, and ensures -- must stay within the decidable fragment. This is a deliberate design choice: by restricting the expression language, the compiler can always determine whether a contract holds without resorting to heuristics.

```
// Allowed
invariant self.balance >= 0.0
requires amount > 0.0 && amount <= self.balance
ensures result >= 0

// Not allowed
invariant self.validate()              // no function calls
invariant self.items[0] > 0           // no indexing
requires compute_limit(self.account)   // no function calls
```

If you need to validate something outside the decidable fragment, use a regular `if` check in your code:

```
fn process(self, items: [int]) {
    if items.len() == 0 {
        return
    }
    // ... safe to work with items
}
```

## Future Contract Types

Pluto's contract system is designed as five layers. Class invariants and pre/post conditions are the first two. Three more are planned:

- **Protocol contracts** -- state machines for channels and RPC connections, verified at compile time
- **Failure semantics** -- annotations like `@idempotent` and `@retryable` that the compiler enforces for distributed safety
- **Interface guarantees** -- contracts on trait methods that all implementations must satisfy

These will be documented as they are implemented.

## Limitations

- Invariants are checked at runtime (after construction and after every method call). A future static verifier will eliminate redundant checks where the compiler can prove the invariant holds.
- `requires` and `ensures` are parsed and type-checked but not yet enforced. They will be enforced once the static obligation propagation system is implemented.
- The decidable fragment does not include indexing, arbitrary method calls, or string operations (besides `.len()`).
- Invariants on generic classes work after monomorphization -- the checks use concrete types.

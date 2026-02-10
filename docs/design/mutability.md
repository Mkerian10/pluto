# Mutability

## Explicit Mutability

Mutability is explicit in Pluto at two levels: **callee-side** (methods declare whether they mutate) and **caller-side** (bindings declare whether mutation is allowed).

### Callee-side: `mut self`

Methods that modify an object's fields must declare `mut self`:

```
fn count(self) int {
    return self.orders_processed          // read-only — no mut required
}

fn process(mut self, order: Order) {
    self.orders_processed += 1            // mutation — mut required
}
```

The compiler enforces this: a method without `mut self` that assigns to `self.field` is a compile error.

### Caller-side: `let mut`

Variables bound with `let` are immutable by default. To allow field assignment or calls to `mut self` methods, use `let mut`:

```
let c = Counter { val: 0 }
c.val = 1                               // ERROR: cannot assign to field of immutable variable 'c'
c.inc()                                  // ERROR: cannot call mutating method 'inc' on immutable variable 'c'
print(c.val)                             // OK: reading is always allowed

let mut c = Counter { val: 0 }
c.val = 1                               // OK
c.inc()                                  // OK
```

**Scope:** `let mut` enforcement applies to field assignment and `mut self` method calls only. Variable reassignment (`x = 2`) does not require `let mut`. Function parameters, for-loop variables, match bindings, and catch bindings are implicitly mutable.

**Nested access:** Immutability is checked on the root variable. If `let o = Outer { inner: Inner { val: 0 } }`, then `o.inner = ...` is rejected because `o` is immutable.

## Why Explicit?

In a distributed, concurrent system, knowing what mutates is critical. Explicit mutability gives the compiler information it can act on. The two-level system means both the method author and the caller opt in to mutation — neither can silently introduce it.

## Compiler Leverage

Explicit mutability enables static analysis:

- **Concurrency safety:** Immutable methods can be called from any number of processes simultaneously without locks. Mutable methods require exclusive access — the compiler enforces this.
- **Channel optimization:** Immutable data sent over channels can be shared without copying. Mutable access requires copying or ownership transfer.
- **Replication:** Immutable data can be freely replicated across pods with no consistency concerns.
- **Dead mutation detection:** The compiler warns if `mut` is declared but no mutation occurs, or if a mutated field is never subsequently read.

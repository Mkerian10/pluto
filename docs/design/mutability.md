# Mutability

## Explicit Mutability

Mutability is explicit in Pluto. Methods that modify an object's fields must declare `mut self`:

```
fn count(self) int {
    return self.orders_processed          // read-only — no mut required
}

fn process(mut self, order: Order) {
    self.orders_processed += 1            // mutation — mut required
}
```

## Why Explicit?

In a distributed, concurrent system, knowing what mutates is critical. Explicit mutability gives the compiler information it can act on.

## Compiler Leverage

Explicit mutability enables static analysis:

- **Concurrency safety:** Immutable methods can be called from any number of processes simultaneously without locks. Mutable methods require exclusive access — the compiler enforces this.
- **Channel optimization:** Immutable data sent over channels can be shared without copying. Mutable access requires copying or ownership transfer.
- **Replication:** Immutable data can be freely replicated across pods with no consistency concerns.
- **Dead mutation detection:** The compiler warns if `mut` is declared but no mutation occurs, or if a mutated field is never subsequently read.

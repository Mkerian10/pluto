# Control Flow

## Variables

```
let x = 42
let mut counter = 0
```

Variables use `let` for bindings and `let mut` for bindings where you need to mutate object state (field assignment, `mut self` method calls). Variable reassignment (`x = new_value`) does not require `mut`. See [Mutability](mutability.md) for details.

## If / Else

If/else is an expression — it returns a value:

```
let status = if order.valid() { "ok" } else { "invalid" }

if condition {
    do_something()
} else if other_condition {
    do_other()
} else {
    fallback()
}
```

## Match

Exhaustive pattern matching on enums:

```
match status {
    Status.Active {
        process()
    }
    Status.Suspended { reason } {
        log(reason)
    }
    Status.Deleted { at } {
        archive(at)
    }
}
```

## Loops

```
for item in items { ... }
while condition { ... }
loop { ... }   // infinite, break to exit
```

## Resolved

- [x] Process spawning — `spawn func(args)` returns `Task<T>`, `.get()` blocks for result (see [Concurrency](../design/concurrency.md))
- [x] Range syntax — `0..n` (exclusive) and `0..=n` (inclusive) for integer iteration in `for` loops
- [x] `loop` keyword — rejected; use `while true` instead
- [x] Closures / lambdas — arrow syntax `(x: int) => x + 1`, capture by value (see [Type System](type-system.md))
- [x] Early return — `return` in functions/methods, early return from loops via `return`

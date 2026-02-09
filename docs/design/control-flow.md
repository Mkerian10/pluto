# Control Flow

## Variables

```
let x = 42
let mut counter = 0
```

Variables are immutable by default. Use `mut` to allow reassignment.

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
    Status.Active => process(),
    Status.Suspended { reason } => log(reason),
    Status.Deleted { at } => archive(at),
}
```

## Loops

```
for item in items { ... }
while condition { ... }
loop { ... }   // infinite, break to exit
```

## Open Questions

- [ ] Process spawning — keyword and semantics (`spawn`, or something else, or declarative?)
- [ ] Range syntax — `for i in 0..10`?
- [ ] `loop` keyword — infinite loop construct (currently use `while true`)

## Resolved

- [x] Closures / lambdas — arrow syntax `(x: int) => x + 1`, capture by value (see [Type System](type-system.md))
- [x] Early return — `return` in functions/methods, early return from loops via `return`

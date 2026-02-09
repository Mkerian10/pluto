# Error Handling

## Philosophy

Errors in Pluto are a first-class language concept — not sum types, not exceptions. They are **typed**, **compiler-inferred**, and **mandatory to handle**.

The error system is split into two layers:
- **Language-level errors:** Typed, recoverable errors handled in user code via `!` and `catch`
- **Runtime-level errors:** Unrecoverable failures (OOM, stack overflow) handled by the Pluto runtime

## Defining Errors

Errors are defined with the `error` keyword. They are lightweight, struct-like types:

```
error NetworkError { addr: string, code: int }
error TimeoutError { duration: Duration }
error NotFoundError { id: string }
```

## Error Inference

The compiler infers which functions can produce errors by analyzing the entire call graph (enabled by whole-program compilation). Programmers do **not** annotate functions with error information.

```
fn foo() int {
    return 42
    // Compiler infers: foo() cannot error
}

fn bar() int {
    let x = baz()   // baz() can error
    return x + 1
    // Compiler infers: bar() can error (because baz can)
}
```

At the call site:
- If a function **cannot error**, the caller uses it directly — no handling required.
- If a function **can error**, the caller **must** handle the error via `!` or `catch`.

## Error Propagation with `!`

The `!` operator propagates an error to the caller:

```
let x = bar()!                        // propagate error
let x = bar()! "loading config"       // propagate with added context
```

Adding a string after `!` attaches context to the error — useful for debugging across distributed call chains.

Note: `?` is reserved for null/Option handling (see [Type System](type-system.md)).

## Handling Errors with `catch`

The `catch` keyword handles errors at the call site. Two forms:

### Wildcard catch

Catches any error, binding it to a variable:

```
let x = bar() catch err {
    log(err)
    fallback_value
}
```

The variable `err` is bound to the error value. The block must evaluate to a value of the same type as the non-error return.

### Shorthand catch

Provides a default value on any error:

```
let x = bar() catch default_value
```

## No Try Blocks

There are no `try { } catch { }` blocks. Each fallible call is handled individually. In distributed systems, knowing **which** call failed is critical.

## Non-Exhaustive Matching

The wildcard form (`catch err`) means exhaustive matching of every error type is **not required**. The compiler enforces that you handle *something*, not that you enumerate every possible error type. This keeps error handling clean even when a function can produce many error types.

## Raising Errors

```
fn validate(self) {
    if self.items.len() == 0 {
        raise ValidationError { field: "items", message: "cannot be empty" }
    }
}
```

## Unrecoverable Errors

Unrecoverable errors (out of memory, stack overflow, assertion failures) are **not** part of the language-level error system. They are handled by the Pluto runtime, which manages process lifecycle, crash recovery, and reporting. See [Runtime](runtime.md).

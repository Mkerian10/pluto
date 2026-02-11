# Error Handling

Every language gets error handling wrong in its own special way.

Go chose explicit returns: `val, err := doThing()`. The intent was good -- errors are values, not magic. But the result is `if err != nil` on every third line, an entire codebase of boilerplate that teaches developers to stop reading error paths. Worse, nothing stops you from ignoring the `err` return entirely.

Java chose checked exceptions. In theory, the signature tells you what can fail. In practice, developers wrap everything in `RuntimeException` to escape the type system, and `throws Exception` becomes the universal surrender flag. The community collectively decided checked exceptions were a mistake.

Rust chose `Result<T, E>`. It is correct. It is also verbose. `map_err`, `From` implementations, custom error enums with `thiserror`, the `?` operator threading through six layers of adapters -- Rust's error handling is powerful enough to model anything and ergonomic enough to model nothing without a crate.

JavaScript chose to pretend errors do not exist. `try-catch` is optional, `async` functions silently swallow unhandled rejections, and `Promise` chains lose context by default. The developer is on their own.

Pluto takes a different path.

## Errors Are a Language Concept

Pluto errors are not exceptions. There is no stack unwinding. There is no hidden control flow. There is no performance penalty for code that does not raise.

Pluto errors are not sum types. You do not write `Result<User, NotFoundError | PermissionError>` and pattern match on variants.

Errors in Pluto are their own thing: **declared types**, **raised explicitly**, **inferred by the compiler**, and **enforced at every call site**.

## Declaring Errors

An error is declared with the `error` keyword. It is a lightweight struct:

```
error NotFound {
    id: int
}

error ValidationError {
    field: string
    message: string
}

error Timeout {}
```

Errors can have fields (for context) or be empty (when the type name says enough).

## Raising Errors

The `raise` keyword creates and throws an error:

```
fn find_user(id: int) User {
    if id < 0 {
        raise NotFound { id: id }
    }
    return lookup(id)
}
```

After a `raise`, execution leaves the function immediately. No error is "returned" -- it is set in a side channel and the caller is responsible for checking it.

## The Compiler Infers Error-Ability

Here is the key difference from every other language: **you do not annotate functions with error information**. The compiler figures it out.

```
fn step1() int {
    raise Timeout {}
    return 0
}

fn step2() int {
    let x = step1()!
    return x + 1
}

fn step3() int {
    let x = step2()!
    return x + 1
}
```

The compiler walks the entire call graph: `step1` is fallible (contains `raise`), `step2` is fallible (propagates from `step1` via `!`), `step3` is fallible (propagates from `step2`).

No `throws` keyword. No `Result` return type. No error type parameter. The compiler does a fixed-point analysis across the whole program and knows exactly which functions can fail, transitively.

This means you never have to update function signatures when error-ability changes deep in a call chain. If `step1` stops raising, the compiler re-infers the entire graph and `step2` and `step3` become infallible automatically.

## Propagation with `!`

The `!` postfix operator propagates an error to the caller:

```
fn process(id: int) int {
    let user = find_user(id)!    // if find_user raises, propagate to our caller
    return user.age
}
```

This is similar to Rust's `?`, but there is no `Result` to unwrap. The `!` checks the error channel, and if an error was raised, it immediately returns from the current function, forwarding the error.

The compiler enforces that `!` is only used on fallible calls. Using `!` on an infallible function is a compile error:

```
fn safe() int { return 42 }

fn main() {
    let x = safe()!    // COMPILE ERROR: safe() is infallible
}
```

## Handling Errors with `catch`

The `catch` keyword handles errors at the call site. There are two forms.

**Shorthand catch** provides a default value:

```
let result = find_user(-1) catch default_user
let port = parse_port(input) catch 8080
```

If the call raises any error, the expression evaluates to the value after `catch`. The type must match.

**Wildcard catch** binds the error and runs a block:

```
let result = find_user(-1) catch err {
    log("User lookup failed")
    return
}
```

The block can contain multiple statements. If it does not return or exit, its final expression becomes the value:

```
let result = find_user(-1) catch err {
    let fallback = 42
    fallback
}
```

## The Compiler Enforces Handling

This is the part that matters most. In Pluto, **you cannot call a fallible function without handling the error**. It is a compile error:

```
fn main() {
    find_user(-1)    // COMPILE ERROR: fallible call must be handled with ! or catch
}
```

There is no way to accidentally ignore an error. There is no way to "forget" to check. The compiler rejects the program.

Similarly, you cannot use `catch` or `!` on infallible functions -- the compiler rejects unnecessary error handling to keep code honest:

```
fn safe() int { return 42 }

fn main() {
    let x = safe() catch 0    // COMPILE ERROR: safe() is infallible
}
```

## Method Error Handling

Error handling works identically on method calls:

```
error ConnectionFailed {
    reason: string
}

class Database {
    _host: string

    fn query(self, sql: string) string {
        if self._host == "" {
            raise ConnectionFailed { reason: "no host" }
        }
        return "result"
    }
}

fn main() {
    let db = Database { _host: "localhost" }
    let result = db.query("SELECT 1") catch "fallback"
    print(result)
}
```

The compiler infers method fallibility the same way it infers function fallibility. If any implementation of a trait method is fallible, the trait dispatch is considered fallible:

```
trait Worker {
    fn work(self) int
}

class Risky impl Worker {
    fn work(self) int {
        raise Fail {}
        return 0
    }
}

fn use_worker(w: Worker) int {
    return w.work()    // COMPILE ERROR: must be handled
}
```

## Errors and Concurrency

When you `spawn` a function that can raise, the error flows through the task handle's `.get()` call:

```
error MathError {
    message: string
}

fn divide(a: int, b: int) int {
    if b == 0 {
        raise MathError { message: "division by zero" }
    }
    return a / b
}

fn main() {
    let task = spawn divide(10, 0)
    let result = task.get() catch -1    // error surfaces here
    print(result)
}
```

The compiler knows that `.get()` on a task wrapping a fallible function is itself fallible. You must handle it with `!` or `catch` -- the same rules apply.

## Comparison

| | Go | Java | Rust | Pluto |
|---|---|---|---|---|
| Error declaration | `errors.New()` | `class extends Exception` | `enum` + `thiserror` | `error Name { fields }` |
| Signaling | Return `(T, error)` | `throw` | Return `Result<T, E>` | `raise` |
| Propagation | Manual `if err != nil { return err }` | Implicit (unchecked) or `throws` | `?` operator | `!` operator |
| Handling | `if err != nil` | `try-catch` | `match` / `map_err` | `catch` |
| Can you ignore an error? | Yes (discard return) | Yes (unchecked exceptions) | Yes (`unwrap`, `let _ =`) | No |
| Annotation burden | None (but no enforcement) | `throws` on every function | `Result<T, E>` everywhere | None (compiler infers) |
| Performance cost | None | Stack unwinding | None | None |

## What This Means in Practice

The error system is designed around one insight: **the compiler already knows the call graph**. Pluto does whole-program compilation. It can trace every call path, determine which functions can raise, and enforce handling at every site -- without the programmer writing a single annotation. The result is the safety of Rust, the ergonomics of Go, and the inference of TypeScript -- without the downsides of any of them.

Errors are not exceptions. No stack unwinding. No performance penalty. No invisible control flow. Just typed values, raised explicitly, propagated with `!`, caught with `catch`, and enforced by the compiler.

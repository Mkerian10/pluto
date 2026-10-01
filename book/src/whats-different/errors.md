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

The `catch` keyword handles errors at the call site. There are three forms.

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

**Typed catch** handles specific error types, with coverage checking. Chain typed handlers, and finish with a wildcard (or `!`) for anything not named:

```
error NotFound {
    id: int
}

error Timeout {}

fn lookup(id: int) int {
    if id < 0 {
        raise NotFound { id: id }
    }
    if id > 1000 {
        raise Timeout {}
    }
    return id * 2
}

fn main() {
    let v = lookup(-1) catch err: NotFound {
        print(f"missing id {err.id}")
        0
    } catch err {
        -1
    }
    print(v)
}
```

Inside a typed handler, the error binding has that error's type — its fields are directly accessible. The compiler checks coverage against the call's error set: name every variant and no wildcard is needed; leave one unnamed without a fallback and compilation fails.

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

## Proofs Shrink Error Sets

Error inference is not just per-function — it is sensitive to what the compiler can *prove* at each call site. If your guard makes a callee's `raise` impossible, the handling obligation disappears at that site:

```
error Insufficient {
    needed: int
}

class Account {
    balance: int

    fn withdraw(mut self, amt: int)
        requires amt > 0
    {
        if amt > self.balance {
            raise Insufficient { needed: amt }
        }
        self.balance = self.balance - amt
    }
}

fn main() {
    let mut account = Account { balance: 100 }
    let amt = 30
    if amt <= account.balance {
        account.withdraw(amt)        // no ! — Insufficient is proven impossible here
        print(account.balance)       // 70
    }
}
```

`withdraw` is fallible — but at this call site, the caller's guard (`amt <= account.balance`) refutes the raise's condition (`amt > self.balance`), so `Insufficient` vanishes from the site's required-handling set. The same call without the guard is the usual compile error:

```
account.withdraw(30)
// COMPILE ERROR: call to fallible method 'withdraw' must be handled with ! or catch
```

Contracts stop being documentation and start removing handling obligations: facts flow in through guards and `requires`, and the visible payoff is code that gets *simpler* as it gets more proven.

**What shrinks.** The shipped analysis is deliberately conservative — soundness is absolute, because a wrongly-dropped variant would be an unhandled runtime error. A variant shrinks only when:

- the `raise` is **direct** in the callee's body, with a dominating guard over the callee's parameters and the receiver's direct int fields, still holding their entry values when the guard is evaluated (a raise inside a loop qualifies too, if its guard is loop-invariant — nothing in the loop body disturbs the guarded paths);
- the call is **direct** — a named function or a method resolved on a concrete class, including generic ones.

Generic callees shrink via the template: a generic function's guard chains are syntactic and hold verbatim for every instantiation, so one summary serves all of them — provided no guard mentions a type-parameter-dependent field or parameter. That proviso is what admits the *typestate* pattern, whose state parameters are phantom and whose guards are over plain int fields:

```
error Degraded {
    code: int
}

class Held {
    tag: int
}

class Lease<S> {
    id: int
    epoch: int

    fn renew(self) Lease<Held> where S == Held {
        if self.epoch > 2 {
            raise Degraded { code: self.epoch }
        }
        return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
    }
}

fn main() {
    let h = Lease<Held> { id: 7, epoch: 1 }
    if h.epoch <= 2 {
        let h2 = h.renew()      // no ! — Degraded is proven impossible here
        print(h2.epoch)         // 2
    }
}
```

`renew` is a `where`-constrained transition on a generic class, and it still shrinks: the caller's fact on `h.epoch` refutes the raise condition, so the transition needs no handler exactly where the lease is provably fresh. Typestates and error shrinking compose — the protocol keeps its state machine and its call sites get simpler.

**What never shrinks.** Variants that arrive via propagation (`!` edges), closures, fn-typed values, trait dispatch, channel operations, and remote boundaries are untouched, and a generic *caller* never shrinks its own sites. And the caller's facts must still be alive at the call: a reassignment, a field write, or an interleaved call that could mutate the receiver kills the guard, and the obligation comes back —

```
if amt <= account.balance {
    account.withdraw(90) catch err {   // this call may change balance...
        print(0)
    }
    account.withdraw(amt)              // COMPILE ERROR: guard no longer trusted
}
```

Shrinking narrows individual call sites only. The callee's canonical error set — and therefore what `!` propagates — never changes, and handling a provably-impossible error stays legal: a `catch` on a shrunk-to-empty site still compiles, so defensive handlers survive refactors that make them unnecessary.

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

## Errors at a Distance: `NetworkError.definite`

When a call crosses a process boundary — a `remote` dependency, an `at` placement bound to another process — the compiler adds `NetworkError` to the call's inferred error set, and handling it is mandatory like any other variant. `NetworkError` carries two fields: `message`, the human-readable cause, and `definite`, the one bit that actually governs what you may do next.

Every failed boundary call is one of exactly two kinds:

- **Definite** (`definite == true`) — the request is *known* not to have been dispatched: no address bound, connection refused, the request frame never completed, or the server itself replied that it refused to dispatch (unknown method, interface-hash mismatch). The world is in a known state. You may react freely — including retrying a non-idempotent call, because the effect did not happen.
- **Ambiguous** (`definite == false`) — the request left the process and no response came back. The effect applied, or it didn't; no local information can tell you which. A blind retry of a non-idempotent effect here is the classic double-charge bug. The legitimate exits are the careful ones: retry only through an idempotency key, read the state back, fence the old attempt out, or escalate honestly.

```
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(21) catch err: NetworkError {
            if err.definite {
                // The request was never dispatched. The world is in a known
                // state — react freely, including retrying a non-idempotent call.
                print("charge did not happen; safe to retry")
            } else {
                // The request left and nothing came back. The charge may or
                // may not exist. Do NOT blind-retry a non-idempotent effect.
                print("charge outcome unknown; reconcile before retrying")
            }
            return
        }
        print(f"charged: {x}")
    }
}
```

The classification is drawn by the runtime — the only layer that knows whether the bytes left the socket — and it is truthful by construction: `definite == true` is only ever set with a warrant (the connection never opened; the length-framed request was never completed, and the server dispatches only on a complete frame; the authority itself reported the refusal). When in doubt, the runtime claims ignorance: the default is ambiguous. There are no hidden transport retries, either — each boundary call dials, sends, and reads exactly once, so retry policy stays with the caller, who is the only party holding the classification.

Two fixes worth knowing rode in with this feature. A typed `catch err: NetworkError` now matches transport-raised failures — previously they carried no type tag, silently fell through typed handlers, and escaped (only a wildcard `catch` caught them). And a server refusing to dispatch now *says so* in its reply instead of closing the connection — converting what the client could only have called ambiguous into a definite rejection, a conversion only the authority has the warrant to make.

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

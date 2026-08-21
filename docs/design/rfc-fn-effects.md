# RFC: Fallibility in function types

**Status:** Implemented
**Depends on:** closure error inference (#266), generic template checking (#268), function references (#182)

## Problem

Error-ability is inferred for named functions and (since #266) for closures
bound to local variables. But a function *value* crossing a typed boundary
loses its error identity:

```pluto
fn apply(f: fn(int) int, x: int) int {
    return f(x)          // assumed infallible — nothing forces handling
}

fn risky(x: int) int {
    raise E { code: x }  // inferred fallible
    return x
}

fn main() {
    apply(risky, 2)      // E leaks through apply at runtime
}
```

Today the only protection is *escape absorption*: passing `risky` away marks
`main` (the passer) fallible, which is both blame-shifting — `apply` is where
the unhandled call happens — and toothless when the passer is `main` itself.
The ignored tests in `generic_error_sets.rs` and `unification_failures.rs`
document the gap, and several of them already use the syntax proposed here
(`maker: fn() T!`), which predates this RFC as the intended design.

## Design

### Syntax: `!` on function *types*

A function type may declare that its value can raise:

```pluto
fn(int) int      // infallible contract: values passed here must not raise
fn(int) int!     // fallible contract: values passed here may raise
fn()!            // fallible, void return
```

`!` composes after the return type, mirroring the `!` propagation operator.
Only function **type annotations** (params, returns, fields) carry the marker
— named functions and closure literals stay fully inferred, preserving the
"no error annotations on functions" ethos. The annotation is a *contract at a
boundary*, the same role parameter types already play.

### Typing rules

`PlutoType::Fn(params, ret)` gains a fallibility flag: `Fn(params, ret, can_raise)`.

- A closure literal / function reference has `can_raise` = (inferred error
  set non-empty). This is computed, never written.
- **Subsumption:** `fn(P) R` (infallible) is assignable where `fn(P) R!` is
  expected. A callee prepared for failure tolerates a value that never fails
  (`!` on an infallible call is a runtime no-op).
- The reverse is an error:

  ```
  cannot pass fallible function 'risky' where infallible 'fn(int) int' is
  expected — handle its errors in a wrapper: (x: int) => risky(x) catch ...
  ```

- The flag participates in type equality/unification with the same
  subsumption rule (infallible value satisfies fallible pattern).

### Enforcement at call sites through values

Calling through a variable whose type is `fn(...)!`:

- must be handled with `!` or `catch`;
- a typed-only `catch` (no wildcard/shorthand arm) is accepted only when the
  value's provenance is known precisely (a local closure literal or a
  `let g = f` alias — the #266 node machinery), because only then is the
  error set known. Through an opaque parameter, a catch-all arm is required.

Calling through a variable typed `fn(...)` (infallible) stays as today:
handling is rejected as applied-to-infallible. No fallible value can inhabit
that variable, so this is sound rather than optimistic.

### Inference through values

When a fallible-typed value call is propagated (`f(x)!`), the caller's
inferred error set grows by:

- the provenance node's set when known (precise), otherwise
- all declared error types (the same conservative widening `task.get()` uses
  for unknown spawn origins).

### Escape absorption narrows

With the flag in the type, fallibility travels *with the value* through the
instrumented boundaries (arguments, `let` annotations, assignments, returns).
Escapes into those boundaries no longer absorb into the passer: a fallible
value into an infallible slot is a compile error at the exact boundary that's
wrong, and into a fallible slot it is the receiver's declared responsibility
(the passer stays clean). The conservative #266-style absorption remains only
for positions the boundary check does not yet see (array/map elements, struct
fields), as a soundness net.

## Migration

Existing `fn(...)` annotations become infallible contracts. Code that passes
fallible closures into them stops compiling — that code was silently leaking
errors at runtime, which is the bug this closes. The fix at each site is
either `!` on the annotation (and handling inside the callee) or a wrapper
closure that handles before the boundary. Stdlib higher-order signatures
(`std.collections`) stay infallible in this RFC; fallible variants can follow
demand.

## Out of scope

- Precise error *lists* in types (`fn() int ! E1, E2`) — the single flag plus
  provenance-precision covers today's needs; lists can be layered on later
  without breaking this syntax.
- Method values / bound methods.
- Effect polymorphism (`fn` types generic over fallibility) — a generic HOF
  that wants to accept both writes the fallible signature and uses `!`.

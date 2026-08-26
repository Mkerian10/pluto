# RFC: Entry-Point Semantics — main(), Exit Codes, Nullability, and Errors

**Status:** Accepted (codifies shipped behavior; restriction on main return types implemented alongside this RFC)
**Author:** Matt Kerian
**Date:** 2026-08-26
**Resolves:** issue #127
**Related:** [Program Structure](program-structure.md), [Stages RFC](rfc-entry-points.md), [Nullability Inference RFC (rejected)](rfc-nullability-inference.md), [Fn Effects RFC](rfc-fn-effects.md)

## Summary

This RFC gives a unified answer to the cluster of questions from issue #127:
what `main()` may return, how errors and `none` leave a program, whether `?`
and `!` work in entry points, and how these rules differ across program
kinds (functions-with-main, apps, stages, scripts, libraries).

The core principle: **`main` is the process boundary.** Inside the program,
nullability and fallibility are typed values and inferred effects with
mandatory handling. At the boundary they degrade into the only vocabulary a
process has — an exit code and stderr. Every rule below follows from that
principle.

Most of this RFC codifies behavior that already shipped incrementally (the
runtime error contract in #274, nullable ergonomics in #276, the campaign
tests in #283–#295). One restriction is new: `main` may only return
exit-code-shaped types.

## Decisions at a glance

| Question (#127) | Decision |
|---|---|
| Nullability inference | **Stays rejected** — see [rfc-nullability-inference.md](rfc-nullability-inference.md). `T?` is explicit; ergonomics come from `??` and flow narrowing. |
| `main` return types | `void`, `int`, or `int?` — nothing else (enforced at typeck) |
| `int` return | The exit code, like C/Go/Rust |
| `int?` return / `?` in main | `none` exits 0; a value exits with that code |
| Errors escaping `main` | Runtime prints `pluto: unhandled error escaped main: <Type>` and exits 1 |
| `?` in `main` | Works — `none` propagation from `main` is a successful early exit |
| `!` in `main` | Works — propagation from `main` hits the runtime boundary above |
| Implicit fallibility of `main` | No — call sites in `main` still require `!`/`catch` like everywhere else; only `raise` and `!`-propagation reach the boundary |
| `app main(self)` | `void` only; the lifecycle owns process exit |
| Scripts | Future work; specified here so it can't drift |

## 1. Nullability stays explicit

Question 1 of #127 was decided separately and is not reopened:
[rfc-nullability-inference.md](rfc-nullability-inference.md) rejected
inferring nullable returns. Signatures carry `T?`; the annotation burden is
addressed by `??` (null coalescing) and flow narrowing (`if x != none`,
guard idioms), which shipped in #276.

The relevance to this RFC: because nullability is explicit, `main`'s
signature fully determines its boundary behavior. There is no "main became
nullable because of an internal change" hazard.

## 2. main() return types

`main` may return exactly:

- **`void`** — the default. Exit code 0 on normal completion.
- **`int`** — the returned value is the process exit code.
- **`int?`** — `none` exits 0; a value exits with that code.

Any other return type is a compile error:

```
error: main must return void, int, or int? (the exit code), found string?
```

Rationale: the return value of `main` **is** the exit code — there is no
caller to receive anything richer. Before this restriction, `fn main()
string?` compiled and truncated a heap pointer into a garbage exit status.
Exit codes are the only meaningful codomain, so the type system now says so.

Why `int?` at all: it makes `?` compose in `main` (§4). `none` mapping to 0
follows from what `?` means at the boundary: "nothing to continue with,
stop here" — which for a process is a successful, empty completion. A
program that wants "absence is failure" says so explicitly
(`return v ?? 1`), keeping the failure decision visible in code.

Rejected alternatives from #127:

- **`Result<(), Error>`-style main** — Pluto deliberately has no result sum
  type; errors are an inferred effect, not a value (see
  [error-handling.md](error-handling.md)). Wrapping them back into a value
  at exactly one place in the language would be a seam.
- **Special-casing `?` inside `main` without a nullable return** — `?` in a
  `void` main works today by the general early-return rule (§4); no special
  case is needed.

## 3. Errors at the boundary

Inside the program, fallibility is inferred and handling is mandatory:
calling a fallible function without `!` or `catch` is a compile error —
**including in `main`**. `main` is not implicitly a `catch`-all; the type
system's guarantees don't relax in the entry point.

What `main` uniquely may do is let an error **escape**: a direct `raise`,
or a `!`-propagation, has no caller to propagate to. The runtime owns that
boundary (shipped in #274):

```
$ ./myprog
pluto: unhandled error escaped main: PaymentDeclined
$ echo $?
1
```

- Exit code is 1 for any escaped error.
- The error's type name goes to stderr; stdout stays clean.
- This is reported, not silent — an escaped error is always visible.

The three options from #127 §4 resolve as: option 1's *behavior* (uncaught
errors become a nonzero exit) with option 2's *ergonomics* (no `!`
annotation on main — fallibility is inferred everywhere, and `fn main()!`
syntax does not exist), while keeping compile-time handling requirements
for calls. Programs that want specific exit codes for specific errors
`catch` them and `return` the code — which is exactly what the `int` return
type is for:

```
fn main() int {
    run() catch e: ConfigError {
        return 2
    } catch e {
        return 1
    }
    return 0
}
```

## 4. `?` and `!` in main

Both work, by the general rules, with the boundary supplying the meaning of
"propagate out of main":

- **`?` in a `void` or `int?` main:** `expr?` on `none` returns from `main`
  early — process exits 0. In an `int?` main the propagated `none` is the
  return value, which maps to 0. The "blocked use case" from #127
  compiles as written:

  ```
  fn main() {
      let val = some_function()?
      print(val)
  }
  ```

- **`!` in main:** `expr!` on an error leaves `main`; the runtime boundary
  prints and exits 1 (§3).

The asymmetry is deliberate and mirrors the semantics of the two effects:
absence (`none`) is an expected outcome, so its boundary meaning is success
with nothing to report; an error is exceptional, so its boundary meaning is
failure with a report.

## 5. Program kinds

The kinds are **orthogonal entry-point models**, not a hierarchy. What
varies is who owns the process boundary:

| Kind | Entry point | Boundary owner | Exit codes |
|---|---|---|---|
| Function program | `fn main()` / `int` / `int?` | the language runtime | via return value / escaped error |
| App | `app A { fn main(self) }` — `void` only | the DI container + lifecycle | lifecycle-owned (future: stage hooks) |
| Stage | inherited lifecycle methods | the stage runtime | stage-defined |
| Script (future) | top-level statements → synthetic `void` main | the language runtime | same rules as `void` main |
| Library | none | n/a | n/a |

**App `main(self)` stays void.** An app is a wired object graph with a
lifecycle, not a function — its `main(self)` is one lifecycle method among
several (start/stop/shutdown hooks in the stage model). Letting it return
an exit code would couple "one method's return" to "process exit" in a
construct whose whole point is that the lifecycle owns the process. Exit
codes for long-running programs belong to the stage lifecycle (a `Daemon`
stage deciding its shutdown status), designed in
[rfc-entry-points.md](rfc-entry-points.md). Errors escaping an app's
lifecycle methods hit the same runtime boundary as §3.

**Scripts are specified, not yet implemented.** When top-level statements
land (per [program-structure.md](program-structure.md)), they wrap into a
synthetic `void` main, and every rule above applies unchanged: `?` early-
exits 0, escaped errors report and exit 1. No `script` keyword — the file
shape (top-level statements present) is the discriminator, and adding a
keyword would only create a redundant way to say the same thing.

## 6. What this unblocks

- The `?`-in-main limitation from #127 is confirmed resolved; the two
  guarded tests in `tests/codegen/_11_nullable.rs` are enabled.
- `fn main() <bad type>` no longer compiles into garbage exit codes.
- Scripts and stage exit-code design have a fixed contract to build
  against.

## Compatibility

Everything here except the return-type restriction is already-shipped
behavior. The restriction only rejects programs whose exit codes were
already meaningless (pointer/float truncation), so no working program
changes behavior.

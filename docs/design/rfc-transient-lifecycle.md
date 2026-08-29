# RFC: Transient Lifecycle

**Status:** Accepted (implemented alongside this RFC)
**Author:** Matt Kerian
**Date:** 2026-08-29
**Related:** [Scoped Classes and Seeds](rfc-manual-bracket-deps.md), [DI Lifecycle RFC (archive)](archive/rfc-di-lifecycle.md)

## Summary

Implements the third and final lifecycle from the archived DI lifecycle RFC:
`transient` classes get a **fresh instance at every injection point**. Two
classes that each declare `[f: Fresh]` get different `Fresh` instances; the
transient's own singleton dependencies stay shared.

```
class Log { n: int }

transient class Fresh[log: Log] { ... }

class A[f: Fresh] { ... }
class B[f: Fresh] { ... }

app MyApp[a: A, b: B] { ... }
// a.f != b.f          — distinct transient instances
// a.f.log == b.f.log  — the singleton beneath them is shared
```

## Rules

- **Fresh per injection point.** Startup wiring creates a new instance for
  every field that injects a transient class (recursively: a transient's
  transient deps are fresh again). Inside a scope block, each injection
  point in the scope's graph gets a fresh instance at scope entry.
- **Transients do not propagate lifecycle.** A class depending on a scoped
  class becomes scoped (its identity must vary per scope). A class
  depending on a *transient* keeps its own lifecycle — the transient is a
  private per-injection copy and ties nothing. A singleton holding a
  transient is therefore legal and stays a singleton.
- **Injected transients must be auto-constructible.** There is no seed
  mechanism for transients (seeds are per-scope; transients are
  per-injection), so a transient with non-injected fields would be silently
  zero-filled. Rejected at compile time:

  ```
  error: transient class 'Cfg' has non-injected fields and cannot be
  auto-created for injection into 'Service'; transient classes must be
  auto-constructible (only injected dependencies)
  ```

  Manual construction (`let c = Cfg { port: 80 }`) remains legal — like
  `scoped`, the keyword governs DI wiring, not plain values.
- **Mixing matrix.**

  | consumer \ dep | singleton | scoped | transient |
  |---|---|---|---|
  | singleton | shared | captive check (#299) | fresh private copy |
  | scoped | shared | per-scope | fresh per injection in scope |
  | transient | shared | scope-only (captive at startup) | fresh, recursively |

  A transient that reaches a seed-requiring scoped class from app startup
  is the existing captive-dependency error; inside a scope block the
  transient wires to the scope's instances.
- **Cycles.** Transient cycles would mean infinite instantiation; the
  existing circular-dependency check rejects them.
- **App/stage lifecycle overrides** (`transient Logger` inside an app)
  continue to work and are validated by the same auto-constructible rule.

## Implementation

- `typeck/register.rs` — lifecycle inference skips transient deps when
  propagating (previously the min-propagation marked consumers of
  transients as transient, which would have made them uncreatable);
  auto-constructible validation at the end of `validate_di_graph`.
- `typeck/env.rs` / `typeck/check.rs` — `FieldWiring::Transient(name)`:
  scope resolution marks transient deps for per-injection creation and
  computes wirings for transient classes transitively (a worklist, since a
  transient's own deps need wiring too).
- `codegen/mod.rs` — startup synthesis skips shared-instance creation for
  transient classes and calls `emit_startup_transient` (recursive fresh
  alloc + wire) at each injection edge, for classes, apps, and stages.
- `codegen/lower/mod.rs` — `emit_transient_instance` handles
  `FieldWiring::Transient` inside scope blocks (creation, seed patching,
  and binding sources).

## Future work (unchanged from the archived RFC)

- Transient initialization beyond auto-construction (factory functions) —
  would lift the auto-constructible restriction.
- Per-`spawn` transient semantics.

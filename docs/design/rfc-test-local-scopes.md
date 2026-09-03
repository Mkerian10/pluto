# RFC: Test-Local DI Containers

**Status:** Accepted (implemented alongside this RFC)
**Author:** Matt Kerian
**Date:** 2026-09-03
**Related:** [Scoped Classes and Seeds](rfc-manual-bracket-deps.md), [Transient Lifecycle](rfc-transient-lifecycle.md)

## Summary

DI-wired code was untestable: test mode strips the `app` (so no singleton
graph exists), singleton classes could not be seeded, and dep-bearing
classes cannot be constructed manually. The scoped-class RFC deferred
"mock substitution" as future DI-override work.

This RFC implements that override mechanism with **zero new syntax**:
inside a `test` body, a `scope()` block is a test-local DI container.

```
class Database {
    tag: string
    fn query(self) string { return self.tag }
}

class Service[db: Database] {
    fn run(self) string { return self.db.query() }
}

test "service with fake db" {
    scope(Database { tag: "fake" }) |svc: Service| {
        expect(svc.run()).to_equal("fake")
    }
}
```

## Rules

- **Inside test bodies only**, `scope()` seeds and bindings may name
  classes of any wiring lifecycle (scoped or singleton). The seed literal
  is the override: it provides the instance the container wires everywhere
  that type is injected.
- **Unseeded singletons reached by the container are auto-created
  scope-locally** if auto-constructible (only injected fields); a stateful
  singleton must be seeded, with the same message shape as scoped seeds.
- **Each scope block is an isolated container** — two blocks in one test
  get independent instances (fresh fakes per block).
- **Outside tests nothing changes**: singleton seeds are still rejected
  (the hint now mentions test bodies), and production singletons keep the
  one-instance contract the DI graph verifies.
- Transient deps behave as always: fresh per injection point, inside test
  containers too.

## Why singletons must be scope-local in tests

Test mode has no `app`, so the synthesized startup wiring never runs and
the singleton globals are never initialized. Before this RFC, a scope
block inside a test that wired a singleton dependency read a null global
and crashed at runtime (silently, mid-test). In test resolutions the
`Singleton` wiring source is never emitted; every singleton the container
needs is either the seed or a scope-local instance.

## What this deliberately is not

Behavior mocks — substituting different *method implementations* — would
require trait-typed bracket deps plus a binding mechanism, overturning two
settled decisions (auto-wire by specific concrete type; no named
bindings). Fakes here are state-driven: fields provided by the seed
determine behavior. If real behavior substitution is ever wanted, it needs
its own RFC.

## Implementation

- `typeck/env.rs` — `test_fns: HashSet<String>` populated from
  `program.test_info` (non-empty only in test mode).
- `typeck/check.rs` (`check_scope_stmt`) — a `scope_local` predicate
  (`scoped || (in_test && singleton)`) relaxes the seed gate, the
  dependency BFS, auto-creatable validation, and the creation set. Wiring
  resolution prefers created-in-scope instances over the singleton-global
  fallback (behavior-neutral outside tests, where singletons are never in
  the creation set).
- Codegen is untouched: test containers reuse the scope-block lowering
  wholesale.

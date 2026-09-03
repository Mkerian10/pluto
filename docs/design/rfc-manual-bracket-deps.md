# RFC: Scoped Classes, Seeds, and the Rejection of Manual Bracket Injection

**Status:** Accepted (implemented alongside this RFC)
**Author:** Matt Kerian
**Date:** 2026-08-26
**Resolves:** issue #125
**Related:** [Dependency Injection](dependency-injection.md), [DI Lifecycle RFC (archive)](archive/rfc-di-lifecycle.md)

## Summary

Issue #125 asked for syntax to manually provide bracket dependencies when
instantiating scoped classes — `Handler[self.logger] { request_id: 100 }`.
This RFC **rejects manual bracket injection** and instead completes the
existing scope-block mechanism so it covers every use case the manual syntax
was meant to serve:

1. **Seeds may now carry bracket deps.** A seed literal for a scoped class
   provides exactly its non-injected fields; the compiler wires the injected
   ones from the graph (singletons, other seeds, auto-created scoped
   instances). This closes the gap where a scoped class with both regular
   fields *and* dependencies could be neither seeded nor auto-created.
2. **The lifecycle rules from the archived DI lifecycle RFC are now
   enforced:** effective lifecycles are inferred (a class depending on a
   scoped class is itself scoped), and captive dependencies — the app's
   startup graph reaching a scoped class that *needs a seed* — are rejected
   at compile time instead of silently zero-filling its fields.

## Why manual injection is rejected

Pluto's DI is **auto-wired by specific type**; that is a settled design
decision, and it is what makes the wiring verifiable. Manual injection
would reintroduce the failure modes DI exists to prevent:

- **Captive deps by hand.** `let h = Handler[self.logger] { ... }` lets a
  singleton smuggle a scoped instance past the lifecycle checks — the exact
  bug the captive-dependency rule exists to reject.
- **Two wiring systems.** Every reader would have to ask "was this wired by
  the compiler or by hand?" per construction site, and refactors that add a
  dependency would break every manual site instead of none.
- **No remaining use case.** Each motivation in #125 is served by scope
  blocks after this RFC (see below). The testing use case — injecting mocks
  — is a *substitution* problem (replacing what the graph wires), which
  manual construction only fakes; it belongs to a future DI-override
  feature, not to construction syntax.

## The completed scope-block model

### Seeds with bracket deps

```
class Logger {
    fn log(self, msg: string) { print(msg) }
}

scoped class Handler[logger: Logger] {
    request_id: int
    user_id: int

    fn process(self) {
        self.logger.log("processing")
        print(self.request_id)
    }
}

app MyApp[logger: Logger] {
    fn main(self) {
        // The seed provides the regular fields; the compiler wires `logger`
        scope(Handler { request_id: 100, user_id: 200 }) |h: Handler| {
            h.process()
        }
    }
}
```

Rules:

- A seed literal provides **exactly the non-injected fields** — all of
  them, and only them. Providing an injected field is an error ("field
  'logger' of 'Handler' is an injected dependency and cannot be provided in
  a literal; the scope block wires it"); missing a regular field is the
  usual arity error, phrased for seeds.
- Injected fields of a seed are wired **after** the scope's auto-created
  instances exist, so a seed's dependencies may be singletons, other seeds,
  or scoped classes created in the same block. Wiring sources are resolved
  at compile time (the existing `FieldWiring` machinery), zero runtime
  lookup.
- Everywhere *outside* seed position, constructing a dep-bearing class
  remains an error ("cannot manually construct class ... with injected
  dependencies") — unchanged.
- A dep-**less** scoped class remains an ordinary constructible value
  anywhere (`let a = A { x: 1 }`): `scoped` governs how the DI graph wires
  and shares instances, not whether a plain value can exist. Seeds are
  precisely this usage.

### Covering #125's use cases

| #125 use case | Mechanism |
|---|---|
| Request handler needing Database/Logger singletons | scope block; singletons wire automatically into seeds and auto-created classes |
| Scoped class with request-specific fields *and* deps | seed with bracket deps (this RFC) |
| Nested scoped objects (`Outer[inner: Inner]`) | seed `Inner`, bind `Outer` — auto-created and wired |
| Test instances without the full container | test-local scope containers: inside `test` bodies, `scope()` may seed singleton classes (the override) and auto-creates the rest — see [Test-Local DI Containers](rfc-test-local-scopes.md) |

### Answers to #125's design questions

- **Syntax:** none added. The struct-literal grammar is unchanged; seed
  position is the only place dep-bearing literals are legal, and they name
  only regular fields.
- **Type checking:** seed literals are checked against the class's
  non-injected field set; injected fields are checked by the scope wiring
  resolver (existing behavior).
- **Partial provision:** impossible by construction — regular fields all
  come from the literal, injected fields all come from the graph. There is
  no mixed mode to specify.
- **Nested deps:** the scope resolver already computes a topological order
  over the block's scoped classes; seeds now participate as wiring
  *targets* as well as sources.

## Lifecycle enforcement

The archived lifecycle RFC specced inference; the existing `di_lifecycle`
tests define how it interacts with the app. Both are now enforced
coherently in the DI validation pass:

- **Inference:** `effective(class) = scoped` if it is declared `scoped` or
  any dependency is scoped-effective (fixpoint over the dependency graph).
  Depending on longer-lived classes is always fine — scoped→singleton is
  the normal case.
- **The app is the root scope.** The app's startup graph MAY hold
  scoped-effective classes: they get one instance for the app's own wiring
  (its "scope" is the process), while scope blocks still create fresh ones.
  This preserves the shipped semantics the `di_lifecycle` suite pins
  (`app MyApp[svc: Svc]` where `Svc[ctx: Ctx]` and `Ctx` is a dep-less
  scoped class — legal, runs, prints).
- **Captive = unseedable at startup.** What the root scope *cannot* do is
  seed: it has no seed expressions. So reaching a scoped class with
  non-injected data fields from the app's bracket deps is rejected —
  previously those fields were silently zero-filled:

  ```
  error: captive dependency: scoped class 'Ctx' has non-injected fields
  and needs a seed, but it is reached from 'MyApp' at app startup, where
  no seed exists; create it inside a scope block instead
  ```

  The inferred variant reports the chain ("'Svc' is scoped (it depends on
  a scoped class) and has non-injected fields...").
- **Ambient registrations are exempt** from the startup BFS: a scoped
  ambient (`app { ambient RequestCtx }`) is seeded per scope block, which
  is its whole point.
- **Scoped-effective classes not reached from the app are always legal** —
  they are simply created via scope blocks.
- **Transient stays deferred.** The keyword parses and transient classes
  are excluded from scope wiring, but transient-vs-scoped/singleton mixing
  rules are unspecified; the tests for them are ignored with that reason
  and will define the acceptance criteria when transient semantics land.

## Implementation

- `typeck/infer.rs` — struct-literal checking gains a seed mode (an env
  flag set only while inferring seed expressions, consumed by the literal
  arm): dep-bearing classes are allowed, exactly the non-injected fields
  required, injected fields in the literal rejected by name.
- `typeck/check.rs` (`check_scope_stmt`) — field wirings are computed for
  seed classes as well as auto-created ones; `ScopeResolution` records the
  seed classes so codegen can find them.
- `codegen` (`lower_scope`) — after auto-created instances are allocated
  and wired, seed instances get their injected slots patched from the
  resolved sources. Seed literals allocate zeroed memory, so unwired slots
  are never garbage.
- `typeck/register.rs` (`validate_di_graph`) — the inference fixpoint and
  the seedability BFS over the app/stage startup bracket deps (ambients
  excluded).

### Known edge (documented, not blocking)

Class invariants on a seed class run at literal construction, before the
scope block patches injected fields — an invariant that reads an injected
dependency would observe a null slot. Invariants over injected deps are
rare (deps are wiring, not data); if this bites, invariant emission for
seed literals can be deferred to post-wiring in a follow-up.

## Compatibility

No existing program changes meaning. Seeds with bracket deps were
previously rejected outright, and every newly-rejected program (captive
dependencies) was previously miswired silently — the app graph constructed
seed-requiring scoped classes with zero-filled fields.

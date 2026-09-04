# The Distributed Model: Logical Placement, Physical Execution

**Status:** Canonical model (semantics below marked established vs. open)
**Date:** 2026-09-04
**Supersedes the framing in:** communication.md (rewritten), archive/rfc-rpc-implementation.md
**Related:** rfc-distributed-safety.md, program-structure.md, rfc-wire-format.md, rfc-evolution-rules.md

## Pluto is not an RPC language

Pluto is a **distributed language**. Programmers still think about placement, data movement, latency, partial failure, concurrency, security, and ownership — Pluto's job is to provide language-level primitives for expressing those things, and a compiler that checks them. RPC may be generated internally, but it is an implementation mechanism, not the programming model. Pluto must never be presented as "making RPC calls look local."

## The central principle

> **Pluto separates logical distribution from physical deployment.**

Most distributed systems collapse three different concerns:

1. **Application logic** — what computation should happen.
2. **Logical placement** — which domain owns the computation, data, and capabilities.
3. **Physical execution** — which process, pod, machine, or region runs it.

Pluto makes logical placement part of the program, and lets the compiler and deployment system derive physical execution.

## Placement expressions: `at`

```pluto
let result = at payments {
    charge(order)
}
```

This means: **"evaluate this expression in the logical `payments` execution domain."** It does not mean "make an RPC call."

Depending on the deployment plan, the compiler may implement it as:

- a network call to another cluster
- communication with another pod
- local IPC
- a direct call inside the same process
- potentially an inlined computation

The compiler may colocate or fuse stages when doing so **preserves their semantics**. A program can be redistributed — split apart or collapsed together — without rewriting business logic and without manually replacing local calls with RPC clients.

The analogy is a database's separation of the **logical query** from its **physical execution plan**. A Pluto program describes a logically distributed computation; the compiler and deployment system produce and optimize its physical plan.

## Distribution is explicit, not invisible

This model must not make distribution disappear. `at` is a visible, syntactic boundary: it tells the reader that this computation belongs to another logical domain, with everything that implies. The programmer still designs for distributed behavior; Pluto's contribution is that the compiler **checks and derives** the things programmers otherwise track by convention:

- which values enter and leave the domain
- whether captured values can be transferred at all
- which capabilities, services, and secrets are available inside the domain
- deadlines and cancellation propagation
- retry and idempotency policies
- parallel and structured distributed computation
- contract compatibility between separately deployed stages
- data-location, security, and residency constraints
- failures where the caller **cannot know whether the operation completed**

That last item deserves emphasis: an `at` boundary can fail ambiguously (the request may or may not have executed). This failure mode is part of the boundary's contract and must be expressible and checkable — it is precisely what "making it look local" papers over.

## Logical boundaries survive colocation

When the physical plan places two domains in the same process, the logical boundary **still means something**. Optimization is legal only when it preserves boundary semantics. Colocation must not:

- introduce shared mutable references across the boundary (values that would have been copied over a wire must not silently alias)
- expose secrets or capabilities of one domain to the other
- change promised cancellation or failure behavior (code written to survive a domain being unreachable must not acquire a new implicit reliability guarantee it will lose when redeployed)

Fusing and inlining are physical-plan optimizations, and like a database's plan optimizations they carry a correctness obligation: same observable semantics, or the optimization is illegal.

## Layered errors

Errors follow the same layered model as placement:

1. **Implementation-level failures** — socket resets, TLS errors, serialization faults — belong to the transport that the physical plan happened to choose.
2. These are translated into **transport- / stage-level failures** — "the `payments` domain is unreachable," "the operation's completion is unknown," "the deadline expired."
3. Application code handles **domain errors** — `PaymentUnavailable`, `PaymentDeclined`.

The original cause may remain attached for diagnostics, but low-level implementation errors do not automatically become public control flow. A caller of `at payments { charge(order) }` should match on payment-domain outcomes, not on TLS alerts — because whether TLS is even involved is a property of the deployment plan, not of the program.

## Relationship to what exists today

The current implementation — `stage` declarations, `serve`, `remote` dependencies, the schema-level wire format, interface hashing — is the **first physical transport** and the proving ground for the checking machinery (typed errors across boundaries, wire-type validation, version-skew rejection). Those checks carry forward. The `remote` keyword's framing does not: today it makes the *transport* part of the code, which is exactly the coupling this model removes. `at` over logical domains is the target programming model; transport synthesis subsumes today's explicit RPC declarations.

Established today (shipped and tested):

- whole-program compilation across service code
- typed errors with inferred fallibility, including across boundaries
- schema-level wire format: a closed, compiler-derived set of shapes (no custom encoder hooks)
- interface hashing: version-skewed caller/callee rejected at the boundary
- stages as lifecycle/DI shells with no state of their own

## Open design questions

These are **unresolved**; nothing below is settled semantics.

1. **Domain declaration and identity.** What declares a logical execution domain? (A `stage`? A new declaration? Is `payments` in `at payments` a stage instance, a domain name bound by the deployment plan, or a DI-resolved capability?)
2. **Isolation model.** What exactly is isolated between colocated domains — heap? capabilities? ambient state? What enforces it in-process (copying at the boundary, freeze/immutability, ownership rules)?
3. **Value-transfer rules.** Which values may cross an `at` boundary? Wire-shaped data is clearly transferable; what about handles (open files, channels, tasks, DI singletons)? Are captures checked against the wire surface, an ownership rule, or a capability judgment?
4. **Failure contract of the boundary.** What is the precise error surface every `at` carries — unreachable, deadline, ambiguous-completion? How do idempotency declarations interact with retries the physical plan inserts? Is ambiguous-completion a distinct error type the caller must handle?
5. **Deadlines and cancellation.** How do deadlines propagate into a domain, and what are the semantics of cancelling an `at` whose physical plan made it a local call vs. a network call? (They must be the same.)
6. **Legal optimizations.** A precise statement of when fusion/inlining/colocation is semantics-preserving — the "plan optimizer's" correctness rules, including the interaction with contracts, secrets, and single-writer ownership.
7. **The deployment plan.** What artifact binds logical domains to physical placement, and when is it checked — compile time, deploy time, or both? (Direction: both — see v1-vision.md on infrastructure as typed compile-time input.)
8. **Structured distributed computation.** What do parallel `at` blocks, scatter/gather, and structured-concurrency scopes across domains look like?

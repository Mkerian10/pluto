# Vision: Pluto's Safety Stack

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-14

## The Thesis

Most languages draw the safety boundary at the process. Rust proves memory safety within a binary. Go catches races with a runtime detector. Erlang isolates failures per process. None of them extend compile-time guarantees across process boundaries, across deployments, or across the full lifecycle of data.

Pluto draws the boundary at the **system**. One compiler, one type checker, one set of guarantees — from a single function up through a distributed deployment. The same analysis that catches a null dereference also catches a wire-incompatible schema change. The same mutability tracking that prevents data races also determines which database columns need migration.

This document describes the full safety stack: four layers of increasingly ambitious compile-time guarantees, plus a data model and evolution system that make those guarantees hold across time.

## The Four Layers

### Layer 1: Value Safety

**Claim:** Every value in a Pluto program is used correctly — no null dereferences, no unhandled errors, no type mismatches.

**Mechanism:**
- **Error inference.** Functions don't annotate error types. The compiler infers which functions can fail via fixed-point analysis of the call graph, then enforces that every fallible call is handled with `!` (propagate) or `catch` (handle). Already implemented.
- **Explicit nullability.** Functions, methods, and fields annotate nullable types with `T?`. The compiler enforces handling of nullable values through `?`, explicit checks (`x != none`), or type-safe control flow.
- **Type checking.** Nominal types throughout. No implicit coercions except `T → T?`. Generics monomorphized. Whole-program compilation means the checker sees everything.

**What this means:** A Pluto program that compiles has no null pointer exceptions, no unhandled errors, and no type confusion. These aren't caught at runtime — they're proven impossible at compile time.

### Layer 2: Mutation Safety

**Claim:** Every mutation in a Pluto program is intentional and visible — no accidental mutation of immutable data, no mutation through non-mut methods, no silent side effects.

**Mechanism:**
- **Two-level mutability.** Methods declare `mut self` if they mutate. Bindings declare `let mut` if they allow mutation. Both sides must opt in. The compiler enforces both.
- **Deep mutability.** Immutability is transitive — an immutable binding prevents mutation through any path, including nested field access, array element assignment, and map value assignment. Currently has implementation gaps; see `rfc-mutability-v2.md`.
- **Contracts.** `invariant` clauses on classes specify properties that hold after every `mut self` method. `requires` clauses on functions specify preconditions. The compiler will eventually prove these statically; currently enforced at runtime.

**What this means:** Reading Pluto code, you can trust that `self` methods don't mutate, that `let` bindings don't change, and that invariants hold. This isn't convention — it's enforced.

### Layer 3: Concurrency Safety

**Claim:** Pluto programs cannot have data races, and the compiler provides strong protection against deadlocks.

**Mechanism:**
- **Copy on spawn.** Values passed to spawned tasks are deep-copied. No aliasing, no races, full mutability in each task. Already implemented in the runtime.
- **Inferred synchronization.** DI singletons accessed from concurrent tasks are auto-wrapped with rwlocks. `self` methods get reader locks (concurrent reads OK), `mut self` methods get writer locks (exclusive). No annotations needed — the compiler infers this from the DI graph and spawn sites. See `rfc-concurrency-v2.md`.
- **Compile-time lock ordering.** When multiple singletons are accessed in sequence, the compiler assigns a total order based on the DI dependency graph and enforces consistent acquisition order, preventing deadlock. See the deadlock addendum in `rfc-concurrency-v2.md`.
- **Structured concurrency.** Task handles are must-use (`Task<T>` consumed via `.get()` or `.detach()`). No fire-and-forget spawns that silently fail.

**What this means:** Pluto programs are data-race-free by construction. The compiler doesn't just detect races — it makes them structurally impossible. Combined with GC (no use-after-free) and lock ordering (no deadlock), this is a comprehensive concurrency safety guarantee.

### Layer 4: Distributed Safety

**Claim:** Pluto's compile-time guarantees extend across process boundaries, across servers, and across deployments.

**Mechanism:**
- **Whole-program compilation.** Pluto sees the entire system — all services, all schemas, all wire types — in a single compilation. This is the foundation. You can't guarantee cross-service safety if you compile services separately.
- **Distributed error inference.** Cross-process calls automatically become fallible with `NetworkError`. The same error inference that handles local errors handles remote failures. See `rfc-distributed-safety.md`.
- **Wire type safety.** Schemas that cross process boundaries are type-checked at both endpoints. The compiler rejects incompatible changes. See `rfc-schema.md`.
- **Topology verification.** The compiler knows the deployment topology and can verify that all endpoints exist, all schemas match, and all error types are handled across the entire system.
- **Deployment manifests.** The compiler outputs ordered deployment plans: SQL migrations, service rollout order, cache invalidation steps. It knows which changes are backward-compatible and which require coordination. See `rfc-migration.md`.

**What this means:** Deploying a Pluto system is like deploying a single binary. The compiler guarantees that all the pieces fit together. Breaking changes are caught at compile time, not at 2 AM.

## The Data Model: Schemas

The safety stack requires a data model that crosses every boundary in the system. Classes are behavioral (methods, DI, side effects) — they stay local. Schemas are pure data — they cross every boundary.

**Schema:** A value-typed, pure data construct. No DI, no side effects, no identity. Like a big int — copyable, comparable, instantiable. Schemas are what gets serialized on the wire, stored in databases, and diffed across deployments.

**Key properties:**
- Value semantics (copy, not reference)
- Pure functions only (deterministic, no side effects)
- Conditional fields (discriminator-driven, flow-sensitive access)
- Spread composition (`...OtherSchema`), no inheritance
- Generic (monomorphized)
- Can implement traits (nominal `impl`)
- `from` clauses for structural migration hints

The schema/class split is the foundation of the evolution system. Schemas are what changes across deployments; the compiler tracks those changes and computes the transition.

See `rfc-schema.md` for the full design.

## The Evolution System

Software changes. Schemas change. Databases change. Pluto's evolution system handles change at compile time rather than at runtime or by convention.

**Snapshot-based migration.** No version numbers anywhere. The compiler diffs the current source code against a snapshot of the previously deployed state and computes the migration automatically. Snapshots are build artifacts, not source code. See `rfc-migration.md`.

**`from` clauses.** Structural migration hints on schema fields that tell the compiler how to transform data when the shape changes:
```
schema Order {
    total_cents: int from total: float => int(total * 100.0)
}
```
Not versioned — fires whenever the diff matches the old shape.

**Storage declarations.** `storage orders: Table<Order>` as first-class language constructs. The compiler knows what's stored where and generates appropriate migrations (SQL `ALTER TABLE`, etc.). See `rfc-storage.md`.

**Evolution rules.** A general-purpose compile-time rule engine that reacts to type diffs and enforces policies: "adding a required field without a default is an error," "renaming a field requires a `from` clause," etc. See `rfc-evolution-rules.md`.

## Why This Matters

Most systems fail at boundaries:
- Process A sends a message that Process B can't parse
- A database migration breaks a running service
- A concurrent access pattern causes a race condition that only manifests under load
- An error propagates across three services and nobody handles it

Pluto eliminates these failure modes at compile time. Not by being restrictive — by being *informed*. Whole-program compilation means the compiler sees everything. Schemas mean data has a single source of truth. Inference means the programmer writes less and the compiler checks more.

The result is a language where:
- You write business logic
- You declare what mutates (`mut self`) and what data looks like (`schema`)
- The compiler infers error handling, synchronization, migration plans, and deployment order, while explicitly enforcing nullability annotations
- If it compiles, the whole system is consistent

## Document Map

| Document | Layer | Description |
|----------|-------|-------------|
| `rfc-schema.md` | Foundation | The schema construct — pure data that crosses every boundary |
| `rfc-nullability-inference.md` | Layer 1 | Rejected proposal (kept for history) |
| `rfc-mutability-v2.md` | Layer 2 | Fix mutability gaps, deep enforcement |
| `rfc-concurrency-v2.md` | Layer 3 | Copy on spawn, inferred sync, deadlock prevention |
| `rfc-storage.md` | Evolution | Storage declarations binding schemas to backends |
| `rfc-migration.md` | Evolution | Snapshot diffing, computed migrations |
| `rfc-distributed-safety.md` | Layer 4 | Cross-process type safety, distributed error inference |
| `rfc-evolution-rules.md` | Evolution | Compile-time rule engine for change policies |

Dependencies flow downward: distributed safety depends on schemas, migration depends on storage, storage depends on schemas, everything depends on mutability.

## Prior Art

No existing language attempts all four layers:
- **Rust** achieves Layers 1-3 (value + mutation + concurrency) through ownership, but has no Layer 4 and no evolution system.
- **Erlang/BEAM** achieves Layer 3 (concurrency) and aspects of Layer 4 (hot code loading, distribution) but with runtime checks rather than compile-time proofs.
- **Pony** achieves Layers 1-3 through reference capabilities but doesn't extend to distribution.
- **Session types** (academic) formalize Layer 4 communication protocols but aren't implemented in production languages.

Pluto's unique contribution is the combination: whole-program compilation that sees the entire distributed system, inference that minimizes annotation burden, and an evolution system that handles change over time.

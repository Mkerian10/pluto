# RFC: The Object Construct

**Status:** Accepted; phase 1 (identity semantics) implemented
**Author:** Design discussion
**Date:** 2026-09-04
**Related:** [v1-vision.md](../v1-vision.md) (Objects), [distributed-model.md](distributed-model.md), [rfc-typestates.md](rfc-typestates.md), [program-structure.md](program-structure.md)

## The distinction (settled by the vision)

Pluto distinguishes **classes** from **objects**:

- A **class** is a data structure — bytes with a known layout. `getValue()` on a class reads a field; if it hit AWS you'd be shocked. A class doesn't *mean* anything beyond its bytes.
- An **object** is an **entity** — it semantically represents *the actual thing*, not a snapshot of it. `getValue()` on a `Secret` object may go to a vault, because that's what the secret *is*. `rotate()` rotates the real secret. Method calls are messages to the entity; state may change between calls; failure, ownership, and access patterns are part of the deal.

**Classes get traits. Objects get inheritance.** Class inheritance breeds fragile base classes because it inherits *data layout*; object inheritance specializes *behavior on entities*, constrained by contracts, which the verification engine can police.

This RFC turns that distinction into concrete semantics. Sections marked **(proposal)** are recommendations to accept, amend, or reject; the **Open questions** section is genuinely unsettled.

## Declaration (proposal)

```pluto
object Secret[vault: VaultClient] {
    path: string

    invariant self.path != ""

    fn value(self) string {
        return self.vault.read(self.path)!
    }

    fn rotate(mut self) {
        self.vault.rotate(self.path)!
    }
}
```

Syntactically an `object` body is a class body: fields, bracket deps, methods, invariants, `uses`. The difference is entirely in the semantics the compiler attaches to *instances*:

| | `class` | `object` |
|---|---|---|
| Represents | data (a value) | an entity (the thing itself) |
| Identity | none — structural | intrinsic — reference identity |
| `==` | value comparison (future) | identity comparison |
| Crossing a spawn | deep-copied (no shared mutable state) | **shared** — it's the same entity |
| Crossing a domain boundary | copied by value (wire-shaped) | **by reference** — a handle to the entity |
| Composition | traits | traits **and** contract-constrained inheritance |
| Observability | none intrinsic | every method call is a traceable operation on a known identity |

### Why "spawn shares objects" is safe (proposal)

Classes are deep-copied into spawned tasks precisely because shared mutable state is dangerous. Objects invert this: the entity is *one thing*, so a copy would be semantically wrong (two secrets?). Sharing is safe because objects come with a stronger concurrency contract: **an object's methods are serialized** — the entity processes one message at a time (implementation: the rwlock machinery already used for synchronized singletons, applied unconditionally to objects). This replaces the heuristic "did both threads touch it?" analysis with a guarantee attached to the construct that means shared identity.

## Objects and the distributed model (proposal)

Objects are what live in domains. This resolves distributed-model.md's open question 1:

> **A logical execution domain is a boundary around a set of objects.** Placing computation with `at payments { ... }` runs it among the payment domain's objects; the domain's capabilities *are* its objects.

Consequences:

- A **class value** crossing a boundary is copied — it's wire-shaped data (the schema-level wire surface, unchanged).
- An **object reference** crossing a boundary stays a *reference*: what serializes is an identity handle (domain, type, id — schema-level data, so the wire stays a closed compiler-derived surface). Calling through it is a placement (`at`) on the entity's home domain.
- An object is local today and distributed tomorrow **without its meaning changing** — the vision's line "the nature of the object was always clear." Colocation and splitting are physical-plan decisions; the entity's identity, serialization of its handle, and its failure envelope are stable.

## Inheritance (proposal)

```pluto
object HTTPServer {
    port: int
    invariant self.port > 0

    fn handle(self, req: Request) Response {
        return Response { status: 404 }
    }

    fn start(mut self) { ... }
}

object ApiServer extends HTTPServer {
    override fn handle(self, req: Request) Response {
        ...
    }
}
```

- **Single inheritance, objects only.** `extends` names one parent object type. Classes cannot use it; objects can also implement traits.
- **Contracts are inherited and binding.** Parent invariants apply to the child; an override may not strengthen `requires` (the same Liskov rule already enforced for trait conformance). The verification engine's job is exactly this: extension cannot break the parent's guarantees.
- **Fields are inherited, never overridden.** Behavior specializes; layout doesn't fork (this is what makes object inheritance safe where class inheritance isn't).
- **Dispatch is dynamic** through the existing vtable machinery (traits already built it). `ApiServer` is usable wherever `HTTPServer` is expected.

## What this unlocks

- **Typestates phase 3** (rfc-typestates.md): `owns`/`holds_lease`/`is_leader` are properties of *entities*; `Lease<Held>` wants to be an object with a typestate parameter.
- **Program structure** (program-structure.md): "objects and their dependency graph are the program" — the ceremony-free pillar defines a program as its object graph; `app` becomes a special case.
- **Derived observability**: an entity with identity makes every method call a meaningful span with no annotation.

## Open questions (genuinely unsettled)

1. **Construction and identity origin.** Who mints an entity? DI (objects as wired singletons/scoped instances) covers services; but `Secret` above is more like a *handle* to pre-existing external state. Are there two construction stories (wired vs. adopted), or one?
2. **Object vs. domain overlap.** Is a served service an object? Is a domain just an object boundary, or can a domain host many objects with one interface? (Proposal above says domains are object sets — needs pressure-testing against `serve`/`at` as implemented.)
3. **Lifecycle.** How do DI lifecycles (singleton/scoped/transient) map onto entities? A transient *entity* seems contradictory; is `object` implicitly singleton-or-scoped?
4. **GC and external state.** An object whose real state lives in a vault: what does collecting the in-process handle mean? Is there a `close`/release protocol (connects to auto-executing `ensures`)?
5. **Handle wire format.** What exactly is in an object reference on the wire (domain id, type, entity id, fencing token?), and how does it interact with interface hashing and evolution rules?
6. **Serialized methods vs. reentrancy.** If object methods are serialized, does an object calling itself (directly or through a cycle of objects) deadlock, queue, or get detected at compile time via the DI graph?
7. **Inheritance vs. monomorphized generics.** Can objects be generic (`object Topic<T>`)? Typestated (`object Lease<S>`)? The typestate machinery suggests yes, but inheritance × monomorphization needs a story.
8. **Migration path.** Several current stdlib/user "service classes" (e.g. `PaymentService` in the placement examples) are conceptually objects. Do they migrate, or do classes-behaving-as-services remain legal indefinitely?

## Suggested phasing (proposal)

1. **Phase 1 — identity semantics** *(implemented)*: `object` parses as a class-bodied declaration (contextual keyword); instances get reference identity, `==` as identity, spawn-sharing with serialized methods (per-type lock in phase 1 — over-serializes multiple instances of one type; per-instance locks are an optimization), and rejection at domain boundaries ("objects cross by reference — not yet implemented"). Generic objects rejected pending the inheritance × monomorphization story (open question 7).
2. **Phase 2 — inheritance**: `extends` + `override` with inherited invariants and Liskov checks, on the trait vtable machinery.
3. **Phase 3 — distributed entities**: object handles across domain boundaries, `at` on an entity's home domain, handle wire format.
4. **Phase 4 — integration**: typestated objects, auto-executing cleanup, observability derivation.

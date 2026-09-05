# RFC: The Object Construct

**Status:** Accepted; phase 1 (identity semantics) and value equality implemented; inheritance REJECTED (see below)
**Author:** Design discussion
**Date:** 2026-09-04
**Related:** [v1-vision.md](../v1-vision.md) (Objects), [distributed-model.md](distributed-model.md), [rfc-typestates.md](rfc-typestates.md), [program-structure.md](program-structure.md)

## The distinction (settled by the vision)

Pluto distinguishes **classes** from **objects**:

- A **class** is a data structure — bytes with a known layout. `getValue()` on a class reads a field; if it hit AWS you'd be shocked. A class doesn't *mean* anything beyond its bytes.
- An **object** is an **entity** — it semantically represents *the actual thing*, not a snapshot of it. `getValue()` on a `Secret` object may go to a vault, because that's what the secret *is*. `rotate()` rotates the real secret. Method calls are messages to the entity; state may change between calls; failure, ownership, and access patterns are part of the deal.

**Classes get traits. Objects get traits plus entity semantics.** (An earlier draft gave objects inheritance; that was considered and **rejected** — see "Inheritance: considered and rejected" below.)

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
| `==` | **structural** — fields compared recursively (arrays/maps/sets/enums/nullables included; nested entities by identity) | identity comparison |
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

## Inheritance: considered and rejected (2026-09-05)

An earlier draft gave objects single, contract-constrained inheritance (`extends` with Liskov-checked overrides). The decision is to **drop it**: the language is already complex, and inheritance fails the test that new complexity must buy expressiveness you can't already get —

- *Behavior specialization / template method* — trait **default methods** (shipped) already provide it: a trait defines `handle` with a default body; an implementor overrides.
- *Liskov-constrained overrides* — trait conformance already enforces exactly this rule.
- *Subtype polymorphism* — trait objects with vtable dispatch already exist.
- *Field inheritance* — the draft had to neuter it ("inherited, never overridden") to stay safe, a sign the mechanism fought the design.

Meanwhile the interaction surface was large: monomorphization, generic/typestated objects, DI injection-by-type, and cross-domain handles ("is a handle to a subtype a handle to the parent?") would each have needed a story. The founding decision — classes + traits, **no inheritance** — stands for objects too; the messaging/entity model is the valuable part of the construct and does not depend on it.

**Revisit criteria:** a concrete framework-extension pattern that default-method traits demonstrably cannot express.

## Value equality (implemented with phase 1)

Dropping inheritance sharpened the remaining question: what *is* the class/object split? Answer: **classes are values, objects are entities**, and `==` now says so —

- **Class/enum/array/map/set/nullable `==` is structural**: `Point{x:1} == Point{x:1}` is `true`. Implemented as a runtime deep-equality (`__pluto_deep_eq`) mirroring `deep_copy`'s traversal: strings by content, collections element-wise (maps/sets order-independent), cycles handled coinductively. Works uniformly for generic instances.
- **Object `==` is identity**, including when entities are *nested inside* compared values: entities allocate under their own GC tag (`GC_TAG_ENTITY`), so structural comparison of two classes holding different-but-equal-state entities is `false`.
- The entity tag also fixed a phase-1 hole: `deep_copy` (spawn captures) now **shares** entities nested inside copied class values instead of copying them — a `Counter` inside a `Holder` handed to `spawn` is the same counter on both sides.
- Caveats: float fields inside structures compare bitwise (slots carry no type info; top-level float `==` remains IEEE); trait-typed values compare by identity (dynamic type unknown).

## What this unlocks

- **Typestates phase 3** (rfc-typestates.md): `owns`/`holds_lease`/`is_leader` are properties of *entities*; `Lease<Held>` wants to be an object with a typestate parameter.
- **Program structure** (program-structure.md): "objects and their dependency graph are the program" — the ceremony-free pillar defines a program as its object graph; `app` becomes a special case.
- **Derived observability**: an entity with identity makes every method call a meaningful span with no annotation.

## Open questions (genuinely unsettled)

1. **Construction and identity origin.** Who mints an entity? DI (objects as wired singletons/scoped instances) covers services; but `Secret` above is more like a *handle* to pre-existing external state. Are there two construction stories (wired vs. adopted), or one?
2. **Object vs. domain overlap.** Is a served service an object? Is a domain just an object boundary, or can a domain host many objects with one interface? (Proposal above says domains are object sets — needs pressure-testing against `serve`/`at` as implemented.)
3. **Lifecycle.** How do DI lifecycles (singleton/scoped/transient) map onto entities? A transient *entity* seems contradictory; is `object` implicitly singleton-or-scoped? (Sharpened by value equality: per-injection distinctness of *transient classes* is now intentionally unobservable — transients carry injected-only fields, so instances are always field-equal, and values have no identity. Anything that needs observable per-injection identity is, by definition, an entity.)
4. **GC and external state.** An object whose real state lives in a vault: what does collecting the in-process handle mean? Is there a `close`/release protocol (connects to auto-executing `ensures`)?
5. **Handle wire format.** What exactly is in an object reference on the wire (domain id, type, entity id, fencing token?), and how does it interact with interface hashing and evolution rules?
6. **Serialized methods vs. reentrancy.** If object methods are serialized, does an object calling itself (directly or through a cycle of objects) deadlock, queue, or get detected at compile time via the DI graph?
7. **Generic and typestated objects.** Can objects be generic (`object Topic<T>`)? Typestated (`object Lease<S>`)? With inheritance gone the open part is entity identity across monomorphized instantiations (is `Topic<int>`'s identity space distinct from `Topic<string>`'s? — presumably yes and trivially so). Likely unblockable with modest design.
8. **Migration path.** Several current stdlib/user "service classes" (e.g. `PaymentService` in the placement examples) are conceptually objects. Do they migrate, or do classes-behaving-as-services remain legal indefinitely?

## Suggested phasing (proposal)

1. **Phase 1 — identity semantics** *(implemented)*: `object` parses as a class-bodied declaration (contextual keyword); instances get reference identity, `==` as identity, spawn-sharing with serialized methods (per-type lock in phase 1 — over-serializes multiple instances of one type; per-instance locks are an optimization), and rejection at domain boundaries ("objects cross by reference — not yet implemented"). Generic objects rejected pending the inheritance × monomorphization story (open question 7).
2. **Phase 2 — distributed entities**: object handles across domain boundaries, `at` on an entity's home domain, handle wire format.
3. **Phase 3 — integration**: typestated objects, auto-executing cleanup, observability derivation. (Inheritance was cut — see above — which also removes the blocker on generic objects; they now wait only on the entity-identity × monomorphization story.)

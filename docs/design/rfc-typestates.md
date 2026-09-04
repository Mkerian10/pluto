# RFC: Typestates via Generics

**Status:** Phase 1 implemented
**Author:** Matt Kerian
**Date:** 2026-09-04
**Related:** [v1-vision.md](../v1-vision.md) (Static Verification), [contracts.md](contracts.md), [rfc-distributed-safety.md](rfc-distributed-safety.md)

## Motivation

From the v1 vision:

> **Typestates via generics.** Objects can carry state as a type parameter. A `Partition<Unowned>` and a `Partition<Owned>` are different types — you cannot call `consume()` on an unowned partition because the method doesn't exist on that type. State transitions are method calls that return the object in its new state. The compiler enforces valid sequencing through the type system, not runtime checks.

This is the first slice of the verification engine: it makes out-of-order protocol calls **inexpressible** rather than runtime-checked, using machinery the language already has.

## What already works (no changes needed)

The generics system, as completed by #303/#305/#314, already supports three of the four typestate ingredients:

```pluto
class Unowned { tag: int }
class Owned { tag: int }

class Partition<S> {
    id: int                                  // 1. phantom type params: S appears in no field — fine

    fn acquire(self) Partition<Owned> {      // 2. transitions: methods returning a different
        return Partition<Owned> { id: self.id }   //    instantiation of the same class — fine
    }
}

fn consume(p: Partition<Owned>) int { ... }

let u = Partition<Unowned> { id: 3 }
consume(u)   // 3. error: argument 1 of 'consume': expected Partition<Owned>,
             //    found Partition<Unowned> — state mismatches already rejected
```

The missing ingredient is **state-restricted methods**: `acquire` above exists on *every* `Partition<S>`, including `Partition<Owned>`. There is no way to say a method only exists in some states.

## Phase 1: `where` state constraints on methods

### Syntax

```pluto
class Partition<S> {
    id: int

    fn acquire(self) Partition<Owned> where S == Unowned {
        return Partition<Owned> { id: self.id }
    }

    fn consume(self) int where S == Owned {
        return self.id
    }

    fn release(self) Partition<Unowned> where S == Owned {
        return Partition<Unowned> { id: self.id }
    }

    fn describe(self) string {          // no clause: exists in every state
        return f"partition {self.id}"
    }
}
```

Grammar: after the return type (and before contract clauses), a method of a generic class may carry
`where <TypeParam> == <StateType> (, <TypeParam> == <StateType>)*`.
Each equality names one of the **class's** type parameters on the left and a concrete named type (class or enum) on the right. Multiple equalities may target different params of a multi-param class.

### Semantics

- **Method existence is per-instantiation.** When `Partition<Owned>` is instantiated, only methods whose constraints are satisfied by the binding `S := Owned` are registered. Calling `consume()` on `Partition<Unowned>` is not a "constraint violation" — the method does not exist on that type, exactly as the vision specifies. The error says why:
  `class 'Partition<Unowned>' has no method 'consume' (method exists only where S == Owned)`.
- **Bodies are checked under the constraint.** A constrained method's body is type-checked with the constrained parameter bound to its state type (not a skolem), so `where S == Owned` methods can construct and return `Partition<Unowned>` etc. without contortions.
- **Traits compose naturally.** Trait conformance for generic classes is already checked per instantiation; an instantiation whose constraints exclude a required method correctly fails conformance for that instantiation.
- **Codegen never sees excluded methods.** Monomorphization skips generating method copies whose constraints the instantiation does not satisfy.

### What phase 1 deliberately does NOT do

- **No linearity.** After `let o = u.acquire()`, the binding `u` still exists and is still a `Partition<Unowned>`. Typestates in phase 1 prevent *wrong-state calls*, not *stale-alias reuse*. This is the same guarantee level as typestate encodings in other GC'd languages (e.g. builder patterns in Java/Kotlin).
- **No `!=` constraints, no bounds on states** (`S: Lockable`), no constraint inference. Equality against named types only.
- **No `where` on free functions or trait declarations.** Free functions already express state via parameter types (`fn f(p: Partition<Owned>)`). Constraints on trait *impl* methods are rejected in phase 1.
- **No generic state arguments** (`where S == Box<int>`). States are plain named classes/enums.

## Phase 2: transition linearity (planned)

The stale-alias gap closes with a *moved-binding* analysis: calling a state-transition method (one whose return type is the same class at a different state) marks the receiver binding consumed; later uses are errors. The compiler already does exactly this shape of dataflow for task handles ("Task handle must be used") and scope-tainted closures, so this is an extension of existing analysis, not new machinery. Design question to settle first: is consumption tied to *transition methods* specifically, or opt-in per class (`linear class Partition<S>`)?

## Phase 3: distributed contract predicates (planned)

The vision's `owns` / `holds_lease` / `is_leader` stdlib patterns become typestate classes plus `requires` clauses bridging value-level facts to type-level states — fencing tokens carried in `Owned`-state fields, leases as `Lease<Held>` with auto-releasing cleanup once the verification engine can prove or generate it. Blocked on the object construct design and the broader verification engine.

## Implementation notes (phase 1)

- `where` becomes a keyword token. No stdlib/example/test source uses it as an identifier.
- Constraints ride in the existing `contracts: Vec<Spanned<ContractClause>>` on `Function` as a new `ContractKind::StateWhere` whose expr is `Ident == Ident` — no new `Function` fields, so every existing constructor site is untouched. Binary schema bumps for the enum variant.
- The per-instantiation gate lives in `ensure_generic_class_instantiated` (typeck/resolve.rs), which is the single choke point where an instantiation's methods are registered; monomorphize applies the same predicate when copying method bodies.
- Runtime contract emission skips `StateWhere` clauses — they are compile-time-only.

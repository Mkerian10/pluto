# RFC: Typestates via Generics

**Status:** Phases 1–3 implemented
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

## Phase 2: transition linearity (implemented)

The stale-alias gap is closed by a *moved-binding* analysis (`src/typeck/linearity.rs`): calling a **transition method** through a local binding consumes that binding; later uses are errors naming the transition and the state the value moved to.

**What counts as a transition.** The consumption decision settled on *automatic, discriminated by state parameters*: a *state parameter* is any type parameter named on the left of a `where` clause anywhere in the class; a *transition method* is one whose return type is the same class with a state-parameter position changed. This keeps data generics completely out of the analysis — a class with no `where` clauses never participates, and `Box<T>.map() Box<U>` (data param change) never consumes. The opt-in `linear class` form remains available as future work if automatic consumption proves too aggressive.

**Flow rules.**
- Reassignment (`u = ...`) or a fresh `let u` revives the binding.
- Branch joins are conservative: consumed on any path ⇒ consumed after the join.
- Loop bodies are analyzed to a fixpoint, so a transition in iteration one is caught as a use in iteration two.
- Closure/spawn bodies are checked against a snapshot (capturing a consumed value is a use); their consumption doesn't escape (captures are by-value).
- Only simple local receivers consume (`u.acquire()`); transitions through fields or temporaries are outside the analysis (documented gap, aligned with the class-level granularity of other analyses).

## Phase 3: degradable typestates + must-release (implemented)

*(Direction settled 2026-09-28, spec in [rfc-verification.md](rfc-verification.md).)* Typestate stays on **values**, never entities: entities are authorities that issue linear, typestated *evidence* (grants, tokens); the authority validates evidence at the point of effect (rfc-objects.md, "Typestate and entities: resolved"). Note the epistemics: `Lease<Held>` claims *session discipline* ("I acquired and have not released"), never remote truth ("the coordinator agrees") — lease windows are liveness, safety lives in the authority's fenced check.

### State-carrying (degradation) errors, auto-discriminated

Error declarations can carry class-typed payload fields, including typestate instantiations. An error with a field whose type is a typestate class with a state parameter **changed relative to a method's receiver** is a *degradation error* for that method — the exact mirror of the transition discrimination rule:

```pluto
error Degraded { lease: Lease<Revoked> }   // degradation error for methods of Lease<S> where S == Held

fn renew(self) Lease<Held> where S == Held {
    if stale { raise Degraded { lease: Lease<Revoked> { ... } } }
    return Lease<Held> { ... }
}
```

**Error-edge consumption.** Raising a degradation error moves the receiver into the payload, so catching it consumes the receiver **on the error path only**: inside the catch body (and after a fall-through catch join, per phase-2's conservative join rules) the receiver binding is consumed, with a diagnostic pointing at the payload; a terminating catch (`return`/`raise`/`break`) leaves the success path's binding valid with no ceremony. Recovery is extracting the payload (`let stale = e.lease`). Propagation (`!`) exits on the error path with the payload riding in the error, so the success path keeps its binding.

### Must-release states

`must_release Held` in the class body (or the general form `must_release S == Held`; the simple form is legal only with exactly one state parameter) marks a state whose bindings are **fully linear**, enforced by the phase-2 flow analysis:

- **Moves consume** — `let b = a`, passing as an argument, returning, and raise payloads move the value; the single obligation travels (caller clean, callee/handler must discharge). Parameters carry the obligation in; `self` is exempt (the class's own methods define the protocol and are the discharge points).
- **Capture is rejected** — a closure or `spawn` capturing a live must-release binding would duplicate the evidence.
- **Field and container stores are rejected** — the obligation would escape the analysis.
- **Scope exit with a live obligation is rejected** — including fall-through, `return`, a literal `raise`, `break`/`continue` past the binding's scope, rebinding, and statement-position drops. Diagnostics name the state and suggest the transitions out (computed from the class's own `where`-constrained transitions). Branch joins are conservative (live on any path stays live; discharged on every fall-through path is discharged); loops analyze to the phase-2 two-pass fixpoint.
- **Discharge** = a consuming transition out of the state, moving the value onward, or returning it.

**Catch obligations.** An error carrying a must-release payload cannot be handled by a wildcard or shorthand catch (the payload would be unreachable); a typed handler takes on the obligation as `var.field`, discharged by extracting the payload. Errors whose payload states are droppable keep the relaxed rules — degradation to a droppable state is the expected common case. Declaration validation rejects `must_release` on non-typestate classes, unknown state names (listing the known states), and the simple form on multi-state-param classes.

**Deliberate gap:** `!` propagation does not check live obligations — the error path is ambient (any carried state rides in the payload), while a literal `raise` is definite and author-visible, and is checked. This mirrors the exceptions-vs-linearity precedent; revisit if leaked obligations on propagation paths bite in practice.

## Implementation notes (phase 1)

- `where` becomes a keyword token. No stdlib/example/test source uses it as an identifier.
- Constraints ride in the existing `contracts: Vec<Spanned<ContractClause>>` on `Function` as a new `ContractKind::StateWhere` whose expr is `Ident == Ident` — no new `Function` fields, so every existing constructor site is untouched. Binary schema bumps for the enum variant.
- The per-instantiation gate lives in `ensure_generic_class_instantiated` (typeck/resolve.rs), which is the single choke point where an instantiation's methods are registered; monomorphize applies the same predicate when copying method bodies.
- Runtime contract emission skips `StateWhere` clauses — they are compile-time-only.

## Implementation notes (phase 3)

- `must_release` is a keyword token (no identifier in stdlib/examples/tests used it). Clauses ride in the existing `invariants: Vec<Spanned<ContractClause>>` on `ClassDecl` as `ContractKind::MustRelease` (expr is `Ident` or `Ident == Ident`) — the StateWhere precedent, no constructor churn. Every value-invariant consumer (static discharge, wire boundary guards, template checking, fragment validation) filters by kind. Binary schema bumps to v11 for the enum variant.
- Error payload fields resolve in a dedicated `resolve_error_fields` pass after class registration (errors used to resolve in pass 0, before classes existed), and are normalized with the other registered types so `Lease<Revoked>` becomes a concrete instantiation.
- Degradation discrimination, error-edge consumption, and the whole must-release analysis live in `src/typeck/linearity.rs`, extending the phase-2 moved-binding pass: same conservative joins and two-pass loop fixpoint, with a consumed-cause enum (transition / move / error path) and an obligation map keyed by binding (or `var.field` for caught payloads). The pass runs after error inference — degradation needs the per-method `fn_errors` sets.

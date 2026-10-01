# RFC: Properties — Named, Checkable Claims

**Status:** Draft — direction accepted (design discussion, 2026-10-01): full property system ("option A"), staged. **Phases 1–3 implemented** (`ensures` with `old()`, two-state invariants, `guarded_by` dominance — slice 1 complete); see Phasing for deviations
**Author:** Design discussion
**Date:** 2026-10-01
**Related:** [rfc-verification.md](rfc-verification.md) (the kernel this extends), [epistemics.md](epistemics.md) (the genericity principle this implements), [contracts.md](contracts.md), [rfc-objects.md](rfc-objects.md), [rfc-distributed-safety.md](rfc-distributed-safety.md)

## Thesis

The genericity principle (epistemics.md) says the language hardcodes no specific
epistemic property — `idempotent`, `monotonic`, `fenced`, `transactional` are
*someone's responsibility*, declared and discharged, never compiler magic. This
RFC gives responsibility a syntax:

- The language ships **proof atoms**: a fixed, decidable, compiler-native set of
  judgment shapes (two-state predicates, dominance, plus the shipped invariant
  and linearity machinery).
- Libraries ship **properties**: named, parameterized bundles of atoms. The
  compiler knows nothing about idempotency; it knows how to check the atoms a
  property's body names.
- Claims are matched by **provides / requires / assume**, every claim carrying a
  named owner and a discharge mode. `pluto analyze` reports the assumption
  surface — everything the system rests on that nobody proved.

## Position: the metaprogramming threshold

Properties are compile-time statements *about program structure* — a property
parameter can denote a field; a body can quantify over write sites. That is
meta-level power, and the design rule that keeps it safe is a founding
constraint of the same rank as "no inheritance" and "no SMT":

> **Judgmental only, generative never.** A property can reject a program. It
> can never change what a program does, generate code, or transform syntax.

This is the tame half of metaprogramming — the half type systems already are
(`fn(int) int` is a meta-level claim; trait bounds are compile-time predicates).
The generative half (macros, templates, user-defined desugarings) is rejected
permanently, which is what spares properties the classic macro pathologies:
code you didn't write, errors in expansions, blind tooling.

One pathology does carry over in judgmental form and must be designed for
up front: **blame attribution**. When a proof fails, the diagnostic must show
both sides — the obligation as the property author wrote it, and the site in
your code that fails it. Two locations, every time.

## Decision record: why A, staged (and not B)

The considered alternative ("option B") was a fixed kernel vocabulary with a
shallow library naming layer — ship the atoms with dedicated syntax, let the
stdlib bind names like `monotonic` as aliases, defer the property form.
**Rejected 2026-10-01.** Two reasons:

1. **The cost accounting doesn't favor it.** The expensive work — two-state
   discharge and dominance analysis in the kernel — is identical under both
   options. A's delta over B is front-end only: the declaration form, typed
   meta-parameters, substitution, blame, hashing. Roughly 1.5–2× B, not 10×.
2. **B's distinctive piece is where the debt would live.** The first real use
   cases force the naming layer to be load-bearing: `idempotent(key = order_id)`
   needs parameterized bindings (a substitution layer built without design
   care), and a retry combinator needs to `require` a property of a
   function-typed argument (properties as first-class named things on types).
   B converges to A accidentally — the definition of tech debt.

The staging keeps A honest: slice 1 is the atoms with their primitive syntax
(needed under any plan — zero throwaway risk); slice 2 is the property form.
The shallow alias layer — the only pure-B component — is never built.

## Slice 1: proof atoms

### Atom 1 — two-state postconditions: `ensures` with `old()`

```pluto
fn advance(mut self) ensures self.epoch == old(self.epoch) + 1 {
    self.epoch = self.epoch + 1
}

fn withdraw(mut self, amt: int) requires amt > 0
    ensures self.balance == old(self.balance) - amt {
    if amt > self.balance { raise Insufficient }
    self.balance = self.balance - amt
}
```

- `old(expr)` denotes `expr` evaluated in the method's **entry state**. The
  expression fragment is the engine's current decidable vocabulary (linear int
  arithmetic over own fields, one-level paths, `len()` terms).
  *(As implemented: own int fields and int parameters, plus `old(...)` of
  those. `len()` terms and one-level foreign paths are excluded from the
  ensures fragment for now — every collection mutation is an opaque call that
  severs the entry relation, so no `len()` ensures could be proven today.)*
- This is the **proof form of `ensures`** promised since contracts.md rejected
  the runtime form: a compile-time obligation at every normal exit, discharged
  by the same symbolic machinery as invariants — which already tracks
  `old(self.f)` ghost terms internally (PR #339); this atom largely *surfaces*
  existing machinery. Strict, like invariants: unprovable exit = compile error.
- **Callers assume the ensures.** This is a precision win at existing call
  boundaries: today, discharge re-anchors to invariant-level facts after any
  call; with ensures, the caller keeps the declared two-state relation
  (`balance` decreased by exactly `amt`). Facts flow out — feeding invariant
  proofs and error-set shrinking downstream.
- On raise paths the ensures is NOT owed (the error contract governs those
  edges; a raise-path two-state story belongs to degradable typestates).

### Atom 2 — two-state invariants: monotonicity without a keyword

A class/object invariant may use `old()`:

```pluto
object BlobAuthority {
    epoch: int
    invariant self.epoch >= old(self.epoch)    // the epoch never decreases
}
```

Semantics: at every obligation site where single-state invariants are checked
today (every `mut self` method exit relative to its own entry; every foreign
write site relative to the pre-write state), the relation must be proven. A
two-state invariant subsumes "monotonic field" with **no new keyword** —
`monotonic` becomes a *name* a library binds in slice 2, not a primitive.

### Atom 3 — dominance: `guarded_by` *(IMPLEMENTED)*

The fencing theorem — "no write to `data` is applied without a currently-valid
grant" — is a control-flow property of write sites. Shipped syntax (the
strawman survived contact with the parser unchanged — open question 1 is
resolved):

```pluto
object BlobAuthority {
    epoch: int
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
}
```

Semantics: every write site of `self.data` (whole-program: the closed write-set
is knowable) must be **dominated**, in its method's CFG, by a conditional the
fact engine proves implies the predicate instantiated with some in-scope value
of the binder type. The entity's serialized methods are the concurrency
side-condition (no interleaving between check and write); the reentrancy
resolution (#348) keeps that sound. This is the proof shape behind fencing and
idempotency-key checks, and was the missing half of the Blob acceptance test —
examples/blob/main.pt now carries the clause and discharges it against its
fence.

Implementation decisions (`src/typeck/dominance.rs`):

- **Structural dominance, no dominator algorithm.** Pluto's control flow is
  structured, so dominance is decided by the fact engine's assumption
  discipline itself: a conditional's facts are visible exactly at the points
  it dominates (taken branch; rest-of-block after a branch whose other path
  terminates) and are killed by anything that could invalidate them in
  between — calls that may run user code (builtin collection methods are
  exempt: they cannot write class int fields), writes to the compared
  fields, binder reassignment, loop boundaries (conservative havoc),
  closure barriers. "Predicate provable from the facts live at the write" ≡
  "dominating check with nothing unsaying it". Unknown ⇒ compile error.
- **Binder instantiation.** At each write site, every in-scope value of the
  binder type (parameters and locals; innermost binding per name) is tried;
  any one proving the predicate discharges the site. A `let` binding of a
  pure trackable term (`let tok = grant.token`) is tracked as an alias, so
  fences phrased through a fence local connect to the binder's fields.
- **Foreign writes rejected outright.** A guarded field may only be written
  through `self` inside its class's own methods — guarded fields are
  protocol-internal; the dominance proof is only meaningful under the
  class's own control flow. Index assignment through the guarded field
  (`self.data[i] = v`) carries the same obligation as a direct write.
  Construction is NOT a write site (birth is `invariant` territory).
- **Fragment.** Predicate: `&&`/`||`/`!` over int comparisons of linear
  arithmetic whose leaves are `self.<f>` (own int fields) and `<binder>.<f>`
  (one-level int fields of the binder class). Binder must be a concrete
  value class (entities rejected — their fields can change concurrently).
  Generic classes rejected, like invariants.
- **Classes vs objects.** On an entity, serialization closes the
  check-then-act window. On a value class the clause is sound for a
  different reason: values don't share (spawn deep-copies, wire copies), so
  no concurrent writer exists to race the check. Both are accepted; neither
  needs extra restrictions.

Slice-1 exit criterion: Blob's theorem is writable **by hand** — a two-state
invariant plus a `guarded_by` — before any property form exists.

## Slice 2: the property form

### Declaration

```pluto
// std.verify (or similar — open question 6)
property monotonic(f: field<int>) {
    invariant f >= old(f)
}

property fenced(f: field, authority: field<int>, grant: type) {
    f guarded_by (g: grant) g.token == authority
}

property idempotent(key: expr) {
    // dedup-check dominance over the effect — body built from the same atoms
}
```

A property is a named, parameterized **bundle of atoms**. Meta-parameter kinds:
`field<T>` (a field of the carrying type), `type`, `expr` / predicates with
binders, `const`. Bodies compose by **conjunction only**; no recursion; no
property-to-property abstraction in slice 2; every atom in the decidable
fragment; the quantifier vocabulary is **fixed** (over write sites of a field,
over exits of a method — never arbitrary syntax queries).

### Use

```pluto
object BlobAuthority satisfies monotonic(self.epoch) {
    ...
}

fn transfer(mut self, req: TransferReq) provides idempotent(key = req.id) { ... }

fn with_retry(f: fn(TransferReq) Receipt! provides idempotent, req: TransferReq) Receipt! {
    // may retry f from an ambiguous failure — the property licenses it
}

extern service S3 {
    fn put(key: string, data: bytes) assume idempotent(key = key)
}
```

- `satisfies` / `provides` instantiate the body by substitution; the resulting
  atoms are checked exactly as slice-1 atoms; the **name survives as an
  exported fact** downstream code can require without reading the body.
- `requires`-side matching: function *types* may carry property requirements
  (precedent: fn types already carry the fallibility contract `!`); a
  higher-order combinator demands the property of its argument, and only
  provably-providing values flow in.
- `assume` is the trust-boundary mode for code the compiler cannot see.
  Explicit, attributed, and reported: `pluto analyze` lists every assumed claim
  — the deployment's complete assumption surface.

### Discharge modes and owners

Every claim has exactly one warrant, per epistemics.md:

- **Proven** — the kernel discharged the instantiated atoms from the body.
- **Checked** — the obligation is discharged by a *proven-to-run* runtime
  guard: the dominance proof shows the check executes before the effect; the
  check's data (a dedup table, a token comparison) lives at runtime. The
  compiler proves the guard's placement, the runtime evaluates its truth.
- **Assumed** — declared at an extern boundary, never silently promoted.

### Evolution and honesty

- Property **bodies hash into interface hashes**: changing a body changes every
  downstream proof's meaning, so it is a visible API change, subject to the
  same evolution rules as wire schemas.
- Names are documentation; bodies are the contract. A property named
  `idempotent` with a vacuous body is a lie the type system cannot catch —
  same risk class as a misleading function name, but trusted at a distance.
  Review/lint territory (open question 5), stated here so nobody mistakes
  name-trust for body-trust.

## Acceptance tests

1. **Blob, slice 1**: `invariant self.epoch >= old(self.epoch)` +
   `data guarded_by ...` both discharge on examples/blob's authority — the
   safety theorem becomes compiler-checked with hand-written atoms.
2. **Blob, slice 2**: the same theorem via
   `satisfies monotonic(self.epoch)` and a `fenced` instantiation from the
   stdlib property module, exported by name, assumable downstream.
3. **Retry combinator**: `with_retry` requires `idempotent` of its fn argument;
   a providing function flows in, a non-providing one is rejected at the
   boundary; an `assume`-discharged extern shows up in the analyze report.

(Conservation-of-money — the ledger's cross-entity sum — remains explicitly
out of scope: it quantifies across entities, a future quantifier. Open
question 4.)

## Open questions

1. **`guarded_by` binder syntax.** RESOLVED (implemented, phase 3): the
   field-level clause with a typed binder shipped exactly as the strawman —
   `data: bytes guarded_by (g: WriteGrant) g.token == self.epoch`, parsed
   after the field's type (binder parenthesized, predicate to end of line).
   Parsing forced no deviation. The considered alternatives (method-level
   guard clause, type-level theorem block) were not needed: the field is the
   natural owner of its write-set, and the clause reads at the declaration
   it protects. Semantics as settled: structural dominance over the closed
   write-set via fact-engine implication, foreign writes rejected.
2. **Foreign-write `old()`.** For two-state invariants at foreign write sites,
   `old` means the pre-write state; for method exits, the entry state. Confirm
   no ambiguity leaks when both apply within one method.
3. **Properties on traits.** Can a trait method declare `provides`? Does
   conformance check it per impl? (Expected: yes, per-impl, like error sets —
   but deferred until a concrete need.)
4. **Cross-entity quantification.** Conservation-style theorems (sums over all
   instances of a type) need a quantifier the kernel does not have and a
   concurrency story beyond per-entity serialization. Phase 4+ of the broader
   roadmap; named here so its absence is a decision, not an oversight.
5. **Honesty lint.** Flag properties whose body is trivially true or unused
   parameters — the vacuous-`idempotent` case.
6. **Namespace.** Where do the standard properties live (`std.verify`?) and
   does the prelude re-export any.

## Phasing

1. **`old()` in `ensures`** — method-level two-state postconditions, strict
   discharge, caller-side assumption of the relation (the call-boundary
   precision win lands immediately). ✅ **Implemented** (branch
   `twostate-proofs`): `ContractKind::Ensures`, registration/fragment
   validation in `discharge::register_ensures`, obligations at every normal
   exit via the ghost proof scope (raise paths exempt), caller-side
   assumption in `discharge::stage_call_ensures` (main fact env, receiver
   and argument substitution shrink.rs-style) and
   `discharge::apply_self_call_ensures` (ghost vocabulary — equality clauses
   pin symbolic field values, which makes sibling-method ensures compose,
   e.g. `double_bump` proving `+2` through two `bump()` calls).
   *Deviations:* the ensures fragment excludes `len()` terms and foreign
   paths (see atom 1 note); ensures is restricted to class/object methods
   (no free functions — there is no receiver state to relate) and rejected
   on generic classes (mirroring invariants-on-generics); a call inside a
   method conservatively severs exact two-state knowledge unless the callee
   is a sibling method with declared ensures (any callee may reach the
   receiver through an alias).
2. **Two-state invariants** — monotonicity expressible with no new keyword;
   Blob's epoch theorem by hand. ✅ **Implemented** (same branch): obligation
   sites are every `mut self` method boundary (relation to the method's
   entry) and every foreign write site (relation to the pre-write state —
   open question 2 resolved: within one method, `old` always means the
   enclosing method's entry at method boundaries and the pre-write state at
   foreign-write sites; the two never collide because a method's own writes
   are symbolic updates, not foreign-write obligations). Construction, DI
   synthesis, and wire decode have no pre-state: two-state clauses are
   code-path-only obligations there (decode validates single-state clauses
   only). Across call boundaries the relation survives by composition, so
   only transitivity-safe cross-state facts (`<`, `<=`, `==`) are carried —
   non-transitive two-state invariants become unprovable after a call
   (strict, conservative). Blob acceptance (the two-state half):
   `invariant self.epoch >= old(self.epoch)` discharges on
3. **`guarded_by` dominance** — the fencing proof shape. ✅ **Implemented**
   (`src/typeck/dominance.rs`, tests/integration/dominance.rs): the Blob
   example carries the clause and discharges it against its fence; together
   with phase 2's monotone invariant, **acceptance test 1 is complete** —
   the Blob safety theorem is fully compiler-checked.
4. **The `property` form** — declaration, meta-parameter typing, substitution,
   `satisfies`/`provides`, two-sided blame diagnostics, body hashing; stdlib
   property module; Blob acceptance test 2.
5. **Requirements and assumptions** — properties on fn types, `requires`
   matching, `assume` at extern boundaries, the analyze assumption-surface
   report; retry-combinator acceptance test.

Each phase is independently shippable, and nothing in phases 1–3 is discarded
by 4–5 — the atoms are the body language of the property form.

# RFC: Properties — Named, Checkable Claims

**Status:** Draft — direction accepted (design discussion, 2026-10-01): full property system ("option A"), staged. **Phases 1–5.5 implemented** (`ensures` with `old()`, two-state invariants, `guarded_by` dominance — slice 1 complete; the `property` form, `satisfies`, two-sided blame, `std.verify` — phase 4; `provides`, fn-type requirements, extern `assume`, the assumption surface — phase 5; the dedup-guard CHECKED discharge for `idempotent` and contract-aware interface hashing — phase 5.5); see Phasing for deviations
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
  Guards on generic classes remain rejected (unlike invariants/ensures,
  which are template-proven on generics when their vocabulary is
  param-independent — see contracts.md "Generics").
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

*(As implemented, phases 4–5: kinds shipped are `field<T>`, bare `field` — a
field of any type, usable only as a guarded_by target — `type`,
`const int`, and (phase 5) `expr` — an expression over the providing
function's parameters, one-level paths allowed, bound with the named form
`key = req.id` at `provides`/`assume` sites only; `expr` parameters cannot
appear inside atoms — except as a `dedup` atom's key — and are rejected by
type-level `satisfies`. Atom kinds are `invariant`, `guarded_by`,
(phase 5) `ensures` — a method-level two-state postcondition over
`field<int>`/`const int` parameters — and (phase 5.5) `dedup <expr-param>`
— the dedup-guard proof shape, standing alone (one per property, unmixed
with other atoms); a body may not mix type-level (invariant/guarded_by)
and method-level (ensures/dedup) atoms, and an EMPTY body is legal as a
*declared-only* property (see
Discharge). Bodies are strictly parametric: a direct `self.x` reference is
rejected — fields are named through `field` parameters; invariant atoms may
use `field<int>` and `const int` parameters plus `old(...)`; guard
predicates may additionally use one-level int fields of the binder.
Satisfies arguments are positional; provides/assume arguments are
positional except `expr` parameters, which take the named form. A field
carries at most one guard clause, so a second `fenced`-style instantiation
on an already-guarded field is rejected.)*

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

*(As implemented, phases 5–5.5 — the fn-level discharge paths:*

- *PROVEN: a class/object method providing an `ensures`-bodied property —
  the atoms substitute into ordinary `ensures` contracts (with provenance
  for two-sided blame) and the existing symbolic prover discharges them.*
- *CHECKED (phase 5.5): a class/object method providing a `dedup`-bodied
  property — the guard-placement proof of `src/typeck/idempotency.rs`
  (see Phasing 5.5): every externally-visible effect must carry an ARMED
  insert of the claim's key into a monotone receiver Set field; the
  compiler proves the placement, the set's contents live at runtime.
  `std.verify.idempotent`'s body is now `dedup key`, so the in-unit
  provider exists.*
- *ASSUMED: `extern fn ... assume <property>(<args>)` — legal for
  declared-only (empty-body) properties and for dedup-shaped ones (an
  external system can implement the dedup internally — an idempotent PUT
  keyed on the request — which is exactly what the boundary vouches for);
  still illegal for state-relating atoms (ensures/invariant/guarded_by),
  which describe fields an invisible body cannot have. Recorded with
  owner, instantiation, and source line.*
- *A declared-only property still has no in-unit path — the discharge-gap
  diagnostic points at extern `assume`. Never silently promoted; never
  vacuously satisfied — `satisfies` of a declared-only property is
  equally rejected. `pluto analyze` reports CHECKED claims alongside the
  assumption surface.)*

### Evolution and honesty

- Property **bodies hash into interface hashes**: changing a body changes every
  downstream proof's meaning, so it is a visible API change, subject to the
  same evolution rules as wire schemas.
  *(SHIPPED, phase 5.5 — `codegen::interface_hash`: the RPC dispatch hash
  now folds in the type-level contract clauses of every boundary-crossing
  type — the hashed service/entity itself plus every value class and enum
  transitively reachable through its dispatchable signatures (recursion
  stops at entities, which cross as identity handles and carry their own
  hash). Clause kinds that hash: `invariant` (single- and two-state),
  `guarded_by`, and `satisfies` instantiations by resolved short name +
  arguments; property BODIES participate through the desugared clauses the
  instantiation injects, so changing a body changes every dependent hash —
  the rule above, realized. Canonicalization: the span-free pretty
  rendering, type names reduced to their last segment (module-prefix
  independence, matching the signature strings). The EVOLUTION RULE:
  changing a contract clause on a wire-crossing type is a BREAKING change,
  exactly like a signature change — downstream proofs assume the clauses —
  and the version-skew rejection at the boundary fires for a consumer
  compiled against the old contract; a consumer pairs by mirroring the
  clauses in its interface declaration (type-level clauses are vacuously
  dischargeable on a stub: no constructions, no writes). Deliberately
  EXCLUDED, pinned by test: method-level clauses (`requires`/`ensures`,
  fn-level `provides`) — a stub cannot honestly mirror an `ensures` (it
  would have to implement it); when a method-level story exists
  (declaration-level mirroring without bodies), they enter by the same
  rule.)*
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
   ✅ **Passes** (phase 4): examples/blob declares
   `satisfies verify.monotonic(self.epoch), verify.fenced(self.data,
   self.epoch, WriteGrant)` in place of the hand-written atoms; `fenced`'s
   parameterization covers the blob predicate exactly (`g.token ==
   authority` with `authority = self.epoch`, `grant = WriteGrant`), so
   nothing stayed hand-written but the two protocol-specific single-state
   invariants. The example compiles and runs identically; a violating
   variant fails with the two-sided blame diagnostic
   (tests/integration/properties.rs).
3. **Retry combinator**: `with_retry` requires `idempotent` of its fn argument;
   a providing function flows in, a non-providing one is rejected at the
   boundary; an `assume`-discharged extern shows up in the analyze report.
   ✅ **Passes** (phase 5): examples/retry — `with_retry(f: fn(string) int!
   provides verify.idempotent, req)` retries only from an AMBIGUOUS
   `NetworkError` (`definite == false`); the extern-assumed provider flows
   in and runs, a plain function (or non-delegating closure) is a
   compile-time boundary rejection, and `pluto analyze` prints
   `assume verify.idempotent(key = s) — extern fn __pluto_string_len
   (line N)` (tests/integration/properties.rs, analyze.rs).
   ✅ **Extended** (phase 5.5): the example also carries an IN-UNIT
   provider — `PaymentLedger.apply provides verify.idempotent(key = req)`,
   discharged CHECKED by the dedup-guard proof — flowing into the same
   combinator through a strict-eta delegation closure
   (`(r: string) => ledger.apply(r)`); same key twice observably performs
   no additional effect.

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
6. **Namespace.** RESOLVED (phase 4): the standard properties live in
   `std.verify` (`stdlib/verify/verify.pt`), imported explicitly like any
   module — the prelude re-exports nothing (a proof bundle should be named
   at its import, not ambient).

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
   (no free functions — there is no receiver state to relate); on generic
   classes the vocabulary must be param-independent (template-proven once
   under skolems, stamped onto every instantiation — contracts.md
   "Generics"); the call-severing
   rule is **purity-aware** (phase 4.5): a call severs exact two-state
   knowledge only when the callee may *reach* the receiver — sibling
   self-calls compose through their declared ensures, while
   builtin/primitive methods and direct calls to functions none of whose
   declared parameter types can transitively reach the receiver's class
   (alias-coarse by type; `facts::call_severity`, the one exemption
   predicate shared by the fact kills, invariant/ensures discharge, and
   `guarded_by` dominance) do not sever at all. Builtin collection
   mutators still invalidate *length* terms (they change lengths through
   aliases), but can never write a class's int field — the load-bearing
   survey lives on `facts::CallSeverity::Collections`. This is what makes
   `ensures count == old(count) + 1` prove through a trailing `print()`,
   and frame ensures (`self.epoch == old(self.epoch)`) provable in methods
   that call builtins on parameters (the #357 census cases). Two further
   4.5 precision fixes landed alongside: branch joins and loop exits
   re-anchor *invariant-level* field knowledge in the caller's fact env
   (kills are flow events that empty every frame; without the re-anchor,
   an `if` containing any call left later ensures instantiation with no
   usable pre-state — the #357 "arguably a bug" item), and construction is
   *transparent* — a struct literal's exact int-field initializers flow
   into the caller's fact env (`acc.balance == 100` after
   `let acc = Account { balance: 100 }`), killed by the usual rules
   thereafter; entities excluded (entity fields never carry facts).
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
   property module; Blob acceptance test 2. ✅ **Implemented** (branch
   `property-form`): top-level `property name(params) { atoms }`
   declarations (importable, `pub`-able, module-prefixed like any
   declaration) with kinds `field<T>` / `field` / `type` / `const int`;
   `satisfies name(args)` clauses on class/object headers, desugared by
   pure substitution (`src/properties.rs`) into ordinary invariant /
   guarded_by clauses that the existing discharge/dominance machinery
   checks unchanged — the property layer never forks the provers.
   Declaration-time validation covers structure/arity/kinds (bodies are
   parametric; invariant atoms over `field<int>`/`const int` terms only);
   instantiation-time validation is the standard fragment validation on
   the substituted clauses. Every injected clause carries provenance
   (property name, atom's source line, instantiation bindings), and every
   diagnostic about it appends the blame suffix — e.g. `required by
   property 'monotonic' (defined at verify, line 20), instantiated with
   f = self.epoch` — under the standard failing-site diagnostic. The
   provided names are retained per type (`TypeEnv.class_properties`) and
   surfaced in DerivedInfo (`provided_properties`), ready for phase-5
   matching. `std.verify` ships `monotonic` and `fenced`.
   *Deviations:* `expr` params deferred (4.5, see above); `provides` on
   fns and body hashing deferred (see Use / Evolution notes); `satisfies`
   on generic classes is allowed when the substituted atoms pass the
   param-independence validation (a `field<int>` argument must name a
   field whose declared type is literally `int`, so param-typed fields
   are rejected at instantiation; guarded atoms still hit the
   guards-on-generics rejection); blame
   names the defining module as referenced in code (the import binding,
   `verify`), not the import path.
5. **Requirements and assumptions** — properties on fn types, `requires`
   matching, `assume` at extern boundaries, the analyze assumption-surface
   report; retry-combinator acceptance test. ✅ **Implemented** (branch
   `properties-phase5`): `provides <prop>(<args>)` on fns/methods and
   `assume <prop>(<args>)` on extern fns (comma-separable; args positional
   except `expr` parameters, which take the named form `key = req.id` over
   the provider's parameters, one-level paths allowed); `expr` property
   parameters (deferred from phase 4) shipped for exactly this use;
   fn *types* carry requirements — `fn(TransferReq) Receipt! provides
   idempotent` extends the type surface alongside the fallibility `!`, and
   matching is by resolved property name with subset subsumption (a value
   providing more satisfies a type requiring fewer; requirements carry no
   arguments — instantiation lives on the providing declaration). A bare
   fn reference's type carries its declaration's provides (so the
   eta-expanded wrapper preserves them); closures provide nothing; trait
   methods cannot declare `provides` (dynamic provision rejected with a
   diagnostic). Discharge is honest: PROVEN only for class/object methods
   providing `ensures`-bodied properties (a new `ensures` atom kind,
   substituted into ordinary ensures contracts and discharged by the
   existing prover with two-sided blame); ASSUMED only at extern
   boundaries and only for declared-only (empty-body) properties; every
   other fn-level claim is a compile error (kind error for type-level
   bodies, discharge-gap for declared-only in-unit). Every assumed claim
   lands in `DerivedInfo.assumptions` (owner, property, instantiation,
   line — SCHEMA_VERSION 15) and `pluto analyze` prints the assumption
   surface. *Deviations:* `requires`-side syntax is the fn-type `provides`
   form only (no standalone `requires <property>` clause yet — nothing in
   slice 1 needs one); provides on methods of generic classes rejected
   (mirroring satisfies); app/stage methods cannot provide.

5.5. **The idempotent discharge gap** — ✅ **Implemented** (branch
   `properties-phase55`): the CHECKED fn-level mode exists, and
   `std.verify.idempotent`'s body is `dedup key` — the dedup-guard proof
   shape, discharged by `src/typeck/idempotency.rs` on class/object
   methods. The obligation (full argument in the module docs): every
   externally-visible effect in the providing method (field/index writes,
   calls that may run user code or do I/O, mutating builtin collection
   methods — conservative; `return`/`raise` are outcomes, not effects)
   must carry, on every path, a live ARMED-insert fact: `self.F.insert
   (key)` executed earlier on the path, itself dominated by a membership
   check observing `key ∉ self.F`, with the key provably denoting its
   entry value at both. Two new membership facts ride the existing fact
   engine (`SetNotContains`, killed like any field fact; `SetInserted`,
   monotone-stable across calls for undotted keys); the engine's
   assumption discipline IS the dominance proof, exactly as in
   `guarded_by`. The side conditions that make the theorem ("at most one
   effect execution per key, per instance") follow: the dedup field is
   enforced insert-only WHOLE-PROGRAM (no remove/clear, no reassignment,
   no aliasing or value use, mutation only through `self` in the owning
   class, fresh-set-literal construction — the closed write-set stays
   closed), and the check→insert→effect window is closed against
   concurrency by entity serialization (value classes are per-copy:
   values do not share). Insert-BEFORE-effect is the load-bearing
   direction: a raise between insert and effect yields at-most-once —
   precisely what retrying from ambiguity needs. The duplicate branch is
   provably effect-free (it runs on every repeat call). *Deviations and
   remainder:* the key is matched syntactically (a parameter or one-level
   field path; no alias tracking; entity-typed roots never match); dotted
   keys die at any user-code call, so their guard must precede calls;
   loops drop the facts (chain per loop body or fully outside); effects
   inside closures created in the providing method are rejected (the
   closure may escape the window); dedup state is Set-only (a map-valued
   cache — `key → receipt` — is the natural next shape); free functions
   cannot provide (no receiver state) — delegation is the bridge: a
   strict-eta closure over a providing method carries its provides. The
   analyze surface reports every CHECKED claim alongside the assumed
   ones.

Each phase is independently shippable, and nothing in phases 1–3 is discarded
by 4–5 — the atoms are the body language of the property form.

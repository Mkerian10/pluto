# RFC: The Verification Engine

**Status:** Draft — direction accepted in design discussion (2026-09-28); phases 1–3 implemented; phase 6 (Blob in stdlib — the acceptance test) complete via `std.blob`
**Author:** Design discussion
**Date:** 2026-09-28
**Related:** [epistemics.md](epistemics.md) (the semantic this engine instantiates), [v1-vision.md](../v1-vision.md) (Static Verification), [contracts.md](contracts.md), [rfc-typestates.md](rfc-typestates.md), [rfc-objects.md](rfc-objects.md), [rfc-distributed-safety.md](rfc-distributed-safety.md), [distributed-model.md](distributed-model.md)

> **Foundations note (2026-09-30):** [epistemics.md](epistemics.md) names the
> semantic this RFC serves and adds the **genericity principle**: the language
> hardcodes no specific epistemic property. The kernel shapes below stay; named
> properties (`idempotent`, `transactional`, `fenced`, `monotonic`) are
> library-defined bundles of kernel obligations with owners and discharge modes
> (proven / checked / assumed). Where this RFC's examples read as built-ins
> (e.g. `monotonic(...)` in the Blob sketch), read them as stdlib property
> vocabulary, not compiler magic.

## Thesis

Pluto does not ship distributed-systems constructs. It ships a **proof kernel** — a way
for facts established by code to become known to the compiler — and the difficult
distributed machinery (leases, fencing, epochs, single-writer stores, idempotent
handlers) becomes **libraries whose correctness theorems the compiler checks**, the
same way it checks user code.

The motivating compression, from the design discussion that produced this RFC —
two problems that look unrelated:

- *Local:* "this field's only write paths guard against negative values, so
  `balance >= 0` should be a known fact."
- *Distributed:* "this blob store's fencing is correct — the epoch only increases,
  and no data write is applied without an epoch-validity check."

**These are the same proof**: a field invariant discharged by examining every write
site. Distribution errors become understandable precisely when they are restatements
of local invariants. The verification engine exists to make that restatement
expressible once, instead of baking each distributed pattern into the language.

## Why Pluto can do this

Two commitments the language already made are the soundness preconditions:

1. **Whole-program compilation** makes "the *only* write path" a knowable fact. In a
   separately-compiled language that sentence is unverifiable — another compilation
   unit might write the field. Pluto sees every write site. The same property that
   powers error inference and wire checking powers invariant discharge.
2. **The value/entity split** makes proofs hold *under concurrency*. An invariant
   proven over write paths is worthless if two writers interleave mid-body. Pluto's
   rule: values don't share (spawn deep-copies), and entities serialize their methods.
   Invariants cannot be observed mid-flight and write paths never interleave.
   **Classes are provable because they don't alias; objects are provable because they
   serialize.** Entities are the unit of invariant soundness under concurrency and
   distribution — this is the deep answer to "why does the object construct exist."

## The fact kernel: one engine, three carriers

Pluto already has three disconnected fact systems. The engine is their unification:

- **Invariants on types** (`invariant self.balance >= 0`) — per-type facts,
  *statically discharged* (phase 2, shipped): the compiler proves each invariant
  at each construction and write site, and an unprovable site is a compile
  error. No runtime checks remain at code sites; the only runtime validation is
  at trust boundaries (wire decode — see the proof ladder below).
- **Flow narrowing** — already shipped for exactly one predicate: `if x != none`
  narrows a nullable. That *is* a flow-sensitive proof. Generalize the domain from
  nullability to comparisons and intervals: `if amt <= self.balance { /* fact:
  amt <= balance */ }`. The nullable-narrowing machinery is the embryo of the engine.
- **Typestates** (rfc-typestates.md, phases 1–2 shipped) — protocol facts carried by
  types. `where S == Held` is a fact about a binding that survives across statements
  because linearity forbids staleness.

One engine, three surfaces. Facts differ only in scope: per-type (invariants),
per-binding-per-program-point (flow), per-protocol-session (typestates).

**Decidable fragment, not SMT.** The kernel proves within a fixed domain: intervals,
equalities, linear arithmetic, and *dominance* ("every write to X is dominated by
check Y"). No general SMT solving — predictability beats power; a proof system that
sometimes times out teaches people to distrust it. This inherits the line contracts.md
already draws (the decidable fragment for invariants/requires).

## The proof ladder

**proof > runtime check > typed error.** Nothing becomes undefined behavior. The
ladder means different things for different fact carriers:

- **Invariants are proof-or-reject** *(decided 2026-09-30)*: a declared invariant
  the compiler can discharge is free at runtime; one it cannot discharge at some
  site is a **compile error** at that site, never a runtime residual. There is no
  gradual fallback for invariants — the diagnostic names the invariant, the site,
  the symbolic state, and the missing fact, so unprovability costs a guard or a
  `requires` clause, not a crash path.
- **`requires` keeps the full ladder, with static call-site discharge
  shipped** *(2026-10-01, `src/typeck/requires.rs`)*: a direct call site
  (named non-generic free function, or method on a class/object-typed
  receiver — the same scope as error-set shrinking slice 1) whose *every*
  requires clause the caller's live flow facts prove routes to an unchecked
  twin of the callee body (`<name>$nochk`), so the entry check is not
  executed for that site. Every other caller — unproven sites, generic
  callees, fn-typed values, trait dispatch, spawn, serve/RPC entries —
  keeps the checked entry, so the runtime check (hard abort on violation)
  fires exactly as before. Proof uses the shrinking kill discipline: facts
  must be live at the call (interleaved calls drop field facts, loop
  headers never prove). Unprovable sites are *not* errors — the ladder's
  runtime rung remains; strict proof-or-reject for `requires` is still
  future work.
- **Boundary validation stays runtime**: data entering the compilation unit
  through wire/marshal decode is *testimony, not proof* — decode re-checks
  the invariants of the target type and raises the existing wire error (a
  typed error, mandatory to handle) on violation. That
  boundary check is the one place runtime invariant validation remains.

## Facts have consequences: proofs shrink error sets

Error inference is site-sensitive today; the engine makes it *fact*-sensitive:

```pluto
fn withdraw(mut self, amt: int) requires amt > 0 {
    if amt > self.balance { raise Insufficient }
    self.balance = self.balance - amt
}

// caller:
if amt <= account.balance {
    account.withdraw(amt)      // Insufficient is impossible here — proven.
}                              // No `!`, no catch: the variant vanishes at this site.
```

Contracts stop being documentation and start *removing handling obligations*: facts
flow in (guards, `requires`), facts flow out (invariants, `ensures`), and the visible
payoff is code that gets simpler as it gets more proven. This also resolves the
status of `ensures`: the runtime form stays rejected (contracts.md); the proof form —
an obligation the compiler discharges and downstream code may assume — is this RFC.

**Implemented (phase 3, slice 1 — `src/typeck/shrink.rs`).** The shipped slice is
deliberately conservative; soundness is absolute (a wrongly-dropped variant would be
an unhandled runtime error):

- **Direct raises only.** A callee summary records, for each `raise X` statement,
  its dominating guard chain — usable only when the guard is over the callee's
  parameters, the receiver's direct int fields, one-level parameter field paths
  (`p.field`, non-entity class params — substituted to the actual's field path
  at the site), or parameter length terms (`p.len()` on declared collection
  params — a pure read, substituted to the actual's length term), still holding
  their *entry* values at evaluation (any preceding impure call, parameter
  assignment, or field write invalidates it; raises inside match arms,
  select/scope blocks, catch handlers, and expression-level blocks are never
  summarized). Variants arriving
  via propagation (`!` edges, escaped closures, dynamic dispatch) and runtime
  raise sources (channel ops, unknown task origins, fallible fn-values, remote
  boundaries) never shrink.
- **Direct calls only.** Named free functions and Class-resolved method calls
  on trackable receivers; calls through fn-typed values, closures, trait
  objects, and `at` placement are untouched.
- **Scope expansion (phase 3.5).** Generic callees shrink via *template
  summaries*: a template's guard chains are syntactic and hold verbatim for
  every instantiation, so one summary (under the template key) serves all
  instance call sites — provided no guard mentions a type-param-dependent
  leaf (a param or `self` field whose declared type mentions a type param);
  such guards never validate. This admits the typestate pattern, whose state
  params are phantom and whose guards are over plain int fields
  (`Lease<S>.renew`'s `self.epoch > 2`). Generic *callers* still never
  shrink their own sites (skolem-checked once, facts are per-instantiation).
  Granularity is per raise site, loop-tolerantly: a loop-resident raise
  summarizes when its guard chain survives the whole loop body's kills
  applied at loop entry (any iteration may rerun the body first), so
  entry-stable guards no longer lose to a loop-resident raise of the same
  variant. Guards over call-produced values (stdlib json's byte-wise
  parsers) remain correctly unshrinkable — those are runtime facts, not
  entry facts.
- **The global graph is untouched.** Shrinking narrows the *required-handling*
  set at individual call sites (enforcement and typed-catch coverage); the
  callee's canonical inferred error set — and therefore propagation through `!` —
  is unchanged. Caller facts are consulted at the call's execution point, under
  the existing kill rules (reassignment, field writes, interleaved calls, loop
  havoc), so a stale guard never shrinks.
- **Ergonomics:** handling a provably-impossible error stays legal (`catch`/`!`
  on a shrunk-to-empty site compiles; the dead handler draws no warning —
  consistent with the silent tolerance of redundant `?` on a narrowed nullable).

## The epistemics of distribution

What can a type honestly claim in a distributed system? The coordinator can revoke
your lease between any two instructions; by the time "you still hold it" arrives, it
describes the past. Facts split into two classes:

- **Locally-stable facts** — things only you can change: "I acquired and have not
  released," "I hold fencing token #42," "I already committed." Sound as types.
- **Remote beliefs** — "the coordinator currently considers me the holder,"
  "I am the leader." Never sound as types: any design that encodes these statically
  is lying, and distribution punishes the lie exactly when it matters (partitions,
  pauses, skew).

**Leases are the bridge**: a grant with a validity window is a remote fact made
*temporarily locally-stable* — "the coordinator committed not to reassign before T"
is a fact about a past promise, monotone until expiry. But the window is a
**liveness** mechanism, never a **safety** one: a process can pause between checking
the deadline and its write landing (the classic check-then-act gap — no client-side
check closes it). Safety is judged **at the point of effect, by the authority**,
via a guarded effect (fencing). The provable theorem is not "no write is attempted
without a valid lease" (physics forbids it) but the one that matters: **"no write is
applied without a currently-valid grant."**

If a design ever wants the window itself to be load-bearing (Spanner-style), that
requires a bounded-clock-error assumption that must be *declared explicitly*, never
defaulted. Out of scope for v1.

### Degradable typestates

Typestates today only know transitions the caller performs. Distribution adds
world-driven ones. Three transition kinds:

1. **Caller transitions** — `release()` (shipped: phases 1–2).
2. **Fallible transitions** — `refresh()` can raise. *Decided 2026-09-28,
   IMPLEMENTED (rfc-typestates.md phase 3)*: the
   error **carries** the value in its post-failure state
   (`Degraded { blob: Blob<Unknown> }`) so obligations — notably must-release —
   survive the raise. Shipped mechanics: errors with a payload field whose type is
   the receiver's class at a changed state are auto-discriminated as degradation
   errors (mirror of the transition rule); catching one consumes the receiver on
   the error path only (terminating handlers keep the success binding; fall-through
   joins consume); recovery is extracting the payload (`let stale = e.lease`).
3. **Degradation** — the world moves you out of a state. Surfaces as typed errors on
   *use* (every effectful method of a degradable state has the degradation error in
   its set; `at`'s mandatory-handling contract makes it unignorable). The protocol
   author programs what degrades, to what, and what evidence renews it.

**Must-release** (decided 2026-09-28, IMPLEMENTED): the marking is the explicit
state-level annotation `must_release Held` in the class body (general form
`must_release S == Held` for multi-state-param classes). A binding in a marked
state is fully linear, powered by the moved-binding linearity analysis: moves
(let/argument/return/raise payloads) travel the single obligation, closure/spawn
capture and field/container stores are rejected, and scope exit un-transitioned
is a compile error suggesting the transitions out. Wildcard/shorthand catches of
errors carrying a must-release payload are rejected; typed handlers take on the
payload's obligation. Deterministic, no runtime magic. Release must be legal (and
idempotent) from degraded states — "clean up your local belief" is an obligation
the world cannot revoke — and degraded states themselves are typically droppable
(only `Held`-like states carry obligations).

### Authorities and evidence (resolves "typestated objects")

Typestate never lives on an entity. The factoring:

- The **entity is the authority** — shared, injected, handle-able, always answering
  with its *current* opinion, reachable only by messaging. Its defining property is
  that it changes without telling you — the one thing a static state claim must
  never be attached to. (Sharing also breaks state soundness directly: another
  task's transition would falsify your binding's type.)
- The **typestated value is evidence** — a snapshot of a past interaction with the
  authority (a grant, a token, a deadline), honest about its own staleness, linear,
  possibly must-release. Beliefs are values; a class *is* "a snapshot of the world,
  possibly stale."

Entities issue linear evidence; evidence gates methods via ordinary `where` state
constraints; the authority validates evidence at the point of effect. No
typestated-object construct exists or is planned. (Recorded in rfc-objects.md.)

## The primitive vocabulary

The teachable surface. Each maps to one proof shape the kernel supports:

| Primitive | Meaning | Proof shape |
|---|---|---|
| **Monotonic field** | epochs, versions, high-water marks | every write increases it (write-path scan) |
| **Guarded effect** | fencing, idempotency keys | every effect dominated by a validity check (dominance) |
| **Serialized entity** | atomic apply | free — entity methods serialize (object construct) |
| **Expiring capability** | leases | a library: monotonic epoch + guarded effect + degradable typestate |

Distribution patterns are compositions of these, written as libraries.

## Acceptance test: Blob is a stdlib module

**COMPLETE** (phase 6). `std.blob` (`stdlib/blob/blob.pt`) ships the fenced
single-writer store as a library: a `BlobAuthority` entity carrying
`satisfies verify.monotonic(self.epoch), verify.fenced(self.data, self.epoch,
WriteGrant)` (both discharged at compile time and exported by name in
analyze/DerivedInfo), the two hand-written protocol invariants, the exact
`ensures self.epoch == old(self.epoch) + 1` mint relation and the
`apply` frame ensures, and the typed `StaleGrant` rejection. A server can
own the authority and clients drive the whole protocol through entity
handles and `at` across processes
(tests/integration/distributed.rs `blob_authority_serves_fenced_writes_across_processes`);
examples/blob/main.pt is now a thin consumer of the module. Deviations from
the sketch below: the lease window and the `Blob<S>` capability-carrying
client view are future liveness/ergonomics layers, not safety, and are not
yet shipped; the boundary payload is `string` (bytes cannot cross a
placement boundary yet).

The original target, as sketched before the engine shipped (syntax
illustrative — see open questions):

```pluto
object BlobAuthority {
    // SHIPPED (properties RFC phase 3): the dominance half of the theorem
    // is real syntax — every write to `data` must be dominated by a check
    // the fact engine proves implies the predicate for some in-scope
    // WriteGrant, and foreign writes are rejected outright. See
    // src/typeck/dominance.rs and examples/blob/main.pt, which discharges
    // this exact clause against its fence.
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    invariant monotonic(self.epoch)          // two-state half: properties RFC atom 2

    fn grant_write(mut self) WriteGrant {    // mints token, advances epoch
        ...
    }

    fn apply(mut self, g: WriteGrant, d: bytes) {
        if g.token != self.epoch { raise Degraded }  // the fenced compare
        self.data = d                                 // atomic: methods serialize
    }
    // exported theorem: no write to `data` is applied without a valid grant
    // (every write dominated by the epoch check) — downstream code assumes it
    // without reading the internals.
}

class Blob<S> {                    // local view: capability-carrying reference
    grant: WriteGrant?             // token + deadline — present in Write state

    fn write(mut self, d: bytes) where S == Write {
        at self.home { apply(self.grant?, d) }!      // raises Degraded — unignorable
    }
}
```

The composed guarantee, each layer with a named owner:

| Guarantee | Owner | Checked by |
|---|---|---|
| Atomicity | entity method serialization | object construct (shipped) |
| Safety (single-writer) | fenced compare in `apply` | dominance proof — **shipped** (`guarded_by`, properties RFC phase 3, `src/typeck/dominance.rs`) |
| Liveness / efficiency | lease window | runtime; pre-flight deadline check is a courtesy, never load-bearing |
| Discipline | `Blob<Write>` typestate + mandatory error handling | typestates + error inference (shipped) + degradable states (this RFC) |

A "lockless distributed file with fully atomic, provably single-writer writes" is
then a stdlib import, not a language feature.

## Design decisions

1. **Declared facts at boundaries; inferred facts within bodies.** *(SETTLED
   2026-09-28.)* Flow facts are
   inferred freely inside a function. Across abstraction boundaries a fact must be
   *declared* (invariant / ensures on the type or function) and proven from the body
   — never silently inferred into the API. An inferred cross-boundary fact is a
   spooky dependency: a distant edit silently breaks downstream proofs with no
   declared contract to point at. (Error inference is precedent for whole-program
   inference, but its failure mode is "handle more"; proof inference's failure mode
   is action-at-a-distance breakage.) The compiler may *suggest*: "this invariant
   holds on all write paths — declare it?"
2. **Strict for invariants.** *(SETTLED 2026-09-30; implemented.)* An unprovable
   invariant site is a compile error — no demotion to per-site runtime residuals,
   no runtime checks at code sites, no opt-in modes. Consequences, all intended:
   invariants outside the provable fragment (floats, strings, booleans,
   collections, `.len()`, nested fields) are rejected at declaration —
   length terms live in the *flow* fragment (guards, asserts, bindings feed
   the prover, including the ghost vocabulary inside `mut self` methods,
   where they are epoch-stamped and go stale across call boundaries), but a
   declared invariant still names only the class's own int fields; every
   construction and write site is proven or rejected with a diagnostic that
   teaches the fix; runtime invariant-check emission is removed from codegen;
   wire/marshal decode keeps (gains) runtime validation at the trust boundary,
   raising the existing wire error. `analyze` proof-coverage reporting for
   invariants is moot (coverage is definitionally 100% of accepted programs);
   coverage reporting may return later for `requires` call-site discharge.
3. **No SMT.** *(Proposal.)* Fixed decidable domain: intervals, equalities, linear
   arithmetic, dominance. Extend the domain deliberately, primitive by primitive.
4. **Integer semantics: defects trap; the provable fragment is justified.**
   *(SETTLED 2026-10-02; issue #416.)* The fact kernel models `int` as
   mathematical integers. That model is sound because the runtime does not
   wrap: signed i64 overflow on `+`, `-`, `*`, unary negation, and
   `i64::MIN / -1` — like division and modulo by zero — is a **defect**: it
   prints a uniform message and aborts the process. Defects are not typed
   errors, never enter error inference, and cannot be caught — conditions
   raise, defects trap. Every normally-completing execution therefore agrees
   with the mathematical model, so everything the kernel proves holds on
   every execution that reaches the proven point (partial correctness).
   Deliberate modular arithmetic (hashes, PRNGs) uses the explicit
   `wrapping_add`/`wrapping_sub`/`wrapping_mul` builtins, whose results are
   outside the affine fragment — no interval fact ever derives from them.
   Phase 2 of #416 closes the loop: interval-proven arithmetic sites elide
   their overflow checks, so contracts literally delete the checks.

## Open questions

1. **Fact syntax.** RESOLVED by [rfc-properties.md](rfc-properties.md)
   (2026-10-01): proof atoms are `old()` two-state ensures/invariants and a
   `guarded_by` dominance clause; exported names come from the library-defined
   `property` form (`satisfies`/`provides`/`assume`), not builtins —
   `monotonic` is a stdlib property, not a keyword. The dominance atom is now
   **shipped** as a declared field clause (`data: bytes guarded_by
   (g: WriteGrant) g.token == self.epoch` — properties RFC phase 3,
   `src/typeck/dominance.rs`), never inferred from the body; theorem
   naming/export remains that RFC's slice 2.
2. **Fragment contents.** Exact initial domain: intervals over ints; equalities over
   which types; what of floats (IEEE comparison pitfalls), strings, collections?
   Partially resolved: `len()` facts over collections are in the flow fragment
   (opaque terms with an automatic `>= 0`, conservative kills — see phasing
   item 1); floats and string contents remain open.
3. **Facts across the wire.** A proven invariant on a schema type — does the receiver
   assume it (same compilation unit, interface hash guards skew) or recheck at the
   boundary as defense in depth? Interaction with schema evolution rules.
4. **Fallible-transition mechanics.** RESOLVED (implemented, rfc-typestates.md
   phase 3): a typed `catch` binds the error and the payload is an ordinary field
   read (`e.lease`) that moves the value out; linearity consumes the receiver on
   the error path only (terminating handlers keep the success binding). One
   deliberate gap: `!` propagation does not check unrelated live obligations —
   the error path is ambient; a literal `raise` is definite and is checked.
5. **Must-release syntax.** RESOLVED (implemented): `must_release Held` in the
   class body, with `must_release S == Held` as the general multi-param form
   (simple form legal only with exactly one state parameter). The named state
   must be a state of that class (a `where` RHS or a transition target).
6. **Reentrancy vs dominance.** Entity self-calls (rfc-objects.md open question 6)
   interact with "methods serialize" as a proof precondition — a reentrant call
   observing a half-updated invariant would be unsound. Needs a story before
   invariant discharge on entities ships.
7. **Contract checking cost.** Residual checks in hot paths — is there a profile
   or per-site opt-out, and does `analyze` rank residuals by cost?

## Phasing (proposal)

1. **Flow-fact generalization** — extend narrowing from nullability to comparisons /
   intervals on ints. Self-contained; immediately useful (dead-check elimination,
   better diagnostics). No new syntax. **Shipped** (`src/typeck/facts.rs`).
   The fragment's term vocabulary covers local int variables, field paths
   rooted at a local (`grant.token`, `self.balance` — entity bases never
   carry field facts), and **length terms**: `xs.len()` over a trackable
   path of collection type (array, string, bytes, map, set) is an opaque
   term with an automatic `>= 0` bound, participating in affine forms
   (`xs.len() - 1`) and relations (`if i < xs.len()` narrows), and a direct
   `let n = xs.len()` binding transfers the term's facts to `n`. Kill
   conservatism is deliberate: reassigning the base kills everything under
   it, any field write kills all field paths, and any statement containing
   an impure call kills every field path *and* length term (the callee may
   reach the object or collection through an alias — mutating uses like
   `push`/`pop`/mut-arg passes are calls and need no special casing). The
   one call exempted from these rules is builtin collection `len()` itself,
   a pure read; indexing and iteration are not calls and kill nothing.
   Deeper nesting than these shapes, and value-to-binding transfer beyond
   direct `len()` bindings, stay out of fragment.
2. **Invariant static discharge** — prove declared class invariants at every
   construction and write site; unprovable sites are compile errors (strict mode,
   see design decision 2); runtime validation only at wire decode boundaries.
   **Shipped** (`src/typeck/discharge.rs`): symbolic strong updates over ghost
   variables inside `mut self` methods with proofs at boundaries (exits, raises,
   calls, loops, branch joins), immediate proofs for foreign writes and
   constructions, and a small fragment extension (`x != const` facts). Invariants
   and ensures on generic classes are supported when their vocabulary is
   param-independent: validated and proven once on the template under skolem
   substitution, stamped onto every instantiation (contracts.md "Generics");
   entity reentrancy is handled conservatively (any call while the
   invariant may be broken is rejected — rfc-objects.md open question 6).
3. **Error-set shrinking** — fact-sensitive error inference: a `raise` proven
   unreachable at a call site removes the variant from that site's inferred set.
   **Shipped** (`src/typeck/shrink.rs`), slice 1: direct raises with dominating
   fragment-expressible guards, direct (non-generic, non-dynamic) calls,
   site-level required-handling sets only — see the implementation note under
   "Facts have consequences" above.
4. **Distribution primitives** — monotonic fields and dominance (guarded effects);
   the proof shapes behind fencing. Spec'd in [rfc-properties.md](rfc-properties.md)
   (slice 1: `old()` ensures, two-state invariants, `guarded_by`; slice 2: the
   `property` form with provides/requires/assume). Dominance **shipped**
   (`guarded_by` field clauses, properties RFC phase 3,
   `src/typeck/dominance.rs`): write sites of a guarded field are proven
   dominated by a fact-engine-implied check over an in-scope binder value;
   foreign writes rejected. Monotonic fields (two-state invariants) remain.
5. **Degradable typestates + must-release** — the three transition kinds, error-
   carried state, must-release linearity. (Client: objects RFC phase 3.)
   **Shipped** (rfc-typestates.md phase 3, `src/typeck/linearity.rs`):
   state-carrying errors auto-discriminated against the receiver, error-edge
   consumption composed with the phase-2 flow rules, `must_release` state
   annotations with full move/capture/store/scope-exit linearity, and
   catch-obligation handling for must-release payloads.
6. **Blob in stdlib** — the acceptance test, end to end. **Shipped**
   (`stdlib/blob/blob.pt`, module `std.blob`): the authority, grant, and
   typed rejection as a library, every claim from the example discharged
   across the module boundary, the theorem exported by name, and the
   protocol proven over two processes (tests/integration/distributed.rs).
   See "Acceptance test: Blob is a stdlib module" above.

Phases 1–3 are pure compiler work with no language-surface change beyond diagnostics,
and each pays for itself independently of the distributed story.

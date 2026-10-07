# RFC: The Boundary Doctrine — Typing the Conversation

**Status:** Draft — awaiting owner review
**Author:** Design discussion
**Date:** 2026-10-02
**Related:** [epistemics.md](epistemics.md) (warrants — the lens), [rfc-entity-lifecycle.md](rfc-entity-lifecycle.md) (leases — accepted; the carrier this RFC builds on), [rfc-typestates.md](rfc-typestates.md), [rfc-verification.md](rfc-verification.md), [rfc-properties.md](rfc-properties.md) (the `assume` mechanism this RFC demotes), [rfc-distributed-safety.md](rfc-distributed-safety.md) (failure classification), [distributed-model.md](distributed-model.md) (the deployment-plan open question, which this RFC answers), [rfc-fs-api.md](rfc-fs-api.md) (the first typed boundary, in hindsight)

## The problem

The verification machinery landed, and inside a process it is honest: STRICT
invariants, two-state ensures, typestate linearity, checked idempotency — all
of it proof, none of it hope. Then a value crosses a wire and the story
degrades to `assume`. The consumer stub mirrors the server's contract clauses
and the client's compiler takes them on faith; the interface hash confirms
both sides agree on what the contracts *say*, which is agreement on
vocabulary, not evidence of truth. The assumption surface records all of this
faithfully — but bookkeeping of trust is not a reduction of it.

Name the failure precisely. A claim can be warranted three ways: **perception**
(I computed it from objects in my hand — what wire-decode validation does),
**proof** (I derived it from things already warranted — what typeck does),
or **testimony** (the only ground is that someone else asserted it). Every
cross-boundary behavioral claim today is testimony — and testimony is not
deficient because peers lie. It is a different category because **warrant does
not travel**: a proof possessed by the server is, to the client,
indistinguishable from a rumor of one. And the claim underneath — "the
process answering this socket is the binary that was proven" — is not even
testimony; it is presumption.

`assume` is a compile-time posture about a runtime relationship. This RFC
replaces the posture.

### The whole-program objection

Doesn't whole-program compilation dissolve this? When one compile produces
both sides, the "server's" ensures was proven by the same invocation that
compiled the client — the client consumes its own theorem, not a foreign
assertion. Conceded: within the compilation, that is proof, not testimony.
But the theorem is about the *code*, and the claim acted on at a call site
is about a *process* — and three gaps survive single-compile deployment,
none of them about authorship:

1. **Deployment identity does not survive time.** The compile proves facts
   about deployment D; the wire never guarantees you are in D. Rolling
   restarts, stale binaries, misrouted ports: two binaries from two honest
   whole-program compiles meet on one socket. The runtime hash handshake is
   the confession — if compilation made peer identity a compile-time fact,
   no handshake would exist, and what the handshake checks is the peer
   *testifying* about its own hash. Binding "this socket, now" to "that
   proof's subject" is not a property of code.
2. **Proofs quantify over executions; conversations span them.** A
   two-state ensures describes one execution of the server. Across a crash
   and restart, `old(...)` is not the old of any execution the proof
   mentioned.
3. **A remote state fact is stale at receipt — always.** Locally,
   ensures-facts stay alive because the facts engine kills them at every
   mutation point it can see. Remotely, the serialized authority keeps
   serving other clients, so every instant is an invisible mutation point:
   the honest severity is sever-always. Even a perfectly warranted claim
   about the authority's state has zero shelf life. This gap holds with
   certified identity and no restarts, and no compilation scheme closes it.

Refined thesis: whole-program compilation upgrades cross-boundary claims
from testimony to **proof about the composition's code**. What remains
unprovable is the binding of this conversation to that composition and of
this instant to that proof. The gap is identity-and-time, not authorship —
which is why mechanism 3 (the deployment plan as composition certificate)
is the honest form of the whole-program intuition, and why the demotion in
mechanism 1 distinguishes ensures over *returned values* (values in hand —
checkable at decode, legitimately kept) from ensures over the *authority's
persistent state* (dead on arrival from staleness, whoever proved them).

## First principles: closure, not trust

What makes the local machinery work is not that local code is trustworthy —
it is that the compiler has **closure**: it sees every write site, every
path, every event that can affect the state it reasons about. STRICT is
justified by totality of vision. Cross the wire and closure is what breaks:
the events affecting remote state are not enumerable by any local compiler.
That is not a weakness of our prover; it is Two Generals and Halpern–Moses
wearing a type system. No mechanism will ever let a client *know* the
remote's current state.

So split the world by closure instead of by location:

- **My conduct** — what I send, in what order, with what evidence attached,
  and what I do with every message I might receive. Fully closed-world: my
  compiler sees all my sends and receives. Provable, statically, today.
- **The world's state** — what is true at the remote, now. Open-world.
  Unprovable by theorem, forever.

Every `assume` is an attempt to smuggle a claim of the second kind into
machinery built for the first. The doctrine is to stop importing them:

> **Prove where you can see. Where you cannot see, type the conversation —
> never the other side's state.**

Each party gets proofs about its own conduct; the protocol is designed so
that jointly conformant conduct implies the global property — which is then
known by no one, because it lives in the only honest place it can: the
composition.

## The four bins

Every claim that matters at a boundary lands in exactly one:

1. **Conduct** — claims about my own behavior in the conversation.
   *Warrant: proof.* Linearity, obligation discharge, protocol conformance.
2. **Values in hand** — claims about data I received.
   *Warrant: perception.* Decode-time invariant validation: I run the
   predicate on the bytes myself. Already shipped, already honest.
3. **Sovereign facts** — claims about a resource the peer *owns*.
   *Warrant: jurisdiction.* "The kernel wrote my bytes." "The authority's
   epoch advanced." "The row was committed." The peer's answer does not
   *report* the fact; it **constitutes** it — a byzantine authority does not
   need to lie about its blob, it owns the blob. Sovereignty is irreducible
   by definition and therefore is not epistemic debt. The language's job is
   to mark it, not to apologize for it.
4. **Shared protocol structure** — claims of the form "the StaleGrant branch
   is effect-free," "commit consumes the transaction."
   *Warrant: composition.* Each side's conformance to its projection of the
   protocol is proven by whoever compiles that side; the protocol identity
   (the interface hash) binds the projections together; a deployment
   artifact can carry both proofs.

Progress at the boundary means exactly one thing: **driving claims out of
testimony and into these four bins.** The assumption surface is the
scoreboard, and after this RFC it should list only genuine imports —
vendored C, foreign systems — where testimony honestly belongs.

## The existence proofs are already in-tree

This doctrine was not invented; it was noticed.

**`File<M, S>` is a typed boundary.** The kernel is a remote authority. We
never assume the OS's ensures and never check its invariants. We typed the
*conversation* — open → use → close, failure as a typed edge (`Degraded`),
obligations linear — and proved our side's conformance at compile time. The
guarantee ("no leaks, no use-after-close, no retry-after-fsyncgate") is a
property of composing our proven conduct with the kernel's sovereign
behavior. Zero testimony consumed. The fs API felt inevitable because this
shape is right.

**Blob is the other half.** Rewriting `std.blob`'s client in this framing
changes *nothing*: the client holds a perceived value (the grant), presents
it, and handles `StaleGrant` — it never consumes the authority's `ensures`.
The fence is the authority conforming to *its* projection ("the Stale branch
dominates the effect"), proven by its own compiler via `verify.fenced`. The
one belief the client acts on — "StaleGrant means no byte landed" — is
sovereign, not assumed: the authority owns the blob either way.

**The projection checkers already shipped, unlabeled.** Client-side
conformance to a protocol is "every message the peer can send you must be
handled" — which is the inferred-error-set machinery with mandatory
handling. Authority-side conformance is dominance + ensures + entity
serialization. The interface hash is the protocol identity. Pluto built a
two-party session-type system by accident, distributed across features
shipped for other reasons. What follows is the deliberate completion.

## Mechanism 1: demote cross-boundary `ensures`

Behavioral clauses stop crossing boundaries as facts. At a remote call site
(`at`, `remote`, served handles), the caller's prover does **not** receive
the callee's `ensures`/`provides` as discharged knowledge. They remain part
of the interface (they still hash, still version, still document), but their
epistemic status at a remote site is *sovereign or structural, never
proven-for-you* — and `pluto analyze` reports exactly which clauses a
program would have consumed across a boundary, so the cost of the demotion
is visible, not silent.

Local calls are untouched: within a process the compiler sees the callee,
and the clause is a theorem. This is a breaking change for any code that
leaned on remote ensures; the Blob exercise suggests well-shaped protocols
never needed them, and the ones that did were the problem.

## Mechanism 2: the conversation is a typestated evidence handle

Today nothing types the *order* of calls on a remote handle:
begin-before-insert, commit-consumes, at-most-one-outstanding. Typestates do
this for local values; entity handles deliberately carry no state params —
because an entity's state param would claim knowledge of the authority's
truth, which was rightly rejected (authorities-vs-evidence).

The resolution is to type a different thing. A conversation handle's state is
not the remote's truth — it is **the client's knowledge state**: "as of the
last message, the conversation stood here." That is honest exactly when every
transition the *world* can force is an edge in the type the client must
handle: revocation, timeout, disconnect, supersession. Which is precisely the
lease discipline rfc-entity-lifecycle.md ratified — the handle **is** a lease,
and degradation edges are its expiry.

**This needs no new construct.** The client's knowledge state is, by
definition, *evidence* — a client-side, linear, typestated value — the exact
counterpart to the authorities (which we typestate-reject) in the
authorities-vs-evidence split. Evidence is already "a typestated linear
value," which the language already has. So the conversation handle is an
**ordinary typestated class**, generated as the client stub of a served
interface; it is not a new `session` declaration form. This mirrors the
module-semantics resolution ("the protocol is the module," no new `protocol`
construct): here, the typestated handle *is* the session.

Concretely, the served interface carries the per-method facts inline, and the
generated client stub is a typestated evidence class:

```pluto
// On the served interface — per-method preconditions and transitions only.
// The handle type Txn<S> is an ordinary typestated class; the stub is generated.
fn insert(self, e: Entry)   where S == Active   // raises Refused
fn commit(self) Txn<Done>   where S == Active   // consuming transition
fn rollback(self) Txn<Done> where S == Active   // consuming transition
```

The world's edges are **not author-written and not per-conversation to
declare** — disconnect (transport died: AMBIGUOUS) and expiry (the authority
reaped the lease: clean) are *universal consequences of remoteness*,
identical for every served handle. The **stub generator injects them**,
because generating a remote stub is precisely where "this handle is remote"
is known; the author never types a `degrades` list. (A genuinely
protocol-specific edge — a `Superseded` that is not generic expiry — is one
extra edge on the single method that can emit it: per-method and correct, not
a smeared per-conversation annotation. No handle-level `volatile`-style marker
is needed; "remote" is already known from the interface being served.)

What the machinery buys, all with existing, *shipped* passes — `std.fs`'s
`File<M, S>` (must_release, `Degraded`-carrying-`Poisoned`) is the local
existence proof: leaking an open transaction is a compile error
(`must_release`, live); using a handle after commit is a compile error
(linearity transitions consume, live); ignoring a degradation edge is a
compile error (error-set coverage — the projection check, live); and the
ambiguous/definite classification rides the rfc-distributed-safety taxonomy,
so "commit sent, no ack" is a typed ambiguous outcome whose safe retry leans
on a checked idempotency key. Blob is the degenerate case: a single-state
conversation, compiling unchanged.

**One axis stays open** (does not block the construct decision): the injected
world-edges may surface either as **degradable-typestate errors** the caller
`catch`es — which composes directly with `!` propagation and the existing
error-set coverage that powers the projection check — or as a **sum the
caller `match`es/narrows** (like nullable narrowing), which is more honest
that an expired lease is a *transition*, not a failure, but must earn the
"every world-edge handled" guarantee from match-exhaustiveness instead of
error-set coverage. Error-surfacing is the lower-risk default (free
integration with the projection checker); narrowing is the cleaner semantics.
The Postgres dogfood (acceptance, below) is the right place to decide.

Because the local building blocks are shipped and proven in `std.fs`/`std.wal`,
the only genuinely new implementation work is **the remote half**: (1) the
stub generator that projects a served interface into the typestated evidence
handle (and injects the universal edges), and (2) transport-delivered edges —
a real network disconnect, and an authority-side lease timeout/revocation,
arriving as degradation — which is where the lifecycle RFC's lease runtime
bites. A conversation against a *local* authority works on today's machinery;
the remote projection + transport-edge delivery is this mechanism's new code.

## Mechanism 3: the deployment plan as composition certificate

Bin 4's warrant — "both projections were proven" — currently lives nowhere.
distributed-model.md left "the deployment-plan artifact" as an open
question; this is its job. The deployment plan records, per boundary: the
protocol identity (interface + session hash), which side's conformance was
proven by which build, and the residual assumption-surface entries for any
side the compiler never saw (a foreign peer, a non-Pluto client). A
deployment where every boundary's two projections carry proofs has a
composition warrant for its global properties; a deployment with a foreign
side has, instead, an honest statement of what conformance was presumed.
Scope here is the schema and `pluto analyze` reporting only — plan
generation and verification tooling is its own later RFC.

## What this RFC refuses

- **No runtime monitors.** Checking a peer's promise after the fact keeps
  claim-shaped thinking and fires too late; nothing here inserts dynamic
  contract checks. (Decode validation stays — that is perception of a value,
  not audit of a promise.)
- **No multiparty protocols.** Two-party sessions first; choreography can
  wait until a real system demands it.
- **No byzantine tolerance beyond fencing.** A non-conformant peer is made
  *ineffective* (fencing, evidence values) or *detected as protocol-illegal*
  (a typed edge), never trusted into correctness.
- **No re-litigation of authorities-vs-evidence.** We do not typestate the
  authority's truth. We typestate the client's conversation. This is the
  completion of that rejection, not its reversal.

## Acceptance

Two programs, two ends of the spectrum:

1. **Blob unchanged** — the single-shot protocol compiles as it stands, its
   assumption-surface entries for remote ensures vanish under mechanism 1
   with no replacement needed, demonstrating that well-shaped existing code
   pays nothing.
2. **A transactional client** — the Postgres library (separate RFC) is the
   real test: a multi-step conversation against a sovereign peer that will
   never run Pluto. begin/insert/commit typed as a typestated evidence
   handle; leaked transactions a compile error; disconnection and expiry as
   degradation edges with honest ambiguity on commit; constraints perceived
   at decode. If a typestated evidence handle cannot type the Postgres
   protocol cleanly, mechanism 2 is wrong and comes back to this document —
   and this is also where the open error-vs-narrow surfacing axis is decided.

## Owner decisions

1. **Demote remote ensures** (mechanism 1) — recommended yes; breaking, with
   `pluto analyze` naming every affected site.
2. **No `session` construct — the conversation is a typestated evidence
   handle** (mechanism 2, revised). The earlier draft proposed a dedicated
   `session … for …` declaration form; it is withdrawn. The handle is the
   client's knowledge state = evidence = an ordinary typestated linear value,
   generated as the client stub of a served interface. Preconditions and
   transitions are per-method on the interface; the universal world-edges
   (disconnect, expiry) are injected by the stub generator, not author-written.
   No new keyword, no handle-level marker. **Recommended.** Open sub-axis,
   deferred to the Postgres dogfood: degradation edges surface as
   `catch`-able errors (composes with error-set coverage — lower risk) vs a
   `match`/narrow sum (more honest; needs exhaustiveness to carry the
   all-edges-handled guarantee).
3. **Conversations are leases** — the handle rides the lifecycle RFC's lease
   runtime (expiry, reaping) rather than growing a parallel mechanism.
   Recommended yes. (Entails #2; the lease is what delivers the injected
   expiry edge.)
4. **Deployment-plan scope** — schema + analyze reporting now, tooling
   later. Recommended yes.
5. **Vocabulary** — "conduct / perceived / sovereign / structural" as the
   diagnostic and documentation terms for the four bins. Naming is doctrine
   here; alternatives welcome.

Note on status: the *local* building blocks for mechanism 2 — typestates,
consuming transitions (linearity), `must_release`, and
degradable-typestate errors carrying the post-state — are implemented and
shipped (`std.fs`'s `File<M, S>` with `Degraded`/`Poisoned`; `std.wal`). Only
the remote half (stub projection + transport-delivered edges) is new work.

## Phasing

1. **The demotion** — mechanism 1, plus assumption-surface accounting of
   every formerly-assumed remote clause. Small, breaking, honest; ships the
   doctrine's teeth first.
2. **Conversation handles** — the client-stub projection of a served
   interface into a typestated evidence handle (and injection of the
   universal world-edges), plus transport-delivered degradation on the lease
   runtime. Linearity, `must_release`, and error-set/degradable-typestate
   enforcement are already shipped (`std.fs`), so this phase is the *remote*
   half only: the generator and the transport edges. Depends on the lifecycle
   RFC's lease runtime.
3. **The certificate** — deployment-plan schema and analyze surface.
4. **The dogfood** — the Postgres RFC and library, exercising all three
   against a peer we do not control.

Each phase is independently shippable; phase 1 alone makes the language stop
saying things it cannot know.

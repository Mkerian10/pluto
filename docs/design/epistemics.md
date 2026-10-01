# Epistemics: The Semantic Under the Language

**Status:** Foundations — the organizing principle behind the verification engine,
the object construct, and the distributed model; direction accepted in design
discussion (2026-09-29/30)
**Author:** Design discussion
**Date:** 2026-09-30
**Related:** [rfc-verification.md](rfc-verification.md), [distributed-model.md](distributed-model.md), [rfc-objects.md](rfc-objects.md), [rfc-typestates.md](rfc-typestates.md), [rfc-distributed-safety.md](rfc-distributed-safety.md), [../v1-vision.md](../v1-vision.md)

This document names the semantic that Pluto's distributed features instantiate.
It is not an implementation RFC; it is the lens future design questions get asked
through: *what knowledge does this action require, and who provides the warrant?*

## The break: regular languages assume knowledge is free

An ordinary language has an **ontic** semantics: program state *is* the world.
A read gives you the truth; memory is authoritative; "what I know" and "what is"
coincide. Every mainstream language is built on that identity, which is why none
of them needs a concept of knowledge at all.

Distribution breaks the identity. The moment part of the world lives outside the
process, every value held about it stops being the world and becomes a **belief
about** the world — a snapshot, acquired at some moment, on some warrant, valid
under some conditions. Regular languages cannot express the difference between
"x is the balance" and "x is what the balance was when I asked." They collapse
belief into fact, so programmers do the epistemic bookkeeping in their heads —
and every classic distributed bug is that bookkeeping failing:

- using a lease after the coordinator revoked it,
- retrying a write whose outcome was unknown,
- reporting an ambiguous outcome as a definite failure,
- trusting a cached read as current truth.

**These are all the same error: acting on a warrant weaker than the action
requires.** Ordinary languages cannot catch this error because they cannot state
it. Pluto's semantic bet: a program is a *knower*, its state is a body of claims
with justifications, and the compiler audits the justifications.

(This is the Halpern–Moses tradition — distributed protocols as transformations
of knowledge states; two-generals as the impossibility of common knowledge over
lossy channels — put *inside* a language and checked by whole-program
compilation, which is where it has never lived.)

## The warrant gradient

Claims differ in how they are justified. Strongest to weakest:

| # | Warrant | Source | Fallibility |
|---|---|---|---|
| 1 | **What I did** | typestate — introspective knowledge of my own actions | infallible |
| 2 | **What all code paths preserve** | invariants — closed-world knowledge (every write site seen; entity methods serialize) | infallible given compiler soundness |
| 3 | **What I made impossible** | fencing — constructive knowledge: the old write *cannot* land, so I need not learn whether it did | infallible given the authority's check |
| 4 | **What was promised until T** | leases — testimony with an expiry; a remote fact made *temporarily* stable | fails only via clock/pause assumptions — which is why windows are liveness, never safety |
| 5 | **What I was told** | any boundary read — testimony, stale the moment it arrives | always |
| 6 | **What I no longer know** | ambiguous failure — ignorance; must be resolved or honestly escalated, never laundered into a definite claim | — |

Two structural notes:

- **Level 1 is why typestate on shared entities was unsound** (rfc-objects.md):
  it claimed introspective certainty about a fact that isn't introspective.
- **Monotone facts are the exception that proves the ladder**: a claim that once
  true can never be false ("epoch ≥ 42", "the set contains x") can be cached and
  shared at level-5 warrant *forever*, because nothing can invalidate it. This is
  the CALM insight (monotone ⇔ coordination-free) and why monotonic fields are a
  kernel primitive.

## Actions have knowledge preconditions

The language's role, restated: every action has a knowledge precondition, and
the compiler checks that the warrants actually held at that point entail it.

- Write to the blob ⇒ hold unexpired testimony-with-promise (level 4) **and**
  the authority applies a constructive check (level 3) at the point of effect.
- Retry an effect ⇒ *know* it did not apply (definite failure), or make the
  retry itself produce knowledge (idempotency).
- Share a fact freely across tasks and boundaries ⇒ it must be monotone.
- Report an outcome upward ⇒ report the *knowledge state*, not a guess:
  ignorance propagates as ignorance.

Failure classification falls out of the same frame. Every failure of an
effectful boundary call is one of exactly two kinds:

- **Definite** — the effect is known not to have applied (connection refused
  before send; the authority's fence or `requires` rejected it). The world is in
  a known state; react freely. A revoked lease surfaces here — "degradation" is
  not a concept of its own, just a definite failure with state-carrying plumbing.
- **Ambiguous** — the request left the process and no response returned. The
  effect applied or it didn't; no local information can say. Ambiguity has
  exactly four legitimate exits: idempotent retry (the retry's outcome is
  knowledge), read-back, self-fencing (convert to definite-not-applied by making
  the in-flight write unable to land), or honest escalation.

The runtime owns the classification (only it knows whether bytes left the
socket); everything built *on* the classification is library policy — see below.
Implemented: `NetworkError.definite` carries the classification, with the
condition→class table and guarantee in
[rfc-distributed-safety.md](rfc-distributed-safety.md) ("Failure classification").

## The value/entity split is the belief/fact split

Pluto already encoded the core distinction before naming it:

- A **value** is a snapshot — a belief, honest about its staleness, structurally
  comparable because beliefs with the same content are the same belief,
  deep-copied across concurrency because beliefs are cheap to duplicate.
- An **entity** is the referent itself — never held, only *asked* (a call is a
  question, a mutation is a command), always current precisely because you
  cannot possess it, identity-compared because referents are not their
  descriptions.
- Crossing a boundary turns the thing into a belief about the thing (wire
  values), or preserves the referent as a handle you can only message.

Every construct maps onto an epistemic role:

| Construct | Epistemic role |
|---|---|
| typestate (`where S == Held`) | introspective record of my own protocol actions |
| linearity / must-release | obligations attached to evidence; evidence is not duplicable |
| `invariant` | universally quantified knowledge, warranted by the closed world |
| fencing / epochs | constructive knowledge; ambiguity → definiteness converter |
| lease / grant values | testimony with a promise window (liveness only) |
| boundary reads / wire values | plain testimony (snapshots) |
| definite vs ambiguous failure | knowledge vs ignorance about an attempted effect |
| error inference | propagation of possible-ignorance through the call graph |
| entity / `at` | the referent; questions and commands to it |
| monotonic fields | invalidation-free knowledge; safe to cache and share |

## The genericity principle: properties are someone's responsibility

**The language hardcodes no specific epistemic property.** Baking in
"idempotency" or "transactionality" as language magic would repeat the rejected
wire-traits and typestated-objects mistakes: hardcoding *instances* of a pattern
instead of the kernel that expresses them. Instead the language ships three
domain-neutral things:

1. **Kernel obligation shapes** (rfc-verification.md): write-path invariant
   preservation, dominance (every effect guarded by a check), monotone writes,
   linear consumption. The decidable fragment; nothing domain-specific.
2. **Named properties, declared in libraries.** `idempotent(key)`, `monotonic`,
   `transactional`, `fenced` are *stdlib definitions* (e.g. `std.distributed`)
   whose meaning bottoms out in kernel shapes. The compiler knows nothing about
   idempotency; it knows how to check the shapes a property names.
3. **Requires/provides matching, with owners and discharge modes.** Every
   property claim has a named owner and exactly one warrant:
   - **Proven** — the kernel discharged it from the body (the dedup check
     provably dominates the effect).
   - **Checked** — a runtime residual enforces it (the dedup table is actually
     consulted; violations raise typed errors).
   - **Assumed** — declared explicitly at a trust boundary. `extern` services
     the compiler cannot see (S3, a legacy API) carry declarations like
     `assume idempotent(key)`; the assumption is visible, attributed, auditable.

So "retry from ambiguity" is not a language rule about idempotency: the stdlib
retry combinator *requires* `idempotent(key)` of its callee; the authority
author *provides* it — by proof if their code is in the compilation unit, by
assumption if extern; the compiler *matches* requirement to provision and never
loses track of who vouched. Responsibility is precise: every warrant in the
system has a named owner.

**The assumption surface.** `pluto analyze` reports every claim discharged by
assumption rather than proof or check — the complete list of things the system
rests on that nobody proved. A deployment's honesty, as an artifact.

Applied retroactively to earlier proposals:

- **Transactional entity methods** are a property, not a blanket guarantee:
  auto-provided (kernel-proven) for methods whose bodies have no nested boundary
  effects — a raise rolls the entity back, so ambiguity is only ever one bit,
  never a torn state — and absent for methods with nested `at` calls, which the
  compiler can see. Callers who need all-or-nothing require the property.
- **Failure policy** (what a caller may do from Definite vs Ambiguous) is
  library combinators with property requirements, not compiler special cases.

## Non-goals

- **No modal-logic surface.** No K-operators, no belief algebra in user syntax.
  The constructs above *are* the surface syntax of warrants; the epistemics is
  the design lens, not a user-facing calculus.
- **No SMT** (per rfc-verification.md). Warrant checking bottoms out in the
  decidable kernel shapes plus property matching.
- **No pretense at trust boundaries.** Extern claims are assumptions and are
  reported as such, never silently promoted.

## Prior art and the gap

- **Halpern–Moses knowledge logic** — the theory; never a programming language
  (formal grounding below).
- **Session types** — knowledge of protocol *position*, not of world state.
- **Information-flow types (Jif etc.)** — who *may* know (deontic), not whether
  you *do* and on what warrant.
- **CRDTs / Bloom / CALM** — the monotone corner only.
- **TLA+** — can model all of it, but outside the program, unchecked against it.

The gap Pluto occupies: a type system that audits *how you know*, checked by a
whole-program compiler that sees both sides of every boundary.

### Formal grounding (Halpern–Moses and successors)

The epistemic-logic tradition in distributed computing supplies proofs for
several claims this document otherwise makes by argument.

**The framework** (Halpern & Moses, *Knowledge and Common Knowledge in a
Distributed Environment*, PODC '84 / JACM '90): a system is its set of possible
**runs**; process *i* **knows** φ at a point iff φ holds at every point
consistent with *i*'s local state (indistinguishability semantics). Knowledge
is a property of the protocol's information structure, not a mental state.

Consequences, mapped:

- **A sound type is a K-claim.** A type must hold at every point consistent
  with local state — exactly the knowledge operator. "The coordinator currently
  considers me the holder" fails (revoked runs are indistinguishable), which
  *proves* the "remote beliefs are never sound types" rule. Typestate passes
  (action history is local state). A lease-until-T passes **only given**
  clock/pause assumptions that exclude the past-deadline runs — the formal
  reason those assumptions must be declared, never defaulted.
- **Common knowledge is unattainable** over uncertain communication (their
  formalization of two-generals; even reliable-but-unbounded-delay channels
  cannot attain it), and simultaneous coordinated action *requires* it. Design
  rule with a theorem behind it: **no Pluto construct may require in-the-moment
  agreement between parties** — no "both sides know" types, no
  synchronized-view abstractions. Everything decomposes into unilateral
  warrants plus authority-side checks (the Blob shape), and version-skew
  handling assumes no synchronized deployments.
- **The attainable weakenings are our primitives.** H–M's hierarchy below full
  common knowledge maps directly: *timestamped* common knowledge ("by time T,
  all know") **is a lease**; *eventual* common knowledge **is the monotone
  corner** (facts that only accrete converge without invalidation — CALM,
  reached from the logic side). The warrant gradient is a classification of
  which runs local state can exclude, and why.
- **Knowledge of Preconditions** (Moses, TARK 2015): in any correct protocol,
  a process taking an action that requires φ must *know* φ when acting.
  Correct distributed programs already satisfy this implicitly, in their
  authors' heads. Pluto's one-sentence pitch: **the compiler enforces KoP** —
  it refuses to compile programs whose actions outrun their knowledge.
- **Knowledge-based programs** (Fagin–Halpern–Moses–Vardi, *Reasoning About
  Knowledge*, 1995): specifications with explicit knowledge tests
  (`if K(φ) then act`), implemented by concrete local-state predicates that
  entail them. The literature's hard problem is synthesis (implementations may
  not exist or be unique); Pluto keeps only the sound direction — the
  programmer supplies the concrete evidence (token, deadline, typestate) and
  the compiler **verifies the entailment**. We type-check against knowledge
  programs; we do not synthesize them.
- **Learning vs run-elimination.** The framework distinguishes coming to know
  by *refining* which run you are in (a response, a read-back) from coming to
  know by *acting so the bad runs cease to exist* (fencing, self-invalidation).
  This is precisely the constructive-knowledge (level 3) vs testimony (level 5)
  distinction, and the formal reason fencing is robust to message loss while
  learning never is.

## Open questions

1. **Property declaration syntax.** How does a library define a named property
   and its kernel obligations? How do properties parameterize
   (`idempotent(key = order_id)`)?
2. **Provision syntax.** How does a function/type declare it provides a
   property; how does a `requires` clause reference one?
3. **Assumption syntax.** The `extern` declaration form and its granularity
   (per-endpoint? per-property?). Interaction with rfc-distributed-safety.md's
   open question on third-party services.
4. **Classification mechanics.** ~~Where exactly the runtime draws
   definite/ambiguous, and how the classification rides on the existing
   NetworkError surface.~~ Resolved: the line is drawn at completion of the
   length-framed request write (an incomplete frame is never dispatched), and
   the classification rides as `NetworkError.definite` — see
   rfc-distributed-safety.md "Failure classification". Still open: client-side
   response deadlines (none exist yet) and their interaction with the
   classification.
5. **Property evolution.** A provider dropping or weakening a property is a
   breaking change for requirers — how does this interact with interface hashing
   and the migration rules?

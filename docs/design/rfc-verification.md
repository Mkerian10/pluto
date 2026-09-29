# RFC: The Verification Engine

**Status:** Draft — direction accepted in design discussion (2026-09-28); nothing implemented
**Author:** Design discussion
**Date:** 2026-09-28
**Related:** [v1-vision.md](../v1-vision.md) (Static Verification), [contracts.md](contracts.md), [rfc-typestates.md](rfc-typestates.md), [rfc-objects.md](rfc-objects.md), [rfc-distributed-safety.md](rfc-distributed-safety.md), [distributed-model.md](distributed-model.md)

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

- **Invariants on types** (`invariant self.balance >= 0`) — per-type facts, today
  runtime-checked. Upgrade: the compiler attempts to *statically discharge* each
  invariant at each write site. What it proves costs nothing at runtime; what it
  can't stays a runtime check (see the proof ladder below).
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

**proof > runtime check > typed error.** Nothing becomes undefined behavior;
unprovability costs a branch and an error-set entry:

- A declared fact the compiler can discharge statically is free at runtime.
- A declared fact it cannot discharge remains a runtime check at the unproven write
  sites only (per-site residuals).
- A runtime check that can fail surfaces as a typed error, and error handling is
  already mandatory.

This is **gradual verification**: today's programs keep working (invariants stay
runtime-checked), `pluto analyze` reports proof coverage (which sites are proven vs
residual), and codebases tighten over time. A strict mode (unproven invariant =
compile error) can exist later as an opt-in.

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
2. **Fallible transitions** — `refresh()` can raise. *Decided 2026-09-28*: the
   error **carries** the value in its post-failure state
   (`Degraded { blob: Blob<Unknown> }`) so obligations — notably must-release —
   survive the raise. Catch-binding ergonomics and linearity accounting for the
   consumed receiver are implementation questions (open question 4).
3. **Degradation** — the world moves you out of a state. Surfaces as typed errors on
   *use* (every effectful method of a degradable state has the degradation error in
   its set; `at`'s mandatory-handling contract makes it unignorable). The protocol
   author programs what degrades, to what, and what evidence renews it.

**Must-release** (decided with this direction; marking is an **explicit state-level
annotation** on the class — decided 2026-09-28, exact syntax TBD): a binding in a
marked must-release state (e.g. `Lease<Held>`) may not go out of scope un-transitioned — compile error,
powered by the existing moved-binding linearity analysis. Deterministic, no runtime
magic. Release must be legal (and idempotent) from degraded states — "clean up your
local belief" is an obligation the world cannot revoke.

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

The engine is done when this is expressible in the standard library, in Pluto, with
its theorem checked (syntax illustrative — see open questions):

```pluto
object BlobAuthority {
    data: bytes
    epoch: int

    invariant monotonic(self.epoch)          // proven: every write increases it

    fn grant_write(mut self) WriteGrant {    // mints token, advances epoch
        ...
    }

    fn apply(mut self, g: WriteGrant, d: bytes) {
        if g.token < self.epoch { raise Degraded }   // the fenced compare
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
| Safety (single-writer) | fenced compare in `apply` | dominance proof (this RFC) |
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
2. **Gradual by default.** *(Proposal — deliberately left OPEN 2026-09-28: whether
   unproven declared invariants demote to per-site runtime residuals or hard-error
   is not yet decided.)* If gradual: residuals per unproven write site, `analyze`
   reports coverage, strict mode later as opt-in.
3. **No SMT.** *(Proposal.)* Fixed decidable domain: intervals, equalities, linear
   arithmetic, dominance. Extend the domain deliberately, primitive by primitive.

## Open questions

1. **Fact syntax.** How are exported theorems named and declared? (`ensures
   no_stale_write` as a named type-level theorem is strawman syntax.) How is
   dominance written — a builtin predicate vocabulary (`monotonic(...)`,
   `guarded_by(...)`) or structural inference from the body?
2. **Fragment contents.** Exact initial domain: intervals over ints; equalities over
   which types; what of floats (IEEE comparison pitfalls), strings, collections
   (`len()` facts)?
3. **Facts across the wire.** A proven invariant on a schema type — does the receiver
   assume it (same compilation unit, interface hash guards skew) or recheck at the
   boundary as defense in depth? Interaction with schema evolution rules.
4. **Fallible-transition mechanics.** (Errors carrying the post-failure-state value
   is DECIDED — remaining:) what does `catch` binding look like for a typestated
   payload, and how does linearity account for the consumed receiver on the raise
   path?
5. **Must-release syntax.** (Explicit state-level annotation is DECIDED — remaining:)
   the concrete surface, e.g. `must_release Held` in the class body vs a marker on
   the state type.
6. **Reentrancy vs dominance.** Entity self-calls (rfc-objects.md open question 6)
   interact with "methods serialize" as a proof precondition — a reentrant call
   observing a half-updated invariant would be unsound. Needs a story before
   invariant discharge on entities ships.
7. **Contract checking cost.** Residual checks in hot paths — is there a profile
   or per-site opt-out, and does `analyze` rank residuals by cost?

## Phasing (proposal)

1. **Flow-fact generalization** — extend narrowing from nullability to comparisons /
   intervals on ints. Self-contained; immediately useful (dead-check elimination,
   better diagnostics). No new syntax.
2. **Invariant static discharge** — prove declared class invariants at write sites
   where the fragment allows; per-site runtime residuals otherwise; `analyze`
   coverage reporting.
3. **Error-set shrinking** — fact-sensitive error inference: a `raise` proven
   unreachable at a call site removes the variant from that site's inferred set.
4. **Distribution primitives** — monotonic fields and dominance (guarded effects);
   the proof shapes behind fencing.
5. **Degradable typestates + must-release** — the three transition kinds, error-
   carried state, must-release linearity. (Client: objects RFC phase 3.)
6. **Blob in stdlib** — the acceptance test, end to end.

Phases 1–3 are pure compiler work with no language-surface change beyond diagnostics,
and each pays for itself independently of the distributed story.

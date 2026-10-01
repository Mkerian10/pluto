# Verified Distribution

> **This chapter is direction, not documentation.** The foundations described here — entities, typestates, compile-time invariant proofs, error-set shrinking — are shipped and covered in Part 2. The destination they point at — a proof kernel that makes distributed-systems correctness a library concern — is not. Nothing in the second half of this chapter should be read as a feature you can use today.

## The bug every distributed system has

An ordinary language has an *ontic* semantics: program state **is** the world. A read gives you the truth; memory is authoritative; "what I know" and "what is" coincide. Every mainstream language is built on that identity, which is why none of them needs a concept of knowledge at all.

Distribution breaks the identity. The moment part of the world lives outside the process, every value you hold about it stops being the world and becomes a **belief about** the world — a snapshot, acquired at some moment, on some warrant, valid under some conditions. Regular languages cannot express the difference between "x is the balance" and "x is what the balance was when I asked." So programmers do the epistemic bookkeeping in their heads, and every classic distributed bug is that bookkeeping failing:

- using a lease after the coordinator revoked it,
- retrying a write whose outcome was unknown,
- reporting an ambiguous outcome as a definite failure,
- trusting a cached read as current truth.

These are all the same error: **acting on a warrant weaker than the action requires.** Ordinary languages cannot catch this error because they cannot state it.

Pluto's bet is that a whole-program compiler can. A program is a knower; its state is a body of claims with justifications; the compiler audits the justifications. This is the Halpern–Moses tradition — distributed protocols analyzed as transformations of knowledge states, two-generals as the impossibility of common knowledge over lossy channels — put *inside* a language, where it has never lived. Moses' Knowledge of Preconditions principle says that in any correct protocol, a process taking an action that requires some fact must *know* that fact when acting. Correct distributed programs already satisfy this implicitly, in their authors' heads. The one-sentence version of this chapter: **the compiler should enforce it** — refusing to compile programs whose actions outrun their knowledge.

## The warrant gradient

Claims differ in how they are justified, strongest to weakest:

| # | Warrant | Mechanism | Can it fail? |
|---|---------|-----------|--------------|
| 1 | What I did | typestate — a record of my own actions | no |
| 2 | What all code paths preserve | invariants — every write site seen and proven | no, given compiler soundness |
| 3 | What I made impossible | fencing — the old write *cannot* land | no, given the authority's check |
| 4 | What was promised until T | leases — testimony with an expiry | only via clock/pause assumptions |
| 5 | What I was told | any boundary read — testimony, stale on arrival | always |
| 6 | What I no longer know | ambiguous failure — ignorance | — |

Two structural observations fall out:

**A sound type is a claim at level 1 or 2.** "I acquired and have not released" survives from check to use because only you can change it — which is exactly what a `Lease<Held>` typestate claims, and why it is honest. "The coordinator currently considers me the holder" is level 5 — the world can falsify it between any two instructions — and any design that encodes it statically is lying. This is why typestate never lives on a shared entity in Pluto, and why the lease example's revocation arrives as a typed `Degraded` error rather than being promised away by a type.

**Monotone facts are the exception that proves the ladder.** A claim that once true can never be false — "the epoch is at least 42," "the set contains x" — can be cached and shared at level-5 warrant *forever*, because nothing can invalidate it. This is the CALM insight (monotone means coordination-free), and it is why monotonic fields are slated to be a kernel primitive.

## What already embodies this

The epistemics is not a future layer bolted on top; it names what the shipped language already does.

**The value/entity split is the belief/fact split.** A class value is a snapshot — a belief, honest about its staleness, structurally comparable because beliefs with the same content are the same belief, deep-copied across concurrency because beliefs are cheap to duplicate. An object is the referent itself — never held, only *asked*; a call is a question, a mutation is a command; identity-compared because referents are not their descriptions. Crossing a boundary turns a thing into a belief about the thing (a wire value) or preserves the referent as a handle you can only message.

**Invariant discharge is closed-world knowledge.** "Every write path preserves `balance >= 0`" is only a knowable fact because the compiler sees every write path — and only a *sound* fact under concurrency because values don't alias and entities serialize their methods. Classes are provable because they don't share; objects are provable because they serialize. That is the deep answer to why the object construct exists.

**Error inference is ignorance propagation.** The error set of a function is the set of ways it might leave you not knowing what you hoped; mandatory handling is the rule that ignorance must be confronted, not discarded. Error-set shrinking is the converse: a proof that removes a possibility removes the obligation to handle it.

Two problems that look unrelated — "this field's write paths all guard against negatives, so `balance >= 0` is a fact" and "this blob store's fencing is correct: the epoch only increases and no write lands without a validity check" — are **the same proof**: an invariant discharged by examining every write site. Distribution errors become tractable precisely when they are restatements of local invariants. That compression is the whole thesis.

## The Blob north star

The acceptance test for this direction: a lockless, single-writer, fenced-atomic blob store should be an ordinary stdlib module — written in Pluto, its correctness theorem checked by the compiler, imported like anything else.

Most of it already runs today — `examples/blob` compiles and executes with shipped features only. The authority is an entity owning the data and a fencing epoch; minting a grant advances the epoch, silently invalidating all older grants; a write presents its grant and the authority judges it *at the point of effect*:

```
error StaleGrant {
    token: int
    epoch: int
}

// Evidence, not authority: a plain value carrying the fencing token.
// Deliberately inert — no methods, no deadline, no "Valid" typestate,
// because validity is a remote belief the world can falsify between
// any two instructions.
class WriteGrant {
    token: int
}

object BlobAuthority {
    data: bytes
    epoch: int
    applied: int   // token of the last write the fence admitted

    // Compile-time proof obligations — proven at every write site, no
    // runtime checks. The second survives grant_write because
    // applied <= epoch < epoch + 1, and survives apply because the
    // fence's fall-through fact is token == epoch.
    invariant self.epoch >= 0
    invariant self.applied <= self.epoch

    // Minting IS the invalidation: every earlier grant's token is now
    // stale. The old holder is not consulted — fencing is constructive
    // knowledge; the old write *cannot* land.
    fn grant_write(mut self) WriteGrant {
        self.epoch = self.epoch + 1
        return WriteGrant { token: self.epoch }
    }

    // The guarded effect. Fence and write sit in one serialized method
    // body, so check-then-act is atomic by construct, judged by the
    // authority — never by the possibly-paused writer.
    fn apply(mut self, grant: WriteGrant, d: string) {
        let tok = grant.token
        if tok != self.epoch {
            raise StaleGrant { token: tok, epoch: self.epoch }
        }
        self.applied = tok
        self.data = d.to_bytes()
    }
}
```

A loser of the race gets a *definite*, typed, inspectable outcome — `at blob { apply(grant_a, "late write") } catch err: StaleGrant { ... }` — not a silent no-op and not corruption. Each guarantee has exactly one owner:

| Guarantee | Owner | Status |
|-----------|-------|--------|
| Atomicity | entity method serialization | shipped (the object construct) |
| Safety (single-writer) | the fenced compare in `apply` | holds by inspection; *proving* it is the roadmap |
| Liveness | holding the newest grant; lease windows | runtime courtesy, never load-bearing |
| Discipline | typestates + mandatory typed-error handling | shipped |

Note where safety lives: not in the client checking "is my grant still valid?" — a process can pause between the check and the write landing, and no client-side check closes that gap. Safety is judged by the authority, at the point of effect. The provable theorem is not "no write is *attempted* without a valid grant" (physics forbids proving that) but the one that matters: **no write is *applied* without a currently-valid grant.** Fencing is constructive knowledge — level 3 on the gradient. You don't learn whether the stale write was attempted; you act so that it cannot land, which is why fencing is robust to message loss while "asking again" never is.

What the verification engine adds is the missing column: today the invariants `self.epoch >= 0` and `self.applied <= self.epoch` are proven at compile time, but *monotonicity* ("every write to epoch increases it" — a two-state claim) and *dominance* ("no write to data escapes the fence" — a control-flow claim about write sites) hold by inspection, not by proof. Those are the two proof shapes on the roadmap:

- **Monotonic field** — every write site provably increases the field (epochs, versions, high-water marks).
- **Guarded effect** — every effect site provably dominated by a validity check (fencing, idempotency keys).

With those two, the authority's comment "exported theorem: no write is applied without a valid grant" becomes a checked artifact that downstream code can *assume* without reading the internals. A sketch of the destination (illustrative syntax — none of this parses today):

```
// ILLUSTRATIVE — not shipped
object BlobAuthority {
    data: bytes
    epoch: int

    invariant monotonic(self.epoch)      // proven: every write increases it

    fn apply(mut self, g: WriteGrant, d: bytes) {
        if g.token != self.epoch { raise StaleGrant }
        self.data = d                    // proven: dominated by the fence
    }
}
```

A "lockless distributed file with provably single-writer writes" is then an import, not a language feature.

## Properties are someone's responsibility

The kernel deliberately stops at domain-neutral proof shapes: write-path preservation, dominance, monotone writes, linear consumption. The language will hardcode no specific distributed property — no built-in `idempotent`, no blessed `transactional`. Baking in instances of a pattern instead of the kernel that expresses them is the mistake Pluto has already rejected twice (wire-traits, typestated objects).

Instead, named properties are planned as *library* definitions whose meaning bottoms out in kernel shapes — and every claim of a property has a named owner and exactly one warrant:

- **Proven** — the kernel discharged it from the body (the dedup check provably dominates the effect).
- **Checked** — a runtime residual enforces it, violations raising typed errors.
- **Assumed** — declared explicitly at a trust boundary: an external service the compiler cannot see carries `assume idempotent(key)`, visible and attributed.

So "retry from ambiguity" is not a language rule about idempotency. A retry combinator *requires* `idempotent(key)` of its callee; the authority's author *provides* it — by proof if the code is in the compilation unit, by assumption if extern; the compiler matches requirement to provision and never loses track of who vouched. And the assumption surface becomes an artifact: `pluto analyze` reporting every claim discharged by assumption rather than proof — the complete list of things your deployment rests on that nobody proved. A deployment's honesty, as a report.

Failure classification gets the same treatment. Every failure of an effectful boundary call is one of exactly two kinds — **definite** (the effect is known not to have applied: connection refused before send, the authority's fence rejected it) or **ambiguous** (the request left; nothing came back; no local information can say). Ambiguity has exactly four legitimate exits: idempotent retry, read-back, self-fencing, or honest escalation. The runtime owns the classification — only it knows whether bytes left the socket; everything built on it is library policy with property requirements.

## Why this and not a framework

Every piece of this exists somewhere: epistemic logic has modeled distributed protocols since the 1980s; session types track protocol position; CRDT languages own the monotone corner; TLA+ can model all of it — outside the program, unchecked against the code you actually ship. The gap Pluto occupies is the combination: a type system that audits *how you know*, checked by a whole-program compiler that sees both sides of every boundary.

No modal logic will surface in user syntax — no knowledge operators, no belief algebra. The constructs you have already seen *are* the surface syntax of warrants: typestates for what you did, invariants for what every path preserves, entities for referents, values for beliefs, typed errors for ignorance, `at` for questions asked across a boundary. The epistemics is the design lens that keeps those constructs honest, and the verification engine is the machinery that turns the lens into rejected programs.

The language will not ship distributed-systems constructs. It will ship a proof kernel — and the difficult machinery (leases, fencing, epochs, single-writer stores, idempotent handlers) becomes libraries whose correctness theorems the compiler checks, the same way it checks yours.

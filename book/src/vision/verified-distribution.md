# Verified Distribution

> **This chapter is direction, not documentation.** The foundations described here — entities, typestates, compile-time invariant proofs (single- and two-state), `ensures` postconditions, `guarded_by` dominance, error-set shrinking, definite/ambiguous failure classification — are shipped and covered in Part 2. The destination they point at — a proof kernel that makes distributed-systems correctness a library concern — is not. Code blocks marked *illustrative* do not parse today. Everything else is shipped syntax, verified against the current compiler — including the one snippet the compiler rejects, which is the point of that snippet — and every quoted diagnostic is real output.

## A brass token, 1850

On a single-track railway there is one way to die that matters more than all the others: two trains entering the same section from opposite ends. The 1850s answer was not a procedure manual and not a faster telegraph. It was a physical object — the **token**, a brass staff, one per section of track. A driver may not enter the section without holding it. To give it to the next train you must give it up. It cannot be duplicated, it cannot be quietly dropped in a ditch, and when a train terminates, the token goes back to the frame. Token-block signaling is where the phrase *fencing token* comes from: a linear, must-release capability, made of brass, a hundred and seventy years before anyone wrote down linear types.

Railway signaling invented the linear capability. Aviation invented the revocable warrant, the fence at the point of effect, and the ownership handoff. Finance invented the idempotency key and the serialized authority. The power grid invented the must-release lock with conjunctive holders. Medicine invented the identity check at the point of effect and evidence with an expiry. Five industries, each paying for the lesson in lives or money, each unaware of the others' vocabulary — and they converged on the same small set of constructs. **The constructs in this book are not novel. They have just never been compiler-checked.** That is this chapter's claim, and the rest of it is the evidence.

## The bug every distributed system has

An ordinary language has an *ontic* semantics: program state **is** the world. A read gives you the truth; memory is authoritative; "what I know" and "what is" coincide. Every mainstream language is built on that identity, which is why none of them needs a concept of knowledge at all.

Distribution breaks the identity. The moment part of the world lives outside the process, every value you hold about it stops being the world and becomes a **belief about** the world — a snapshot, acquired at some moment, on some warrant, valid under some conditions. Regular languages cannot express the difference between "x is the balance" and "x is what the balance was when I asked." So programmers do the epistemic bookkeeping in their heads, and every classic distributed bug is that bookkeeping failing:

- using a lease after the coordinator revoked it,
- retrying a write whose outcome was unknown,
- reporting an ambiguous outcome as a definite failure,
- trusting a cached read as current truth.

These are all the same error: **acting on a warrant weaker than the action requires.** Ordinary languages cannot catch this error because they cannot state it.

Pluto's bet is that a whole-program compiler can. A program is a knower; its state is a body of claims with justifications; the compiler audits the justifications. This is the Halpern–Moses tradition — distributed protocols analyzed as transformations of knowledge states, two-generals as the impossibility of common knowledge over lossy channels — put *inside* a language, where it has never lived. Moses' Knowledge of Preconditions principle says that in any correct protocol, a process taking an action that requires some fact must *know* that fact when acting. Correct distributed programs already satisfy this implicitly, in their authors' heads — and, as the rest of this chapter shows, correct *industries* already satisfy it explicitly, in their regulations. The one-sentence version: **the compiler should enforce it** — refusing to compile programs whose actions outrun their knowledge.

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

**Monotone facts are the exception that proves the ladder.** A claim that once true can never be false — "the epoch is at least 42," "the set contains x" — can be cached and shared at level-5 warrant *forever*, because nothing can invalidate it. This is the CALM insight (monotone means coordination-free), and it is why monotonicity was the first two-state proof shape to ship: `invariant self.epoch >= old(self.epoch)` is a compile-time theorem today, not a roadmap item.

No domain makes the gradient more honest than spaceflight, because there the staleness is enforced by physics. A Mars rover driver's belief about the vehicle is fourteen light-minutes old *at best* — there is no "just check again." So the discipline is total: commands are armed through a typestate chain (build, validate, arm, execute — each step gated on the one before, exactly a `Command<Armed>` that cannot exist without a `Command<Validated>`); a lost command acknowledgment is ambiguity with no fast resolution, and operations treats it as its own state, never rounded to "it failed"; and onboard fault protection is the perfect fence — the rover rejects unsafe commands *at the point of effect*, regardless of what ground believes, because ground's beliefs are known to be testimony. Nobody argues with light-speed. The rest of us have the same problem with the speed of a datacenter hop; we just lie to ourselves about it.

## What the shipped language already encodes

The epistemics is not a future layer bolted on top; it names what the shipped language already does. **The value/entity split is the belief/fact split**: a class value is a snapshot — a belief, honest about its staleness, structurally comparable, deep-copied across concurrency because beliefs are cheap to duplicate — while an object is the referent itself, never held, only asked, identity-compared because referents are not their descriptions. **Invariant discharge is closed-world knowledge**: "every write path preserves `balance >= 0`" is only knowable because the compiler sees every write path, and only *sound* under concurrency because values don't alias and entities serialize their methods. **Error inference is ignorance propagation**: the error set of a function is the set of ways it might leave you not knowing what you hoped; mandatory handling is the rule that ignorance must be confronted, not discarded, and error-set shrinking is the converse — a proof that removes a possibility removes the obligation to handle it.

Two problems that look unrelated — "this field's write paths all guard against negatives, so `balance >= 0` is a fact" and "this store's fencing is correct: the epoch only increases and no write lands without a validity check" — are **the same proof**: an invariant discharged by examining every write site. Distribution errors become tractable precisely when they are restatements of local invariants. That compression is the thesis, stated from the compiler's side. The industries state it from the other side.

## Air traffic control: the discipline, compiled

Aviation is the best-documented case of an industry independently inventing warrant discipline — procedurally, in phraseology and regulation, at the cost of decades of accidents. Every construct in Part 2 of this book has an exact counterpart in the controller's vocabulary, and the mappings are not analogies. They are the same constructs:

- **A clearance is evidence.** "Cleared to descend flight level 240" is a granted, expiring, revocable warrant — not a fact about the sky. An amendment or cancellation is degradation: the world moved you out of a state you did not choose to leave.
- **Readback/hearback is the fence.** The pilot reads the clearance back; the controller validates it against the current picture *at the point of effect*. A stale clearance — superseded by an amendment the pilot hasn't heard — is rejected there, by the authority, never by the holder's own opinion of its validity.
- **Handoff is linear ownership transfer.** The rule is verbatim must-release linearity: *every aircraft is under positive control of exactly one controller at all times.* A track is never dropped. It is handed off or terminated — the only two exits.
- **The aircraft track is an entity.** It is THE plane, not a snapshot of it; sectors are domains; the flight strip is the handle one controller holds.
- **Separation is the invariant** — a property quantified across *all* tracks, which is exactly what marks it as the far horizon (more below).

Here is the track and its fence, in shipped syntax — the sketch compiles and runs today:

```
error StaleClearance {
    seq: int
    current: int
}

// Evidence, not authority: the clearance as issued, carried by the pilot.
class Clearance {
    seq: int
    altitude: int
}

object AircraftTrack {
    callsign: string
    altitude: int
    seq: int        // newest clearance sequence — an amendment bumps it

    invariant self.seq >= 0

    // Amending IS the invalidation: every earlier clearance is now stale.
    fn amend(mut self, altitude: int) Clearance {
        self.seq = self.seq + 1
        return Clearance { seq: self.seq, altitude: altitude }
    }

    // The readback/hearback fence: validity judged at the point of
    // effect, by the authority — never by the possibly-out-of-date holder.
    fn execute(mut self, c: Clearance) {
        if c.seq != self.seq {
            raise StaleClearance { seq: c.seq, current: self.seq }
        }
        self.altitude = c.altitude
    }
}
```

And the handoff, as a must-release typestate (also shipped):

```
class Offered { tag: int }
class UnderControl { tag: int }
class Closed { tag: int }

// The flight strip: one per aircraft, held by exactly one controller.
class Strip<S> {
    callsign: string

    must_release UnderControl

    fn accept(self) Strip<UnderControl> where S == Offered {
        return Strip<UnderControl> { callsign: self.callsign }
    }

    fn handoff(self) Strip<Offered> where S == UnderControl {
        return Strip<Offered> { callsign: self.callsign }
    }

    fn terminate(self) Strip<Closed> where S == UnderControl {
        return Strip<Closed> { callsign: self.callsign }
    }
}
```

Accept a strip and end your shift without discharging it — `let mine = s.accept()` and then just a `return` — and the compiler answers in the controller's own terms (real output): `'mine' still holds Strip<UnderControl>, a must_release state, when it goes out of scope; transition it out of 'UnderControl' (e.g. .handoff()), move it onward, or return it`. Positive control of exactly one controller at all times — as a type error.

**Lost communication is the Definite/Ambiguous split.** A pilot who stops hearing the controller does not know whether the last transmission arrived; the controller does not know what the pilot will do. No local information can resolve it — and aviation's lost-comm procedures are the legitimate exits from ambiguity, written as regulation: fly the last **acknowledged** clearance (act only on fenced evidence, never on testimony that was still in flight); squawk 7600 (escalate honestly — broadcast your ignorance instead of laundering it into a guess); then follow the published procedure (resolution by protocol, agreed before the failure, because agreement *during* the failure is exactly what you no longer have).

### The composition test: a handoff during an amendment with a comm failure

North stars are cheap when each feature gets its own slide. The real test is one scenario that needs all of them at once — and ATC runs it daily.

Sector A is handing UAL232 to sector B. Mid-handoff, A amends the descent clearance. The amendment's acknowledgment never arrives.

Walk the constructs through it. The *track* is an entity: identity survives the handoff — B receives THE aircraft, not a copy whose state forked at the boundary. The *clearance* is a typestated value — give the `Clearance` sketch the `Strip` treatment and proposed, issued, acknowledged become different types, with "acknowledged" the only one anyone may act on. The *handoff* is a linear transfer: A's strip binding is consumed, B's is created, and no interleaving of the amendment can produce zero controllers or two. The *amendment* is degradation: the old clearance's seq is dead, and the pilot's copy of it is now provably stale evidence. The *readback fence* catches exactly that: if the stale clearance is executed against the track, the authority rejects it at the point of effect. The *comm failure* is ambiguity: the amendment may or may not have been heard, and the system's answer is not a guess — it is the lost-comm protocol, keyed on what was *acknowledged*, which the typestate recorded. And *separation* is the invariant that must hold across every interleaving of all of the above.

Now remove any one construct and watch the scenario break — not degrade, break:

- No entity identity → A and B hold diverging snapshots of "the same" aircraft; the amendment lands on one of them.
- No typestates → nothing distinguishes an issued clearance from an acknowledged one, and the lost-comm procedure has no fact to key on.
- No linearity → a strip can be dropped mid-handoff; some aircraft is under positive control of nobody.
- No fence → the pilot's stale clearance executes, because the holder judged its own evidence.
- No degradation channel → the amendment is a silent overwrite; the old clearance fails as a mystery instead of carrying its post-failure state.
- No Definite/Ambiguous split → the unacknowledged amendment gets rounded to "it applied" or "it didn't," each wrong half the time.
- No invariant → all of the above succeed locally and the aircraft converge anyway.

The features do not add; they multiply. Each one is the reason another one's guarantee survives contact with the scenario. That is what "more than themselves" means, and aviation needed decades of accident reports to assemble the same set procedurally. This language is that discipline, compiled.

One construct in the list is beyond the shipped proof engine, and it is the one that marks the horizon. Separation is a *cross-entity* invariant — quantified over all tracks, not a field predicate on one:

```
// ILLUSTRATIVE — not shipped. Cross-entity invariants are the
// phase-4+ horizon, not the current fragment.
system Sector {
    invariant forall a, b in AircraftTrack where a != b:
        separated(a, b)
}
```

Today's fragment proves single-entity facts at every write site. The roadmap's proof shapes (below) are the steps from here to there.

## The ledger: money is neither created nor destroyed

The grand claim, stated plainly: a Pluto ledger should carry **conservation of money as a named, compiler-checked theorem** — across all accounts, the sum of balances equals issuance minus redemption, and no interleaving of transfers, retries, or crashes can violate it. Like separation, conservation is a cross-entity invariant: honestly phase-4+, not a feature you can use today.

Unlike the rest of this chapter's horizon, though, the ledger's foundations are not a sketch. They compile now, and the receipts are real.

**Receipt one: `balance >= 0` is a proven invariant, strictly.** Write the unguarded version —

```
class Account {
    balance: int

    invariant self.balance >= 0

    fn withdraw(mut self, amount: int) {
        self.balance = self.balance - amount
    }
}
```

— and the compiler rejects it at the write site, with the symbolic state it reached (real output, today):

```
error: cannot prove invariant 'self.balance >= 0' of class 'Account' holds
at the end of this method in method 'withdraw': at this point self.balance =
-amount + old(self.balance). The invariant may be broken temporarily between
writes, but must be re-established at every boundary (method exits, raises,
calls, loops, branch joins). Establish the missing bound before this point
with a guard, a 'requires' clause, or an 'assert'
(e.g. 'if amt <= self.balance { ... }')
```

No runtime check was declined here — there was never going to be one. An invariant the compiler cannot discharge is a compile error, full stop.

**Receipt two: proofs delete error handling.** Give `Account` a raising variant —

```
error Insufficient {
    needed: int
}

// added to Account:
fn try_withdraw(mut self, amount: int) int
    requires amount > 0
{
    if amount > self.balance {
        raise Insufficient { needed: amount }
    }
    self.balance = self.balance - amount
    return self.balance
}
```

— and guard the call, and the `Insufficient` error provably vanishes at that site. This compiles today, with no `!` and no `catch`:

```
fn main() {
    let mut account = Account { balance: 100 }
    let amount = 80
    if amount <= account.balance {
        let after = account.try_withdraw(amount)   // no handler — proven safe
        print(after)
    }
}
```

Remove the guard and it is the usual compile error. The guard is not defensive programming; it is the proof, and the compiler read it. (`examples/contracts` in the repository is the runnable version.)

**Receipt three: the movement itself is a theorem.** With the proof-form `ensures` shipped, the withdrawal can state exactly what it did to the balance, and the compiler proves it at every normal exit:

```
fn try_withdraw(mut self, amount: int) int
    requires amount > 0
    ensures self.balance == old(self.balance) - amount
{
    if amount > self.balance {
        raise Insufficient { needed: amount }
    }
    self.balance = self.balance - amount
    return self.balance
}
```

This compiles today, and callers *assume* the relation: after a proven `try_withdraw(amount)`, the caller's facts carry `balance == old(balance) - amount`, feeding construction proofs and error-set shrinking downstream. A debit that debits exactly what it says is no longer a comment — it is the per-method half of conservation, checked. (Receipt three's sibling shipped too: `invariant self.epoch >= old(self.epoch)` makes monotone fields a one-line theorem — see the blob store below.)

The rest of the ledger is the shipped vocabulary applied straight: **accounts are entities** — one referent per account, methods serialized, so a transfer's debit-credit pair cannot interleave with another transfer mid-method; **transfers carry idempotency keys on the wire** and wear their protocol as a typestate:

```
class Pending { tag: int }
class Committed { tag: int }

class Transfer<S> {
    key: string      // idempotency key — rides the wire with the transfer
    amount: int

    fn commit(self) Transfer<Committed> where S == Pending {
        return Transfer<Committed> { key: self.key, amount: self.amount }
    }
}
```

A commit whose acknowledgment is lost is an *ambiguous* failure, and the key is what makes "retry" one of the legitimate exits: the retry's outcome is knowledge. Every trader, every payments engineer already runs this protocol by hand. The theorem on the horizon is the compiler running it instead:

```
// ILLUSTRATIVE — not shipped. Conservation as a checked artifact.
system Ledger {
    invariant sum(Account.balance) == self.issued - self.redeemed
}
```

"The compiler proves money is neither created nor destroyed" is a sentence worth being honest about, so: the per-account halves of it are proven today (receipt one); the cross-entity sum is the same kind of claim as separation, and it arrives the same way.

## The first fully-built protocol

The acceptance test for this direction has been built: `examples/blob` is a lockless, single-writer, fenced-atomic blob store — written in Pluto, running today on shipped features only. An authority entity owns the data and a fencing epoch; minting a grant advances the epoch, silently invalidating all older grants; a write presents its grant and the authority judges it at the point of effect. If the shape sounds familiar, it should: `BlobAuthority.apply` and `AircraftTrack.execute` are the *same method* — the fence is one construct, whether what it guards is bytes or altitudes. A loser of the race gets a definite, typed, inspectable `StaleGrant` — not a silent no-op and not corruption. Each guarantee has exactly one owner:

| Guarantee | Owner | Status |
|-----------|-------|--------|
| Atomicity | entity method serialization | shipped (the object construct) |
| Safety (single-writer) | the fenced compare in `apply` | **shipped — compiler-checked**: `guarded_by` dominance + the monotone epoch invariant |
| Liveness | holding the newest grant; lease windows | runtime courtesy, never load-bearing |
| Discipline | typestates + mandatory typed-error handling | shipped |

Note where safety lives: not in the client checking "is my grant still valid?" — a process can pause between the check and the write landing, and no client-side check closes that gap. The provable theorem is not "no write is *attempted* without a valid grant" (physics forbids proving that) but the one that matters: **no write is *applied* without a currently-valid grant.** Fencing is constructive knowledge — level 3 on the gradient. You don't learn whether the stale write was attempted; you act so that it cannot land, which is why fencing is robust to message loss while "asking again" never is.

When this chapter was first written, that column had a hole: the store's single-state invariants (`epoch >= 0`, `applied <= epoch`) were proven at compile time, but *monotonicity* ("every write to epoch increases it" — a two-state claim) and *dominance* ("no write to data escapes the fence" — a control-flow claim about write sites) held by inspection, not by proof. Those were the two proof shapes on the roadmap. **Both have shipped**, and `examples/blob` now carries them as clauses — this is shipped syntax, not an illustration:

```
object BlobAuthority {
    data: bytes guarded_by (g: WriteGrant) g.token == self.epoch
    epoch: int

    invariant self.epoch >= old(self.epoch)   // proven: no write decreases it

    fn apply(mut self, grant: WriteGrant, d: string) {
        if grant.token != self.epoch {
            raise StaleGrant { token: grant.token, epoch: self.epoch }
        }
        self.data = d.to_bytes()              // proven: dominated by the fence
    }
}
```

The two-state invariant is monotonicity with no keyword: every method exit and every write site must prove the epoch did not decrease, so a minted token can never become current again. The `guarded_by` clause makes the fence structural: every write to `data`, across the whole program's closed write-set, must be dominated by a conditional the prover can show implies `g.token == self.epoch` — remove the fence, move the write above it, or slip a call between them, and the store stops compiling. (Both shapes are documented in [Contracts](../whats-different/contracts.md); the full runnable protocol is `examples/blob`.)

So the safety theorem — **no write is applied without a currently-valid grant** — is now a checked artifact, not an inspection note. What remains is the naming layer: a *property form* that binds theorems like these to names (`monotonic`, `fenced`, `idempotent`) a library can export and downstream code can require without reading the internals — making "a lockless distributed file with provably single-writer writes" an import, not a language feature. The shipped shapes are also the on-ramp to the cross-entity horizon: separation and conservation decompose into exactly these per-entity pieces plus a quantified sum.

## Properties are someone's responsibility

The kernel deliberately stops at domain-neutral proof shapes: write-path preservation, dominance, monotone writes, linear consumption. The language will hardcode no specific distributed property — no built-in `idempotent`, no blessed `transactional`. Baking in instances of a pattern instead of the kernel that expresses them is the mistake Pluto has already rejected twice (wire-traits, typestated objects).

Instead, named properties are planned as *library* definitions whose meaning bottoms out in kernel shapes — and every claim of a property has a named owner and exactly one warrant:

- **Proven** — the kernel discharged it from the body (the dedup check provably dominates the effect).
- **Checked** — a runtime residual enforces it, violations raising typed errors.
- **Assumed** — declared explicitly at a trust boundary, visible and attributed. The ATC system's radar feed is extern: "this track position is current" is an `assume`, not a proof, and it says so. A clearance relayed through an outside system carries the same honesty marker. S3's write semantics carry `assume idempotent(key)`.

So "retry from ambiguity" is not a language rule about idempotency. A retry combinator *requires* `idempotent(key)` of its callee; the ledger's transfer authority *provides* it — by proof if the dedup table is in the compilation unit, by assumption if the processor is extern; the compiler matches requirement to provision and never loses track of who vouched. And the assumption surface becomes an artifact: `pluto analyze` reporting every claim discharged by assumption rather than proof — the complete list of things your deployment rests on that nobody proved. A deployment's honesty, as a report.

Failure classification gets the same treatment — and this piece has shipped. Every failure of an effectful boundary call is one of exactly two kinds — **definite** (the effect is known not to have applied: connection refused before send, the authority's fence rejected it) or **ambiguous** (the request left; nothing came back; no local information can say). Ambiguity has exactly four legitimate exits: idempotent retry, read-back, self-fencing, or honest escalation — the same four aviation wrote into its lost-comm regulations. The runtime owns the classification — only it knows whether bytes left the socket — and today it delivers it on every boundary failure as [`NetworkError.definite`](../whats-different/errors.md#errors-at-a-distance-networkerrordefinite), set truthfully: definite only with a warrant, ambiguous whenever in doubt. What stays library policy with property requirements is everything built on top — the retry combinator that demands idempotency before consuming an ambiguous failure, the read-back, the escalation.

## Why this and not a framework

Every piece of this exists somewhere: epistemic logic has modeled distributed protocols since the 1980s; session types track protocol position; CRDT languages own the monotone corner; TLA+ can model all of it — outside the program, unchecked against the code you actually ship. The gap Pluto occupies is the combination: a type system that audits *how you know*, checked by a whole-program compiler that sees both sides of every boundary.

No modal logic will surface in user syntax — no knowledge operators, no belief algebra. The constructs you have already seen *are* the surface syntax of warrants: typestates for what you did, invariants for what every path preserves, entities for referents, values for beliefs, typed errors for ignorance, `at` for questions asked across a boundary. The epistemics is the design lens that keeps those constructs honest, and the verification engine is the machinery that turns the lens into rejected programs.

The concept chapters trace the same pattern library through the rest of reality: the exchange whose single-threaded matching engine is entity serialization at world scale ([Objects](../whats-different/objects.md), [Concurrency](../whats-different/concurrency.md)); the web's famously broken certificate revocation, which is a belief outliving its warrant ([Contracts](../whats-different/contracts.md)); the electrician's lockout-tagout padlock, which is a must-release token you hold in your hand ([Typestates](../whats-different/typestates.md)); the transfusion ward's wristband check, which is the fence at the point of effect ([Objects](../whats-different/objects.md)). Even organ allocation runs the protocol: an organ is offered to exactly one center at a time, under a hard deadline, with the cold-ischemia clock as the revoking authority — a linear, expiring warrant, administered by fax and phone call.

None of these industries waited for language support. They built warrants, fences, and linear evidence out of brass, phraseology, padlocks, and wristbands, because the alternative was collisions, mid-airs, double-spends, electrocutions, and wrong-blood transfusions. The language will not ship distributed-systems constructs, and it will not ship aviation constructs either. It will ship a proof kernel — and the difficult machinery (leases, fencing, epochs, handoffs, single-writer stores, idempotent handlers) becomes libraries whose correctness theorems the compiler checks, the same way it checks yours.

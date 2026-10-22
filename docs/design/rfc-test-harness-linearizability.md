# RFC: Exhaustive Linearizability Checking (test-harness v2, D4)

**Status:** Draft — awaiting owner review
**Author:** Design discussion
**Date:** 2026-10-07
**Related:** [rfc-test-harness.md](rfc-test-harness.md) (the DPOR harness v1 this builds on), [rfc-objects.md](rfc-objects.md) (entities, per-instance serialization), [rfc-verification.md](rfc-verification.md) (the proof engine; this is its runtime counterpart), [epistemics.md](epistemics.md) (warrants), issue #369 bench context
**Prerequisite:** the entity-boundary deep-copy fix (rfc-test-harness open question 5 / the #440 interior-alias residual) — see Dependencies.

## Summary

The deterministic DPOR harness (v1, phases 0–4) already *enumerates* every meaningfully distinct interleaving of a concurrent test and replays any one of them from a `ptsched:v1:` token. It verifies whatever `expect` assertions the test author writes. It does **not** know whether the concurrent object the test exercises behaved *correctly* — only whether the author's hand-written assertions held.

This RFC adds the missing oracle: **linearizability checking**. For a concurrent test over an `object` (entity), the harness records the *operation history* — who called which method, with what arguments, what it returned, and when relative to other operations — and checks that history against the entity's own methods run serially. The entity **is** the sequential specification; no separate model is authored. Combined with the exhaustive explorer, this is **exhaustive linearizability checking**: every interleaving produces a history, every history is checked, and a non-linearizable one is reported with its replayable schedule token.

"Races" (the D4 shorthand) resolve to exactly this: a data race on an entity manifests as a history no serial order can explain. The verdict is "this schedule produced a non-linearizable history," delivered through the existing repro machinery.

## Motivation

Pluto's concurrency pitch rests on the value/entity split: values copy across every boundary, entities serialize through a per-instance lock, so "sharing is safe." Phase 4 of the harness made entity locks real preemption points and the explorer now *finds* the get-then-set lost update it once falsely certified — but only because a test author wrote an assertion that happened to catch it. For the general case, the author would have to hand-write the correctness condition for every concurrent object, which is both laborious and exactly the thing most people get wrong.

Linearizability is the correctness condition for a shared object, and it is checkable mechanically from an observed history against a sequential model. Pluto is in a nearly unique position to do this *exhaustively* rather than by random testing (Jepsen's domain): the DPOR explorer already walks all interleavings, and the entity already *is* the sequential model. The static verification engine proves properties over one method body; this is its runtime dual — proving, over all interleavings the microscope can reach, that the concurrent object is indistinguishable from a serial one. Styx's D4 ("more verification: races + a linearizability history checker") is the motivating consumer; the capability is general.

## What exists (grounding)

From the harness internals (verified against current tree):

- **The scheduling trace is scheduling-only.** `trace_record(kind, choice)` (threading.c) appends LEB128 varint pairs; the only kinds are `DEC_FIBER_PICK` and `DEC_SELECT_ARM`. The `ptsched:v1:` token is base64 of that byte stream. **No operation-level data is recorded** — not entity, method, args, or return.
- **The entity method-call boundary is observable** at three codegen sites (`lower_method_call`, the serve/dispatch path, the domain/colocated path). At each, the entity pointer is in hand at lock-acquire and the **return value is materialized before unlock**. Lock acquire/release are a recorded preemption point (via `DEC_FIBER_PICK`) and feed the DPOR dependency matrix (`exhaustive_record_channel` keyed by the lock pointer).
- **The C lock stubs see only `void *entity`** — not method identity, args, or return. So recording "F invoked M(args)→R on E" at the C boundary would mean growing the stub signatures.
- **`std.verify` is compile-time only.** Its `property`/`satisfies` atoms (invariant, guarded_by, ensures, dedup) are static proof obligations discharged over one body. Linearizability is a *runtime* property of a *history*; it does not fit that surface and needs a distinct one.
- **Failure reporting funnels through `__pluto_test_print_repro`** (strategy/seed/iteration + `trace_print_token` + rerun line) for `expect`/deadlock/replay-divergence. **Exhaustive is the exception**: it emits one `ptsched` token per failing schedule inline and accumulates a failure list.
- **No notion of history, model, or sequential spec exists** anywhere. This is built from scratch.

## Design

### The entity is the specification

A linearizability checker needs a sequential model to check against. For a Pluto entity, that model already exists and is executable: **the entity's own methods, run serially on a single instance.** There is nothing to author. A history over `object Register { val: int; fn read(self) int; fn write(mut self, v: int) }` is linearizable iff there is a total order of the observed operations, consistent with real time and per-fiber program order, that — replayed on a fresh `Register` — reproduces every observed return value.

This is the crucial simplification over Jepsen/Knossos, where the model is hand-written and separate. Here the model is the code under test, so the checker cannot drift from the implementation's own sequential semantics.

### Operation history: in-language thunks + a C real-time skeleton

The friction the grounding flags is that args and return values are arbitrarily typed and live only at the codegen boundary, not in the C trace. Rather than serialize them through C, split the recording:

1. **C side — the real-time skeleton.** At the three entity-call sites, codegen emits an `__pluto_op_invoke(op_id, entity, fiber)` at acquire and `__pluto_op_return(op_id)` at release (before unlock, where the return is already live). The runtime stamps each with the virtual clock (invoke-tick, return-tick) and records `(op_id, entity, fiber, invoke_tick, return_tick)`. This is cheap, fixed-width, and gives the real-time partial order: operation *a* precedes *b* iff `a.return_tick < b.invoke_tick`.

2. **Pluto side — the operation payload.** The same op is also recorded as a **re-invocable thunk plus its observed return**: a closure `(e: Entity) => e.method(captured_args)` and the value the live run returned. Both are ordinary typed Pluto values; the return is comparable by the language's structural `==` (`__pluto_deep_eq`). Correlated to the C skeleton by `op_id`.

The checker thus has: a real-time partial order (from C), and for each op a way to re-execute it on a candidate instance and a value to compare against (from Pluto). Nothing arbitrary crosses the C boundary.

### The checker

Standard linearizability search (Wing & Gong), run per completed schedule:

- Candidate linearizations respect two constraints: **real-time** (`a.return_tick < b.invoke_tick` ⟹ *a* before *b*) and **per-fiber program order**.
- Search incrementally: maintain a fresh entity instance; at each step pick a *minimal* pending operation (one with no un-chosen real-time predecessor), re-invoke its thunk on the instance, and require the result to `==` the observed return; backtrack on mismatch. Memoize on (linearized-set, entity-state) to prune — the classic optimization that makes small histories tractable.
- A history is linearizable iff some complete linearization succeeds. If none does, the schedule is a **counterexample**.

Complexity is exponential in concurrent-operation count — acceptable because it runs inside the *exhaustive microscope* (2–4 fibers, short histories), the same scaling regime the harness already commits to. The RFC does **not** propose running this over whole-system DST; that tier uses contracts as the oracle (future work in the harness RFC).

### Exhaustive + checker = exhaustive linearizability

Under `STRATEGY_EXHAUSTIVE`, every interleaving is already walked. With checking enabled, each completed schedule's recorded history is run through the checker. "All interleavings explored and every one linearizes" is a genuinely strong statement — within the fiber-count bound, a *proof* of linearizability by exhaustion, not a sampling. A failing schedule is reported alongside the existing per-schedule `ptsched` token, so it replays immediately.

### Surface

Opt-in, because the search is exponential and most tests don't need it. Reusing phase 3's bracket-config mechanism:

```pluto
tests[scheduler: Exhaustive, check: linearizable] {
    test "concurrent register" {
        let r = Register { val: 0 }
        let a = spawn writer(r, 1)
        let b = spawn writer(r, 2)
        let x = reader(r)
        a.get()!  b.get()!
    }
}
```

`check: linearizable` turns on history recording and the per-schedule checker for every entity touched in the block. Off by default; no cost when absent (the `__pluto_op_*` calls are emitted only under the check flag, so non-checked builds are unchanged). Whether this is the final surface — vs a per-entity `object Register satisfies linearizable` declaration, or a `std.verify`-adjacent form — is an open question below.

### The race verdict

"Race" is defined precisely as **a non-linearizable history**: no serial order explains the observed returns, which is the observable signature of unsynchronized concurrent access to the object. This subsumes the lost-update class phase 4 catches by assertion, now caught by oracle without a hand-written condition. Two narrower diagnostics ride along when cheaply available: (a) a **real-time violation** (an operation returns a value only explicable by a future operation) is reported as the specific linearization-point conflict; (b) an **interior-alias escape** (a returned mutable reference observed mutating after its return event — the P5 hazard) is flagged distinctly, since it is a language-soundness bug, not merely a non-linearizable schedule.

## Dependencies

**This RFC depends on the entity-boundary deep-copy fix** (rfc-test-harness open question 5; the #440 interior-alias residual). Response-value matching is only sound if the recorded return is a stable snapshot. Today an entity method returning a mutable field returns a live alias, so the recorded "return" could change after the return event, and the checker would compare against a value that no longer reflects what the caller observed. Sequencing options:

1. **Land boundary-copy first** (the uniform rule: values deep-copy across every entity method boundary, entities cross as handles). Then this RFC's response capture is sound by construction. Recommended — it also closes a standing production data race, independent of testing.
2. **Snapshot-at-return in the harness only** (deep-copy the recorded return inside `__pluto_op_return`, test-mode only). Unblocks the checker without the language change, but leaves the production race open and means the harness tests a semantics the compiler doesn't yet enforce. Not recommended as the end state; acceptable as a bring-up shim.

No other new dependencies: linearity, the DPOR explorer, the virtual clock, the repro/token machinery, and structural `==` are all shipped.

## What this RFC refuses

- **No hand-written models.** The entity is the spec. If you want to check against a different model, write that model as an entity.
- **No whole-system exhaustive linearizability.** Combinatorially impossible; that tier uses contracts/convergence as the oracle (harness RFC future work). This is the microscope's capability.
- **No weak-memory or relaxed histories.** The fiber scheduler is sequentially consistent by construction (one interleaving = one total order of steps); there is no relaxed-memory history to check, consistent with the "no atomics, SC-only" doctrine.
- **No runtime linearizability monitor in production.** This is a *test-harness* oracle over scheduled runs, not a production check. (The production guarantee is the entity lock plus the static engine.)

## Phasing

1. **History recording** — the `__pluto_op_invoke`/`__pluto_op_return` C skeleton at the three codegen sites (under the check flag), the virtual-clock stamps, and the in-language thunk+return payload. Lands the `ptsched`-correlated operation log with no checker yet (observable via a debug dump).
2. **The checker** — Wing-Gong search with memoization over the recorded history; the `check: linearizable` bracket; the non-linearizable verdict wired into `__pluto_test_print_repro` and the Exhaustive per-schedule path. Depends on the boundary-copy dependency being resolved (land it, or the test-mode snapshot shim).
3. **Diagnostics** — render a failing history readably (the operation sequence, the real-time overlaps, and the point where no linearization survives), and the distinct interior-alias-escape flag.
4. **The dogfood** — Styx's register/log/offset operations checked under Exhaustive; a committed regression repro for any non-linearizable schedule found. This is also where the surface decision (below) gets its real-world test.

Phase 1 is independently useful (an operation log is a debugging aid on its own). Phase 2 is the capability.

## Owner decisions

1. **Dependency sequencing** — land the entity-boundary deep-copy fix first (recommended; closes a real production race too), or start with the test-mode snapshot shim and let boundary-copy land on its own track?
2. **Surface** — `tests[check: linearizable]` block bracket (recommended; reuses phase 3, off by default, zero cost absent) vs a per-entity `object R satisfies linearizable` declaration vs a `std.verify`-adjacent property. The bracket is a *test* directive; the declaration would assert "this type is meant to be linearizable" as documentation the harness enforces — arguably more honest, but it bakes a testing concern into the type. Recommended: the bracket for v1, revisit a declaration if Styx wants the intent on the type.
3. **Checker algorithm** — Wing-Gong search + memoization (recommended; simplest, matches the small-history microscope regime) vs Elle-style dependency-cycle detection (scales better, but needs per-datatype dependency extraction and is more machinery). Recommended: Wing-Gong for v1; note Elle as the escape hatch if histories outgrow it.
4. **Scope of "race"** — is the non-linearizable-history verdict the whole of D4's "races," or do you also want a separate lighter check (e.g. flag *any* entity accessed by >1 fiber in a schedule where a lock elision/degenerate-entity lowering applied)? Recommended: non-linearizability is the definition; the interior-alias-escape flag is the one extra, because it is a soundness bug.

## Acceptance

A `Register` and a `Counter` entity, each with an intentionally-broken variant (a method that reads and writes without the lock — expressible once lock-elision/degenerate lowering exists, or simulated), checked under Exhaustive: the correct versions pass all schedules; the broken versions produce a non-linearizable history with a replayable token that, rerun, reproduces the exact counterexample. Then the Styx offset/commit path as the real multi-operation target.

# RFC: Deterministic scheduler harness

**Status:** Draft
**Builds on:** `docs/design/archive/rfc-deterministic-concurrency-testing.md` (implemented core), `docs/design/channels.md` (timeout-as-choice), `docs/design/rfc-objects.md` (entity serialization)

## Goal

Any failure observed once — under any strategy, at any scale — becomes a
committed regression test that replays the exact execution that failed. "Any
error ever" covers both halves of the failure space: *interleaving* errors
(races, deadlocks, atomicity violations) and *injected* errors (every member
of every operation's inferred error set, timeouts included).

## Problem

The hard parts already exist: a cooperative fiber scheduler where every
blocking operation is an explicit yield point, a seeded Random strategy, and
an exhaustive DFS explorer with DPOR pruning (`runtime/threading.c:265-347`,
`:515-599`). What is missing is the loop closure — a failure found once
cannot be pinned as a test — plus several soundness holes that make even the
existing exploration results untrustworthy:

**Repro gaps**
- An `expect` failure exits with no seed, iteration, or repro hint
  (`runtime/builtins.c:3491`); only deadlocks print a seed
  (`threading.c:611-616`), in hex that `--seed` (decimal `u64` in clap)
  cannot parse back.
- No `--test` filter, no `--strategy` flag, no in-source seed/iteration
  config; codegen hardcodes `seed=0, iterations=100`
  (`src/codegen/mod.rs:962-976`).
- An exhaustive-found failure has **no repro artifact at all** — no seed
  generates a DPOR schedule, so it cannot be re-run even by hand.

**Determinism leaks** (a fixed seed does not pin execution)
- Select-arm shuffle is seeded from `buffer_ptr ^ __pluto_time_ns()`
  (`threading.c:2019` test mode, `:2100` production).
- `std.random` lazily seeds from `clock_gettime` real entropy
  (`runtime/builtins.c:1168-1175`), unlinked to the scheduler seed.
- `now()`/`monotonic()` return real OS time in every mode
  (`builtins.c:1134-1145`).

**Soundness holes**
- Entity/singleton locks are no-ops in test mode (`threading.c:2383-2392`)
  and fibers only yield at *blocking* ops — so a race whose window does not
  straddle a channel/task/sleep op is invisible to every strategy. Two
  fibers each doing `counter.get()` then `counter.set(v+1)` never interleave
  in test mode; Exhaustive "proves" correct a program that loses updates
  under production pthreads. A false certificate, not just a blind spot.
- Interior aliasing: entity method boundaries copy nothing, so a method
  returning a mutable field (`fn get(self) [int] { return self.items }`)
  returns a live reference into the entity's state (verified empirically:
  mutation through the returned array is visible inside the entity). Two
  fibers sharing the entity can each obtain the alias and mutate it with no
  lock involved — an unsynchronized production data race that also breaks
  the rfc-objects guarantee ("sharing is safe because methods serialize").
  Values are deep-copied in exactly one place today: spawn capture
  (`src/codegen/lower/mod.rs:3493`).

**Robustness (measured, reproducible)**
- The GC fiber-stack registry (`runtime/gc/marksweep.c:116`) is append-only
  for the process lifetime (cap `GC_MAX_FIBER_STACKS` = 256, no reset, no
  unregister). `test_run_single` re-runs the body once per schedule,
  registering each run's freshly malloc'd stacks (`threading.c:491`) and
  freeing them at run end — so the registry fills with dangling pointers,
  silently stops registering after 256 cumulative fibers, and
  `__pluto_gc_mark_fiber_complete(fiber_id)` indexes a per-run id into a
  cumulative array (wrong entries from run 2 onward). Exhaustive on a
  3-producer × 2-message channel test segfaults in `__pluto_gc_collect`
  scanning a freed stack. Random at high `--iterations` has the same latent
  corruption and passes only when no collection triggers.
- The CLI swallows signal deaths: `status.code()` is `None` for a signaled
  child, so `pluto test` exits 1 with zero output (`src/main.rs:441`). A
  crashed explorer is indistinguishable from... nothing.

## Design

### One decision oracle, always recording

Every nondeterministic choice in test mode becomes a call to a single
scheduler function:

```c
long sched_decide(DecisionKind kind, long n_choices);
```

and the sequence of `(kind, choice)` pairs is appended to a growable
recording buffer on every run (a few bytes per decision — negligible).

Decision kinds:

| # | Kind        | Today                                            | Change |
|---|-------------|--------------------------------------------------|--------|
| 1 | Fiber pick  | `pick_next_fiber` (`threading.c:265`)            | route through oracle (mechanical) |
| 2 | Select arm  | wall-clock Fisher–Yates shuffle (`:2019`)        | oracle picks among *ready* arms |
| 3 | Timeout race| enabled-transition choice (issue #370, `:176-189`)| route through oracle so it lands in the trace |
| 4 | Fault inject| only `PLUTO_FS_SYNC_FAIL_AT` (`builtins.c:2730`) | generalize: "succeed, or raise error *k* of this op's error set" |
| 5 | Lock acquire| test-mode no-op (`:2383`)                        | fiber-aware lock + yield point (see Soundness) |

The timeout-as-choice model (durations erased, both race outcomes enabled)
is **unchanged** — it is settled design (`channels.md:69,236`). Only the
mechanics route through the oracle.

### Strategies

Existing strategies keep their meaning; all of them record.

- **Sequential / RoundRobin** — unchanged, deterministic.
- **Random(seed)** — decisions from the LCG; the seed alone is usually a
  sufficient repro.
- **Exhaustive** — unchanged DFS + DPOR, but since select arms, faults, and
  lock acquires are now decisions, it explores them; every explored schedule
  is exportable as a trace.
- **Replay(trace)** — new. Decisions are read from a recorded trace. This is
  what a regression test *is*.

Candidate later additions (see Scaling): preemption-bounded exploration and
PCT (probabilistic concurrency testing).

### Repro artifacts: two tiers

**Tier 1 — seed pin**: `(strategy, seed, iteration)`. Tiny, human-readable,
reasonably robust to unrelated code change. Preferred whenever the failure
came from a seeded run.

**Tier 2 — trace pin**: the explicit decision list, encoded as a versioned
token:

```
ptsched:v1:<base64( varint stream of (kind, choice) records )>
```

Needed for exhaustive-found failures, hand-written adversarial schedules,
and shrunk repros. Long tokens may live in a sidecar file (see Surface).

**Divergence is a hard failure.** A replay that diverges (decision-kind
mismatch, or `choice >= n_choices`) fails the test loudly, naming the
decision index — never silently passes, never falls back to the seed. A
regression test that stopped testing anything is worse than a deleted one.
The honest cost: refactoring a test's concurrency structure stales its trace
pins; the error message says how to re-mint. This is why tier 1 is preferred
when available, and why the test's assertion must still state the real
invariant.

### Failure output: every failure prints its pin

Every failure path — `expect` failure, deadlock, uncaught raise, trap —
prints a repro block before exiting:

```
FAIL counter.pt (line 12): expected 3, got 2
  strategy: Random  seed: 0x2a  iteration: 7
  schedule: ptsched:v1:CgEDAgQB...
  rerun:    pluto test counter.pt --test "concurrent increments" --schedule ptsched:v1:CgEDAgQB...
  pin:      test "concurrent increments" [schedule: "ptsched:v1:CgEDAgQB..."] { ... }
```

This requires routing the `expect` failure path (`builtins.c:3491`) and
trap/uncaught-error paths through scheduler reporting — the single most
valuable plumbing change in the RFC. Signal deaths get the same treatment at
the CLI level: report "crashed with SIGSEGV (signal 11)" instead of silence.

### Surface

In-source, extending the existing bracket style (`tests[scheduler: X]`,
`class Foo[dep: T]`) rather than introducing annotations:

```pluto
tests[scheduler: Random, seed: 0x2a, iterations: 1000] {
    test "finds the race" { ... }

    // regression pins:
    test "regression: lost increment, #512" [schedule: "ptsched:v1:CgEDAg..."] { ... }
    test "regression: recv after close, #530" [seed: 0x2a, iteration: 7] { ... }
    test "regression: fsync fails mid-write, #533" [schedule_file: "schedules/533.ptsched"] { ... }
}
```

Per-test brackets override the block. This removes the one-strategy-per-file
limit and requires `__pluto_test_run` to take per-test config (AST change →
`SCHEMA_VERSION` bump in `src/binary.rs`).

CLI additions to `pluto test`:

```
--test <name>          run one test by display name
--strategy <s>         override strategy
--schedule <token>     replay a trace
--seed <n>             accept hex (0x...) as the runtime already does
--iterations <n>       existing
--until-failure        loop random seeds until something breaks, print the pin
--max-schedules <n>    exhaustive bound (today env-only)
--max-depth <n>        exhaustive bound (today env-only)
```

## Soundness prerequisites

Each lands as an independent change; together they make "a fixed seed or
trace pins the execution" true and make Exhaustive's verdicts trustworthy.

**P0 — GC fiber-stack registry + CLI crash reporting** (bug fixes, no design
decisions). Reset the registry per `test_run_single` (or make registration
run-scoped); report signal deaths in the CLI. Unblocks all further
exhaustive-mode work — today the explorer segfaults past toy sizes.

**P1 — Select shuffle → oracle.** Kills the wall-clock leak at
`threading.c:2019`; fix the production-side `:2100` while there (any seed
but the clock).

**P2 — `std.random` scheduler-seeded in test mode.** Seed the xorshift state
from `run_seed` per iteration instead of the monotonic clock; an explicit
`random.seed()` call by the program still wins.

**P3 — Virtual clock.** In test mode `now()`/`monotonic()` return logical
time (epoch + scheduler step count suffices for v1). Deliberately does *not*
reopen timeout-as-choice — durations stay erased; only observed time
*values* become reproducible.

**P4 — Entity locks become real (and preemption points) in test mode.**
Two halves, both required:

1. *Yield at acquire*: `__pluto_entity_rdlock`/`wrlock` (and the per-type
   `__pluto_rwlock_*` of synchronized singletons/served classes) call the
   oracle before proceeding. Codegen already emits these calls
   unconditionally at every call site (`src/codegen/lower/mod.rs:920`,
   `:3072`, `:5086`), so no codegen change is needed — only the test-mode C
   stubs change. This makes method-granularity races (get-then-set lost
   updates) explorable and pinnable.
2. *Model the lock*: once fibers can interleave at method boundaries, the
   no-op lock would let Exhaustive explore interleavings production forbids
   (two writers inside one method — false positives). Test mode needs a
   fiber-aware rwlock mirroring `PlutoRwlock` semantics
   (`threading.c:2262`): `FIBER_BLOCKED_LOCK` state, wake-on-unlock,
   owner/depth reentrancy, read/read concurrency, read/write exclusion.

DPOR dependency extends naturally: two fibers conflict iff they touch the
same entity instance, with the read/write distinction (already computed —
it is `mut self`) giving extra pruning, analogous to the existing per-channel
dep matrix.

**P5 — Close the entity interior-aliasing hole** (language change; likely
its own RFC, referenced here as a dependency of the soundness claim).
Deep-copy mutable heap values (array/map/class/bytes) at entity method
boundaries, in and out — the same rule spawn capture already applies and the
same rule domains already have: *values copy across concurrency boundaries;
entities cross as handles*. Nested entities inside copied values still share
(as `deep_copy` already does), correctly, since they carry their own locks.
Without this, racing writes through escaped interior references are
invisible to any lock-site-based scheduler, and no instrumentation strategy
short of every-heap-write interception (schedule-space explosion, and it
papers over a production data race) restores soundness. With it, the claim
is clean: **entity lock sites + channel ops + timeouts = the complete
preemption-point set**, at method granularity, tractable for DPOR.

**P6 — Determinism audit.** No pointer-derived seeds anywhere; map/set
iteration order must be deterministic; no remaining real-clock reads inside
the determinism boundary.

## Fault injection: the "any error *ever*" half

Interleavings cover concurrency bugs; the rest of the failure universe is
reachable because the compiler already knows every operation's inferred
error set — a language-native advantage no bolt-on chaos tool has. Every
fallible runtime op becomes a kind-4 decision point: the oracle answers
"succeed" or "raise error *k* of your set."

- A regression test for "fsync failed mid-write" is a trace containing that
  inject decision — same token, same replay, same pin syntax.
- Exhaustive mode can prove every `catch` arm reachable and correct.
  Fault exploration multiplies the schedule space, so it is opt-in per test:
  `[faults: exhaustive]` / `[faults: random]` / default `off`.
- The existing `PLUTO_FS_SYNC_FAIL_AT` hook is the prototype and gets
  subsumed, so fs faults land in the same replayable trace.

v1 scope: channel ops and timeouts (already choices — formalize). v2: fs,
then net/socket — which begins pulling real I/O inside the determinism
boundary (today a blocking `accept` stalls the whole fiber scheduler and any
real-I/O test is outside the harness's guarantees).

## Scaling doctrine: what runs where

Exhaustive cost ≈ (schedules explored) × (one run of the body). Per-schedule
re-runs are sub-millisecond for small bodies (measured: 40 schedules in
~0.1s including startup). The count is the problem: interleavings of *n*
fibers with *k* dependent ops each grow as (nk)!/(k!)ⁿ — 2×5 is 252, 3×5 is
~750K, 10×10 is ~10³⁵. DPOR prunes independent operations, but the count
stays exponential in genuinely conflicting ops, and a broker-shaped program
is nothing but conflicting ops. Whole-program exhaustive is not slow; it is
information-theoretically out of reach. The architecture is a pyramid:

1. **Exhaustive as a microscope** — per protocol step, 2–4 fibers, bounded
   ops (replica handoff, one commit round, one rebalance decision). Minutes
   of budget; a genuine proof over that scope.
2. **Bounded exploration** — preemption bounding (schedules with ≤c
   preemptions: polynomial, and empirically most real concurrency bugs need
   ≤2) and PCT (randomized priorities with a known probability of hitting
   any depth-d bug). Future strategies for mid-sized components.
3. **Seeded randomized simulation** — the system-level workhorse (the
   FoundationDB / TigerBeetle model): the whole app in one process, every
   domain a fiber, virtual time, faults as oracle decisions, contracts and
   invariants as the oracle. The currency is seeds per night: at ~100ms per
   whole-system run, one core does ~300K seeds overnight, and single-threaded
   determinism makes it embarrassingly parallel across cores and machines.
   Every failure mints a trace token; the regression suite replays pins in
   milliseconds.

Exhaustive and random are the same decision oracle under different budgets —
proof at small scope, probability at system scope, and failures from either
tier are equally pinnable.

## Future work (recorded, not committed)

- **Schedule shrinking** — `--shrink` minimizes a failing trace toward
  round-robin / fewest preemptions before minting the pin.
- **I/O simulation** — fs/net behind the oracle; deterministic results +
  faults; removes the "real I/O blocks the scheduler" exclusion.
- **Domain-level DST** — simulated `at`-domain boundaries with message
  delay/drop/reorder as oracle decisions; the straight-line extension of the
  decision-stream design to the distributed model.
- **Convergence declarations** — proven-commutative entity methods become
  DPOR-independent even on the same instance (collapsing schedule families),
  and the exhaustive explorer doubles as an empirical convergence checker
  ("all schedules reach equal final state") where proofs don't reach. Ties
  into rfc-properties machinery; sequenced after this harness and P5.
- **Inferred lock splitting / degenerate-entity atomic lowering** — both are
  physical-plan optimizations whose correctness claims are schedule-space
  claims; ship them only after this harness can regression-pin them.

## Phasing

| Phase | Contents | Depends on |
|-------|----------|------------|
| 0 | GC registry fix, CLI signal reporting | — |
| 1 | Determinism: select→oracle, std.random, virtual clock, audit (P1–P3, P6) | — |
| 2 | Repro plumbing: always-record buffer, failure repro block, `--test`/`--strategy`/hex `--seed`/`--until-failure` | 0 |
| 3 | Replay: strategy, token codec, per-test bracket config (SCHEMA_VERSION bump) | 2 |
| 4 | Entity-lock soundness (P4) | 0; P5 for the full soundness claim |
| 5 | Fault injection on error sets (channels/timeouts, then fs) | 3 |
| 6+ | Bounded strategies, shrinking, I/O sim, domain DST, convergence | 3–5 |

Phases 0–2 alone deliver most of the daily value: every failure becomes
copy-paste reproducible.

## Open questions

1. **Pin syntax** — per-test bracket config (proposed; consistent with
   `class Foo[dep: T]` and `tests[scheduler: X]`) vs. the archived RFC's
   `@random(...)`-style annotations.
2. **Virtual clock semantics** — step-counter logical time (proposed) vs.
   leaving `now()` real and documenting it as outside the determinism
   boundary.
3. **Divergence policy** — hard-fail (proposed) vs. warn-and-fall-back.
4. **Trace placement** — inline token with `schedule_file:` escape hatch
   (proposed) vs. sidecar-only.
5. **P5 sequencing** — boundary-copy as its own RFC merged first, or folded
   into this one? It changes observable semantics (a getter returning a
   mutable field starts returning a copy) and deserves its own review.
6. **Fault-exploration bounding** — per-test opt-in is proposed; is a
   site-count bound (explore faults at ≤f sites per schedule) also needed to
   keep Exhaustive×faults tractable?

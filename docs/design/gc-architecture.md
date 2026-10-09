# Garbage collection for Pluto: requirements, designs, evaluation

Status: lab design (branch `gc-lab`). Governs which collectors are built and
how they are judged. Collectors here are selectable with `--gc <name>` or,
for whole test runs, `PLUTO_GC_BACKEND=<name>`.

## 1. What a Pluto collector must satisfy

Requirements are derived from the language's semantics and from measured
workloads, not from a catalogue of GC techniques.

**R1 — Soundness under conservative roots.** Pluto has no stack maps; roots
are found by conservatively scanning stacks, registers and registered
globals. Heap objects of class type are also traced conservatively (no
pointer maps). Any design must be sound when an integer happens to look like
a pointer (over-retention is acceptable; freeing a live object is not), and
must not move an object a conservative root might reference.

**R2 — The value/entity split is the sharing boundary.** Values are
deep-copied wherever they cross a thread boundary: spawn captures and channel
sends are copied by the compiler (`__pluto_deep_copy`). Only *entities*
(`object` declarations), DI singletons, the app instance, and channel and
task handles are shared by identity. A collector may rely on this: the type
system, not runtime discovery, says which objects can be shared.

**R3 — Server workloads.** The target is long-running backend services:
spawn-per-connection concurrency (Styx), large resident in-memory state
(Kerberos: a persistent page trie where "the GC is the MVCC vacuum"),
sensitivity to tail latency and to RSS. Throughput matters, but the failure
mode users notice is a pause or a footprint they cannot predict.

**R4 — A thread's pause must not scale with other threads' live data.**
Measured on the 8-thread `threads` benchmark: 136 ms of its 153 ms total
pause is *marking*, and its worst pause (21 ms) comes at only 11 MB live.
Every global collection re-marks all eight threads' retained data,
serially, on one core (latency-bound pointer chasing, ~47 ns per node).
The handshake itself is not the cost: replacing its 100 µs sleep-polling
with condition-variable parking changed nothing measurable (reverted), and
parallel marking does not help on this hardware (§2). The generational
backend won `threads` (0.42 s vs 0.58 s) only by collecting half as often.
Global stop-the-world collection is the scaling bottleneck.

**R5 — Determinism in test mode.** `pluto test` explores schedules with a
DPOR fiber scheduler and replays failures from a recorded token. GC work must
be a deterministic function of the program's allocation sequence, never of
wall-clock time, or replay breaks.

**R6 — Verifiable.** Developers trust a collector they can check. Every
design states its invariants and ships: a heap/invariant checker
(`PLUTO_GC_VERIFY`), a torture mode that collects every N allocations
(`PLUTO_GC_TORTURE`), and differential testing (identical program output
across backends on the benchmark suite and the integration tests).

## 2. Evidence from the exploration round

Six backends were measured on eleven benchmarks (`benchmarks/gc`, harness
`gclab.py`, Apple M-series 4P+6E, best-of-5 median, outputs verified
identical). Selected results — wall time / total pause / max pause:

| benchmark | marksweep | lazy | gen | parmark | noop | legacy |
|---|---|---|---|---|---|---|
| binary_trees | 0.38 s / 136 / 1.7 | 0.40 / 103 / 1.8 | **0.33 / 62 / 5.0** | 0.55 / 231 / 4.1 | 1.11 (971 MB) | 2.23 / 1756 / 29 |
| churn | 0.05 / 7.4 / 0.14 | 0.05 / **0.5** / 0.01 | 0.06 / 9.1 / 0.7 | 0.05 / 10.1 / 0.2 | 0.24 (252 MB) | 0.19 / 121 / 2.4 |
| maps | 0.09 / 32 / 4.8 | 0.09 / **14 / 1.4** | 0.10 / 20 / 5.5 | 0.11 / 42 / 3.1 | 0.16 | 0.40 / 318 / 22 |
| old_mutation | **0.11** / 45 / 6.2 | 0.11 / 37 / 5.5 | 0.15 / 64 / 7.8 | 0.11 / 52 / 6.7 | 0.21 | 0.52 / 443 / 68 |
| threads | 0.58 / 173 / 25 | 0.59 / 176 / 22 | **0.42 / 85 / 9** | 0.57 / 186 / 24 | 0.65 | 1.41 / 998 / 126 |
| struct_eq | 0.051 | 0.054 | **0.045** | 0.056 | 0.11 | >60 s |

What it established:

- **Collecting beats not collecting.** `noop` (never frees) is 2–4x slower
  than mark-sweep: it keeps touching fresh memory while a collector reuses a
  small, cache-warm heap.
- **Sweeping belongs in the allocator.** Lazy sweeping cut total pause by
  up to 15x (churn) at equal throughput, and lowered max pause everywhere.
- **A page-protection write barrier is blind to malloc'd backing stores.**
  `gen` must rescan every old container on every minor collection, which is
  why it lost on `old_mutation` and `containers`. Its wins came from fewer
  collections, not cheaper ones.
- **Parallel marking does not pay here.** Even with work spread across all
  eight workers (~8k shares per collection), marking a 2M-node tree was not
  faster: the mark is memory-latency bound and most helpers run on
  efficiency cores.
- **Global stop-the-world is the scaling limit** (R4).

## 3. The designs

### D1. Stop-the-world mark-sweep, refined (`marksweep`)

The yardstick: block heap with page map (merged, PR #517), lazy sweeping,
and a stop-the-world handshake that does not sleep in 100 µs quanta.
Predictable, simple, and the fallback every other design is compared to.

### D2. Shared-nothing heaps (`tlh`)

Each task (OS thread) owns a private heap and collects it alone, without
stopping anyone else. A shared heap holds the objects R2 says can be shared.

**Invariant I (no inward pointers).** No shared object holds a pointer to a
private object. Private objects may point to shared ones and to their own
heap's objects only.

From I it follows that a thread's private collection needs only that
thread's own roots (stack, registers, thread-locals) and never needs to stop
or scan another thread; that another thread's stale stack words pointing into
this heap can be ignored (they cannot be real references); and that when a
task ends, everything still private to it is garbage (its result and error
are promoted first) — so the whole heap is reclaimed in bulk, without
marking. For spawn-per-connection servers this makes each task a region.

**Maintaining I.**
- Objects that start shared: entities, DI singletons, the app instance,
  channel and task handles, and every object allocated by a boundary copy
  (spawn captures, channel sends).
- **Promotion barrier:** storing a pointer into a shared object first
  promotes the stored value's transitive (conservatively traced) closure to
  shared. Promotion is non-moving: an ownership bit flips; the object stays
  where it is. Codegen emits the barrier on field stores whose type is not a
  scalar; runtime container mutators (array/map/set writes, task result and
  error slots) apply it to the stored value.
- Over-promotion (an integer that looks like a private pointer) is safe: it
  only delays reclamation to the next shared collection.

**Collections.** Private: triggered per thread by that thread's allocation
budget; marks only objects it owns that are still private; stops at shared
objects. Shared: a global stop-the-world collection of everything, triggered
by shared-heap growth (promotions and shared allocations), which R2 makes
rare. Allocation is thread-local (per-thread size-class block lists), so it
takes no global lock.

**Costs.** A barrier on non-scalar field stores (one header check when the
target is private). Workloads that funnel many values through entities or
channels promote often and fall back to global collections.

### D3. Bounded-pause incremental collector (`incr`)

Targets R3's tail latency independent of heap size (a 64 MB resident tree
currently costs a 23 ms pause). Marking proceeds in small increments
interleaved with allocation: every K bytes allocated during a cycle performs
a fixed quantum of mark work. Determinism (R5) comes from pacing by
allocation, not time.

**Invariant T (tri-colour, incremental update).** At the end of marking, no
black object points to a white one. Stores into already-scanned objects are
caught by the page-protection barrier (dirty blocks are re-scanned in the
final, short stop-the-world step); objects allocated during marking are
allocated black. Malloc'd backing stores are invisible to the barrier, so
containers modified during a cycle are re-scanned at the final step (the
cost measured in `gen`, bounded here because only containers touched during
the cycle — tracked by the runtime mutators — need it).

Max pause becomes O(roots + dirty blocks + touched containers) instead of
O(heap).

## 4. Evaluation

Every design is judged on: wall time; total, max and p99 pause; peak RSS;
scaling from 1 to 8 threads; and the verification suite (R6) passing under
torture. New benchmarks target each claim: a spawn-per-request server loop
and an entity-heavy sharing loop for D2 (its best and worst cases), and
pause-distribution measurements on large resident heaps for D3, plus the
real workloads (Kerberos SLT, Styx).

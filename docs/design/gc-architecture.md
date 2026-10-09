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

**Task exit (region reclamation).** The exit path publishes the task's
result and error through barriered stores before the thread deregisters,
so by invariant I everything still private to it is dead. A sweep with no
marking frees it in bulk; blocks still holding shared objects pass to the
shared heap.

**Lock freedom of local work.** A local collection takes no lock. Lookups
are confined to the heap's own private objects through acquire loads of
the page map and the block owner (a block owned by H is only changed by H
or by a stop-the-world collection, which cannot overlap a local one).
Anything needing `gc_mutex` — returning emptied blocks, unmapping large
objects, global byte accounting — is queued on the heap and handed over
at the next allocation report.

**Costs.** A barrier on non-scalar field stores (one header check when the
target is private). Workloads that funnel many values through entities or
channels promote often and fall back to global collections. A thread whose
own private heap is large still pauses itself for a full local mark.

### D3. Bounded-pause incremental collector (`incr`)

Targets R3's tail latency independent of heap size. Marking proceeds in
short stop-the-world steps interleaved with allocation; sweeping is lazy.
Determinism (R5) comes from pacing by allocation, never by time.

**Invariant S (snapshot at the beginning).** Every object reachable when a
cycle starts, and every object allocated during it, is marked when the
cycle ends. Roots are scanned once, at the start step; stacks are never
rescanned. Objects allocated while marking are allocated black. While a
cycle marks, a *deletion* barrier logs every reference that is overwritten
in, or removed from, a heap object; each step shades the logs.

Why deletion (Yuasa) rather than insertion (Dijkstra): an insertion barrier
with black allocation must also cover initializing stores (struct literals,
closure captures, enum payloads, trait boxes), and would depend on codegen
never allocating between an object's allocation and its field stores;
insertion barriers also force a final stack rescan. Initializing stores
overwrite nothing, so the deletion barrier needs none of that, and the
final pause is only the log drain. Every overwriting store already goes
through the runtime mutators (array set, map overwrite, removals, clears,
channel receives) or the codegen field-store barrier.

**Large containers** are scanned 1024 slots per chunk through a
continuation stack, so no step pays for a whole million-element array.
Chunked scanning adds one obligation: an element that moves toward lower
slots can slip behind the scan cursor, so while marking the runtime also
logs elements moved by `remove_at`, `reverse`, map/set deletion shifts and
rehashing (each already O(n) in what it touches).

**Barrier selection.** `__pluto_gc_barrier_mode`, defined by every backend,
is read by codegen before each non-scalar field store and by the runtime
mutators: 0 none, 1 promotion (D2), 2 deletion logging (D3, only while a
cycle marks). Outside a cycle the barrier costs one load and a branch.

Max pause becomes O(logged references + one trace quantum); the start step
adds the root scan. The previous cycle's leftover sweep runs in bounded
batches from the allocator, without stopping the world, before the next
cycle starts.

### D4. Generational shared-nothing heaps (`hybrid`)

D2 and D3 each leave one pause unbounded: D2's local collection of a
thread whose own data is large, D3's stop-the-world steps on every thread.
D4 combines them through an observation about invariant I: *no shared
object points at a private one* is exactly *no old object points at a
young one*. So:

- **Tenuring.** A private object that survives two local collections (one,
  when most of what was allocated since the last collection survived) is
  promoted. A thread's private heap then holds young data, and a local
  collection is a minor collection with **no remembered set**: the
  promotion barrier already is the generational write barrier (a store
  into an old object tenures what is stored). The young generation is
  capped at 4 MiB of allocation between local collections, so a local
  pause is bounded by young data, not by the thread's resident state.
- **Incremental global cycle.** The shared (old) heap is collected by D3's
  snapshot-at-the-beginning machinery (barrier mode 3: promote + log),
  marking with bit 1 of the mark byte so local collections keep running
  between steps with bit 0. Steps are paced by allocation checkpoints.
- **Region reclamation at task exit** and lock-free allocation carry over
  from D2.

**Coexistence rules** (each one guards a failure that was observed):

1. A local sweep keeps private objects the running global cycle has marked
   (they may be on its worklist).
2. An object promoted while the cycle marks is *logged*, not marked: marking
   it black would skip the shared objects only it reaches.
3. **Freeing is deleting.** While a cycle marks, a local sweep logs the
   out-edges of every object it frees; otherwise a shared object reachable
   at the snapshot only through a freed private object is missed.
4. Dead shared objects are freed by each thread heap itself (owner sweep)
   after the cycle; a new cycle first completes any pending owner sweep so
   no stale mark leaks into it.
5. **Objects are invisible to collections until their allocation returns.**
   Every checkpoint runs before the object is allocated. Runtime code
   initializes fresh objects with plain stores (array slices are filled
   raw); a collection that ran inside the allocating call once flagged a
   still-empty array as "clean", and the raw fill then hid a live element.
   Found on the Kerberos SLT suite as silently wrong query results.
6. **Root-referenced objects are never tenured.** Generated code and the
   runtime allocate an object, run more allocating code (struct literals
   evaluate their fields after allocating; deep copy fills copies with
   copied children), and only then initialize it. A half-built object is
   reachable only from a root, so excluding root-referenced objects from
   tenuring keeps those plain stores from breaking invariant I.
7. Every store into container storage goes through `PLUTO_GC_STORE`,
   including runtime builders that span allocations (deep copy).

**Making local collections proportional to young data.** Local marks skip
*clean* containers (a private container whose elements were all shared at
its last scan; any store clears the flag); local sweeps skip blocks with no
private objects (an exact per-block count: allocation adds, promotion
subtracts, sweeps recount) and park full blocks of shared objects outside
the sweep set. Each optimization has a `PLUTO_GC_VERIFY` check: clean
containers hold no private element; skipped blocks hold no private object.

## 4. Evaluation

Every design is judged on: wall time; total, max and p99 pause; peak RSS;
scaling from 1 to 8 threads; and the verification suite (R6) passing under
torture. New benchmarks target each claim: a spawn-per-request server loop
and an entity-heavy sharing loop for D2 (its best and worst cases), and
pause-distribution measurements on large resident heaps for D3, plus the
real workloads (Kerberos SLT, Styx).

## 5. Results (2026-10-09)

14 benchmarks × 5 collectors, best-of-5 median, outputs identical across
collectors; Kerberos SLT (5 heaviest files, interleaved) and Styx
(8 TCP clients, spawn-per-connection, interleaved). Full report with
charts: https://claude.ai/artifact/V5AZjXAhDcrUU5C6C2QpF8

| | wall (geo-mean vs marksweep) | worst STW pause | worst thread-local pause | RSS (geo-mean) | Kerberos (5 files) |
|---|---|---|---|---|---|
| marksweep | 1.00× | 27 ms | — | 1.00× | 64.2 s, 22.5 s STW |
| tlab | 0.78× | 26 ms | — | 1.01× | — |
| tlh | **0.70×** | 23 ms | 45 ms | 1.49× | **57.0 s**, 0.05 s STW |
| incr | 0.99× | 5.6 ms | — | 1.14× | 66.9 s, 10.9 s STW |
| hybrid | 1.02× | **2.8 ms** | **7.6 ms** | **0.99×** | 91.8 s, 5.5 s STW |

- Multi-threaded allocation (`threads`, 8 tasks): marksweep 0.36 s, tlh
  0.061 s, hybrid 0.052 s.
- Spawn-per-request with a resident cache: marksweep 0.25 s, tlh 0.11 s.
- Large live heap (`resident_service`, 58 MB): max pause marksweep 27 ms,
  incr 1.1 ms, hybrid 0.43 ms STW / 4 ms local.
- Single-threaded structure building is hybrid's weak spot (1.6–3.3×
  slower than tlh), as is Kerberos; its young generation should be paced
  to a pause budget instead of a fixed 4 MiB.
- Found and fixed on master along the way: task `get()` stalled every
  stop-the-world collection by up to 10 ms (#526); noop field count (#518).


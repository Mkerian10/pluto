# RFC: Deterministic Concurrency Testing

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-10

## Summary

Pluto programs that use `spawn`, channels, and shared DI singletons should be testable deterministically using the existing `test "name" { }` framework. This RFC describes a **deterministic test scheduler** that replaces real pthreads with cooperative fibers in test mode, giving the scheduler full control over which task runs when. Same Pluto source code, different runtime behavior in tests.

## Motivation

### The Problem

Real pthreads produce non-deterministic scheduling. The OS decides which thread runs when, and the answer changes on every execution. This makes concurrent bugs — race conditions, deadlocks, ordering violations — intermittent and unreproducible:

```
test "transfer is atomic" {
    let s = Sender<int>()
    let r = Receiver<int>()
    let ch = chan<int>(1)

    let t1 = spawn send_values(ch.sender())
    let t2 = spawn recv_values(ch.receiver())

    // Does t1 run first? t2? Interleaved?
    // The test passes 99% of the time. The 1% is a production incident.
    t1.get()!
    t2.get()!
}
```

"Run and hope" testing is insufficient for concurrent code. A test that passes a thousand times may fail on the thousand-and-first because the OS happened to schedule threads differently.

### Industry Precedent

Other languages have recognized this problem:

- **Loom** (Rust) — replaces `std::sync` primitives with instrumented versions, explores all possible interleavings using dynamic partial order reduction (DPOR)
- **CHESS** (Microsoft Research) — systematic concurrency testing for C/C++, explores schedules exhaustively
- **jcstress** (Java) — stress testing framework for JVM concurrency
- **Go race detector** — dynamic analysis that flags data races at runtime (detects, doesn't prevent)

Pluto can do better than all of these because:

1. **Copy-on-spawn eliminates data races by construction.** We don't need a race detector — races can't happen.
2. **All concurrency goes through known primitives.** `spawn`, `task.get()`, channel ops, and (future) DI singleton access are the only concurrency points. No raw locks, no atomics, no `unsafe`.
3. **The compiler controls both sides.** `plutoc test` compiles the test binary — we can link against a completely different runtime without the user doing anything.

### Why Now

This RFC is written before Phase 3 (structured concurrency) for three reasons:

1. **Yield points shape the API.** Phase 3 adds `.cancel()`, `.detach()`, and `select` — all new yield points. Designing the test scheduler first ensures every new primitive gets a yield point from day one.
2. **Cheap now, expensive later.** Retrofitting testability onto an already-built runtime requires refactoring. Building it in from the start means the runtime has test hooks from the beginning.
3. **Validates the concurrency model.** Writing this RFC forces us to enumerate every blocking point and concurrency primitive, which catches design gaps early.

---

## Design

### Core Idea: Dual Runtime

When `plutoc test` compiles a test file, it links against a **test scheduler runtime** instead of the pthread runtime. The same Pluto source code compiles to different behavior:

| | `plutoc compile` / `plutoc run` | `plutoc test` |
|---|---|---|
| `spawn` | `pthread_create` | Create cooperative fiber, register with scheduler |
| `task.get()` | `pthread_cond_wait` | Yield to scheduler, resume when task completes |
| `chan.send()` | Lock + `pthread_cond_wait` | Yield if full, scheduler resumes when space available |
| `chan.recv()` | Lock + `pthread_cond_wait` | Yield if empty, scheduler resumes when data available |
| `task.cancel()` | Sets atomic flag | Yield, target checks at next yield point |
| `task.detach()` | Releases handle | Immediate (no yield needed) |

Switched at **compile time**, not runtime. Test binaries link against the test scheduler; production binaries link against pthreads. No `if (test_mode)` branches in production code.

### Cooperative Fibers

Each `spawn` in test mode creates a **fiber** — a lightweight, cooperatively-scheduled execution context. Fibers yield at well-defined points and only resume when the scheduler explicitly selects them.

Implementation options (chosen at build time for the test runtime):
- **`ucontext`** (POSIX) — `makecontext`/`swapcontext`. Portable, well-understood. ~200 bytes per fiber.
- **`setjmp`/`longjmp` + manual stack** — lighter weight but less portable.
- **Platform fibers** — Windows fibers, macOS `_XOPEN_SOURCE` ucontext. Future optimization.

The MVP uses `ucontext` for simplicity. Each fiber gets a small stack (64KB default, configurable). The scheduler maintains a ready queue and a blocked set.

### Yield Points

Every blocking or synchronization operation is a **yield point** — a place where the scheduler can switch to a different fiber. This is the complete list:

| Operation | Yield behavior |
|-----------|---------------|
| `task.get()` | Yield if task not done. Resume when target fiber completes. |
| `chan.send(value)` | Yield if channel full. Resume when receiver consumes a value. |
| `chan.recv()` | Yield if channel empty. Resume when sender produces a value. |
| `chan.try_send(value)` | No yield (non-blocking by definition). |
| `chan.try_recv()` | No yield (non-blocking by definition). |
| `chan.close()` | No yield. Wakes all blocked fibers on this channel. |
| `task.cancel()` | No yield for caller. Target yields at next yield point. |
| `task.detach()` | No yield. |
| `select { }` (Phase 3) | Yield. Scheduler tries each branch, resumes when one is ready. |
| DI singleton `mut self` call (Phase 4) | Yield if writer lock contended. Resume when lock available. |
| DI singleton `self` call (Phase 4) | Yield if writer lock held. Resume when readers allowed. |

**Key invariant:** Copy-on-spawn still applies in test mode. Each fiber gets its own deep-copied data. The scheduler controls interleaving but doesn't change the memory isolation model.

### Scheduling Strategies

Different strategies serve different testing goals. The strategy is selected per-test via annotation:

```
test "counter increments correctly" {
    // Default: sequential. Each spawn runs to completion before next.
}

test "no deadlock under all interleavings" @exhaustive {
    // Tries all possible schedules. Exponential, bounded by depth.
}

test "stress test with random scheduling" @random(iterations: 1000) {
    // 1000 runs with different random schedules.
}

test "round robin interleaving" @round_robin {
    // Fibers take turns at each yield point.
}
```

#### Sequential (default)

Each spawned task runs to completion before the next starts. The main fiber spawns a task, that task runs entirely, control returns to the main fiber.

- **Simplest and most predictable.** Good for basic correctness testing.
- **Matches single-threaded mental model.** Easy to reason about.
- **Catches logic bugs but not concurrency bugs.** A test that passes with sequential scheduling may deadlock under interleaving.

This is the default because most tests care about correctness, not concurrency. Concurrency-specific tests opt into other strategies explicitly.

#### Round-Robin

At each yield point, the scheduler advances to the next fiber in creation order. Every fiber gets equal turns.

- **Deterministic interleaving.** Same result every time.
- **Tests interleaved access patterns** without combinatorial explosion.
- **Good for channel pipelines** where producer/consumer alternation matters.

#### Random

At each yield point, the scheduler picks a random fiber from the ready set. Run N iterations with different seeds to explore the schedule space.

- **Seed printed on failure.** Rerun with the same seed to reproduce.
- **Good for fuzzing.** Finds bugs that deterministic strategies miss.
- **Configurable iteration count.** `@random(iterations: 100)` for quick checks, `@random(iterations: 10000)` for thorough exploration.

```
// On failure, the test runner prints:
// FAILED: "stress test" (seed: 0xDEADBEEF, iteration: 847)
// Rerun with: plutoc test file.pluto --seed 0xDEADBEEF --test "stress test"
```

#### Exhaustive

Systematically explores all possible schedules using **dynamic partial order reduction (DPOR)**. At each yield point with N ready fibers, the scheduler forks into N paths, exploring each choice.

- **Finds every possible bug.** If a schedule can trigger a failure, exhaustive will find it.
- **Exponential cost.** Bounded by `max_depth` (default: 100 yield points) and `max_schedules` (default: 10,000).
- **DPOR prunes equivalent schedules.** Two schedules that differ only in the ordering of independent operations (no shared state, no communication) produce the same result. DPOR detects this and skips redundant exploration.

```
test "no deadlock" @exhaustive(max_schedules: 50000) {
    // Override the default bound for thorough exploration
}
```

In Pluto, DPOR is particularly effective because copy-on-spawn means most fibers are independent. Only fibers that communicate through channels or (Phase 4+) access the same DI singleton have dependencies. This dramatically reduces the schedule space compared to shared-memory languages.

---

## Deadlock Detection

### How It Works

The scheduler detects deadlock when **all fibers are blocked** — no fiber is in the ready set, but at least one fiber hasn't completed.

A fiber is blocked when:
- It called `task.get()` on a task whose fiber hasn't completed
- It called `chan.send()` on a full channel with no ready receivers
- It called `chan.recv()` on an empty channel with no ready senders
- (Phase 4) It's waiting on a writer lock held by another blocked fiber

### Reporting

On deadlock detection, the test reports:

```
DEADLOCK in test "producer consumer":
  Fiber 0 (main): blocked on task.get() for Fiber 1
  Fiber 1 (spawn at line 12): blocked on chan.recv() — channel empty, no active senders
  Fiber 2 (spawn at line 15): blocked on chan.send() — channel full, no active receivers

Dependency cycle: Fiber 0 → Fiber 1 → Fiber 2 → (no progress possible)
```

The scheduler builds a **wait-for graph** — fibers are nodes, "blocked on" relationships are edges. A cycle in this graph is a deadlock. A state where all fibers are blocked but no cycle exists is a **resource starvation** (all waiting on external events that will never arrive, like receiving from a channel with no senders).

### Livelock Detection

Livelock — fibers making progress but never completing — is harder to detect automatically. The scheduler uses a **progress bound**: if no fiber completes or makes observable progress (produces/consumes a channel message, completes a `task.get()`) within N yield points, report a potential livelock:

```
POTENTIAL LIVELOCK in test "busy loop":
  After 10000 yield points, no fiber has completed.
  Fiber 0 (main): yielded 3400 times
  Fiber 1 (spawn at line 8): yielded 6600 times
```

The bound is configurable: `@random(iterations: 100, progress_bound: 50000)`.

---

## Integration with Existing Test Framework

### Current Infrastructure

Today's test framework:
- `test "name" { body }` is desugared to `__test_N()` functions at parse time
- The test runner main calls `__pluto_test_start(name)`, `__test_N()`, `__pluto_test_pass()` for each test
- Assertions: `expect(x).to_equal(y)` — hard abort on failure
- Tests run sequentially in a single process

### What Changes

**Runtime substitution.** When `plutoc test` compiles, it links against test versions of the concurrency runtime functions:

```c
// Production runtime (builtins.c)
long __pluto_task_spawn(long closure_ptr) {
    // pthread_create, real thread
}

// Test runtime (builtins_test.c)
long __pluto_task_spawn(long closure_ptr) {
    // fiber_create, register with scheduler
}
```

Both expose the same ABI. The compiler doesn't need to know which is linked — it emits the same IR either way.

**Per-test scheduler state.** Each test gets its own fresh scheduler. No state leaks between tests. The test runner:

1. Calls `__pluto_test_scheduler_init(strategy)` before each test
2. Calls `__test_N()` (which spawns fibers via the test runtime)
3. Calls `__pluto_test_scheduler_run()` to execute fibers until all complete or deadlock
4. Calls `__pluto_test_scheduler_destroy()` to clean up

**Test annotations.** The parser recognizes `@exhaustive`, `@random(...)`, `@round_robin` after the test name. These are stored as metadata on the test AST node and passed to `__pluto_test_scheduler_init()`:

```
test "name" @random(iterations: 100) { body }
```

Annotations are optional. No annotation means sequential scheduling.

### Assertion Behavior

Assertions (`expect(x).to_equal(y)`) abort the current fiber, not the whole test. The scheduler:

1. Catches the assertion failure (fiber's stack unwinds via `longjmp` or similar)
2. Records which fiber failed and the assertion message
3. Marks the test as failed
4. Continues running other fibers if the strategy requires it (e.g., exhaustive needs to explore other schedules)

For sequential and round-robin, an assertion failure in any fiber fails the test immediately. For random and exhaustive, the scheduler records the failure and continues exploring (to find all bugs, not just the first one).

---

## Runtime Architecture

### Test Scheduler State

```c
typedef struct {
    Fiber *fibers;          // Array of all fibers
    int fiber_count;
    int ready_head;         // Ready queue (circular buffer or linked list)
    int current_fiber;      // Currently executing fiber
    Strategy strategy;      // SEQUENTIAL, ROUND_ROBIN, RANDOM, EXHAUSTIVE
    uint64_t seed;          // For random strategy
    int yield_count;        // Total yields (for livelock detection)
    int progress_bound;     // Max yields without progress
    WaitGraph wait_graph;   // For deadlock detection
} TestScheduler;

typedef struct {
    ucontext_t context;     // Execution context
    FiberState state;       // READY, RUNNING, BLOCKED, COMPLETED, FAILED
    long result;            // Return value when completed
    long error;             // Error value if raised
    void *blocked_on;       // What this fiber is waiting for (task, channel)
    int id;
} Fiber;
```

### Function Replacement Table

| Production function | Test replacement | Behavior change |
|---|---|---|
| `__pluto_task_spawn(closure)` | `__pluto_test_task_spawn(closure)` | Creates fiber instead of pthread |
| `__pluto_task_get(task)` | `__pluto_test_task_get(task)` | Yields to scheduler if task not done |
| `__pluto_chan_send(handle, value)` | `__pluto_test_chan_send(handle, value)` | Yields if channel full |
| `__pluto_chan_recv(handle)` | `__pluto_test_chan_recv(handle)` | Yields if channel empty |
| `__pluto_chan_try_send(handle, value)` | `__pluto_test_chan_try_send(handle, value)` | No yield (same as production) |
| `__pluto_chan_try_recv(handle)` | `__pluto_test_chan_try_recv(handle)` | No yield (same as production) |
| `__pluto_chan_close(handle)` | `__pluto_test_chan_close(handle)` | Wakes blocked fibers |

The function names stay the same — the linker resolves to the test versions when linking against `builtins_test.o` instead of `builtins.o`.

### GC in Test Mode

In production, GC is suppressed while tasks are active (`atomic_int __pluto_active_tasks`). In test mode, everything runs on a single thread — no GC race conditions are possible. The test runtime can run GC normally at any yield point since no concurrent mutation exists.

### Error Propagation

Errors in fibers work the same as in production:

1. A fiber raises an error → stored in the fiber's error field
2. Another fiber calls `task.get()` → scheduler resumes it, the test runtime copies the error to TLS (`__pluto_current_error`)
3. The calling fiber handles it with `!` or `catch` as usual

The single-threaded test runtime simplifies this: TLS is just a global variable, no synchronization needed for error state.

---

## Implementation Phases

### Phase A: Sequential Scheduler (MVP)

**Goal:** Validate the dual-runtime approach. In test mode, `spawn` runs tasks to completion immediately (effectively synchronous execution).

**Scope:**
1. Create `runtime/builtins_test.c` with test versions of `__pluto_task_spawn` and `__pluto_task_get`
2. `__pluto_test_task_spawn` stores the closure, creates a fiber context, and immediately runs it to completion (no actual scheduling yet — just sequential execution)
3. `__pluto_test_task_get` returns the stored result (already computed)
4. Modify `plutoc test` compilation to link against `builtins_test.o`
5. All existing concurrency tests should pass unchanged (sequential scheduling produces the same results as "lucky" thread scheduling)

**Deliverable:** `plutoc test` works with `spawn` — tasks execute deterministically in creation order.

**Why this matters:** Establishes the infrastructure (dual runtime, test-specific linking) that all subsequent phases build on. If the MVP works, the hard part (compiler plumbing) is done.

### Phase B: Fiber Scheduler + Round-Robin + Random

**Goal:** Real cooperative scheduling with interleaving.

**Scope:**
1. Implement `ucontext`-based fiber creation and context switching
2. Implement scheduler loop: pick next fiber from ready set, `swapcontext` to it, handle yields
3. Yield points at `task.get()` and channel operations
4. Round-robin strategy: deterministic interleaving
5. Random strategy: seed-based random fiber selection, `--seed` CLI flag for reproducibility
6. Test annotations: `@round_robin`, `@random(iterations: N)`
7. Deadlock detection: all fibers blocked with none ready

**Deliverable:** Tests can explore interleaved schedules. Random strategy finds ordering-dependent bugs. Failed tests print seeds for reproduction.

### Phase C: Exhaustive Exploration + DPOR

**Goal:** Systematic exploration of all possible schedules.

**Scope:**
1. Schedule tree: at each yield point with N choices, fork into N branches
2. DPOR implementation: identify independent operations, prune equivalent schedules
3. Bounded exploration: `max_schedules`, `max_depth` limits
4. `@exhaustive` annotation with configurable bounds
5. Livelock detection: progress bound monitoring
6. Multi-failure reporting: collect all distinct failures across all explored schedules

**Deliverable:** `@exhaustive` proves correctness across all possible interleavings (within bounds). Deadlocks and ordering bugs are found deterministically, not by luck.

### Phase D: Integration with Future Concurrency Features

**Goal:** Extend the test scheduler as new concurrency primitives land.

**Scope (aligns with concurrency v2 phases):**
1. **Phase 3 integration (structured concurrency):** `.cancel()` and `.detach()` yield points. Cancellation propagation in test scheduler. Must-use enforcement on fiber handles.
2. **Phase 4 integration (inferred sync):** rwlock acquire/release as yield points. Test scheduler explores reader/writer interleavings — can multiple readers proceed? Does a writer starve?
3. **Phase 5 integration (distributed):** Multi-pod simulation. Each "pod" is a set of fibers with isolated memory. Channel communication between pods simulates network. Inject failures (message loss, reordering, partition) to test distributed protocols.

---

## Test Annotations: Full Grammar

```
test "name" { body }                              // Sequential (default)
test "name" @sequential { body }                  // Explicit sequential
test "name" @round_robin { body }                 // Round-robin interleaving
test "name" @random(iterations: 1000) { body }    // Random, 1000 iterations
test "name" @random(iterations: 100, seed: 42) { body }  // Fixed seed
test "name" @exhaustive { body }                  // Exhaustive with default bounds
test "name" @exhaustive(max_schedules: 50000) { body }    // Custom bound
```

Annotations are parsed as part of the test declaration. Invalid annotations are compile errors. Unknown annotation names are compile errors (forward-compatible — new strategies can be added).

---

## CLI Integration

### Running Tests

```bash
plutoc test file.pluto                    # Run all tests (default strategies)
plutoc test file.pluto --seed 0xDEADBEEF  # Override seed for @random tests
plutoc test file.pluto --test "name"      # Run a single test
```

### Output

```
test basic spawn ........................... ok
test channel pipeline @round_robin ........ ok
test no deadlock @exhaustive .............. ok (explored 847 schedules)
test stress test @random(1000) ............ ok (1000 iterations)
test ordering bug @random(1000) ........... FAILED (seed: 0x1A2B3C4D, iteration: 312)
  Fiber 1 (spawn at line 12): expect(result).to_equal(42) — got 0
  Schedule: [0, 1, 0, 1, 1, 0, ...]

5 tests: 4 passed, 1 failed
```

The schedule trace (list of fiber IDs chosen at each yield point) is printed on failure, enabling exact reproduction.

---

## What This Means for Future Phases

### Phase 3: Structured Concurrency

`.cancel()` and `.detach()` are new yield points from day one:

- `.cancel()` sets a flag on the target fiber. The scheduler checks the flag at yield points and terminates the fiber if set. Tests can verify cancellation behavior deterministically.
- `.detach()` releases the handle. The scheduler continues running the detached fiber. Tests can verify that detached tasks complete (or don't) under different schedules.
- `select { }` is a compound yield point — the scheduler evaluates which branches are ready and picks one. Under exhaustive scheduling, all ready branches are explored.

### Phase 4: Inferred Synchronization

rwlock acquire/release become yield points. The test scheduler can explore:

- **Reader-reader concurrency:** Multiple fibers holding read locks proceed without blocking.
- **Writer exclusion:** A fiber requesting a write lock yields until all readers release.
- **Writer starvation:** Under round-robin, do readers continuously block a waiting writer? The scheduler can detect this.
- **Deadlock from lock ordering:** Two singletons accessed in different orders by different fibers.

### Phase 5: Distributed

The scheduler model extends to multi-pod simulation:

- Each "pod" is a group of fibers with isolated memory
- Channel communication between pods goes through a simulated network layer
- The scheduler can inject failures: message loss, reordering, network partitions
- Exhaustive exploration finds distributed consensus bugs

---

## Resolved Questions

1. **Why not just use Loom?** Loom is a Rust library for testing Rust concurrency primitives. Pluto's concurrency model is fundamentally different — copy-on-spawn eliminates shared memory, and all concurrency goes through a small set of compiler-known primitives. A custom scheduler tailored to Pluto's model is simpler and more effective than adapting a shared-memory testing framework.

2. **Performance of exhaustive exploration.** Exponential in the worst case, but DPOR makes it practical. Copy-on-spawn means most fibers are independent (no shared state to create dependencies), so DPOR prunes aggressively. For typical Pluto programs with 2-5 tasks communicating through channels, the schedule space is manageable.

3. **`ucontext` portability.** `ucontext` is deprecated on macOS but still works (and is used by many fiber libraries). For long-term portability, Phase B can add platform-specific backends. The scheduler API is backend-agnostic — swap `ucontext` for `setjmp`/`longjmp` or platform fibers without changing the scheduler logic.

4. **Memory overhead of fibers.** 64KB stack per fiber. For tests with <100 concurrent spawns (typical), that's <6.4MB. Configurable if needed. Production runtime (pthreads) is unaffected.

5. **Non-blocking operations (`try_send`, `try_recv`).** These don't yield because they're non-blocking by definition — they return immediately with success or error. The scheduler doesn't need to intervene. This preserves the same semantics as production.

6. **Deep copy in test mode.** Copy-on-spawn deep copy still happens in test mode, even though everything is single-threaded. This ensures tests exercise the same code paths as production. A fiber mutating its copied data doesn't affect other fibers, just like in production.

## Open Questions

1. **Should sequential scheduling call tasks inline or create fibers?** Inline (run the closure immediately in `__pluto_test_task_spawn`) is simpler for Phase A but doesn't exercise the fiber machinery. Creating a fiber and immediately running it to completion exercises more of the stack but is more complex. Decision: start inline (Phase A), switch to fibers (Phase B).

2. **How should `@exhaustive` interact with `@random` in the same test file?** If one test uses `@exhaustive` (slow) and another uses `@random(iterations: 10000)` (also slow), the total test time could be large. Should there be a global time budget? A `--fast` flag that caps iterations? Defer to user experience feedback.

3. **Should the test scheduler support custom yield points?** A `yield()` builtin that explicitly yields the current fiber, useful for testing interleaving at specific points. This would be test-only (no-op in production). Useful but adds a new keyword — defer until there's a concrete need.

## Alternatives Considered

### Runtime Flag Instead of Dual Runtime

A single runtime with `if (__pluto_test_mode)` checks at every concurrency operation. Rejected because:
- Adds branches to every spawn/get/send/recv in production
- Production binary carries dead test code
- Harder to reason about — two code paths interleaved in one function

### Thread Sanitizer (TSan) Integration

Use compiler sanitizers to detect races. Rejected because:
- Pluto's copy-on-spawn already prevents data races — TSan would find nothing
- TSan detects races, it doesn't test correctness under different schedules
- Doesn't help with deadlocks or ordering bugs

### Model Checking (TLA+ / Spin)

Write a formal model of the program and verify properties. Rejected as the primary approach because:
- Requires the programmer to write a model separate from their code
- Model and code can diverge
- Pluto's approach: the code IS the model, the scheduler explores its behavior directly
- Could complement this RFC as a future static verification tool (Phase 6 of contracts)

### Green Threads Everywhere (Always Cooperative)

Make Pluto always use cooperative scheduling, even in production. Rejected because:
- Doesn't exploit multi-core parallelism in production
- Real pthreads give actual concurrency
- The goal is deterministic *testing*, not deterministic *execution*

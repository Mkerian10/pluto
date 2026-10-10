# RFC: Green tasks (two-tier concurrency) — language surface & semantics

Status: draft for owner review. Decision to pursue the two-tier model is made
(#369). Runtime core is prototyped + measured (`runtime/green/`). This fixes the
*language* surface and the cross-tier semantics before codegen.

## Principle

`spawn` and `green` are the SAME concurrency model — tasks + channels + select,
same sharing rules (channels copy values, entities cross as serialized handles)
— differing only in the **executor**:

- `spawn f(args)` → a preemptive OS thread. For parallelism / CPU-bound work.
- `green f(args)` → a cooperative green task on a scheduler thread. For waiting-
  bound work (many idle connections). ~30 ns switch, no ~6k thread ceiling.

No function coloring, no `async`/`await`. A function is just a function; the
*call site* picks the executor. This is the firm #369 constraint.

## Surface

Symmetric with `spawn`, same `Task<T>` handle:

    let t = green handle(conn)      // start a green task
    let r = t.get()                 // await its result (see cross-tier await)
    green log_rotate().detach()     // fire-and-forget

Parsed like `spawn` (an executor flag on the spawn node), so eta-expansion,
closure capture, error propagation through `.get()`, and the `Task<T>` type all
reuse the existing machinery.

## Scheduler lifecycle (ceremony-free)

- The runtime owns the green scheduler(s); the user never creates one. v1: a
  single scheduler thread, started lazily on the first `green` call. Scaling
  step: N schedulers = N threads (thread-per-core), each with its OWN run queue.
- **Shared-nothing, no work-stealing in v1.** A green task stays on the
  scheduler it was created on. Simplest correct model; matches the shared-
  nothing semantics (#460/#473) and keeps DPOR tractable. Work-stealing is a
  later, measured optimization.
- Idle scheduler (no ready tasks) blocks in its event wait (future kqueue)
  inside a GC safe region, so STW still converges.
- Shutdown: program/app exit abandons unfinished green tasks (mirrors detached
  `spawn`); `.get()`/channel joins are how you wait for results you need.

## Cross-tier interactions (the semantic core)

**Await (`t.get()`):**
- From a green task awaiting a green task: cooperative — park the caller, resume
  when the callee finishes. No OS block.
- From a green task awaiting a `spawn` (pthread) task: park the green caller;
  the spawn's completion signals the scheduler to wake it (a pthread→scheduler
  wake).
- From main/pthread awaiting a green task: block the OS thread on a condvar; the
  scheduler runs the green task and signals on completion. (Scheduler is a
  separate thread, so this makes progress.)

**Channels are cross-tier.** One channel may have a green sender and a pthread
receiver (or vice versa). The channel records each waiter's *kind*; `send`/`recv`
wakes a green waiter via `green_wake` (onto its scheduler's ready queue) and a
pthread waiter via `cond_signal`. This unifies the production channel and the
prototype green channel into one implementation with a kind-tagged wait queue.
Value-copy semantics are unchanged (no shared mutable state crosses).

**Entities** cross as serialized handles exactly as today; an entity method
call from a green task takes the per-instance lock — on contention it *yields*
(cooperative) rather than OS-blocking, so entity calls are NOT green-illegal.

## Blocking-effect guardrail — refined leaf set

v1 rejects a `green` task that can reach an op which blocks the OS thread with
no cooperative/readiness form, naming the path (analysis already built + sound,
`src/typeck/blocking.rs`). **Important refinement the semantics force:** the
guardrail's leaf set is NARROWER than the full blocking surface. Ops that gain a
cooperative form are NOT guardrail leaves:

- NOT leaves (become scheduler yields): channel send/recv/recv_timeout, task
  `.get()`, entity/singleton lock waits, `select`.
- ARE leaves (no cooperative form in v1 → reject from green): fs open/read/
  write/sync, stdin (`io.read_line`), `time.sleep` without the timer wheel, and
  sockets until kqueue readiness lands (then they leave the set).

So `blocking.rs` needs a *purpose-scoped* leaf set: the current set is the
"stalls a scheduler" set; the green guardrail uses "stalls with no cooperative
form". Concretely: drop channel/task/lock/select from the guardrail leaves;
keep fs/stdin/sleep/socket. This is a small, well-scoped change to the leaf
constants + a flag on the inference.

## Test-mode / DPOR unification (a correctness gift)

The deterministic harness's fiber scheduler (`PLUTO_TEST_MODE`) IS a cooperative
green scheduler with DPOR strategies. Promoting green to production means test
mode and production share ONE model: green tasks run under DPOR in tests
(exploring interleavings, catching races) and under the asm-switch scheduler in
production — same semantics, different executor. Green-task concurrency is thus
deterministically testable from day one, which is the correctness backbone for
the whole tier.

## Build order from here

1. ✅ runtime core (switch, scheduler, park/wake, channel) + GC design, proven.
2. Parser/AST: `green` executor flag on the spawn node; typeck reuses `Task<T>`.
3. Refine `blocking.rs` leaf set for the green guardrail; enforce at `green`.
4. Codegen: lower `green f()` to scheduler enqueue; wire the scheduler + GC
   context registry (GC_INTEGRATION.md) into the runtime build.
5. Cross-tier channel (kind-tagged waiters); cross-tier await.
6. kqueue readiness (sockets/timers leave the leaf set); fs/stdin offload.

---

## Production-scheduler integration plan (the remaining phase)

The runtime core, GC registry, keyword, analysis and build-wiring are done. The
remaining work is one tightly-coupled chunk; this sequences it with the hazards
called out, so it is built carefully rather than dribbled.

### Scheduler lifecycle (v1)
- One global green scheduler, a pthread started **lazily** on the first `green`
  spawn (double-checked init under a mutex — the init race is the first hazard).
- A **thread-safe ready queue**: producers (`green f()` on any thread) lock,
  enqueue, signal a wake condvar; the scheduler pops under the lock. (The
  standalone prototype's lock-free queue assumed single-threaded; production is
  multi-producer, so the queue gains a mutex — distinct from the per-fiber
  cooperative switching, which stays lock-free *within* the scheduler thread.)
- Idle scheduler (empty queue) waits on the wake condvar **inside a GC safe
  region** (so STW converges) — not `green_run`'s "return on empty".

### Running a real Pluto closure
- A green task wraps a lifted Pluto closure `[fn_ptr, captures…]`. The fiber
  entry trampoline calls `fn_ptr(closure)` — the SAME ABI closure-lift already
  emits for `spawn`, so codegen reuses it. Result/error land in the task.

### Cross-tier await (`.get()`)
- Task handle layout matches today's `Task<T>` (result slot + error slot +
  `TaskSync{mutex,cond}`) so `.get()` codegen is unchanged. The scheduler
  signals that condvar on completion; a main/pthread `.get()` blocks on it; a
  green-task `.get()` parks on the scheduler instead (cooperative). Hazard:
  green-awaiting-green must park (not block the scheduler thread).

### GC integration (wiring the already-landed registry)
- The scheduler pthread registers as a GC thread (`__pluto_gc_register_thread_
  stack`) like spawn trampolines. Each fiber registers a green context
  (`__pluto_gc_register_green_context(stack_top, sp)`) on first run; updates
  `live_sp` on every switch-out; unregisters on finish.
- **Running-fiber-at-STW (the sharpest hazard):** when STW hits while a fiber
  runs, that fiber's live_sp must be the thread's current sp. Handle by having
  the scheduler thread's safepoint handler publish the running context's
  live_sp = current sp before it parks (reuse `gc_record_park`'s stack_cur).

### Test-mode routing
- Under `PLUTO_TEST_MODE`, `green` routes to the EXISTING DPOR fiber scheduler
  (threading.c test-mode path) — same cooperative model, with interleaving
  exploration. So codegen lowers `green` to a `__pluto_green_spawn` symbol that
  has a production impl (new scheduler) and a test-mode impl (DPOR fibers). This
  is the unification the RFC's "test-mode gift" promised.

### Codegen
- `Expr::Spawn { green: true }` lowers to `__pluto_green_spawn(closure)` instead
  of `__pluto_task_spawn`; everything else (closure lift, Task<T>, `.get()`)
  is shared with `spawn`. One new runtime symbol, two impls (prod / test).

### Checkpoints (each must pass before the next)
1. Production scheduler skeleton (thread + locked queue + lazy init), C-tested:
   spawn C tasks from a producer thread, await results. No GC/closures yet.
2. GC-context registration + a forced-collection C test: green tasks that
   `gc_alloc` survive a collection (the real scheduler+GC proof).
3. Codegen lowers `green` → `__pluto_green_spawn`; `green f()` runs on the
   scheduler (not a thread); green integration tests pass.
4. Cross-tier channel (kind-tagged waiters) + green-awaiting-green parking.
5. Guardrail enforcement at the `green` boundary (consume `green_illegal_fns`).
6. kqueue readiness (sockets/timers leave the leaf set) → fs/stdin offload.

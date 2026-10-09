# GC integration for the green scheduler (#369 Phase 4 design)

The load-bearing correctness item: real Pluto green tasks allocate, so the
mark-sweep collector must find roots in every fiber's stack and stop-the-world
must coordinate with the scheduler thread(s). This note is the design; the
soundness claim below is empirically validated (`green_gc_scan_test.c`).

## What the collector must scan

A scheduler thread multiplexes many fiber stacks, so the existing per-thread
`[stack_lo, stack_hi]` model (marksweep.c `GCThreadStack`) does not directly
fit. Instead, register each live green context:

    { stack_base, stack_top, live_sp, state }   // one per fiber + one per scheduler

- **Parked / ready fiber**: `live_sp = GTask.sp` (saved by `pluto_ctx_swap`).
  Scan `[live_sp, stack_top)`.
- **Running fiber** (the one executing when STW hits): its `live_sp` is the
  scheduler thread's *current* sp, captured when that thread parks at the
  safepoint (the collector already records this as `stack_cur` in
  `gc_record_park`). Scan `[stack_cur, running_fiber_top)`.
- **The scheduler's own C stack** is itself a context: when a fiber runs, the
  scheduler loop is suspended at its `ctx_swap` call with `live_sp = sched_sp`;
  scan `[sched_sp, scheduler_stack_top)`. When the scheduler loop runs (no
  fiber), it's the thread's current stack as usual.

## Soundness: why no register snapshot is needed (the key result)

A green task only ever switches out at a **cooperative** point — a call to
`green_yield` / `green_park` (reached through `gchan_recv`, a future
`green_spawn().get()`, etc.). At any *call*, the AAPCS/SysV ABI requires live
caller-saved values to already be spilled to the stack, and `pluto_ctx_swap`
pushes the callee-saved registers into `[live_sp, …)` itself. Therefore every
root a task holds live across a switch is within `[live_sp, stack_top)` — on
the stack above `live_sp`, or in the ctx_swap-saved callee register block
inside that range. **No separate `setjmp`/`park_regs` snapshot is required**,
unlike the preemptive pthread park path.

Validated in `green_gc_scan_test.c`: a heap pointer reachable *only* via a
fiber's argument (not reloadable from any global) is found by a conservative
scan of `[sp, stack_top)` after the fiber parks — at -O0, -O1 and -O2 (i.e.
whether the compiler kept it on the stack or in a callee-saved register).

Corollary / caveat: this property is **specific to cooperative switching**. A
preemptive green scheduler (timer-interrupt yields) would reintroduce the need
to snapshot all registers at the interrupt point. The #369 v1 is cooperative
(non-goal: preemptive green), so we get this simplification for free — and it
is a reason to keep green cooperative.

## Stop-the-world coordination

One scheduler == one pthread, already a registered GC thread, so STW already
stops it at a safepoint. Additions:

1. At a safepoint/safe-region park, besides recording its own `stack_cur`, a
   scheduler thread must make its green contexts visible to the collector (a
   per-scheduler context list, published under `gc_mutex`).
2. A scheduler blocked in its event-loop wait (future kqueue) enters a **safe
   region** exactly like any blocking wait today, so it counts as stopped.
3. Allocation inside a running fiber (`gc_alloc` on the scheduler thread) can
   itself trigger STW; the collector then scans all other threads + every
   green context, including its own running fiber (via `stack_cur`).

## Open items (tracked, not yet built)

- Per-scheduler context registry: dynamic (grow like `gc_thread_stacks`),
  published under `gc_mutex`; O(1) add/remove on spawn/finish.
- Green-spawn pending-root analog (a fiber handle is reachable only from the
  spawner's stack until enqueued) — mirror `gc_pending_roots`.
- Multiple schedulers (thread-per-core): each registers its own contexts; STW
  enumerates across all. Shared-nothing keeps this simple (no cross-scheduler
  stack references — values cross via channels by copy, entities by handle).
- Stack-size vs resident memory (one 16 KB page/fiber today): segmented or
  copying stacks would pack idle fibers sub-page — a later optimization.

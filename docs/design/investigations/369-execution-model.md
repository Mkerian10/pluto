# Execution model study: the two-tier green-scheduler hypothesis

Investigation of issue #369 — the thread-per-`spawn`/thread-per-connection
execution model and its scaling ceiling.

**Status:** investigation only. No runtime or compiler change is proposed here;
this doc exists to let the owner answer one question (§10). Measurements are from
one machine: **aarch64 macOS (M-series, 10 cores, 24 GB), debug-compiled Pluto
programs, default `ulimit` (`-u 4000`, `-s 8176 KB`, `-n 1048576`).** Treat the
absolute numbers as this-machine facts, not portable constants; the *shape* is
what generalizes.

---

## 0. TL;DR

- Today every unit of concurrency is an OS thread. Measured hard ceiling on this
  machine: **~6000–7000 live threads**, after which `pthread_create` fails with
  `EAGAIN` (errno 35). Resident cost **~17–19 KB/task** (and **512 KB reserved
  stack each**). A `spawn`-parked task and an idle TCP connection cost the same,
  because `serve` handlers use the identical pthread path.
- The runtime already contains a **single-threaded cooperative fiber scheduler**
  (test mode, for DPOR). The hypothesis is to **promote that execution model to
  production** as a second tier beside `spawn`.
- Sharing rules come for free: channels deep-copy (#460), entities serialize
  through their locks (#473). A green tier rides those unchanged — only the
  executor differs. DPOR-equivalence becomes a **gift**, not a constraint.
- The load-bearing design problem is the **blocking surface**: ~30 stdlib/runtime
  calls make a blocking syscall on the calling thread. On a green scheduler, one
  such call stalls *every* task on that scheduler.
- A **compiler-inferred "blocking effect"**, cloned almost verbatim from the
  existing error-ability inference, can make "green task reaches a blocking call"
  a **compile error in v1**. Feasibility: high (see §6). A dormant pre-wired hook
  (`program.fallible_extern_fns`) already exists to model the leaf sources on.

---

## 1. What the model is today

| Construct | Lowers to | Entry point |
|---|---|---|
| `spawn f(x)` | one detached `pthread` + GC registration | `__pluto_task_spawn` → `__pluto_spawn_trampoline`, `runtime/threading.c:1255/1191` |
| `serve` connection handler | one detached `pthread` per accepted conn | `__pluto_serve_handler_spawn` → `serve_handler_trampoline`, `runtime/threading.c:1175/1140` |
| test mode only | cooperative fibers (ucontext), DPOR-explored | `runtime/threading.c:43–1118` (`#ifdef PLUTO_TEST_MODE`) |

Neither production path sets a custom stack size, so each thread takes the macOS
default **512 KB** secondary-thread stack (the GC source states this outright,
`runtime/gc/marksweep.c:61`). Both register their stack with the GC's
stop-the-world thread registry exactly the same way — which is why they cost the
same (§2).

---

## 2. Measured ceiling (status quo)

Benchmarks: `docs/design/investigations/369-bench/{spawn,tcp}/main.pt`. Driver:
`spawn` N idle tasks (or hold N idle loopback connections), print spawn/connect
time, then sample RSS with `ps -o rss=`. Reproduction commands in §A.

### 2a. `spawn` N tasks parked on channel `recv`

| N | RSS (MB) | RSS/task (KB) | spawn time | status |
|---:|---:|---:|---:|---|
| 0 (baseline) | 1.2 | — | — | ok |
| 1 000 | 18.0 | 18.5 | 7 ms (~7 µs/task) | ok |
| 2 000 | 34.8 | 17.8 | — | ok |
| 3 000 | 51.7 | 17.6 | — | ok |
| 4 000 | 68.9 | 17.6 | — | ok |
| 5 000 | 84.7 | 17.3 | 176 ms (~35 µs/task) | ok |
| 6 000 | 101.8 | 17.3 | — | ok |
| 7 000 | — | — | — | **`pthread_create`: EAGAIN (errno 35)** |
| ≥ 10 000 | — | — | — | EAGAIN |

### 2b. Loopback TCP server holding N idle connections

| N | RSS (MB) | RSS/conn (KB) | connect time | status |
|---:|---:|---:|---:|---|
| 1 000 | 18.5 | 18.9 | 39 ms (~39 µs/conn) | ok |
| 2 000 | 35.0 | 17.9 | 64 ms | ok |
| 3 000 | 52.1 | 17.8 | 97 ms | ok |
| 5 000 | 93.8 | 19.2 | 231 ms (~46 µs/conn) | ok |

### Reading the numbers

- **Per-unit resident cost is ~17–19 KB**, flat in N — touched stack pages + the
  task/channel/entity objects + GC bookkeeping. The **512 KB reserved** stack is
  virtual, not resident, but it is what the kernel accounts against the
  thread-count limit.
- **Connections cost the same as spawns** (18.9 vs 18.5 KB at N=1000),
  empirically confirming they are the same pthread mechanism.
- **The ceiling is thread *count*, not memory.** At the ~6–7k wall we are using
  ~100 MB of 24 GB — RAM is nowhere near exhausted. `EAGAIN` is the per-process
  kernel thread limit (interacting with `ulimit -u`). Raising limits buys maybe
  one order of magnitude, never the ~10⁵–10⁶ idle connections the issue's
  Kafka-broker case wants.
- **spawn/connect latency is ~7–46 µs and rises with N** (thread creation + GC
  registration + scheduler pressure). At 5k it is already 0.18–0.23 s of pure
  setup.

**Verdict:** the ceiling is real, it is low-thousands, and it is a count limit no
amount of RAM fixes. It does not block a tens-to-hundreds-connection v1, exactly
as the issue says.

---

## 3. Fiber vs pthread cost

The test-mode scheduler is the existence proof that Pluto can run cooperative
green tasks today.

| | pthread (prod) | fiber (test mode) |
|---|---|---|
| Stack | 512 KB reserved, ~17 KB resident | `FIBER_STACK_SIZE` = **64 KB** fixed (`threading.c:47`) |
| Create | `pthread_create` ~7–46 µs, **syscall + kernel thread** | `makecontext` + `malloc` — user space, no syscall |
| Switch | kernel context switch ~1–5 µs | `swapcontext` — user-space register save/restore, sub-µs |
| Ceiling | ~6–7k (kernel thread limit) | heap-bound (test cap `MAX_FIBERS` = 256 is a DPOR limit, not a memory one) |

The memory/ceiling win is **10×–100×+**: a green task needs no kernel thread and
no 512 KB reservation, so the limit moves from a ~6k kernel wall to "how much
heap for stacks." A right-sized or growable green stack (the 64 KB figure is a
fixed test allocation, not a floor) pushes per-task cost toward single-digit KB.
The raw *switch* speedup is real but secondary — the point is removing the kernel
thread, not shaving microseconds.

Each green **scheduler** is one pthread running its tasks single-threaded
(cooperative); M schedulers = M pthreads (Erlang's N-schedulers model;
Seastar/glommio thread-per-core). Cross-scheduler sharing is shared-nothing *by
construction* because it reuses existing semantics — channel values deep-copy
(#460), entities serialize through their per-instance locks (#473). The green
tier's sharing rules are therefore **identical to `spawn`'s**; only the executor
differs.

---

## 4. GC: what a green runtime needs (the load-bearing GC question)

Production GC is a stop-the-world mark-sweep with a **dynamic thread registry**
and a **safe-region protocol** (`runtime/gc/marksweep.c:190–345`):

- Every pthread registers `[stack_lo, stack_hi)`; the collector parks all threads
  at a safepoint (`__pluto_safepoint`) or counts them as parked while they sit in
  a **safe region** (`__pluto_gc_enter/leave_safe_region`) — the bracket already
  wrapped around every blocking syscall (fs, socket, `nanosleep`, cond waits).
  The closing condition is `stopped + safe ≥ registered`.
- Test mode has a **separate** fiber-stack registry
  (`__pluto_gc_register_fiber_stack`, `marksweep.c:102–181`): a fixed **256-slot**,
  **run-scoped** array; the collector conservatively scans each live fiber stack
  plus a scheduler root region holding suspended fibers' saved registers
  (`ucontext_t`). The adaptive GC floor **already** scales its budget per
  registered thread *and names fibers* (`GC_FLOOR_PER_THREAD`, comment at
  `marksweep.c:54–63`, explicitly costing "a 1000-idle-task server").

What a production green runtime needs from the GC — all **generalizations of
code that already exists**, not new mechanisms:

1. A green **scheduler pthread** registers in the normal thread registry (it *is*
   a pthread) — STW handshake works unchanged.
2. The **per-scheduler set of suspended green-task stacks** must be scannable:
   promote the test-mode fiber-stack registry from a fixed 256-slot run-scoped
   array to a **dynamic, per-scheduler, lifetime-correct** structure, and fold it
   into STW so the collector scans every scheduler's green stacks + their saved
   register snapshots.
3. A scheduler blocked in its event-loop wait (kqueue) enters a **safe region** —
   the existing protocol already covers "thread blocked in a syscall."

**This is the single biggest implementation item, but it is incremental, not
greenfield.** And because the test-mode model and the production green model
would be the *same* execution model, DPOR results transfer directly — the
"must stay semantically equivalent" worry from the issue becomes free.

---

## 5. Blocking-surface inventory (the key deliverable)

Every stdlib/runtime call that blocks the calling OS thread. On a green scheduler,
each of these stalls the whole scheduler unless made nonblocking+yield. Line
refs are `runtime/builtins.c` / `runtime/threading.c`.

| Surface | C entry (file:line) | Blocking primitive | Nonblock+yield feasible? |
|---|---|---|---|
| fs open | `__pluto_fs_open_*` builtins.c:2577–2598 | `open(2)` | Hard — no portable async `open`; needs a thread-pool offload |
| fs read/write | `__pluto_fs_read`/`_write` builtins.c:2614/2643 (+`pread`/`pwrite` 2710/2760) | `read`/`write`/`pread`/`pwrite` | Disk fds aren't kqueue-readiness-pollable → **thread-pool offload** |
| fs sync | `__pluto_fs_sync*` builtins.c:3059–3098 | `F_FULLFSYNC` / `fsync` / `fdatasync` | No async form; offload (and see §7 group-commit) |
| fs bulk/copy | `__pluto_fs_read_all`/`write_all`/`copy` builtins.c:~3235/3296/3610 | `open`+`read`/`write` loops | Offload |
| socket accept | `__pluto_socket_accept` builtins.c:1513 | `accept(2)` | **Yes** — kqueue `EVFILT_READ` on listener, then `accept` |
| socket connect | `__pluto_socket_connect` builtins.c:1522 | `connect(2)` | **Yes** — `O_NONBLOCK` + `EVFILT_WRITE` readiness |
| socket read | `__pluto_socket_read`/`_bytes` builtins.c:1555/1590 | `read(2)` | **Yes** — kqueue readiness |
| socket write | `__pluto_socket_write`/`_bytes` builtins.c:1579/1615 | `write(2)` | **Yes** — `EVFILT_WRITE` on `EAGAIN` |
| framed RPC xport | `__pluto_write_framed`/`_read_framed` builtins.c:1648/1682 | `read`/`write` loops | Yes (built on socket) |
| channel send | `__pluto_chan_send` threading.c:2095 | `pthread_cond_wait` | **Trivial** — becomes a scheduler yield (exactly what test mode does, threading.c:1805) |
| channel recv | `__pluto_chan_recv` threading.c:2123 | `pthread_cond_wait` | **Trivial** — scheduler yield |
| channel recv-timeout | `__pluto_chan_recv_timeout` threading.c:2161 | `pthread_cond_timedwait` | Yield + timer wheel |
| select | `__pluto_select` threading.c:2468 | **spin + `usleep`** (not a syscall block) | Yield; already poll-shaped |
| task `.get()` join | `__pluto_task_get` threading.c:1286 | `pthread_cond_timedwait` | Yield until task done |
| entity rwlock | `__pluto_rwlock_rd/wrlock` threading.c:2678/2693 | `pthread_cond_wait` | Yield on contended lock |
| `time.sleep` | `__pluto_time_sleep_ns` builtins.c:1411 | `nanosleep` (test mode: yield, 1418) | **Yes** — timer wheel + yield |
| http read req | `__pluto_http_read_request` builtins.c:3950 | `read(2)` loop | Yes (socket); note: not GC-safe-region-wrapped today |
| io.read_line | `__pluto_io_read_line` builtins.c:177 | `getline`→`read(2)` on stdin | tty readiness or offload |
| rpc.call | `__pluto_http_post` threading.c:3061 | **none — stub returns dummy JSON** | n/a (no real transport yet) |

Audit result (both C files swept): **the runtime does no readiness/nonblocking
I/O anywhere** — no kqueue/epoll/poll/`select(2)`, no `O_NONBLOCK`. The only
"poll" is the channel `select` spin-loop. So a green scheduler's event loop is
net-new infrastructure, but the surface it must cover is small and well-bounded.

Two natural tiers fall out: **socket + channel + time + task-join + locks** map
cleanly to kqueue-readiness / scheduler-yield (the common server path); **fs +
stdin** have no async syscall form and need a small **thread-pool offload** (the
scheduler hands the blocking op to a worker pthread and yields). A v1 can ship
the first tier and reject the second from green tasks (§6).

---

## 6. Blocking-effect inference (compile-time rejection in v1)

**Claim:** the compiler can infer a "blocking effect" exactly like it infers
error-ability, and make a green task that reaches a blocking call a **compile
error**. Assessed against the real error-inference code (`src/typeck/errors.rs`):

| Error-ability mechanism | file:line | Reusable for a blocking effect? |
|---|---|---|
| Graph = `direct` map + `propagation_edges` map | errors.rs:9–12 | Verbatim — same string-keyed node graph |
| Fixed point (union callees until stable) | errors.rs:108–133 | Verbatim — second map, second fixpoint |
| Closure nodes (`<closure@span>`) | errors.rs:281–357 | Verbatim |
| Function-reference edges | errors.rs:778–784 | Verbatim |
| Generics via template-name collection + instance→template bridge + copy | errors.rs:14–106, 177–225 | Verbatim; same pre-monomorphization skolem placement (mod.rs:204–220) |
| Leaf "source" from `raise` | errors.rs:398 | Analog: leaf = a blocking C intrinsic |
| Leaf source from resolved builtins (`ChannelSend`→`ChannelClosed`, etc.) | errors.rs:583–650 | Analog: mark the blocking `MethodResolution`s |
| **FFI leaf seed: `program.fallible_extern_fns` → `env.fn_errors`** | mod.rs:194–200; field `ast.rs:21` | **Dormant pre-wired hook** — currently never populated. A `blocking_extern_fns` sibling seeds the blocking leaves identically |

So inference itself is **clone-level effort**: a second string-keyed graph, a
second union fixpoint, a leaf set listing the §5 C entry points, and a boundary
check mirroring `enforce_error_handling`. Generic functions, closures, and fn-refs
are all already handled by the pattern being cloned.

**One real gap — the named path in the diagnostic.** Today `fn_errors` stores only
`HashSet<String>` with **no provenance**; enforcement names only the *immediate*
callee ("call to fallible function 'X'…", errors.rs:1136), never a chain. The
"this green task blocks because it calls X → Y → `socket.read`" path the brief
asks for is **net-new**: either change the fixpoint value to carry a predecessor
edge, or reconstruct the path with a reverse BFS over `propagation_edges` at
diagnostic time. Modest, well-scoped work — but not free like the rest.

**Verdict: feasible, and the cheap part is the bulk of it.** A v1 marks the §5
leaves as blocking, runs the fixpoint, and rejects any blocking call reachable
from a green-task body (fs/stdin included) with a named path. Later versions
*remove* leaves from the set as each gets a nonblocking+yield form — the
diagnostic shrinks as the runtime grows, no language change per step.

---

## 7. Batching (assess, not commit)

A single-threaded scheduler batches naturally per tick: drain the kqueue
ready-set in one `kevent` call, submit accumulated I/O together — adaptive, with
zero latency tax (batch size = whatever arrived; an idle scheduler still wakes
immediately). The real wins, in order:

1. **Group-commit of `fsync`** (ms-class) — the big one. Belongs as a **`std.wal`
   API** (`stdlib/wal` already exists), *not* a language construct: callers opt a
   batch of writes into one durability barrier.
2. **Syscall amortization** — one `kevent` for N ready fds instead of N blocking
   reads.
3. **I-cache locality** — the same handler code over N connections per tick.

Context-switch savings are *not* a real win here — once tasks are green, switches
are already cheap. **Open horizons, explicitly not designed here:** an explicit
`[Request]` batch-handler `serve` mode, and unifying `Stream` (the type exists)
with batch delivery. Flagged so they aren't reinvented; out of scope.

---

## 8. Options, costed against the hypothesis

| | (a) Status quo + raised limits | (b) **Two-tier green schedulers** (the hypothesis) | (c) Transparent thread-pool interception |
|---|---|---|---|
| Shape | keep pthread-per-unit; raise `ulimit`/sysctl | `spawn` = pthread; new green tier on single-threaded schedulers; block = compile error in v1 | keep blocking API; a pool of worker threads + work-steal behind the scenes |
| Ceiling | low-10ks at best, still count-bound | ~10⁵–10⁶ green tasks (heap-bound) | pool-size-bound; blocked tasks still pin a pool thread |
| GC impact | none | generalize fiber-stack scan to dynamic per-scheduler + STW (§4) | pool threads register like spawns; modest |
| Effort class | trivial | **large** (scheduler + event loop + green stacks + effect inference) | medium–large (hidden scheduler, but no language story) |
| Stdlib contracts changed | none | blocking calls gain a nonblocking+yield impl incrementally; v1 rejects the rest from green | blocking calls silently offloaded — semantics muddied |
| DPOR | unchanged | **production == test model** → results transfer | diverges from test model — DPOR weakens |
| What breaks | nothing | nothing silently — effect checker is the guardrail | blocking-in-pool starvation becomes invisible/implicit |
| Fit to pitch | fails the Kafka-broker case | **matches it** | works but hides the model (against temperament) |

Option (c) is rejected on temperament as much as cost: it makes the concurrency
model *implicit*, and Pluto keeps distribution and effects *explicit*. Option (a)
is the honest short-term stopgap (and worth doing regardless, as a one-line
limits bump). Option (b) is the only one that reaches the target scale and keeps
the model visible and DPOR-checkable.

---

## 9. Residual footguns

- **Cooperative starvation.** A green task that spins on CPU without hitting a
  yield point starves its scheduler's siblings. There is **no static fix** (the
  halting problem in a hat). The escape hatch is the two-tier design itself:
  CPU-bound work goes on `spawn` (a real pthread, preemptive), not on a green
  task. Documentation + the effect checker nudging CPU loops toward `spawn` is
  the mitigation; preemptive green scheduling (signal-based) is a deliberate
  non-goal for v1.
- **Visible async / function coloring is off the table** by design temperament —
  the blocking effect is an *inference + rejection*, not an `async`/`await`
  keyword pair the user threads through signatures. Green and normal code look
  identical; the checker, not the syntax, enforces the boundary.

---

## 10. Recommendation — the one question

Everything above reduces to a single decision for the owner:

> **Adopt the two-tier green-scheduler model — `spawn` stays a pthread, a new
> green-task construct runs on single-threaded shared-nothing schedulers, and a
> green task reaching a blocking call is a compile error in v1 — as the
> production concurrency direction? (yes / no)**

If **yes**, the natural first slices are: (1) a one-line `ulimit`/sysctl bump as
the interim stopgap (option a, independent of everything else); (2) the
blocking-effect inference (§6, cheap, and useful as documentation even before a
scheduler exists); (3) the green scheduler + kqueue event loop over the §5 socket
tier; (4) the GC generalization (§4). The fs/stdin thread-pool offload and the
batching/`[Request]`/`Stream` horizons (§7) come later and need their own RFCs.

If **no**, the ceiling stands as a documented constraint and option (a) is the
only lever.

---

## Appendix A — Reproducing the benchmarks

Debug-build caveat: the Pluto compiler here is `cargo build` (debug); there is no
release flag for the emitted program in this harness. RSS is dominated by thread
stacks and runtime objects, so debug vs release barely moves it; spawn/connect
*latency* would improve somewhat under optimization. Numbers are directional.

```bash
# from the worktree root
cargo build                                  # debug compiler at target/debug/pluto

# spawn ceiling (edit `let N` in the .pt, or script a sweep)
target/debug/pluto compile \
  docs/design/investigations/369-bench/spawn/main.pt -o /tmp/spawn --stdlib stdlib
/tmp/spawn & PID=$!
# wait for the "ready" line, then:
ps -o rss=,vsz= -p $PID      # RSS KB ÷ N = per-task cost
kill $PID

# idle TCP connections
target/debug/pluto compile \
  docs/design/investigations/369-bench/tcp/main.pt -o /tmp/tcp --stdlib stdlib
/tmp/tcp & PID=$!
ps -o rss= -p $PID           # RSS KB ÷ N = per-connection cost
kill $PID
```

A failed run prints `pluto: failed to create thread: 35` (EAGAIN) — that line
marks the ceiling. The `.pt` files carry per-file N and the same instructions.
Measured on aarch64 macOS, 10 cores / 24 GB, `ulimit -u 4000`.

*Part of #369.*

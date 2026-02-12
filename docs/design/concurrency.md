# Concurrency

## Overview

Pluto provides two concurrency primitives. The programmer chooses which to use:

- **Tasks** — lightweight, runtime-managed green threads. M:N scheduled on a thread pool. Best for I/O-bound work, fan-out, and concurrent operations.
- **Threads** — OS-level threads. 1:1 with kernel threads. Best for CPU-bound work, FFI, and when you need real parallelism guarantees.

Both follow the same error contract: spawned work preserves the error type of the spawned function, and waiting for a result is fallible.

## Tasks

### Spawning

`spawn` creates a task and returns a handle:

```
let task = spawn fetch(url)
let result = task.get()!
```

`spawn` takes a function call and schedules it as a lightweight task on the runtime's thread pool. It returns a `Task<T>` handle immediately without blocking.

### Fan-Out

```
let tasks = urls.map((url: string) => spawn fetch(url))
let results = tasks.map((t: Task<string>) => t.get()!)
```

### Task<T>

`Task<T>` is the handle to a running task. Key operations:

| Method | Description |
|--------|-------------|
| `.get()` | Block until the task completes, return result. Fallible. |
| `.cancel()` | Request cooperative cancellation. |
| `.detach()` | Release the handle; task runs in the background. |

### Error Propagation

Error types flow through `spawn` transparently. The compiler infers the error set of the spawned function and preserves it through `.get()`:

```
// fetch() can raise NetworkError
let task = spawn fetch(url)

// task.get() can raise NetworkError | TaskCancelled
let result = task.get()!
```

Rules:

1. If the spawned function can raise errors `E`, then `.get()` can raise `E | TaskCancelled`.
2. If the spawned function is infallible, `.get()` can only raise `TaskCancelled`.
3. The compiler infers all of this — no manual annotation needed.

`TaskCancelled` is a built-in error type:

```
error TaskCancelled {}
```

### Cancellation

Cancellation is cooperative. Calling `task.cancel()` sets a cancellation flag. The task terminates at the next cancellation checkpoint:

- I/O operations (read, write, network calls)
- Channel operations (send, receive)
- Explicit `Task.check_cancelled()` calls

When a cancelled task terminates, `.get()` raises `TaskCancelled`:

```
let task = spawn long_running_job()
task.cancel()

let result = task.get() catch err {
    // err is TaskCancelled
    fallback_value
}
```

### Structured Concurrency

Tasks are structured by default: a `Task<T>` handle must be consumed. If a task handle is dropped without calling `.get()` or `.detach()`, the compiler emits an error (similar to Rust's `#[must_use]`).

```
// OK — result consumed
let task = spawn work()
let result = task.get()!

// OK — explicitly detached
let task = spawn background_work()
task.detach()

// COMPILE ERROR — task handle dropped without .get() or .detach()
spawn work()
```

This prevents accidentally ignoring task results or errors.

### Detach

`.detach()` releases the handle and lets the task run in the background. Once detached:

- The task runs to completion (or until cancelled by the runtime)
- Errors are unrecoverable — they go to the runtime's error handler
- There is no way to get the result

Detach is the escape hatch for fire-and-forget work:

```
let task = spawn emit_metrics()
task.detach()
```

## Threads

### Spawning

`Thread.spawn()` creates an OS-level thread:

```
let thread = Thread.spawn(() => cpu_heavy_work())
let result = thread.join()!
```

### Thread<T>

`Thread<T>` is the handle to an OS thread. Operations:

| Method | Description |
|--------|-------------|
| `.join()` | Block until the thread completes, return result. Fallible. |
| `.cancel()` | Request cooperative cancellation (same mechanism as tasks). |

Threads cannot be detached — they must always be joined. This prevents resource leaks at the OS level.

### Error Propagation

Same rules as tasks:

```
// heavy_work() can raise ComputeError
let thread = Thread.spawn(() => heavy_work())

// thread.join() can raise ComputeError | TaskCancelled
let result = thread.join()!
```

### When to Use Threads

Use threads when:

- Work is CPU-bound and benefits from true parallelism
- Calling FFI code that may block the OS thread
- You need deterministic scheduling (no green thread preemption)

Use tasks for everything else. Tasks are cheaper (thousands of them are fine) and integrate with the runtime's I/O scheduler.

## No Shared Mutable State

Tasks and threads communicate through **channels** and **ownership transfer** — not shared mutable references.

`mut self` alone is not sufficient for race safety without ownership or borrow tracking. Instead, Pluto enforces isolation:

- Values passed to `spawn` are moved into the task. Using them after spawn is a compile error.
- Shared data flows through channels (see [Communication](communication.md)).
- Immutable data can be freely shared (no mutation, no races).

```
let data = load_data()

// data is moved into the task — cannot be used after this line
let task = spawn process(data)

// COMPILE ERROR — data was moved
print(data.len())
```

### Channels for Communication

Channels (designed in [Communication](communication.md)) are the primary mechanism for inter-task communication:

```
let (tx, rx) = chan<Order>()

let producer = spawn {
    for order in get_orders() {
        tx <- order
    }
}

let consumer = spawn {
    for order in rx {
        process(order)
    }
}

producer.get()!
consumer.get()!
```

## Compiler Leverage

Whole-program compilation enables optimizations invisible to the programmer:

- **Task scheduling:** The runtime uses work-stealing across a thread pool sized to the CPU core count.
- **I/O integration:** Tasks waiting on I/O are parked without consuming a thread (epoll/kqueue under the hood).
- **Cancellation propagation:** When a parent task is cancelled, the runtime can propagate cancellation to child tasks.
- **Dead task detection:** The compiler can warn about tasks that are spawned but whose results are never used (without `.detach()`).

## Examples

### Parallel HTTP Requests

```
fn fetch_all(urls: [string]) [string] {
    let tasks = urls.map((url: string) => spawn http_get(url))
    return tasks.map((t: Task<string>) => t.get()!)
}
```

### Producer-Consumer

```
fn pipeline(items: [Order]) {
    let (tx, rx) = chan<Order>(buffer: 100)

    let producer = spawn {
        for item in items {
            tx <- item
        }
        tx.close()
    }

    let consumer = spawn {
        for order in rx {
            process(order)!
        }
    }

    producer.get()!
    consumer.get()!
}
```

### CPU-Bound Parallelism

```
fn parallel_compute(chunks: [Data]) [Result] {
    let threads = chunks.map((chunk: Data) => {
        Thread.spawn(() => heavy_computation(chunk))
    })
    return threads.map((t: Thread<Result>) => t.join()!)
}
```

### Timeout Pattern

```
fn fetch_with_timeout(url: string) string {
    let task = spawn fetch(url)

    let timer = spawn {
        sleep(5000)
        task.cancel()
    }

    let result = task.get() catch err {
        timer.cancel()
        raise TimeoutError { url: url }
    }

    timer.cancel()
    return result
}
```

## Open Questions

- [ ] **Move semantics details** — how does move-on-spawn interact with closures that capture by value? Do we need explicit `move` annotations?
- [ ] **Structured concurrency scope** — should there be a `task_group` or `scope` construct for managing multiple tasks with automatic cancellation?
- [ ] **Thread pool configuration** — runtime-level or compile-time config for thread pool size?
- [ ] **Task priority** — should tasks have priority levels?
- [ ] **Select/race** — a `select` construct for waiting on the first of multiple tasks/channels to complete
- [ ] **Spawn block syntax** — should `spawn { ... }` work (spawn an anonymous block) in addition to `spawn func()`?

## Phase 1 Implementation Notes

Phase 1 implements `spawn func(args)`, `Task<T>`, and `.get()` with OS threads (pthreads). The full design above (M:N tasks, structured concurrency, cancellation, detach, channels, move semantics) is future work.

### What's implemented

- `spawn func(args)` — direct function calls only (no method calls, no closures, no `spawn { block }`)
- `Task<T>` — built-in type, GC-allocated handle
- `.get()` — blocks until task completes, returns result or propagates error
- Error propagation: `.get()!` propagates, `.get() catch val` handles. Compiler infers fallibility from the spawned function.
- Spawn arg restrictions: no `!` propagation in args, no bare fallible calls in args. Users must evaluate fallible args before spawn.

### Not yet implemented

- `.cancel()`, `.detach()`, `TaskCancelled` error type
- Structured concurrency (must-use handles)
- `Thread.spawn()` (OS thread API)
- Move semantics / ownership transfer
- Channels
- M:N task scheduler

### GC suppression tradeoff

GC collection is suppressed while any spawned task is active (`atomic_load(&__pluto_active_tasks) > 0`). This prevents the GC from scanning only the main thread's stack while worker threads hold live references. Consequence: long-running or stuck tasks cause unbounded heap growth (capped at 1 GB, then fail-fast abort). Phase 2 will address this with per-thread root registration or a concurrent GC.

### Shared mutable state (data race risk)

Spawn captures variables by value, which for heap types (arrays, maps, sets, classes) means copying the pointer. Both the spawning function and the spawned task share the same underlying heap object. Runtime mutators (`array.push`, `map.insert`, field assignment, etc.) are not thread-safe. Mutating shared heap objects from multiple threads is undefined behavior. This is the programmer's responsibility in phase 1. Phase 2 will add move semantics or deep-copy.

### Conservative fallibility

When the compiler cannot statically determine which function was spawned (aliased/reassigned task handles, non-identifier `.get()` targets), `.get()` is treated as conservatively fallible. Users must handle with `!` or `catch` even if the underlying function is infallible.

## Runtime Implementation

The concurrency runtime is implemented in `runtime/threading.c` (2056 lines), which is compiled separately from the GC and core builtins. See `docs/design/runtime-architecture.md` for the full runtime module structure.

### Dual-Mode Runtime

The threading runtime operates in two modes:

**Test mode** (`-DPLUTO_TEST_MODE`):
- Cooperative fiber scheduler using `setcontext`/`swapcontext`
- Exhaustive DPOR (Dynamic Partial Order Reduction) state exploration
- Deterministic execution order for reproducible tests
- 256 fibers max, 64 KB stack per fiber
- Channels use fiber queues (no mutexes)
- All concurrency integration tests run in test mode

**Production mode** (default):
- Pthread-based tasks (`pthread_create`)
- Mutex-protected channels with condition variables
- Real OS-level parallelism
- Thread-local storage for error state and task handles
- Per-thread stack registration with GC for root scanning

### Task Implementation

Tasks are GC-allocated handles with the following layout:

```c
// GC_TAG_TASK = 7
long task[7] = {
    [0] closure_ptr,   // spawned function + captures
    [1] result,        // return value (after completion)
    [2] error,         // error object (if function raised)
    [3] done,          // 0=running, 1=completed
    [4] sync_ptr,      // TaskSync* (mutex+cond in production, fiber_id in test)
    [5] detached,      // 0=joinable, 1=detached
    [6] cancelled      // 0=running, 1=cancelled (future)
};
```

**Test mode:** `sync_ptr` stores fiber ID for cross-referencing.

**Production mode:** `sync_ptr` points to malloc'd `pthread_mutex_t` and `pthread_cond_t` for signaling completion.

### Channel Implementation

Channels are GC-allocated handles with the following layout:

```c
// GC_TAG_CHANNEL = 9
long channel[7] = {
    [0] sync_ptr,   // ChannelSync* (mutex+2 conds in production, NULL in test)
    [1] buf_ptr,    // circular buffer for values
    [2] capacity,   // max buffered items
    [3] count,      // current number of items
    [4] head,       // read index
    [5] tail,       // write index
    [6] closed      // 0=open, 1=closed
};
```

**Test mode:** Channels block/unblock fibers directly via scheduler state.

**Production mode:** Channels use `pthread_cond_wait`/`pthread_cond_signal` on `not_empty` and `not_full` condition variables.

### Select Implementation

`select` waits on multiple channels by polling them in random order (to avoid bias). Test mode tries channels in exhaustive permutations for DPOR. Production mode randomizes the order each iteration.

### Deep Copy for Spawn Isolation

`__pluto_deep_copy(value, type_id)` recursively copies heap objects:
- **Primitives** (int, float, bool): copied by value (no-op, just return)
- **Strings**: allocate new GC string, copy bytes
- **Arrays**: recursively copy each element
- **Maps/Sets**: copy key-value pairs and metadata
- **Classes**: copy field-by-field
- **Enums**: copy discriminant and data variant recursively
- **Tasks/Channels**: **error** (cannot deep copy concurrency primitives)

Deep copy ensures value isolation between spawner and task, preventing data races on mutable heap objects.

### GC Integration

**Production mode:** Spawned tasks register their thread stacks with the GC via `__pluto_gc_register_thread_stack(stack_lo, stack_hi)`. The GC scans all registered thread stacks as roots during collection. When a task completes, it deregisters via `__pluto_gc_deregister_thread_stack()`.

**Test mode:** Fibers register their stacks with `__pluto_gc_register_fiber_stack(base, size)`. The GC scans active fiber stacks based on the scheduler's current state.

**GC suppression:** In phase 1, the GC is suppressed while any tasks are active (`atomic_int __pluto_active_tasks > 0`). This prevents incomplete stack scanning but causes unbounded heap growth during long-running tasks. Phase 2 will remove this suppression by completing per-thread root registration.

### Rwlocks for Contract Synchronization

Classes with invariants use rwlocks to ensure thread-safe contract enforcement:
- Non-mut methods acquire **read locks** (shared, multiple readers allowed)
- Mut methods acquire **write locks** (exclusive, blocks all readers and writers)
- Invariants are checked after releasing write locks

Rwlocks are only used for classes with `invariant` clauses. Regular classes have no synchronization overhead.

See `docs/design/contracts.md` for contract enforcement details.

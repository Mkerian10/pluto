# RFC: Concurrency Model v2

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-10

## Summary

Redesign Pluto's concurrency model to work coherently across single-machine threads, multi-pod distribution, and shared state. The core insight: **`mut self` is the universal lever**. It determines what can be shared freely, what must be copied, what must be exclusive, and what gets replicated across pods. Everything flows from one distinction: does this operation mutate?

## Motivation

The Phase 1 concurrency implementation (`spawn`, `Task<T>`, `.get()`) works but has fundamental gaps:

1. **Data races.** Spawn copies pointers for heap types. Two threads can mutate the same object with no synchronization. This is UB today.
2. **No shared state primitive.** Real systems need shared caches, configuration, metrics. The only option today is "don't do that" or channels.
3. **Single-machine and distributed are disconnected.** The concurrency model (threads) and the distribution model (cross-pod RPC, channels) were designed separately. They should be one coherent model.
4. **`mut self` exists in spec but isn't implemented.** The mutability doc already says "mutable methods require exclusive access — the compiler enforces this." But the compiler doesn't enforce it yet. Everything downstream is blocked.

This RFC unifies concurrency, distribution, and shared state around explicit mutability.

## Motivating Example

A service registry that syncs with an external discovery service, serves lookups to concurrent request handlers, and needs zero concurrency annotations:

```
class ServiceRegistry[discovery: DiscoveryClient] {
    services: Map<string, ServiceEndpoint>

    fn lookup(self, name: string) ServiceEndpoint? {
        return self.services[name]
    }

    fn refresh(mut self) {
        let latest = self.discovery.fetch_all()!
        self.services = latest
    }

    fn start_sync(mut self) {
        while true {
            self.refresh() catch err {
                print("sync failed: {err}")
            }
            sleep(30000)
        }
    }
}

app MyApp[registry: ServiceRegistry, handler: RequestHandler] {
    fn main(self) {
        // Background sync task
        let sync = spawn self.registry.start_sync()
        sync.detach()

        // Concurrent request handling
        for conn in listen(8080) {
            spawn self.handler.handle(conn)
        }
    }
}
```

`ServiceRegistry` is a regular class. No `shared` keyword, no lock annotations, no special treatment. The compiler sees that `lookup()` is called from request handler tasks and `start_sync()` runs in a background task — both hitting the same singleton concurrently. It auto-wraps `self` methods with reader locks and `mut self` methods with writer locks. Hundreds of concurrent lookups proceed unblocked. The 30-second refresh briefly takes exclusive access.

DI provides `DiscoveryClient` — swap it for a mock in tests. The sync logic lives with the data it manages, not in a separate service. In a distributed deployment, the orchestration layer could mark `ServiceRegistry` as replicated, and `refresh()` on one pod propagates to all pods. Same code.

## Design Principles

1. **One rule, applied everywhere.** `self` methods are safe to call concurrently. `mut self` methods require exclusive access. This holds for threads, pods, and shared state.
2. **Copy on spawn.** Values passed to spawned tasks are deep-copied. The spawned task gets its own world. No aliasing, no races, full mutability.
3. **Shared state is inferred, not declared.** The compiler already knows the DI graph, which methods mutate, and which code runs concurrently. It adds synchronization automatically where needed. No `shared` keyword. No annotations.
4. **Same semantics at every scale.** A singleton accessed from multiple threads on one machine behaves the same as one accessed from multiple pods. The runtime handles the difference.

---

## Part 1: `mut self` Tracking

### What Changes

The compiler enforces what the spec already describes:

```
class Counter {
    count: int

    fn get(self) int {
        return self.count           // OK: read in non-mut method
    }

    fn increment(mut self) {
        self.count = self.count + 1 // OK: mutation in mut method
    }

    fn broken(self) {
        self.count = self.count + 1 // COMPILE ERROR: mutation without mut self
    }
}
```

### `let mut` Enforcement

```
let c = Counter { count: 0 }
c.get()                              // OK: non-mut method on immutable binding
c.increment()                        // COMPILE ERROR: mut method on immutable binding
print(c.count)                       // OK: field read always allowed

let mut c = Counter { count: 0 }
c.increment()                        // OK
```

### Why This Comes First

Everything in this RFC depends on the compiler knowing which methods mutate. Without `mut self` tracking:
- Can't enforce copy-on-spawn correctly (don't know what's safe to share)
- Can't implement shared state (don't know which operations need synchronization)
- Can't narrow invariant checks (contracts Phase 4+)
- Can't optimize channels (share immutable data, copy mutable data)

This is the foundation.

---

## Part 2: Copy on Spawn

### The Rule

When you `spawn func(args)`, every argument is **deep-copied** into the spawned task. The task gets its own independent copy. The caller retains the original, unaffected.

```
let mut data = [1, 2, 3]
let task = spawn process(data)   // data is deep-copied

data.push(4)                     // fine — caller's copy
let result = task.get()!         // task worked on its own copy
```

### Why Copy, Not Move

- **Pluto is garbage-collected.** Move semantics exist to solve "who frees this?" — a problem Pluto doesn't have.
- **No new concepts.** Copy-on-spawn doesn't add ownership, borrowing, or use-after-move to the language.
- **Matches distribution.** Cross-pod RPC serializes data anyway. Copy-on-spawn gives single-machine the same semantics: spawning is like sending data somewhere else.
- **Full mutability.** Both sides can freely mutate their copy. No restrictions, no read-only views.

### What Gets Copied

| Type | Copy behavior |
|------|--------------|
| `int`, `float`, `bool`, `byte` | Value copy (already happens today) |
| `string` | Pointer copy (strings are immutable in the runtime) |
| Class instances | Deep copy — allocate new object, recursively copy fields |
| Arrays | Deep copy — new array, recursively copy elements |
| Maps, Sets | Deep copy — new container, copy all entries |
| Closures | Deep copy — new closure object, copy captures |
| Channels | **Not copied** — shared by reference (this is the point of channels) |
| DI singletons (via `self`) | **Not copied** — accessed through `self` fields, synchronized if needed (see Part 3) |

### Performance

Deep copy has a cost. For most spawn use cases (processing a request, running a computation), the data is small and the copy is negligible compared to the work. For large data:

- **Channels** are available for streaming large datasets without copying
- **DI singletons** are never copied — they're accessed through `self` and auto-synchronized (Part 3)
- A future optimization: compiler could elide copies when it proves the caller doesn't use the value after spawn (effectively a move, but inferred, not annotated)

### Structured Concurrency

Task handles are must-use. A `Task<T>` must be consumed via `.get()` or `.detach()`. Dropping a task handle without consuming it is a compile error:

```
let task = spawn work()
task.get()!                      // OK: consumed

spawn work()                     // COMPILE ERROR: task handle dropped

let task = spawn fire_and_forget()
task.detach()                    // OK: explicitly detached
```

### Cancellation

`.cancel()` requests cooperative cancellation. The task terminates at the next checkpoint (I/O, channel op, or explicit `Task.check_cancelled()`). `.get()` on a cancelled task raises `TaskCancelled`:

```
let task = spawn long_work()
task.cancel()
task.get() catch {
    TaskCancelled => print("cancelled")
}
```

---

## Part 3: Inferred Synchronization

### The Problem

Some state genuinely needs to be shared across threads and pods:
- A session cache accessed by every request handler
- A rate limiter shared across all pods
- Configuration that updates at runtime
- Metrics counters aggregated across the system

Copy-on-spawn doesn't help here — the whole point is that everyone sees the same state.

### The Insight: The Compiler Already Knows

The compiler has three pieces of information:

1. **The DI graph.** It knows every singleton, every dependency, every injection point.
2. **`mut self`.** It knows which methods mutate and which are read-only.
3. **Concurrency analysis.** It knows which code runs in spawned tasks, which singletons are reachable from concurrent contexts.

That's everything needed to generate synchronization automatically. No new keywords. No annotations. You write a normal class, inject it normally, and the compiler figures out if it needs locking.

### How It Works

```
class SessionCache {
    sessions: Map<string, Session>

    fn get(self, id: string) Session? {
        return self.sessions[id]
    }

    fn put(mut self, id: string, session: Session) {
        self.sessions[id] = session
    }

    fn remove(mut self, id: string) {
        self.sessions.remove(id)
    }
}
```

This is a normal class. Nothing special about it.

Now suppose it's used like this:

```
class RequestHandler[cache: SessionCache] {
    fn handle(mut self, req: Request) Response {
        let session = self.cache.get(req.session_id)?
        // ...
    }
}

app MyApp[handler: RequestHandler] {
    fn main(self) {
        // handler is used from multiple spawned tasks
        for conn in listen(8080) {
            spawn self.handler.handle(conn)
        }
    }
}
```

The compiler sees:
1. `SessionCache` is a singleton (DI)
2. `RequestHandler.handle()` is called from spawned tasks (concurrency analysis)
3. `handle()` reaches `SessionCache.get()` (a `self` method) and potentially `SessionCache.put()` (a `mut self` method)

Therefore: `SessionCache` needs synchronization. The compiler wraps:
- `self` methods → reader lock (concurrent reads OK)
- `mut self` methods → writer lock (exclusive access)

If `SessionCache` were only ever accessed from a single thread, no locks would be generated. **Zero overhead when not needed.**

### What the Compiler Analyzes

The concurrency analysis pass walks the DI graph and spawn sites:

1. For each `spawn` call, identify which singletons are reachable from the spawned function (through DI deps, transitive)
2. A singleton is **concurrently accessed** if it's reachable from:
   - Two or more spawn sites, OR
   - A spawn site AND the spawning thread (after the spawn)
3. For each concurrently-accessed singleton, inject rwlock synchronization on its methods

This is the same kind of whole-program analysis Pluto already does for error inference — walk the call graph, propagate facts, reach a fixed point.

### Single Machine Behavior

For concurrently-accessed singletons, the codegen wraps method calls:

- **`self` methods** → `pthread_rwlock_rdlock` / `pthread_rwlock_unlock`
- **`mut self` methods** → `pthread_rwlock_wrlock` / `pthread_rwlock_unlock`

```
// Two tasks can call .get() simultaneously (reader locks)
let task1 = spawn handler.handle(req1)
let task2 = spawn handler.handle(req2)

// .put() waits for readers to finish, takes exclusive access
// (this happens automatically inside any method that calls cache.put())
```

### Distributed Behavior

Across pods, the orchestration layer (not the language) decides which singletons should be replicated. For replicated singletons:

- **`self` methods** read from the local replica. Fast, no network.
- **`mut self` methods** propagate mutations to other pods. The consistency model (eventual, strong, causal) is configured in the orchestration layer.

The same `SessionCache` code works identically on one machine or across 50 pods. The programmer doesn't change anything. The compiler and runtime handle the difference.

### Why Not a `shared` Keyword?

A `shared class` modifier would work, but it goes against Pluto's design philosophy:

- **Pluto infers error-ability** — you don't annotate functions as fallible, the compiler figures it out from the call graph.
- **Pluto infers DI lifecycle** — with the lifecycle RFC, you don't annotate most classes as scoped, the compiler infers it from dependencies.
- **Pluto should infer synchronization** — you don't annotate classes as shared, the compiler infers it from usage.

The programmer's job is to write correct business logic and declare mutability honestly (`self` vs `mut self`). The compiler's job is to make it safe and fast.

### Escape Hatch: Explicit Opt-In / Opt-Out

Sometimes the compiler's analysis is too conservative (adds locks where none are needed) or the programmer wants to force synchronization regardless of current usage (forward-compatibility). For these cases:

```
// TODO: Decide if we need this. The compiler analysis may be good enough.
// Options:
//   @synchronized class Foo { ... }      // force locks even if not currently concurrent
//   @unsynchronized class Foo { ... }    // suppress locks even if concurrent (unsafe)
```

This is future work. Start with inference, add escape hatches only if needed.

### Channels vs Inferred Synchronization

Both are communication mechanisms. When to use which:

| | Channels | DI Singletons (synchronized) |
|---|---------|-------------|
| **Model** | Message passing | Shared memory with auto-locking |
| **Declaration** | Explicit `chan<T>()` | Normal class + DI injection |
| **Synchronization** | Built into channel ops | Compiler-inferred from `mut self` |
| **Cross-pod** | Yes (serialized) | Yes (replicated, configured in orchestration) |
| **Best for** | Pipelines, streaming, decoupling | Caches, config, counters, registries |

---

## Part 4: Interaction with Other Features

### Contracts

`mut self` tracking narrows invariant checks:

- **Today:** Invariants checked after *every* method call (wasteful for read-only methods)
- **With `mut self`:** Invariants checked only after `mut self` methods (correct — only mutations can break invariants)
- **With inferred sync:** Invariant check runs inside the writer lock, before the lock releases. Concurrent readers never see a broken invariant.

### DI Lifecycle Scopes

Scoped and synchronized singletons interact naturally:

```
scoped class RequestCtx { ... }          // per-request

class RateLimiter {                      // singleton, auto-synchronized if concurrent
    counts: Map<string, int>
    limit: int

    fn check(mut self, client_id: string) { ... }
}

class Handler[limiter: RateLimiter, ctx: RequestCtx] {
    fn handle(self, req: Request) {
        self.limiter.check(self.ctx.client_id)!
        // ... handle request ...
    }
}
```

Singletons outlive any scope. Scoped classes can depend on singletons (reading from a synchronized cache within a request scope is perfectly fine). The compiler infers that `RateLimiter` needs synchronization if `Handler.handle()` is called from spawned tasks.

### Error Handling

Synchronized methods can raise errors like any other method:

```
error RateLimitExceeded {
    client_id: string
}

class RateLimiter {
    counts: Map<string, int>
    limit: int

    fn check(mut self, client_id: string) {
        let current = self.counts[client_id] catch 0
        if current >= self.limit {
            raise RateLimitExceeded { client_id: client_id }
        }
        self.counts[client_id] = current + 1
    }
}
```

If a `mut self` method raises while holding a writer lock, the lock releases and the state is unchanged. This matches transactional semantics — errors roll back.

### Cross-Pod RPC

When the compiler generates RPC for cross-pod function calls, it serializes arguments. This is conceptually the same as copy-on-spawn — data crosses a boundary, the other side gets its own copy. The mental model is consistent:

- **spawn** = send a copy to another thread
- **RPC** = send a copy to another pod
- **DI singletons** = auto-synchronized, auto-replicated where needed
- **channels** = streaming copies between tasks/pods

---

## Implementation Phases

### Phase 1: `mut self` Tracking

**Scope:** Parser + typeck changes only. No runtime changes.

1. Parser: distinguish `fn method(self)` from `fn method(mut self)` in class method declarations
2. Typeck: track mutability of self parameter on each method
3. Typeck: error if a non-mut method assigns to `self.field`
4. Typeck: error if a non-mut method calls a `mut self` method on `self`
5. Typeck: enforce `let mut` at call sites — calling a `mut self` method on an immutable binding is an error
6. Narrow invariant checks to `mut self` methods only

**Test:** All existing tests continue to pass (add `mut self` where needed). New tests for mutability enforcement.

### Phase 2: Copy on Spawn

**Scope:** Runtime deep-copy + codegen changes.

1. Add `__pluto_deep_copy(ptr)` to the runtime (recursive copy for GC-tracked objects)
2. Codegen: at spawn sites, emit deep-copy for each heap-type argument
3. Channels and shared instances are exempted (reference-shared)
4. Fix GC suppression: with copy-on-spawn, tasks don't share heap objects with the main thread, so GC can run safely on the main thread. Tasks manage their own allocations.

**Test:** Race condition tests from Phase 1 should now pass (no shared mutation). New tests for deep-copy correctness.

### Phase 3: Structured Concurrency

**Scope:** Typeck enforcement.

1. `Task<T>` is must-use — compile error if handle dropped without `.get()` or `.detach()`
2. `.detach()` — releases handle, task runs to completion, errors go to runtime error handler
3. `.cancel()` — sets cancellation flag, task checks at I/O/channel/explicit checkpoints

### Phase 4: Inferred Synchronization

**Scope:** Concurrency analysis pass + runtime synchronization.

1. Concurrency analysis: walk DI graph + spawn sites to identify singletons reachable from concurrent contexts
2. Mark concurrently-accessed singletons in `TypeEnv` / `ClassInfo`
3. Runtime: allocate `pthread_rwlock_t` per synchronized singleton (alongside the instance in synthetic main)
4. Codegen: for synchronized singletons, wrap `self` method calls in `rdlock`/`unlock`, `mut self` calls in `wrlock`/`unlock`
5. Integrate with contracts (invariant checks inside writer lock, before unlock)
6. No parser changes. No new keywords. Just analysis + codegen.

### Phase 5: Distributed Replication

**Scope:** Runtime replication layer (depends on cross-pod RPC infrastructure).

1. Orchestration layer configuration marks which singletons should be replicated
2. Runtime connects replicated singletons to replication layer at startup
3. `mut self` calls on replicated singletons propagate mutations to other pods
4. Consistency model configured at orchestration layer (eventual, strong, causal)
5. Conflict resolution strategy for concurrent writes across pods

---

## Open Questions

1. **Deep copy performance.** Should there be an opt-out for copy-on-spawn? e.g., `spawn func(ref x)` to explicitly share a reference? This reintroduces the race risk but gives an escape hatch for performance-critical paths.

2. **Analysis precision.** The concurrency analysis determines which singletons are concurrently accessed. How precise does this need to be? Conservative (any singleton reachable from any spawn = synchronized) is simple but may add unnecessary locks. Precise (track exact call paths) is better but more complex. Start conservative, refine later?

3. **Consistency configuration.** How does the orchestration layer specify replication consistency for singletons? Per-class? Per-deployment? This is probably outside the language spec.

4. **Reentrant access.** If a `mut self` method on a synchronized singleton calls another method on the same instance, that's a lock-inside-lock situation. Need reentrant rwlocks, or the compiler could detect and prevent this pattern.

5. **Scoped + concurrent.** If a scope block spawns tasks that access scoped singletons, those scoped singletons also need synchronization. The analysis should handle scoped instances the same way — if concurrently accessed, synchronize.

6. **Spawn block syntax.** Should `spawn { ... }` (anonymous block) be supported in addition to `spawn func(args)`? This affects how copy-on-spawn works — a block captures variables from the enclosing scope rather than taking explicit arguments.

7. **GC implications of copy-on-spawn.** Each spawned task creates its own copies of data. With many tasks, this could increase memory pressure. The GC should handle this (copies become garbage when tasks complete), but the working set is larger.

8. **Non-DI concurrently accessed objects.** The inference works for DI singletons because the compiler sees the full graph. What about objects created manually and passed to spawn? Copy-on-spawn handles most cases (the task gets its own copy), but what about objects passed through channels that multiple tasks then access? Channels transfer ownership (send gives up the value), so this may not be an issue.

9. **Compiler diagnostics.** Should the compiler tell you which singletons it decided to synchronize? A warning or info diagnostic ("SessionCache will be synchronized: accessed from spawn sites at lines 42, 67") would help developers understand the generated code.

## Alternatives Considered

### Move Semantics (Rust-style)

Move-on-spawn with use-after-move errors. Rejected because:
- Pluto is GC'd — moves don't serve a memory management purpose
- Adds ownership concepts that don't exist elsewhere in the language
- High implementation complexity for an isolated feature
- Copy-on-spawn achieves the same safety with less conceptual weight

### Freeze on Share (Immutable Views)

Objects crossing spawn boundaries become read-only. Rejected because:
- Restricts what spawned tasks can do (user wants full mutability)
- Requires runtime tracking of "frozen" objects
- Doesn't match distribution model (cross-pod data is independently mutable)

### No Safety (Status Quo + Documentation)

Document "don't share mutable state" and move on. Rejected because:
- Every language that does this regrets it (C++, Go before the race detector)
- Pluto's goal is compiler-enforced correctness
- The spec already promises "no shared mutable state"

### Shared Everything with Locks (Java-style)

Every object has a monitor, `synchronized` blocks. Rejected because:
- Pervasive locking overhead even when not needed
- Deadlock risk
- Doesn't scale to distribution (can't replicate arbitrary lock-based protocols)
- Pluto should be opinionated, not permissive

### Explicit `shared` Keyword

A `shared class` modifier that declares synchronized + replicated state. Rejected because:
- Goes against Pluto's inference philosophy (errors inferred, DI lifecycle inferred)
- Adds a new concept to the language when the compiler has enough information to infer it
- Creates a "two kinds of classes" split that makes the type system feel heavier
- The compiler already has DI graph + `mut self` + spawn analysis — that's everything needed

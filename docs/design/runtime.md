# Runtime

## Current Implementation

The current compiler links a small C runtime (`runtime/builtins.c`) that provides printing, allocation, string/array/map/set operations, error handling, and garbage collection. For the concrete ABI and data layouts, see `docs/design/compiler-runtime-abi.md`.

### Garbage Collection

The runtime includes a mark-and-sweep garbage collector:

- **Tag-based tracing:** Every GC-managed allocation has a tag byte identifying its type (string, array, class, map, set), enabling the collector to trace references correctly.
- **Root scanning:** The collector walks a shadow stack of GC roots maintained by compiler-generated code.
- **Trigger:** Collection runs when total heap usage exceeds a threshold (currently 1 MB, grows dynamically).
- **Built-in:** `gc_heap_size()` returns current heap usage in bytes.
- **Scope:** Collects strings, arrays, class instances, maps, and sets.

## The Pluto Runtime ("VM")

Every Pluto program runs inside the Pluto runtime. Despite compiling to native code, the runtime provides a lightweight execution wrapper — conceptually similar to a Kubernetes pod manager but as a no-op when not needed.

## Responsibilities

The runtime handles concerns that don't belong in application code:

- **Dependency resolution:** Satisfying bracket dep and ambient dep declarations based on environment configuration
- **Process lifecycle:** Starting, monitoring, and restarting processes
- **Unrecoverable error handling:** OOM, stack overflow, assertion failures — caught by the runtime, not user code
- **Channel routing:** Managing channel endpoints across pod boundaries
- **Geographic scheduling:** Placing processes based on region/locality constraints

## Crash Recovery

When a process crashes due to an unrecoverable error, the runtime:

1. Captures diagnostic information
2. Notifies connected processes (channels become disconnected)
3. Optionally restarts the process based on configuration
4. Reports to the orchestration layer (if present)

## Relationship to the Language

The runtime is **not** a virtual machine in the traditional sense — Pluto compiles to native code. The runtime is a thin management layer that:
- Wraps the native binary
- Provides the DI container
- Manages process supervision
- Handles the distributed fabric (channel routing, serialization, etc.)

Think of it as: Go has a runtime (goroutine scheduler, GC). Pluto has a runtime (process supervisor, DI container, channel router, GC).

## Open Questions

- [ ] Configuration format for the runtime (how do you specify DI bindings, region constraints, restart policies?)
- [ ] Supervision strategies (one-for-one, one-for-all, rest-for-one — a la Erlang?)
- [ ] Observability — built-in metrics, tracing, logging hooks?
- [ ] How does the runtime interact with the orchestration layer?

# Runtime

> **Implementation status:** The current runtime is a minimal C library (`runtime/builtins.c`) providing `print`, memory allocation, string/array operations, and error handling primitives (`pluto_get_error`, `pluto_set_error`, `pluto_clear_error`). The full process supervisor, channel router, and GC described below are not yet implemented.

## The Pluto Runtime ("VM")

Every Pluto program runs inside the Pluto runtime. Despite compiling to native code, the runtime provides a lightweight execution wrapper — conceptually similar to a Kubernetes pod manager but as a no-op when not needed.

## Responsibilities

The runtime handles concerns that don't belong in application code:

- **Dependency resolution:** Satisfying `inject` declarations based on environment configuration
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

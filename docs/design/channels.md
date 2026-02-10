# Channels

> **Implementation status:** Phase 1 implemented. Core channel operations, blocking/non-blocking send/recv, error integration, for-in iteration, directional types.

## Overview

Channels are Pluto's primitive for communication between concurrent tasks. A channel is a typed, directional conduit: one end sends values, the other receives them.

The long-term vision is that channels become the **universal I/O primitive** — HTTP connections, file streams, socket reads, and cross-pod communication would all expose the same channel interface. Phase 1 covers the core inter-task messaging; stdlib I/O integration is future work.

## Design Principles

1. **Method syntax, not operators.** `.send()` and `.recv()` instead of `<-` arrows. Consistent with the rest of Pluto, no new lexer tokens.
2. **Fallible by default.** Every channel operation can fail (closed, full, empty). Integrates with Pluto's error system — `!` to propagate, `catch` to handle.
3. **Blocking by default, opt-in non-blocking.** `.send()` blocks until space is available. `.try_send()` and `.try_recv()` are the non-blocking alternatives.
4. **Directional types.** `Sender<T>` and `Receiver<T>` are separate types — you cannot accidentally send on a receiver or receive on a sender.

## Core API

### Creating Channels

```
// Buffered channel — send blocks only when buffer is full
let (tx, rx) = chan<int>(100)

// Default capacity (capacity 1) — nearly unbuffered
let (tx, rx) = chan<string>()
```

`chan<T>()` returns a `(Sender<T>, Receiver<T>)` pair. This is the only destructuring form in Pluto — it's specific to `chan`, not a general tuple system. The parser recognizes `let (a, b) = chan<T>(...)` as a channel creation statement.

`chan<T>()` with no argument uses capacity 1 (single-slot handoff). True rendezvous (capacity 0, sender blocks until receiver is ready) is deferred to Phase 2.

### Sending

```
tx.send(value)!              // Block until sent. Propagate error on failure.
tx.send(value) catch { ... } // Block until sent. Handle error on failure.
tx.try_send(value)!          // Non-blocking. Fails immediately if buffer full.
```

`send()` blocks the calling task until there is space in the buffer.

`send()` fails with `ChannelClosed` if the channel has been closed.

`try_send()` fails with `ChannelFull` if the buffer is at capacity, or `ChannelClosed` if closed.

### Receiving

```
let val = rx.recv()!              // Block until a value arrives.
let val = rx.recv() catch -1      // Block, with fallback on error.
let val = rx.try_recv()!          // Non-blocking. Fails immediately if empty.
```

`recv()` blocks the calling task until a value is available.

`recv()` fails with `ChannelClosed` if the channel is closed and the buffer is empty. Buffered values are drained before the error is raised.

`try_recv()` fails with `ChannelEmpty` if the buffer is empty, or `ChannelClosed` if closed and empty.

### Closing

```
tx.close()    // Signal that no more values will be sent.
```

After `close()`:
- Further `send()` calls raise `ChannelClosed`
- `recv()` continues to drain any buffered values, then raises `ChannelClosed`
- Blocked receivers are woken and receive `ChannelClosed`

`close()` is idempotent — closing an already-closed channel is a no-op.

### Errors

```
error ChannelClosed { message: string }
error ChannelFull { message: string }
error ChannelEmpty { message: string }
```

These are built-in error types registered by the compiler. The error inference system automatically knows that `tx.send()` can raise `ChannelClosed`, etc.

## Iteration

`for-in` over a receiver drains it until the channel is closed:

```
for msg in rx {
    process(msg)
}
// Loop exits when the channel is closed and drained.
```

This is equivalent to:
```
while true {
    let msg = rx.recv() catch { break }
    process(msg)
}
```

The `for-in` form is the idiomatic way to consume a channel. `break` and `continue` work normally inside the loop.

## Passing Channels

Sender and Receiver are first-class values. They can be passed as function arguments:

```
fn producer(tx: Sender<int>) {
    for i in 0..10 {
        tx.send(i)!
    }
    tx.close()
}

fn consumer(rx: Receiver<int>) {
    for val in rx {
        print(val)
    }
}
```

Multiple senders can write to the same channel (fan-in):
```
let (tx, rx) = chan<int>(10)
spawn send_val(tx, 1)
spawn send_val(tx, 2)
spawn send_val(tx, 3)
```

## Compiler Integration

### Type System

Two variants in `PlutoType`:
```rust
Sender(Box<PlutoType>)
Receiver(Box<PlutoType>)
```

Both are built-in generics — like `Map` and `Set`, they're special-cased in the type checker, not user-defined generics.

### AST

`Stmt::LetChan { sender, receiver, elem_type, capacity }` — a dedicated statement node. There is no `Expr::ChanCreate`; `chan<T>()` cannot appear as a standalone expression.

### Error Inference

The compiler infers channel errors automatically:
- `tx.send(v)` → can raise `ChannelClosed`
- `rx.recv()` → can raise `ChannelClosed`
- `tx.try_send(v)` → can raise `ChannelClosed`, `ChannelFull`
- `rx.try_recv()` → can raise `ChannelClosed`, `ChannelEmpty`

### For-in Desugaring

For-in on a `Receiver<T>` is lowered in codegen (not desugared at the AST level). The codegen emits: call `recv()`, check TLS error, if error then clear and exit loop, otherwise process value and loop back.

## Runtime Implementation

### Data Structure

`GC_TAG_CHANNEL = 9`. Channel handle is 56 bytes (7 × i64 slots):

```
[0] sync_ptr  — pointer to malloc'd ChannelSync (mutex + 2 condvars)
[1] buf_ptr   — pointer to malloc'd circular buffer of i64 slots
[2] capacity  — int (always >= 1; chan<T>() uses capacity 1)
[3] count     — int (items currently in buffer)
[4] head      — int (read position)
[5] tail      — int (write position)
[6] closed    — int (0 or 1)
```

`ChannelSync` is a separately malloc'd struct containing `pthread_mutex_t` + two `pthread_cond_t` (not_empty, not_full). It's freed during GC sweep.

### Sender/Receiver Representation

Both `Sender<T>` and `Receiver<T>` are I64 pointers to the same underlying channel handle. The type distinction exists only in the type checker — at runtime they're the same pointer.

### GC Integration

**Mark:** Trace live buffer slots — `count` items starting at `head`, wrapping at `capacity`.

**Sweep:** Destroy mutex + condvars, free ChannelSync and buffer.

### Runtime Functions

- `__pluto_chan_create(capacity) -> handle`
- `__pluto_chan_send(handle, value) -> value`
- `__pluto_chan_recv(handle) -> value`
- `__pluto_chan_try_send(handle, value) -> value`
- `__pluto_chan_try_recv(handle) -> value`
- `__pluto_chan_close(handle)`

## Phase 1 Scope (implemented)

- `chan<T>()` and `chan<T>(capacity)` syntax
- `Sender<T>` and `Receiver<T>` types
- `.send()`, `.recv()` (blocking)
- `.try_send()`, `.try_recv()` (non-blocking)
- `.close()` (explicit, idempotent)
- `ChannelClosed`, `ChannelFull`, `ChannelEmpty` error types
- `for item in rx { ... }` iteration
- Channels as function parameters
- 36 integration tests, producer-consumer example
- GC integration with buffer slot tracing

## Deferred to Later Phases

- **Reference counting / implicit close-on-drop** — Auto-close when last sender goes out of scope. Needs scope-exit hooks. Phase 2.
- **True rendezvous (capacity 0)** — Sender blocks until receiver is ready. Phase 2.
- **Stdlib channel methods** (`.connections()`, `.lines()`, `.stream()`) — Expose I/O as channels.
- **Select/race** — Multiplexing across multiple channels. Needs a runtime primitive.
- **Timeouts** (`rx.recv_timeout(duration)`) — Needs a `Duration` type.
- **Move semantics** — Send copies the pointer for heap types (shared reference). Real ownership transfer is future work.
- **Cross-pod channels** — Compiler-synthesized serialization for channels across pod boundaries.

## Open Questions

- **Channel of channels** — Should `chan<Sender<T>>()` work? Useful for request-response patterns but adds complexity.
- **Typed close values** — Should `close()` carry a final value or error?
- **Deadlock detection** — Can the runtime detect when all tasks are blocked on channels?

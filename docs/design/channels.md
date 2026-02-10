# Channels

> **Implementation status:** Not yet implemented. This document describes the design for Pluto's channel primitive and its role as the universal I/O abstraction.

## Overview

Channels are Pluto's primitive for communication between concurrent tasks and for streaming I/O. A channel is a typed, directional conduit: one end sends values, the other receives them.

The key insight is that channels are not just for inter-task messaging. They are the **universal I/O primitive** — HTTP connections, file streams, socket reads, and cross-pod communication all expose the same channel interface. This means there is one concurrency pattern to learn, and the runtime handles the underlying complexity (threads, epoll/kqueue, serialization) behind the scenes.

## Design Principles

1. **One abstraction for all I/O.** HTTP connections, file streams, inter-task messages, and cross-pod calls all look like channels. The programmer learns one pattern.
2. **Method syntax, not operators.** `.send()` and `.recv()` instead of `<-` arrows. Consistent with the rest of Pluto, no new lexer tokens.
3. **Fallible by default.** Every channel operation can fail (disconnected, closed, full). Integrates with Pluto's error system — `!` to propagate, `catch` to handle.
4. **Blocking by default, opt-in non-blocking.** Unbuffered `.send()` blocks until the receiver is ready. `.try_send()` and `.try_recv()` are the non-blocking alternatives.
5. **Channels are the bridge between sync and async.** User code reads synchronously from a channel; the runtime feeds it asynchronously from I/O threads.

## Core API

### Creating Channels

```
// Unbuffered channel — send blocks until receiver is ready
let (tx, rx) = chan<string>()

// Buffered channel — send blocks only when buffer is full
let (tx, rx) = chan<int>(100)
```

`chan<T>()` returns a tuple of `(Sender<T>, Receiver<T>)`. The two ends are separate types — you cannot accidentally send on a receiver or receive on a sender.

### Types

```
Sender<T>       // Send end — can send values of type T
Receiver<T>     // Receive end — can receive values of type T
```

Both are first-class values. They can be passed to functions, stored in fields, and sent to spawned tasks.

### Sending

```
tx.send(value)!              // Block until sent. Propagate error on failure.
tx.send(value) catch { ... } // Block until sent. Handle error on failure.
tx.try_send(value)!          // Non-blocking. Fails immediately if not ready.
```

`send()` blocks the calling task until:
- The value is received (unbuffered), or
- There is space in the buffer (buffered)

`send()` fails if:
- The receiver has been dropped (channel disconnected)
- The channel has been explicitly closed

### Receiving

```
let val = rx.recv()!              // Block until a value arrives.
let val = rx.recv() catch { ... } // Block until a value arrives. Handle error.
let val = rx.try_recv()!          // Non-blocking. Fails immediately if empty.
```

`recv()` blocks the calling task until:
- A value is available

`recv()` fails if:
- The sender has been dropped and the buffer is empty (channel disconnected)
- The channel has been explicitly closed

### Closing

```
tx.close()    // Signal that no more values will be sent.
```

After `close()`:
- Further `send()` calls raise `ChannelClosed`
- `recv()` continues to drain any buffered values, then raises `ChannelClosed`

Dropping the sender (letting it go out of scope) implicitly closes the channel. This is the normal way channels end — explicit `close()` is for when you need to signal completion while still holding the sender variable.

### Errors

```
error ChannelClosed {}
error ChannelFull {}        // Only from try_send()
error ChannelEmpty {}       // Only from try_recv()
```

These are built-in error types, like `TaskCancelled`. The compiler infers them automatically — if you call `tx.send()`, the compiler knows that expression can raise `ChannelClosed`.

## Iteration

Channels implement the iteration protocol. A `for-in` loop over a receiver drains it until closed:

```
for msg in rx {
    process(msg)
}
// Loop exits when the channel is closed and drained.
```

This desugars to:

```
while true {
    let msg = rx.recv() catch {
        break
    }
    process(msg)
}
```

The `for-in` form is the idiomatic way to consume a channel. It's clean, handles the close signal automatically, and reads like "for each message from the channel."

## Channels as the Universal I/O Primitive

### The Pattern

Every I/O source in Pluto can expose a channel interface:

```
// The runtime accepts connections on a background thread.
// Each connection appears as a value on the channel.
fn connections(self) Receiver<HttpConnection>

// The runtime reads lines on a background thread.
// Each line appears as a value on the channel.
fn lines(self) Receiver<string>

// The runtime reads chunks on a background thread.
// Each chunk appears as a value on the channel.
fn stream(self) Receiver<string>
```

The user reads synchronously from the channel. The runtime feeds it asynchronously from I/O threads. The channel is the bridge.

### HTTP Server

Today's HTTP server requires a manual accept loop:

```
let server = http.listen("0.0.0.0", 8080)!
while true {
    let conn = server.accept()!
    let req = conn.read_request()!
    conn.send_response(handle(req))
    conn.close()
}
```

With channels, the server yields connections:

```
let server = http.listen("0.0.0.0", 8080)!
for conn in server.connections() {
    spawn handle_connection(conn)
}
```

One line to accept, one line to dispatch. The runtime handles the accept loop on a background thread. Each connection gets its own task. Backpressure is automatic — if tasks aren't consuming connections fast enough, the channel blocks the accept thread.

The connection itself can also be channel-based for streaming:

```
fn handle_connection(conn: http.HttpConnection) {
    let req = conn.read_request()!
    let resp = handle(req)
    conn.send_response(resp)
    conn.close()
}
```

This stays synchronous per-connection — channels shine at the **dispatch** level, not necessarily inside each handler.

### File I/O

Simple reads stay synchronous (no channel needed):

```
let content = fs.read_all("data.txt")!
```

Streaming reads use channels:

```
let file = fs.open("large.csv")!
for line in file.lines() {
    process(line)
}
```

The runtime reads ahead on a background thread, buffering lines into the channel. The user's `for` loop consumes them at its own pace. If the consumer is slow, the channel's buffer fills and the read-ahead thread blocks — automatic backpressure.

### Sockets / TCP

```
let listener = net.listen("0.0.0.0", 9000)!
for conn in listener.connections() {
    spawn handle(conn)
}

fn handle(conn: net.TcpConnection) {
    for chunk in conn.stream() {
        process(chunk)
    }
}
```

### The Layering

```
┌─────────────────────────────────────────────┐
│  User code                                  │
│  for conn in server.connections() { ... }   │
├─────────────────────────────────────────────┤
│  Stdlib (std.http, std.fs, std.net)         │
│  Exposes sync methods + channel methods     │
├─────────────────────────────────────────────┤
│  Channel primitive                          │
│  Sender<T> / Receiver<T>, blocking queue    │
├─────────────────────────────────────────────┤
│  Runtime (C)                                │
│  pthreads, epoll/kqueue, GC integration     │
└─────────────────────────────────────────────┘
```

Stdlib modules expose **both** sync convenience methods (for simple cases) and channel-based streaming methods (for concurrent/streaming cases). They're not either/or — the same HTTP server class has `.accept()` (blocking, returns one connection) and `.connections()` (returns a channel of connections).

## Inter-Task Communication

Channels are also how tasks talk to each other:

```
let (tx, rx) = chan<Order>(100)

let producer = spawn produce_orders(tx)
let consumer = spawn consume_orders(rx)

producer.get()!
consumer.get()!
```

```
fn produce_orders(tx: Sender<Order>) {
    let orders = load_orders()!
    for order in orders {
        tx.send(order)!
    }
    tx.close()
}

fn consume_orders(rx: Receiver<Order>) {
    for order in rx {
        process(order)!
    }
}
```

### Fan-Out

Multiple consumers on the same receiver:

```
let (tx, rx) = chan<Job>(1000)

// 4 worker tasks consuming from the same channel
let workers = [0, 0, 0, 0].map((i: int) => spawn worker(rx))

// Producer sends jobs
for job in get_jobs() {
    tx.send(job)!
}
tx.close()

// Wait for all workers
for w in workers {
    w.get()!
}
```

Each job goes to exactly one worker (first to call `recv()` wins). This is a natural work-stealing pattern.

### Fan-In

Multiple senders into the same channel:

```
let (tx, rx) = chan<Result>(100)

// Multiple producers
for url in urls {
    spawn fetch_and_send(url, tx)
}

// Single consumer collects results
for result in rx {
    aggregate(result)
}
```

Sender is cloneable — multiple tasks can hold copies of the same sender.

## Compiler Integration

### Type System

Two new types in `PlutoType`:

```rust
Sender(Box<PlutoType>)    // Sender<T>
Receiver(Box<PlutoType>)  // Receiver<T>
```

`chan<T>()` is a built-in generic — like `Map` and `Set`, it's special-cased in the type checker, not a user-defined generic.

### Error Inference

The compiler infers channel errors automatically:

- `tx.send(v)` → can raise `ChannelClosed`
- `rx.recv()` → can raise `ChannelClosed`
- `tx.try_send(v)` → can raise `ChannelClosed | ChannelFull`
- `rx.try_recv()` → can raise `ChannelClosed | ChannelEmpty`

These flow through the existing error inference system. A function that calls `tx.send()!` becomes fallible, and its callers must handle the error.

### Destructuring

`chan<T>()` returns a tuple-like pair. Since Pluto doesn't have tuples, this is special-cased in the parser/typechecker:

```
let (tx, rx) = chan<string>()
```

This is the **only** destructuring form — it's specific to `chan`, not a general tuple system. The parser recognizes `let (a, b) = chan<T>(...)` as a channel creation expression.

## Runtime Implementation

### Data Structure

A channel is a GC-allocated structure (new `GC_TAG_CHANNEL`):

```
┌──────────────────────────────────────────────┐
│ mutex: pthread_mutex_t                       │
│ not_empty: pthread_cond_t                    │
│ not_full: pthread_cond_t                     │
│ buffer: pointer to circular array of slots   │
│ capacity: int (0 = unbuffered)               │
│ count: int (items currently buffered)        │
│ head: int (read position)                    │
│ tail: int (write position)                   │
│ closed: bool                                 │
│ sender_count: atomic int (for multi-sender)  │
│ receiver_count: atomic int                   │
└──────────────────────────────────────────────┘
```

For unbuffered channels (`capacity == 0`), send blocks on `not_empty` until a receiver calls recv, and vice versa. This is a synchronous rendezvous.

For buffered channels, send blocks on `not_full` when the buffer is at capacity. Recv blocks on `not_empty` when the buffer is empty.

### GC Integration

Channel buffers contain Pluto values (which may be heap pointers). The GC must trace:
- The channel structure itself
- Every slot in the buffer that contains a live value

The existing GC tag system handles this: `GC_TAG_CHANNEL` with `field_count` pointing to the number of live buffer slots. The GC scans buffer slots as roots during mark phase.

### Sender/Receiver Representation

In Cranelift IR, both `Sender<T>` and `Receiver<T>` are I64 pointers to the same underlying channel structure. The type distinction exists only in the type checker — at runtime they're the same pointer. This is safe because the type system prevents misuse (you can't call `.recv()` on a `Sender`).

### Reference Counting for Multi-Sender

`Sender<T>` uses `sender_count` (atomic int) to track how many senders exist. When the last sender is dropped, the channel is implicitly closed. Similarly for `receiver_count`. This enables fan-in without explicit close calls.

## Phase 1 Scope (v0.2)

### Included

- `chan<T>()` and `chan<T>(capacity)` syntax
- `Sender<T>` and `Receiver<T>` types
- `.send()`, `.recv()` (blocking)
- `.try_send()`, `.try_recv()` (non-blocking)
- `.close()` (explicit close)
- Implicit close on last sender drop (via reference counting)
- `ChannelClosed`, `ChannelFull`, `ChannelEmpty` error types
- `for item in rx { ... }` iteration
- `let (tx, rx) = chan<T>()` destructuring syntax
- Runtime: mutex + condvar blocking queue in `builtins.c`
- GC integration: `GC_TAG_CHANNEL`, buffer slot tracing
- Error inference integration
- 30+ integration tests

### Deferred to Later Phases

- **Stdlib channel methods** (`.connections()`, `.lines()`, `.stream()`) — Phase 2, after core channels are stable. Stdlib continues to work with sync methods in the meantime.
- **Select/race** — Phase 3. Requires a multiplexing primitive in the runtime. Design TBD.
- **Timeouts** (`rx.recv_timeout(duration)`) — needs a `Duration` type first.
- **Move semantics** — send copies the value (pointer copy for heap types). Same shared-pointer caveat as `spawn`. Real ownership transfer is future work.
- **Cross-pod channels** — Phase 4. Compiler-synthesized serialization for channels that cross pod boundaries.
- **Sender cloning** — Phase 1 supports passing the same sender to multiple tasks (reference counted). Explicit `.clone()` method deferred.

## Examples

### Producer-Consumer Pipeline

```
fn main() {
    let (tx, rx) = chan<int>(50)

    let producer = spawn produce(tx)
    let consumer = spawn consume(rx)

    producer.get()!
    consumer.get()!
}

fn produce(tx: Sender<int>) {
    let i = 0
    while i < 1000 {
        tx.send(i)!
        i = i + 1
    }
    tx.close()
}

fn consume(rx: Receiver<int>) {
    let sum = 0
    for val in rx {
        sum = sum + val
    }
    print("sum = {sum}")
}
```

### Worker Pool

```
fn process_jobs(jobs: [Job]) {
    let (tx, rx) = chan<Job>(100)
    let (results_tx, results_rx) = chan<Result>(100)

    // Start 4 workers
    let workers = [0, 0, 0, 0].map((i: int) => {
        return spawn worker(rx, results_tx)
    })

    // Feed jobs
    for job in jobs {
        tx.send(job)!
    }
    tx.close()

    // Collect results
    for result in results_rx {
        print(result)
    }
}

fn worker(jobs: Receiver<Job>, results: Sender<Result>) {
    for job in jobs {
        let result = run(job)!
        results.send(result)!
    }
}
```

### Future: Channel-Based HTTP Server (Phase 2)

```
import std.http

fn main() {
    let server = http.listen("0.0.0.0", 8080)!
    print("listening on :8080")

    for conn in server.connections() {
        spawn handle(conn)
    }
}

fn handle(conn: http.HttpConnection) {
    let req = conn.read_request()!
    let resp = route(req)
    conn.send_response(resp)
    conn.close()
}
```

## Open Questions

- [ ] **Buffer sizing heuristics** — should the compiler warn if a channel is created with capacity 0 in a hot loop? Or is unbuffered always fine?
- [ ] **Channel of channels** — should `chan<Sender<T>>()` work? Useful for request-response patterns but adds complexity.
- [ ] **Typed close values** — should `close()` be able to carry a final value or error? Go channels just close; Rust's `mpsc` doesn't. Keep it simple for now.
- [ ] **Deadlock detection** — can the runtime detect when all tasks are blocked on channels? Log a warning? Abort?

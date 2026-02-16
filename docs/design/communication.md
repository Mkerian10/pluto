# Communication Model

> **Implementation status:** Not yet implemented. This document describes the design vision for Pluto's communication model. The current compiler supports local function calls only.

## Overview

Pluto has a two-tier communication model:

1. **Synchronous function calls (default)** — cross-service calls look like local method calls. The compiler determines what crosses pod/network boundaries and generates appropriate code.
2. **Channels (opt-in)** — for asynchronous and streaming use cases.

Most code (~90%) uses synchronous calls. Channels are the tool for when you genuinely need async/streaming behavior.

---

## Synchronous Communication

### How It Works

Cross-service communication looks like regular method calls:

```
class OrderService[accounts: AccountsService] {
    fn process(mut self, order: Order) {
        let user = self.accounts.get_user(order.user_id)!
        // This might be a local call or a cross-pod RPC.
        // The programmer doesn't know or care.
        // The compiler generates the right code.
    }
}
```

### Compiler Behavior

The compiler (via whole-program analysis):
- Knows whether `accounts` is local or remote
- Generates serialization/deserialization if crossing pod boundaries
- Infers that the call can error (network, timeout, etc.) if remote
- Optimizes to a direct function call if both are on the same pod

### Error Implications

Remote calls can fail in ways local calls cannot (network errors, timeouts, pod unavailability). The compiler infers these error possibilities automatically — a function that calls a remote service is fallible even if it would be infallible locally.

---

## Channels

### When to Use

Channels are for when synchronous call/response is not the right model:
- Streaming data (continuous flow of values)
- Pub/sub patterns
- Fan-out / fan-in
- Fire-and-forget
- Decoupling producer and consumer speeds

### Creating Channels

Channels are **directional** — separate send and receive ends:

```
let ch = chan<Order>()

// ch.sender and ch.receiver are separate typed handles
```

### Sending and Receiving

Channel operations use method syntax:

```
// Send — can fail (disconnected)
ch.sender.send(msg)!

// Receive — can fail (empty, disconnected)
let val = ch.receiver.recv()!

// Non-blocking variants
let sent = ch.sender.try_send(msg)   // returns bool
let val = ch.receiver.try_recv()      // returns T?
```

### Iteration

Receivers support for-in iteration:

```
for msg in ch.receiver {
    process(msg)
}
```

### Compiler Optimizations

The compiler optimizes channel implementation based on topology:

| Scenario                    | Generated code                          |
| --------------------------- | --------------------------------------- |
| Both ends in same process   | In-memory queue, no serialization       |
| Same pod, different process | Shared memory, no network               |
| Cross-pod, same region      | Serialization + local network           |
| Cross-pod, cross-region     | Serialization + handles latency/retries |

### Auto-Serialization

Any type sent over a cross-pod channel must implement the `Serializable` trait. The compiler enforces this at compile time and can auto-derive `Serializable` for simple types.

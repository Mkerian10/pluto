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
let (tx, rx) = chan<Order>()

// Buffered channel
let (tx, rx) = chan<Order>(buffer: 100)
```

### Sending and Receiving

Channel operations are **fallible** and must be handled:

```
// Send — can fail (full, disconnected, network error)
tx <- msg ! "sending order"
tx <- msg catch err { log(err) }

// Receive — can fail (empty, disconnected, timeout)
let val = <-rx ! "waiting for response"
let val = <-rx catch err {
    // handle error
    default_value
}
```

### Explicit Blocking

Blocking is opt-in, not the default:

```
// Block until operation succeeds
tx.wait() <- msg
let val = <-rx.wait()

// Bounded wait
let val = <-rx.timeout(5.seconds) catch {
    TimeoutError => retry(),
    Disconnected => shutdown(),
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

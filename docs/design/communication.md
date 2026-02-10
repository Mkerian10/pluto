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

Channels are Pluto's second communication primitive — for streaming, fan-out/fan-in, and decoupling producers from consumers. They also serve as the **universal I/O abstraction**: HTTP connections, file streams, and socket reads all expose the same channel interface.

See **[channels.md](channels.md)** for the full design, including:
- Core API (`chan<T>()`, `Sender<T>`, `Receiver<T>`)
- Method syntax (`.send()`, `.recv()`, `.try_send()`, `.try_recv()`)
- Error integration (`ChannelClosed`, `ChannelFull`, `ChannelEmpty`)
- Channel iteration (`for item in rx { ... }`)
- Universal I/O pattern (HTTP, files, sockets as channels)
- Runtime implementation (mutex + condvar blocking queue)
- Phase 1 scope and examples

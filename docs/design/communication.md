# Communication Model

> **Framing:** Pluto is a distributed language, not an RPC language. The canonical model is [distributed-model.md](distributed-model.md): programs express **logical placement** with `at` expressions over execution domains; the compiler and deployment plan derive **physical execution** (network, IPC, in-process, or fused). This document covers how computation and data move between domains.
>
> **Implementation status:** The current compiler ships one concrete transport — `serve` + `remote` dependencies over sockets with the schema-level wire format. The `at` placement model is design-stage; see the open questions in distributed-model.md.

## Two-tier model

1. **Placement expressions (default)** — `at domain { expr }` evaluates an expression in another logical execution domain and yields its result. Synchronous from the caller's perspective; the physical mechanism is the deployment plan's concern.
2. **Channels (opt-in)** — for streaming, pub/sub, fan-out/fan-in, and decoupled producers/consumers.

Most code uses placement. Channels are the tool when a call/response shape is genuinely wrong.

## Placement

```pluto
let result = at payments {
    charge(order)
}
```

What the programmer is saying: *this computation belongs to the `payments` domain* — its data, capabilities, and failure envelope live there. What the programmer is **not** saying: how the boundary is physically crossed.

What the compiler does at the boundary:

- checks which values enter and leave, and that they are transferable (wire-shaped or otherwise legal to cross)
- verifies the domain provides the capabilities/services the block uses
- extends the caller's inferred error set with the boundary's failure contract — unreachability, deadline expiry, and ambiguous completion are part of the type-checked surface, never silent
- generates whatever the physical plan requires: serialization and a network call, IPC, or a direct call — with boundary semantics (isolation, secrets, cancellation behavior) preserved even when fused into one process

Crossing a boundary is explicit in the source. Code that reads as local *is* local.

### Errors are layered

Transport failures (socket reset, TLS, serialization) are translated to stage-level failures ("`payments` unreachable", "completion unknown", "deadline exceeded"), which application code maps into domain errors (`PaymentUnavailable`). Causes stay attached for diagnostics; low-level errors are not public control flow. See distributed-model.md.

## Channels

### When to use

- Streaming data (continuous flow of values)
- Pub/sub patterns
- Fan-out / fan-in
- Fire-and-forget
- Decoupling producer and consumer speeds

### Creating channels

Channels are **directional** — separate send and receive ends:

```
let ch = chan<Order>()

// ch.sender and ch.receiver are separate typed handles
```

### Sending and receiving

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

```
for msg in ch.receiver {
    process(msg)
}
```

### Physical plans for channels

Like `at`, a channel whose ends land in different domains gets a physical implementation chosen by the plan — in-memory queue, shared memory, or serialized network transport. The same preservation rule applies: the channel's semantics (ordering, failure modes, backpressure behavior) do not change with the plan, and a cross-domain channel's element type must be transferable.

### Auto-serialization

Any type crossing a physical process boundary must be wire-shaped. The compiler enforces this at compile time; the wire surface is schema-level and compiler-derived (see rfc-wire-format.md and rfc-schema.md).

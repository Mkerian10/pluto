# RFC: Distributed Safety

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-14
**Depends on:** `rfc-schema.md`, `rfc-concurrency-v2.md`
**Framing:** subordinate to [distributed-model.md](distributed-model.md) — the checks below apply to logical domain boundaries (`at` expressions), whatever physical transport the deployment plan chooses. Where this document says "RPC," read "a boundary whose physical plan chose a network transport.

## Summary

Extend Pluto's compile-time guarantees across process boundaries. When Pluto compiles a distributed system (multiple services in one compilation unit), the compiler verifies wire type compatibility, infers distributed error sets, validates the deployment topology, and guarantees that all cross-process communication is type-safe.

## Motivation

Every distributed system has the same class of bugs:

1. **Wire incompatibility.** Service A sends a message that Service B can't parse because the schema changed. Caught at 2 AM, not at compile time.
2. **Unhandled remote failures.** Service A calls Service B but doesn't handle the case where B is unreachable, times out, or returns an error A doesn't expect.
3. **Topology errors.** Service A calls an endpoint that doesn't exist, or expects a response type that doesn't match what the endpoint returns.
4. **Deployment races.** Service A is deployed with a new schema, but Service B still expects the old schema. The window between deployments is a correctness gap.

These bugs exist because services are compiled separately. Each service has its own type system, its own build, its own deployment. Nobody checks that the pieces fit together.

Pluto's whole-program compilation eliminates this class by compiling the entire system at once.

## Design

### Whole-Program Compilation Is the Foundation

Pluto already compiles entire applications — the `app` declaration, all its DI dependencies, all modules — in one compilation. Distributed safety extends this to **multi-app compilation**: multiple `app` declarations compiled together, with the compiler verifying their interactions.

```
// service_a.pluto
app ServiceA[handler: RequestHandler] {
    fn main(self) {
        serve(8080, self.handler)
    }
}

// service_b.pluto
app ServiceB[processor: EventProcessor] {
    fn main(self) {
        consume(self.processor)
    }
}

// The compiler sees both. It verifies their interactions.
```

### Cross-Process Communication

Services communicate across logical domain boundaries — placement expressions, message queues, event streams. The compiler type-checks both endpoints of every boundary regardless of the transport the plan selects.

**Stage pub methods** define the cross-process API:

```
stage OrderService {
    pub fn create(data: OrderData) string {
        // Implementation
    }

    pub fn get(id: string) Order {
        // Implementation
    }
}
```

When Service A calls `order_service.create(data)`, the compiler verifies:
- `OrderData` is a schema type (can cross the wire)
- The return type `string` is a schema type (can cross the wire)
- The `OrderService` stage exists in the compilation
- The method signature matches at both call site and definition

### Distributed Error Inference

Cross-process calls are inherently fallible. The compiler automatically infers additional error types for remote calls:

```
// Local call:
fn process(order: OrderData) string {
    let id = order_service.create(order)!    // Inferred: OrderService errors
    return id
}

// Cross-process call (same syntax, different inference):
fn process(order: OrderData) string {
    let id = order_service.create(order)!    // Inferred: OrderService errors + NetworkError
    return id
}
```

When the compiler determines that `order_service.create()` is a remote call (the stage runs in a different app/process), it automatically adds `NetworkError` to the inferred error set. `NetworkError` includes:

```
error NetworkError {
    Timeout,
    ConnectionRefused,
    ConnectionReset,
    Unreachable,
}
```

This means:
- **Local calls** within the same process infer the callee's direct error set (same as today)
- **Remote calls** across processes infer the callee's error set PLUS `NetworkError`
- The caller must handle both — `catch` handles all, `!` propagates all
- No annotation needed — the compiler infers it from the deployment topology

### Wire Type Validation

Every type that crosses a process boundary must be a schema. The compiler rejects:

```
// COMPILE ERROR: class types cannot cross process boundaries
stage BadService {
    pub fn get_service() UserService {    // UserService is a class, not a schema
        ...
    }
}
```

For schema types, the compiler generates serialization/deserialization code (marshaling) automatically. The marshal layer already exists (`src/marshal.rs`) — this RFC extends it with cross-process awareness.

### Topology Verification

The compiler knows the full deployment topology:
- Which apps exist
- Which stages each app runs
- Which apps communicate with which other apps
- What schemas flow between them

The compiler verifies:

1. **Endpoint existence.** Every remote call targets a stage that exists in the compilation.
2. **Signature match.** Parameters and return types match between call site and definition.
3. **Schema compatibility.** The serialized schema at the sender matches the expected schema at the receiver.
4. **Error completeness.** All possible errors (local + network) are handled by the caller.

### Schema Evolution Across Services

When a schema changes, the compiler checks all services that use it:

```
// Old: schema Order has field "total: float"
// New: schema Order has field "total_cents: int"

// Service A sends Order
// Service B receives Order
// The compiler sees both — it knows this is a breaking change
// and requires both services to be updated together
```

The migration system (`rfc-migration.md`) generates a deployment plan that accounts for cross-service schema changes:
- If the change is backward-compatible (added optional field), services can be deployed independently
- If the change is breaking (type change, removed field), services must be deployed together with a two-phase migration

### Consistency-Aware Compilation

When DI singletons are marked for replication across pods (see `rfc-concurrency-v2.md`, Phase 5), the compiler can reason about consistency:

- **`self` methods** (read-only) can read from local replicas — eventually consistent reads are safe for read-only operations
- **`mut self` methods** (write) must propagate — the compiler knows which operations need cross-pod consistency
- The consistency model (eventual, strong, causal) is configured in the orchestration layer, but the compiler verifies that the code's assumptions match the configured consistency

```
class LeaderboardCache {
    scores: Map<string, int>

    fn get_rank(self, user_id: string) int {
        // Read-only — safe to serve from local replica
        return self.scores[user_id] catch 0
    }

    fn update_score(mut self, user_id: string, score: int) {
        // Mutation — must propagate to other replicas
        self.scores[user_id] = score
    }
}
```

### Idempotency Inference

Remote calls can be retried (network failures, timeouts). The compiler can help identify safe retries:

- **Read-only calls** (`self` methods on stages) are inherently idempotent — safe to retry
- **Mutating calls** (`mut self` methods) may not be idempotent — the compiler flags retry without explicit idempotency handling

Future work: `@idempotent(key = order_id)` annotations that the compiler enforces (deduplication at the receiver).

## Implementation Phases

### Phase 1: Multi-App Compilation

**Scope:** Compiler infrastructure.

1. Support compiling multiple `app` declarations in one compilation unit
2. Build a cross-app call graph (which apps call which stages)
3. Validate endpoint existence and signature matching
4. No runtime changes — just compile-time checking

### Phase 2: Distributed Error Inference

**Scope:** Extend error inference across process boundaries.

1. Identify remote call sites (calls to stages in other apps)
2. Add `NetworkError` to the inferred error set for remote calls
3. Enforce handling at call sites (same as local error handling)
4. Generate error types for each remote endpoint (union of callee errors + NetworkError)

### Phase 3: Wire Type Safety

**Scope:** Schema validation across services.

1. Verify all cross-process parameters and return types are schemas
2. Generate marshaling code for cross-process schema types
3. Validate schema compatibility between sender and receiver
4. Reject changes that break wire compatibility without migration

### Phase 4: Deployment Topology

**Scope:** Cross-service deployment planning.

1. Build the full deployment topology from app declarations and stage dependencies
2. Generate deployment ordering (which services deploy first)
3. Integrate with migration system for cross-service schema changes
4. Generate deployment manifests that include service rollout order

### Phase 5: Consistency Verification

**Scope:** Distributed state consistency.

1. Track replicated singletons across pods
2. Verify read/write patterns match the configured consistency model
3. Flag potential consistency issues (e.g., reading stale data after a write on another pod)
4. Generate replication configuration from code analysis

## Interaction with Other Features

### Error Handling

Distributed error inference composes with local error inference:
- Local errors: inferred from `raise` statements
- Remote errors: local errors + `NetworkError`
- The union of both is propagated through `!` or handled with `catch`

### Contracts

Contracts on stage methods apply across process boundaries:
- `requires` on a stage method is checked at the caller (before serialization)
- `invariant` on a schema is checked at construction and after mutation (before serialization)
- Contracts travel with the type — they're enforced wherever the type is used

### Concurrency

Cross-process calls are inherently concurrent (async network I/O). The concurrency model applies:
- Remote calls can be spawned for parallel execution
- Copy-on-spawn applies to schemas sent to remote calls
- Channel-based communication across processes uses the same channel semantics as local channels

## Prior Art

- **Erlang/OTP:** Distribution built into the runtime. Process communication works across nodes. But no compile-time type checking — messages are dynamically typed.
- **gRPC/Protobuf:** Schema-based cross-service communication with code generation. But schemas are defined separately from code, versioned independently, and type checking happens at build time per service (not whole-system).
- **Session types:** Formal type systems for communication protocols. Verify that two parties follow a compatible sequence of sends/receives. Pluto's approach is simpler — verify type compatibility at each call site rather than modeling the full protocol.
- **Unison:** Content-addressed code with no builds. Functions are identified by hash, not name. Relevant to the idea of "compile the whole system," but focuses on code identity rather than distributed safety.

## Open Questions

- [ ] **Service boundaries.** How does the compiler know which apps run in which processes/pods? Is this explicit in the source, or configured externally?
- [ ] **Versioned APIs.** Should Pluto support API versioning for gradual migration? Or does whole-program compilation eliminate the need? (Whole-program compilation means all services update together, so versioning is less necessary.)
- [ ] **Third-party services.** How does Pluto handle calls to services outside the compilation unit (external APIs, legacy systems)? These are opaque — the compiler can't verify them. `extern` stage declarations with explicit schema types could work.
- [ ] **Scale.** Compiling 50 microservices in one compilation unit is ambitious. Are there performance implications? Can the compiler parallelize cross-app analysis?
- [ ] **Partial compilation.** Can you compile a subset of services for faster iteration? What guarantees are lost when not compiling the full system?

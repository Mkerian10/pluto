# RFC: RPC Code Generation Implementation

> **Status:** Planning
> **Priority:** CRITICAL — Blocks multi-service distribution
> **Effort:** 2-3 weeks
> **Date:** February 2026

## Executive Summary

This RFC outlines a phased approach to implementing RPC code generation in Pluto. Cross-pod function calls currently look identical to local calls, but the compiler must generate serialization, network transport, and error handling code for remote calls.

**Objective:** Enable the same Pluto code to transparently work across service boundaries, with the compiler automatically generating RPC infrastructure.

---

## Background

### Current State
- Local function calls fully implemented
- Design exists in `docs/design/communication.md`
- Type system, error handling, and DI all support remote calls conceptually
- No serialization, transport, or RPC codegen yet

### Design Principles
1. **Transparency:** Cross-pod calls look like local calls to the programmer
2. **Compiler-driven:** Whole-program analysis determines pod boundaries
3. **Type safety:** All RPC payloads are type-checked at compile time
4. **Error inference:** Network errors are inferred and must be handled
5. **Optimization:** Same-pod calls compile to direct function calls (zero overhead)

---

## Phase 1: Wire Format & Serialization (1 week)

### Goals
- Define binary serialization format for all Pluto types
- Implement encoder/decoder trait and basic implementations
- Support primitives (int, float, bool, string, byte)

### Tasks
1. **Define wire format specification** (`docs/design/rfc-wire-format.md`)
   - Header (magic, version, type descriptor)
   - Primitive encoding (big-endian, IEEE 754 for float)
   - String format (length-prefixed UTF-8)
   - Composite types (classes, arrays, maps)
   - Null representation for nullable types

2. **Create `std.wire` module**
   - `WireValue` enum (wrapper for all types)
   - `Encoder` trait → `fn encode(self, WireValue)`
   - `Decoder` trait → `fn decode(WireValue) -> self?`
   - Builtin implementations for primitives and collections

3. **Implement codegen for struct serialization**
   - Automatic `impl Encoder` for classes (in codegen)
   - Automatic `impl Decoder` for classes (in codegen)
   - Field order consistency check

4. **Add tests**
   - Encode/decode roundtrip for each type
   - Interop with other services (future)
   - Error cases (malformed input)

### Success Criteria
- [ ] `std.wire` module exists and documents protocol
- [ ] Primitives and collections serialize/deserialize correctly
- [ ] 20+ integration tests for roundtrip
- [ ] All Pluto types can be represented in wire format

---

## Phase 2: Local Service Model (1 week)

### Goals
- Define the service/pod model in Pluto
- Mark service boundaries via `stage` or `service` declaration
- Implement pod topology analysis in compiler

### Tasks
1. **Design service declaration syntax**
   ```pluto
   stage UserService {
       class User { id: int, name: string }
       pub fn get_user(id: int) User? { ... }
       fn verify(u: User) bool { ... }
   }
   ```
   - `stage` = deployable unit
   - `pub` methods = RPC endpoints
   - Private methods = local-only
   - Bracket deps for DI across services

2. **Implement parser changes**
   - Add `stage` keyword to lexer
   - Parse stage declaration with body

3. **Add to typeck**
   - Register stages separately from regular functions
   - Validate `pub` visibility rules
   - Check that cross-stage calls only use pub methods

4. **Codegen preparation**
   - Generate "service descriptor" metadata
   - Mark which calls are local vs remote

### Success Criteria
- [ ] Parser accepts `stage` declarations
- [ ] Typeck validates stage visibility rules
- [ ] Metadata correctly identifies local vs remote calls
- [ ] 10+ tests for stage parsing and visibility

---

## Phase 3: RPC Codegen (1 week)

### Goals
- Generate serialization/deserialization code for RPC calls
- Implement basic HTTP transport skeleton
- Handle error propagation over RPC

### Tasks
1. **RPC call codegen**
   - For remote calls: serialize args → send via transport → deserialize result
   - For local calls: direct function call (zero overhead)
   - Error handling: network errors become new error types

2. **Error extension**
   - Remote calls can fail with: `NetworkError`, `TimeoutError`, `ServiceUnavailable`
   - These are inferred by compiler and propagated via `?`

3. **HTTP transport skeleton** (`std.rpc`)
   - `Client { endpoint: string }` class
   - `call<T>(method: string, args: [u8]) T?`
   - Connection pooling (future)

4. **Compiler integration**
   - During codegen, detect cross-stage calls
   - Rewrite to `RPC_call(stage_name, method, serialized_args)`
   - Deserialize and handle errors

### Success Criteria
- [ ] Cross-stage function calls compile to RPC code
- [ ] Same-stage calls remain direct (no overhead)
- [ ] Error types extended with network failures
- [ ] 15+ integration tests for RPC calls

---

## Phase 4: Service Discovery (1 week)

### Goals
- Enable runtime location of services
- Support multi-environment deployment (dev, staging, prod)

### Tasks
1. **Service registry design**
   - Configuration format (YAML/TOML with stage names, endpoints)
   - Runtime lookup by stage name → endpoint URL

2. **Config file format**
   ```toml
   [services]
   UserService = "http://localhost:8001"
   OrderService = "http://localhost:8002"
   ```

3. **Compiler changes**
   - Embed stage descriptors in binary
   - At runtime, load config to resolve endpoints

4. **DI integration**
   - Bracket deps can reference stages: `class Handler[users: UserService]`
   - DI system loads config and wires remote service clients

### Success Criteria
- [ ] Config format defined and documented
- [ ] Runtime service lookup works
- [ ] DI system creates RPC clients from stage deps
- [ ] 10+ tests for service discovery

---

## Phase 5: Multi-Service Binary (1 week)

### Goals
- Build executable that spans multiple stages
- Each stage compiles to separate binary or isolated process
- Verify RPC works end-to-end

### Tasks
1. **Multi-service compilation**
   - `plutoc compile system.pluto` produces bin/stage_A, bin/stage_B, etc.
   - Each binary is self-contained and stateless (except DI)

2. **Testing harness**
   - Spawn multiple binaries in test
   - Verify RPC calls work across them
   - Test error handling (service down, timeout, etc.)

3. **Documentation**
   - "Building a Multi-Service System" tutorial
   - Example: Order service → User service → Payment service

### Success Criteria
- [ ] Can compile multi-stage system
- [ ] Each stage binary runs independently
- [ ] RPC calls work across stages
- [ ] Comprehensive example included

---

## Implementation Strategy

### Timeline
- **Week 1:** Phase 1 (wire format)
- **Week 2:** Phases 2-3 (services, RPC codegen)
- **Week 3:** Phases 4-5 (discovery, multi-service)

### Milestones
1. `cargo test` passes with wire format tests
2. `stage` declarations parse and validate
3. Cross-stage calls generate RPC code
4. Multi-service example runs end-to-end

### Risk Mitigation
- **Wire format instability:** Finalize spec before implementation, version protocol
- **Scope creep:** Skip advanced features (compression, batch RPC, streaming) in Phase 1
- **Integration complexity:** Build up incrementally; test at each phase

---

## Future Enhancements (Out of Scope for MVP)

1. **Message compression** — gzip or snappy for large payloads
2. **Batch RPC** — send multiple calls in one request
3. **Streaming RPC** — use channels for bidirectional streams
4. **gRPC integration** — compile to gRPC protobuf
5. **Service mesh** — Istio/Linkerd observability
6. **Load balancing** — multiple instances per service
7. **Circuit breakers** — fault tolerance and retry logic
8. **Distributed tracing** — OpenTelemetry integration

---

## Testing Strategy

### Unit Tests
- Wire format encode/decode for each type
- Service descriptor generation
- Call graph analysis (local vs remote)

### Integration Tests
- Two-stage system with RPC calls
- Error handling (network failure, timeout)
- Type mismatches (compile error)
- Multi-service with complex DI chains

### End-to-End Tests
- Full example system (3+ services)
- Config-driven service discovery
- Load testing (100+ RPC calls/sec)

---

## Rollout Plan

1. **Initial PR:** Wire format + `std.wire` module
2. **Second PR:** `stage` declaration + typeck validation
3. **Third PR:** RPC codegen + HTTP transport
4. **Fourth PR:** Service discovery + config system
5. **Final PR:** Multi-service example + documentation

---

## Open Questions

1. **Service mesh:** Should we eventually support Istio/Linkerd, or keep it simple?
2. **Serialization format:** Stick with custom binary, or use protobuf/Cap'n Proto?
3. **Transport:** HTTP only, or support gRPC/QUIC in future?
4. **Service naming:** Flat namespace (UserService) or hierarchical (api.UserService)?
5. **Code organization:** Should stages live in separate `.pluto` files?

---

## References

- `docs/design/communication.md` — Overall communication model
- `docs/design/program-structure.md` — Stage vs app semantics
- `docs/design/dependency-injection.md` — DI with stages

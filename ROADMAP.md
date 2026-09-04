# Pluto Language Roadmap

**Last Updated:** 2026-09-04
**Vision:** A domain-specific language for distributed backend systems with geographic awareness

See also: [docs/v1-vision.md](docs/v1-vision.md) for the long-form vision and [docs/design/](docs/design/) for individual RFCs.

---

## Vision

Pluto makes building **distributed, geographically-aware backend systems** as simple as writing a monolith. The compiler handles the complexity of:

- **Cross-service RPC** — calls across services look identical to local calls, type-checked across the boundary
- **Dependency injection** — compile-time resolution and wiring, language-level lifecycles
- **Typed errors** — compiler-inferred fallibility with enforced handling, across service boundaries
- **Schema-level wire format** — serialization derived from types, never hand-written; evolution checkable by the compiler
- **AI-native tooling** — semantic binary representation with stable UUIDs, SDK for agents

**Core principles:**

1. **Correctness by default** — contracts, type safety, enforced error handling
2. **Distributed-first** — RPC, service boundaries, and wire schemas are language concerns
3. **Compiler does the hard work** — whole-program analysis, automatic serialization, dependency wiring
4. **Explicit over implicit** — clear syntax, predictable behavior, no magic
5. **The wire is schema-level** — a closed, compiler-derived set of shapes; no custom encoder hooks, so interface hashing and evolution checking always hold

---

## Where we are (September 2026)

The February roadmap's "v0.2 Production Foundations" and "Distributed Systems MVP" milestones are **complete**, along with most of what was planned for Q3/Q4:

### Language
- Core language: functions, classes (no inheritance), traits, enums + match (incl. wildcard arms, field binding/renaming), closures, function references, generics (monomorphized), if/match as expressions, methods on primitives, f-strings, nullable types with flow narrowing and `??`
- **Generics completed** — generic traits, generic trait methods, generic classes implementing generic traits, explicit type args with argument validation
- **Error handling completed** — typed errors, `raise`/`!`/`catch` (shorthand, wildcard, typed with coverage), fixed-point fallibility inference through closures, function references, generics (skolem-checked pre-monomorphization), and function-type contracts (`fn(int) int!`)
- **Mutability enforced** — `let mut`, `mut self`, `mut` params; immutable bindings, loop vars, and match bindings enforced
- **Contracts** — class invariants and `requires` clauses with runtime checks. `ensures` was **eliminated by design** (invariants + return types cover it; see docs/design/contracts.md)
- **Concurrency** — spawn/Task, directional channels, select, synchronized state; concurrent mark-sweep GC with safe-region STW protocol

### Distributed systems
- **RPC over sockets** — `serve` generates the server (accept loop, dispatch, framing); `remote` deps make typed cross-service calls; `PLUTO_REMOTE_<SVC>` endpoint config
- **Wire type surface** — int, float, bool, string, classes, enums, arrays, maps, sets, and nullables of all of these, composing recursively, as arguments, returns, and error fields
- **Version-skew safety** — interface hashing on every call; mismatched client/server rejected at the boundary
- **Cross-boundary errors** — typed errors propagate over RPC and reconstruct on the caller's side; `NetworkError` inferred on every remote call
- **Stage declarations** and app/system topology for multi-service programs

### Platform & tooling
- **19 stdlib modules** — base64, collections, env, fs, http (client + server), io, json, log, math, net, path, random, regex, rpc, socket, strings, time, uuid, wire
- **Package manager** — `pluto.toml` manifests, path and git dependencies, transitive resolution, `pluto update`
- **Toolchain management** — `pluto install/use/versions`, experimental release channel with a hardened CI gate (nextest + doc-tests)
- **Dev loop** — `pluto watch`, `pluto coverage`, built-in test framework with test-local DI containers (scope blocks may seed singleton fakes)
- **AI-native foundations** — binary `.pluto` container with stable UUIDs (`emit-ast`, `generate-pt`, `sync`, `analyze`), `pluto-sdk` for programmatic editing, read-only MCP server (docs, check, compile, run, test) with hot-reload supervision

### Quality
- ~6,300 tests green in CI; ignored tests down from ~2,000 to 75, every one carrying a documented reason (they are the feature-gap ledger)
- Vacuous-test audits eliminated all empty should-fail expectations; audits and triage surfaced and fixed a dozen real compiler bugs
- Property tests, snapshot-tested error messages, lexer/parser fuzzing, compiler and runtime benchmarks

---

## Current milestone: v0.3 — Developer Experience

**Focus:** make Pluto pleasant to use from an editor, and finish the loose ends the distributed push left behind.

- [ ] **LSP server** — `pluto lsp` on the existing CompilerService backend: diagnostics, hover, go-to-definition
- [ ] **Code formatter** — `pluto fmt` (the pretty-printer already exists for `generate-pt`; formatting is an extension of it)
- [ ] **Release tooling round 2** — `pluto --version` build stamping, generated release notes, nightly dispatch
- [ ] **Wire round 3** — generic class instances (e.g. `Box<int>`) as RPC types and container elements
- [ ] **TaskCancelled inference** — model cancellation as a fallible outcome of `task.get()` (see docs/design/open-questions.md)
- [ ] **Docs consolidation** — three overlapping doc systems (SPEC.md, spec/ formal book, book/ user guide) need one story

## Next milestone: v1.0 — Production Readiness

- [ ] **Incremental compilation** — build cache (design: docs/design/build-cache.md)
- [ ] **Observability hooks** — metrics, tracing, structured logging for served services
- [ ] **Supervision strategies** — crash recovery for long-running services
- [ ] **Structured concurrency** — task groups and scopes
- [ ] **Ignored-test ledger to zero** — burn down the 75 documented feature gaps (temporal safety, ambiguous-inference detection, octal literals, …)
- [ ] **Documentation complete** — installation, tutorial, stdlib reference, distributed-systems guide

---

## Long-term vision (2027+)

### Geographic distribution (the differentiator)
- Geographic annotations (`@region("us-east")`), data-locality enforcement (GDPR residency), latency-aware routing, cross-region failover — deploy close to users with compiler-checked constraints. See docs/design/vision-safety-stack.md.

### AI-native representation (in progress on long-lived branches)
- Binary `.pluto` as the **canonical** format; `.pt` text as human-readable views with `pluto sync` reconciliation; agents edit through `pluto-sdk`. Phase 1 (stable AST UUIDs) and the canonical flip are active on the `ast-uuids` and `canonical-flip` branches. See docs/design/ai-native-representation.md.

### Advanced contracts
- Quantifiers (`forall item in items: …`), protocol/state-machine contracts, static verification of a decidable subset, contract-derived test generation. (Note: `ensures` stays out — that decision is settled.)

### Schema evolution & migration
- Compiler-checked rolling deploys: interface hashes today; full evolution rules and data migration as designed in docs/design/rfc-evolution-rules.md and rfc-migration.md.

---

## Principles for what gets built next

- User needs drive priorities; foundational features before advanced ones
- Ship incrementally behind tests; CI is the gate, master stays green
- Settled design decisions stay settled unless new evidence appears — notably: no inheritance, no `ensures`, schema-level wire only, typed errors over sum types
- Dates are targets, not commitments — quality over deadlines

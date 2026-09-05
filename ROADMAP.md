# Pluto Language Roadmap

**Last Updated:** 2026-09-04

The canonical statement of where Pluto is going is **[docs/v1-vision.md](docs/v1-vision.md)** — the rocket-engine vision: whole-program compilation of entire distributed systems, contracts as compile-time proofs, and distributed safety the way Rust delivered memory safety. This file tracks where we are against that vision's roadmap. When they disagree, the vision wins.

---

## The pillars (from the vision)

1. **Whole-program compilation** — the compiler sees every service, boundary, and data flow as one program
2. **Static verification** — contracts as compile-time proofs; ownership as contract patterns (`owns`, `holds_lease`, `is_leader`); typestates via generics; auto-executing `ensures`
3. **Objects vs classes** — classes are values (structural `==`, copied at boundaries); objects are entities (identity, shared, serialized messages, possibly remote). Both compose through traits — inheritance was considered and rejected (rfc-objects.md)
4. **DI-driven topology** — wiring determines what's local and what's remote; moving a service out is a topology change, not a code change
5. **Infrastructure in the type system** — k8s/SQL/Terraform artifacts as typed compile-time inputs, validated against service code
6. **Migrations as a language capability** — schema diffing, change classification, compiler-checked evolution across the whole system

## Near-term (per the vision)

- [ ] **Static verification engine** — contracts as compile-time proofs; typestates via generics; auto-executing ensures. (Today: runtime-checked invariants and `requires`; the decidable-fragment validator in `src/contracts.rs` is the seed. Runtime-`ensures`-as-assertion was rejected — `ensures` returns as a *proof* obligation, not a runtime check.)
- [ ] **Object construct** — phase 1 (identity semantics, by-construct safe sharing, value/entity == split) shipped; next: identity handles across domains. Inheritance rejected (rfc-objects.md).
- [ ] **Program structure** — bare file to distributed system with no ceremony threshold; objects and their dependency graph *are* the program (no mandatory `app`/`main`).
- [ ] **DI-driven topology & the placement model** — `at` expressions over logical execution domains; the wiring and deployment plan, not the code, decide physical boundaries, and the compiler synthesizes transport, serialization, and error paths at each one (docs/design/distributed-model.md). (Today: `app`/stage DI with lifecycles is compile-time-wired; `remote` deps + `serve` exist but couple the transport into the code — exactly what this pillar removes.)

## Mid-term (per the vision)

- [ ] LSP server and editor integration (CompilerService backend exists)
- [ ] `pluto fmt`; package registry + lock file (manifests with path/git deps shipped)
- [ ] Infrastructure as Pluto code — topology declarations, deploy-time validation
- [ ] Stdlib expansion — database drivers, messaging, crypto, observability
- [x] Cross-boundary type checking and serialization codegen — shipped for the socket transport: interface hashing against version skew, schema-derived wire marshaling (scalars, classes, enums, nullables, arrays, maps, sets), typed errors across boundaries
- [ ] Cloud infrastructure APIs — type-safe Kubernetes and AWS interfaces
- [ ] Distributed contract predicates — stdlib ownership/lease/leader patterns
- [ ] Migration engine — schema diffing, validation, generation (design: docs/design/rfc-migration.md, rfc-evolution-rules.md)

## Long-term (per the vision)

Incremental compilation · advanced verification (protocol contracts, bounded quantifiers) · geographic annotations and placement constraints · derived observability (traces/metrics/topology from language constructs) · comprehensive docs · real-world hardening.

**1.0** = all pillars end-to-end, pristine DX, batteries-included stdlib with cloud-native APIs, and at least one real distributed system built entirely in Pluto.

---

## Foundation already in place (September 2026)

The substrate the pillars build on is largely shipped and tested (~6,300 tests in CI):

- **Language core** — classes/traits/enums (wildcard + destructuring match), closures and function references, full generics (generic traits, generic trait methods), nullable types with flow narrowing, f-strings, explicit mutability, methods on primitives
- **Errors** — typed errors with whole-program inferred fallibility (through closures, generics, function types), `!`/`catch` enforcement — the vision's error model, working today
- **DI** — compile-time resolution and wiring, singleton/scoped/transient lifecycles, captive-dependency detection, test-local containers
- **Distributed** — `serve`/`remote` over sockets as the first physical transport, with the full schema-level wire surface; **the wire is schema-level only** (closed, compiler-derived shapes — no custom encoder hooks; settled 2026-09-04). The target programming model is `at` placement over logical domains (docs/design/distributed-model.md); today's explicit RPC declarations are the mechanism it subsumes
- **Concurrency & runtime** — spawn/Task, channels, select, synchronized state, concurrent mark-sweep GC; native compilation via Cranelift
- **Toolchain** — `compile/run/test/watch/coverage/check`-style dev loop, package manifests with git deps, toolchain versioning (`install/use/versions`), experimental release channel
- **AI-native foundations** — binary `.pluto` container with stable UUIDs, `emit-ast`/`generate-pt`/`sync`/`analyze`, `pluto-sdk`, read-only MCP server; canonical-flip work active on the `ast-uuids`/`canonical-flip` branches

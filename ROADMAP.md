# Pluto Language Roadmap

**Last Updated:** 2026-02-11
**Vision:** A domain-specific language for distributed backend systems with geographic awareness

See also: [FEATURES.md](FEATURES.md) for detailed feature tracking | [BUGS_AND_FEATURES.md](BUGS_AND_FEATURES.md) for bugs

---

## 🎯 Vision

Pluto is designed to make building **distributed, geographically-aware backend systems** as simple as writing a monolith. The compiler handles the complexity of:
- **Cross-pod RPC** - Function calls across services look identical to local calls
- **Dependency injection** - Compile-time resolution, environment-specific wiring
- **Contracts** - Runtime-verified correctness guarantees
- **Geographic distribution** - Deploy code close to users, enforce locality constraints
- **AI-native tooling** - Semantic representation optimized for AI agent collaboration

**Core Principles:**
1. **Correctness by default** - Contracts, type safety, error handling
2. **Distributed-first** - RPC, service boundaries, geographic awareness built into the language
3. **Compiler does the hard work** - Whole-program analysis, automatic serialization, dependency wiring
4. **Explicit over implicit** - Clear syntax, predictable behavior, no magic
5. **AI collaboration** - Canonical representation with stable UUIDs, SDK for agents

---

## 🚀 Current Milestone: **v0.2 - Production Foundations**

**Target:** Q2 2026
**Focus:** Make Pluto viable for real projects - stability, ergonomics, core distributed features

### Must-Have for v0.2
- ✅ Trait method validation (PR #43)
- [ ] **Fix critical bugs** (errors in closures, immutable reassignment, trait-field coercion)
- [ ] **Unannotated empty array literals** - Blocks too much natural code
- [ ] **If/Match as expressions** - Essential for functional style
- [ ] **HTTP client** - Replace stub with real implementation
- [ ] **RPC Phase 1** - Wire format + std.wire module
- [ ] **RPC Phase 2** - Stage declarations + service model
- [ ] **Package manager foundations** - Basic dependency resolution

### Nice-to-Have for v0.2
- [ ] Methods on primitives (`42.to_string()`)
- [ ] Field binding in match arms
- [ ] Binary/scientific notation literals
- [ ] Code formatter (`plutoc fmt`)

**Success Metrics:**
- Build a non-trivial distributed app (multi-service)
- Zero P0 bugs
- <5 P1 bugs
- Documentation complete (installation, tutorial, stdlib reference)

---

## 📅 Quarterly Roadmap

### Q1 2026 (Jan-Mar) - **Stability & Ergonomics** ✅ In Progress

**Theme:** Fix blockers, improve developer experience

**Completed:**
- ✅ Static trait calls parser support (PR #41)
- ✅ Trait method validation (PR #43)
- ✅ GitHub Pages deployment for documentation
- ✅ Improved error messages and test coverage

**In Progress:**
- 🔵 RPC Phase 1 - Wire format design
- 🔵 Compile-time reflection Phase 1

**Remaining:**
- [ ] Fix errors in closures bug (P0)
- [ ] Fix immutable reassignment bug (P1)
- [ ] Unannotated empty array literals
- [ ] If/Match as expressions

### Q2 2026 (Apr-Jun) - **Distributed Systems MVP**

**Theme:** Core RPC features for multi-service deployments

**Goals:**
- [ ] **RPC Phase 1-3** - Wire format, stage declarations, codegen
- [ ] **HTTP client** - Real implementation
- [ ] **Service discovery basics** - Static config file
- [ ] **Package manager** - Dependency resolution + lock files
- [ ] **Documentation** - Distributed systems guide

**Deliverable:** Build a real distributed app with 3+ services communicating via RPC

### Q3 2026 (Jul-Sep) - **Developer Experience**

**Theme:** Tooling, ergonomics, quality of life

**Goals:**
- [ ] **Code formatter** - `plutoc fmt`
- [ ] **Linter** - Static analysis for common mistakes
- [ ] **LSP foundations** - Autocomplete, go-to-definition
- [ ] **Improved error messages** - More context, suggestions
- [ ] **Methods on primitives** - `42.to_string()`
- [ ] **Reflection Phase 2** - Loop unrolling, compiler transforms
- [ ] **JsonEncoding** - Auto-generated serialization

**Deliverable:** Smooth onboarding experience for new users

### Q4 2026 (Oct-Dec) - **Production Readiness**

**Theme:** Performance, observability, reliability

**Goals:**
- [ ] **Incremental compilation** - Faster builds
- [ ] **Compiler metrics** - Performance profiling
- [ ] **Observability hooks** - Metrics, tracing, logging
- [ ] **Supervision strategies** - Crash recovery (Erlang-style)
- [ ] **Structured concurrency** - Task groups, scopes
- [ ] **Move semantics** - Prevent shared mutable state bugs
- [ ] **Contract enforcement** - Requires/ensures runtime checks

**Deliverable:** Run Pluto services in production with confidence

---

## 🏔️ Long-Term Vision (2027+)

### Geographic Distribution (The Differentiator)

**Vision:** Deploy code close to users, enforce locality constraints automatically

**Features:**
- **Geographic annotations** - `@region("us-east") class UserService`
- **Multi-region deployment** - Compiler-assisted orchestration
- **Data locality** - Enforce data residency rules (GDPR, etc.)
- **Automatic failover** - Cross-region redundancy
- **Latency-aware routing** - Send requests to nearest instance

**Impact:** Makes geo-distribution as easy as deploying to one region

### AI-Native Tooling (The Future)

**Vision:** Canonical binary representation optimized for AI collaboration

**Features:**
- **Binary `.pluto` format** - Stable UUIDs, semantic graph representation
- **Human-readable `.pt` views** - Text files for humans, `plutoc sync` reconciles
- **`plutoc-sdk`** - Python/TS bindings for AI agents
- **Incremental analysis** - planned `plutoc analyze` command computes derived data on demand
- **Collaborative editing** - Multiple AI agents, same codebase (CRDT-based)

**Impact:** AI agents as first-class contributors to Pluto projects

### Advanced Contracts (Correctness at Scale)

**Features:**
- **Quantifiers** - `forall item in items: item.valid()`
- **Protocol contracts** - State machine enforcement (open → read → close)
- **Static verification** - Prove correctness at compile-time (subset of contracts)
- **Contract testing mode** - Generate test cases from contracts

**Impact:** Catch bugs before deployment, not in production

---

## 🎓 Milestones Completed

### v0.1 (2025-2026) - **Language Foundations** ✅

**Highlights:**
- ✅ Core language features (functions, classes, traits, enums, generics)
- ✅ Module system with visibility
- ✅ Closures with capture-by-value
- ✅ Dependency injection (compile-time)
- ✅ Error handling (`!`, `catch`, `raise`)
- ✅ Concurrency (spawn, Task, channels, select)
- ✅ Contracts Phase 1-3 (invariants, requires/ensures, trait contracts)
- ✅ Nullable types (`T?`, `none`, `?` operator)
- ✅ String interpolation (f-strings)
- ✅ Maps and Sets
- ✅ Test framework
- ✅ Stdlib modules (strings, math, json, http, fs, collections, log, time)
- ✅ Compilation to native code (Cranelift)

**Shipped:** Jan 2026

---

## 📊 Progress Tracking

### By Theme

| Theme | Status | Progress |
|-------|--------|----------|
| **Core Language** | ✅ Complete | 100% |
| **Type System** | ✅ Complete | 100% |
| **Error Handling** | ✅ Complete | 100% |
| **Concurrency** | 🟡 Phase 1 | 60% (Phase 2: structured concurrency pending) |
| **Contracts** | 🟡 Phase 3 | 60% (enforcement, quantifiers pending) |
| **Dependency Injection** | 🟡 Phase 1 | 70% (scoped services pending) |
| **RPC/Distribution** | 🔵 Starting | 5% (wire format in progress) |
| **Reflection** | 🔵 Phase 1 | 20% (parser done, typeck/codegen pending) |
| **Tooling** | 🟡 Basic | 30% (formatter, linter, LSP pending) |
| **Geographic** | ⬜ Not Started | 0% |
| **AI-Native** | 🟡 Foundations | 20% (PLTO container + emit/sync/sdk shipped; analyze workflow pending) |

### By Quarter

| Quarter | Theme | Completion |
|---------|-------|------------|
| Q4 2025 | Foundation | ✅ 100% |
| Q1 2026 | Stability | 🔵 60% (in progress) |
| Q2 2026 | Distributed | ⬜ 0% (planned) |
| Q3 2026 | Tooling | ⬜ 0% (planned) |
| Q4 2026 | Production | ⬜ 0% (planned) |

---

## 🎯 Success Criteria

### v0.2 Success Criteria (Q2 2026)
- [ ] Build a real distributed app (3+ services, RPC communication)
- [ ] Zero P0 bugs, <5 P1 bugs
- [ ] Documentation complete (tutorial, stdlib docs, distributed guide)
- [ ] 10+ external users trying Pluto
- [ ] Package manager with 10+ published libraries

### v1.0 Success Criteria (Q4 2026)
- [ ] 100+ external users
- [ ] 3+ production deployments
- [ ] <1% compiler crash rate
- [ ] Complete stdlib (http, db, queues, caching)
- [ ] LSP with IDE support (VSCode, IntelliJ)
- [ ] Incremental compilation (<1s for small changes)

### v2.0 Success Criteria (2027)
- [ ] Geographic distribution in production
- [ ] AI-native tooling (binary format, SDK)
- [ ] Static contract verification (subset)
- [ ] 1000+ users, 50+ production deployments

---

## 🔄 Review Cadence

**Weekly:**
- Feature progress updates
- Bug triage (P0/P1 only)
- Blocker resolution

**Monthly:**
- Roadmap review
- Milestone progress assessment
- Reprioritization based on user feedback

**Quarterly:**
- Major milestone retrospective
- Next quarter planning
- Long-term vision refinement

---

## 📢 Community & Feedback

**How to influence the roadmap:**
1. Open an issue on GitHub
2. Propose features in [FEATURES.md](FEATURES.md)
3. Share use cases that aren't well-served
4. Contribute PRs for features you need

**Roadmap principles:**
- User needs drive priorities
- Foundational features before advanced features
- Ship incrementally, gather feedback, iterate
- Maintain high code quality (no technical debt shortcuts)

---

## 📝 Notes

- Dates are targets, not commitments - quality over deadlines
- Priorities may shift based on user feedback and blockers
- Long-term vision (2027+) is aspirational, timelines fluid
- This roadmap is reviewed and updated monthly

**Last reviewed:** 2026-02-11
**Next review:** 2026-03-11

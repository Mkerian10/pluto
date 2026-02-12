# Pluto Feature Tracker

**Last Updated:** 2026-02-11
**Purpose:** Detailed feature tracking with priorities, effort estimates, and implementation status

See also: [ROADMAP.md](ROADMAP.md) for high-level goals and milestones

---

## 📊 Status Key

- 🟢 **Ready** - Designed, ready to implement
- 🟡 **Design** - Needs design/RFC before implementation
- 🔴 **Blocked** - Waiting on other work
- 🔵 **In Progress** - Currently being implemented
- ✅ **Done** - Completed

**Priority:** P0 (Critical) | P1 (High) | P2 (Medium) | P3 (Low)
**Effort:** S (Small, <1 week) | M (Medium, 1-3 weeks) | L (Large, 1-2 months) | XL (Epic, >2 months)
**Impact:** High | Medium | Low

---

## 🔥 P0 - Critical Features (Must Have)

### Language Features

#### 1. Empty Array Literals
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** High
- **Description:** Support `let arr: [int] = []` with type inference from annotation
- **Rationale:** Common pattern, blocks natural code. Workaround: `let arr = [0]; arr = []` is clunky
- **Tests:** 7 ignored tests in `tests/codegen/_12_edge_cases.rs`, `_01_type_representation.rs`
- **Implementation:** Typeck enhancement to infer element type from context

#### 2. If-as-Expression
- **Status:** 🟢 Ready
- **Effort:** M
- **Impact:** High
- **Description:** `let x = if cond { a } else { b }`
- **Rationale:** Reduces verbosity, enables functional style
- **Tests:** `tests/codegen/_05_control_flow.rs:174`, `_13_codegen_correctness.rs:487`
- **Dependencies:** None
- **Implementation:** Parser + typeck to treat if/else as expression when all branches return same type

#### 3. Match-as-Expression
- **Status:** 🟢 Ready
- **Effort:** M
- **Impact:** High
- **Description:** `let x = match y { A => 1, B => 2 }`
- **Rationale:** Natural extension of if-as-expression
- **Tests:** `tests/codegen/_05_control_flow.rs:605`, `:973`
- **Dependencies:** If-as-expression (similar implementation)

### Runtime

#### 4. HTTP Client Implementation
- **Status:** 🟢 Ready
- **Effort:** M
- **Impact:** High
- **Description:** Real HTTP client using libcurl or sockets (replace stub)
- **File:** `runtime/builtins.c:4423`
- **Rationale:** Needed for real-world applications
- **Implementation:** Link libcurl, implement GET/POST/PUT/DELETE methods

### RPC (Distributed Systems)

#### 5. Wire Format (Phase 1)
- **Status:** 🔵 In Progress
- **Effort:** M
- **Impact:** High
- **Description:** Serialize/deserialize all Pluto types for RPC
- **RFC:** `docs/design/rfc-rpc-implementation.md`
- **Deliverables:**
  - `std.wire` module
  - Serialize: int, float, bool, string, arrays, classes, enums, maps, sets, nullables
  - Deserialize with error handling
- **Next Steps:** Design wire format (JSON vs protobuf vs custom binary)

---

## 🎯 P1 - High Priority Features

### Language Features

#### 6. Field Binding in Match Arms
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Medium
- **Description:** `match shape { Circle { radius: r } => use_radius(r) }`
- **Test:** `tests/codegen/_15_platform_specific.rs:466`
- **Implementation:** Parser + typeck to support binding syntax in match arms

#### 7. Methods on Primitives
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** High
- **Description:** `42.to_string()`, `"hello".len()`, `3.14.round()`
- **Test:** `tests/codegen/_14_abi_compliance.rs:123`
- **Rationale:** More ergonomic than `int_to_string(42)`
- **Design Questions:**
  - Impl blocks on primitives? `impl int { fn to_string(self) string }`
  - Extension traits? `trait IntExt { fn to_string(self) string }`
  - Built-in methods only?

#### 8. Nested Closures
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** Closure returning closure: `let adder = (x: int) => (y: int) => x + y`
- **Test:** `tests/codegen/_04_function_calls.rs:740`
- **Blocker:** Current closure lifting pass doesn't handle nested captures
- **Design:** Need to thread environment through multiple levels

#### 9. `?` Operator in `main()`
- **Status:** 🟡 Design
- **Effort:** S
- **Impact:** Medium
- **Description:** Allow `?` in main, treat as early exit with non-zero code
- **Tests:** `tests/codegen/_11_nullable.rs:139`, `:478`
- **Design Question:** What exit code? 1 for errors, 0 for none?

#### 10. Scientific Notation Literals
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Low
- **Description:** `1.7976931348623157e308`, `6.022e23`
- **Tests:** 4 ignored tests in `tests/codegen/_12_edge_cases.rs`, `_01_type_representation.rs`
- **Implementation:** Lexer enhancement to parse exponent notation

#### 11. Binary Literal Syntax
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Low
- **Description:** `0b1010`, `0b11111111`
- **Tests:** `tests/codegen/_13_codegen_correctness.rs:256`, `_15_platform_specific.rs:395`
- **Implementation:** Lexer + parser for `0b` prefix

#### 12. Fixed-Size Array Syntax
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** `let buffer: [byte; 256]` for stack-allocated arrays
- **Test:** `tests/codegen/_07_concurrency.rs:148`
- **Design Questions:**
  - Stack vs heap allocation?
  - Bounds checking strategy?
  - Integration with existing array type?

#### 13. Null Byte Escape `\0`
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Low
- **Description:** Support `\0` in string literals
- **Test:** `tests/codegen/_01_type_representation.rs:355`
- **Implementation:** Lexer + string escape handling

### App/DI

#### 14. Apps with Regular Fields
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** Allow `app MyApp { config: Config [db: Database] fn main(self) {} }`
- **Tests:** 3 ignored tests in `tests/codegen/_09_dependency_injection.rs`
- **Design Question:** Initialization order? Constructor syntax?

#### 15. App Main Non-Void Return
- **Status:** 🟡 Design
- **Effort:** S
- **Impact:** Low
- **Description:** `fn main(self) int` to return exit code
- **Test:** `tests/codegen/_09_dependency_injection.rs:261`
- **Design:** Map return value to process exit code

#### 16. Manual Bracket Dependency Provision
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** `Outer[dep: provided_dep] { field: value }`
- **Tests:** 2 ignored tests in `tests/codegen/_09_dependency_injection.rs`
- **Use Case:** Testing, manual wiring override
- **Design:** Syntax + validation that provided dep matches type

### RPC

#### 17. Stage Declaration (Phase 2)
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** High
- **Description:** `stage api { ... }` to define service boundaries
- **RFC:** `docs/design/rfc-rpc-implementation.md`
- **Depends On:** Wire Format (Feature #5)
- **Deliverables:**
  - Parser support for `stage` keyword
  - `pub` visibility for RPC endpoints
  - Service boundary validation

#### 18. RPC Codegen + HTTP Transport (Phase 3)
- **Status:** 🔴 Blocked
- **Effort:** L
- **Impact:** High
- **Description:** Generate RPC calls, handle network errors automatically
- **Depends On:** Stage Declaration (Feature #17)
- **Deliverables:**
  - Whole-program analysis to detect cross-stage calls
  - HTTP client/server codegen
  - Network error handling

### Contracts

#### 19. Requires/Ensures Enforcement (Phase 2)
- **Status:** 🟢 Ready
- **Effort:** M
- **Impact:** High
- **Description:** Runtime enforcement of function preconditions/postconditions
- **Current:** Parsed but not enforced
- **Implementation:** Codegen checks before/after function body
- **Tests:** Need comprehensive test suite

### Concurrency

#### 20. Move Semantics on Spawn
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** High
- **Description:** `spawn move { ... }` for explicit ownership transfer
- **Rationale:** Prevent shared mutable state bugs
- **Design Questions:**
  - Syntax: `spawn move func()` or `spawn { move(x); func(x) }`?
  - Borrow checker integration?
  - Compatibility with current capture-by-value?

#### 21. Structured Concurrency (Task Groups)
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High
- **Description:** `scope { spawn ...; spawn ...; }` with auto-join/cancel
- **RFC:** `docs/design/rfc-concurrency-v2.md`
- **Design:** Cancellation tokens, error propagation, timeout support

#### 22. Channel Close Wakes Blocked Senders
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Medium
- **Description:** Fix `close()` to wake all blocked senders
- **Test:** `tests/integration/deterministic.rs:1459`
- **Implementation:** Broadcast on close condition variable

### Tooling

#### 23. Package Manager
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High
- **Description:** Dependency resolution, versioning, publishing
- **Design Questions:**
  - Centralized registry or decentralized (git)?
  - Lock file format?
  - Semantic versioning enforcement?
- **Inspiration:** Cargo, npm, go modules

#### 24. Code Formatter
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** High
- **Description:** `plutoc fmt` — canonical code formatting (like `go fmt`)
- **Design:** Should we enforce one style or allow config?
- **Implementation:** Pretty-printer already exists, needs CLI integration + format-on-save

#### 25. Linter
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** Medium
- **Description:** Static analysis for common mistakes, style issues
- **Example Rules:**
  - Unused variables/imports
  - Dead code
  - Naming conventions
  - Overly complex functions

---

## 📦 P2 - Medium Priority Features

### Language Features

#### 26. Compile-Time Reflection (Phase 1)
- **Status:** 🔵 In Progress
- **Effort:** L
- **Impact:** High
- **Description:** `TypeInfo::kind<T>()` for introspecting types at compile-time
- **RFC:** `docs/design/rfc-compile-time-reflection-and-encodings.md`
- **Use Cases:** JSON/Protobuf encodings, ORMs, serialization
- **Current:** Static trait calls parser support merged (PR #41)
- **Next:** Typeck + codegen + stdlib `reflection` module

#### 27. Compile-Time Reflection (Phase 2)
- **Status:** 🔴 Blocked
- **Effort:** L
- **Impact:** High
- **Description:** Loop unrolling, compiler transforms for generated code
- **Depends On:** Phase 1 (Feature #26)

#### 28. Generic JsonEncoding (Phase 3)
- **Status:** 🔴 Blocked
- **Effort:** M
- **Impact:** High
- **Description:** `class User impl JsonEncoding` auto-generates encode/decode
- **Depends On:** Reflection Phase 2 (Feature #27)

#### 29. String Literals Phase 2
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** High
- **Description:** Break `"..."` (no interpolation), require `f"..."` for interpolation
- **RFC:** `docs/design/rfc-string-literals.md`
- **Breaking Change:** Requires migration of all existing code
- **Deliverables:**
  - Update all Pluto code in repo
  - Regular strings reject `{...}` with helpful error
  - Migration guide

### Contracts

#### 30. Contract Quantifiers
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** Medium
- **Description:** `forall item in self.items: item.price > 0`
- **Design:** Syntax + verification strategy (runtime loops? static analysis?)

#### 31. `old()` in Ensures
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** `ensures self.balance == old(self.balance) + amount`
- **Design Question:** Deep copy semantics for complex types?

#### 32. Protocol Contracts
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** Medium
- **Description:** State machine protocols (e.g., `open() -> read() -> close()`)
- **RFC:** Needs design doc

### DI Lifecycle

#### 33. Scoped Services (Phase 1-3)
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** High
- **Description:** `scoped class Handler[db: Database]` + `scope { ... }` blocks
- **RFC:** `docs/design/rfc-di-lifecycle.md`
- **Use Cases:** Request-scoped DB connections, test fixtures
- **Deliverables:**
  - Syntax for scoped classes
  - Scope block typeck
  - Codegen for scope entry/exit

### Stdlib

#### 34. Regex Full Escape Sequences
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Low
- **Description:** `\d`, `\w`, `\s` in regex patterns
- **File:** `tests/integration/stdlib_tests.rs:128`
- **Implementation:** Extend regex runtime support

#### 35. Installation Documentation
- **Status:** 🟢 Ready
- **Effort:** S
- **Impact:** Medium
- **Description:** Complete `book/src/getting-started/installation.md`
- **Content:** Cargo install, binary downloads, build from source

### Runtime

#### 36. Service Discovery (Phase 4)
- **Status:** 🔴 Blocked
- **Effort:** M
- **Impact:** High
- **Description:** Runtime endpoint resolution from config files
- **Depends On:** RPC Codegen (Feature #18)
- **Design:** Static config vs dynamic discovery (Consul, etcd)?

#### 37. Configuration Format
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** High
- **Description:** DI bindings, region constraints, restart policies
- **Design Question:** TOML, YAML, or custom DSL?
- **Features:** Environment-specific configs (dev/staging/prod)

#### 38. Supervision Strategies
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** Medium
- **Description:** one-for-one, one-for-all, rest-for-one crash recovery
- **Design:** Inspired by Erlang/OTP supervisors

#### 39. Observability Hooks
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** High
- **Description:** Built-in metrics, tracing, logging hooks
- **Integration:** Prometheus, OpenTelemetry, structured logging

---

## 🔮 P3 - Nice to Have / Future

### AI-Native Representation

#### 40. Binary `.pluto` Format
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High (for AI workflows)
- **Description:** Protobuf/FlatBuffers canonical representation with UUIDs
- **RFC:** `docs/design/ai-native-representation.md`
- **Design Questions:**
  - Format: protobuf vs FlatBuffers vs Cap'n Proto vs custom?
  - Derived data staleness detection
  - Incremental analysis strategy

#### 41. Human-Readable `.pt` Views
- **Status:** 🔴 Blocked
- **Effort:** XL
- **Impact:** High (for AI workflows)
- **Description:** Text files for human editing, `plutoc sync` to reconcile
- **Depends On:** Binary format (Feature #40)

#### 42. `plutoc-sdk` for AI Agents
- **Status:** 🔴 Blocked
- **Effort:** XL
- **Impact:** High (for AI workflows)
- **Description:** Python/TS bindings for AI agents to write `.pluto` directly
- **Depends On:** Binary format (Feature #40)

### Geographic Awareness

#### 43. Geographic Annotations
- **Status:** 🟡 Design
- **Effort:** L
- **Impact:** High (long-term vision)
- **Description:** Syntax for region/locality constraints
- **Example:** `@region("us-east") class UserService { ... }`
- **Design:** Annotation syntax vs dedicated DSL?

#### 44. Multi-Region Deployment
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High (long-term vision)
- **Description:** Compiler-assisted multi-region orchestration
- **Integration:** Cloud provider APIs (AWS, GCP, Azure)

### Testing

#### 45. Deterministic Concurrency Testing
- **Status:** 🔴 Blocked
- **Effort:** XL
- **Impact:** Medium
- **Description:** Loom-style deterministic scheduler for concurrency tests
- **RFC:** `docs/design/rfc-deterministic-concurrency-testing.md`
- **Depends On:** Structured concurrency (Feature #21)

#### 46. Property-Based Testing
- **Status:** 🟡 Design
- **Effort:** M
- **Impact:** Medium
- **Description:** QuickCheck/Hypothesis-style property tests
- **Example:** `property fn sorted_is_idempotent(arr: [int]) { sort(sort(arr)) == sort(arr) }`

### Compiler

#### 47. Compiler Metrics
- **Status:** 🟢 Ready
- **Effort:** M
- **Impact:** Low
- **Description:** Pipeline timing, code characteristics, diagnostic metrics
- **RFC:** `docs/design/rfc-compiler-metrics.md`
- **Use Cases:** Performance profiling, optimization guidance

#### 48. Incremental Compilation
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High
- **Description:** Only recompile changed modules
- **Design:** Dependency tracking, cache invalidation strategy

#### 49. LSP (Language Server Protocol)
- **Status:** 🟡 Design
- **Effort:** XL
- **Impact:** High
- **Description:** IDE integration (autocomplete, go-to-def, diagnostics)
- **Implementation:** Reuse parser/typeck, add incremental parsing

---

## 📈 Feature Statistics

**Total Features:** 49
**By Status:**
- 🟢 Ready: 15
- 🟡 Design: 24
- 🔴 Blocked: 6
- 🔵 In Progress: 4

**By Priority:**
- P0: 5 features
- P1: 18 features
- P2: 16 features
- P3: 10 features

**By Effort:**
- Small (<1w): 11 features
- Medium (1-3w): 18 features
- Large (1-2m): 13 features
- XL (>2m): 7 features

---

## 🎯 Recommended Next Steps

### Quick Wins (High Impact, Low Effort)
1. Empty array literals (P0, S)
2. Field binding in match arms (P1, S)
3. Scientific notation literals (P1, S)
4. Binary literal syntax (P1, S)
5. Channel close fix (P1, S)

### High-Value Epics
1. RPC implementation (P0/P1, phases 1-5)
2. Compile-time reflection (P2, phases 1-3)
3. Structured concurrency (P1, XL)
4. Package manager (P1, XL)

### Foundation for Other Features
1. If/Match-as-expression (enables functional style, unblocks other features)
2. Methods on primitives (ergonomics unlock)
3. HTTP client (enables real-world apps)

---

## 📝 Notes

- This document tracks **features only**. See [BUGS_AND_FEATURES.md](BUGS_AND_FEATURES.md) for bugs and limitations.
- Features marked 🟡 Design need an RFC or design document before implementation
- Features marked 🔴 Blocked have explicit dependencies listed
- Effort estimates are for experienced contributor, may vary
- Priorities may shift based on user feedback and project direction

**How to propose a feature:**
1. Add to this document with 🟡 Design status
2. Create RFC in `docs/design/rfc-<name>.md`
3. Discuss and refine
4. Update status to 🟢 Ready when approved
5. Implement and mark 🔵 In Progress
6. Mark ✅ Done and move to "Recently Completed" section

**Last reviewed:** 2026-02-11

# Bugs, Limitations, and Missing Features

**Last Updated:** 2026-02-12
**Purpose:** Centralized tracking of known issues, limitations, and planned features for the Pluto compiler

**📋 Quick Navigation:**
- [FEATURES.md](FEATURES.md) - Detailed feature tracker with priorities and effort estimates
- [ROADMAP.md](ROADMAP.md) - High-level vision, milestones, and quarterly goals

---

## 📊 Key Metrics

**Active Bugs:** 5 total (2 P0, 3 P1)
**Known Limitations:** 20
**Missing Features:** See [FEATURES.md](FEATURES.md) (49 tracked features)

---

## 📊 Status Indicators

- 🔴 **Critical** - Blocks significant use cases, no good workaround
- 🟡 **Active** - Known issue with workaround available
- 🟢 **Low Impact** - Minor inconvenience, rarely encountered
- ✅ **Fixed** - Recently resolved

**Effort Estimates:** S (Small, <3 days) | M (Medium, 1 week) | L (Large, 2+ weeks)

---

## 🐛 Active Bugs (Need Fixing)

### P0 - Critical (Compiler Crashes)

#### 1. 🔴 Nested Field Access Parsed as Enum Variant
- **Status:** 🟡 Active (workaround exists)
- **Effort:** M (1 week)
- **Impact:** Blocks any code with `obj.field.field` patterns
- **File:** `bugs/nested-field-access.md`
- **Example:**
  ```pluto
  let v = o.inner.value  // ERROR: unknown enum 'o.inner'
  let count = self.registry.gauges.len()  // ERROR
  ```
- **Workaround:** Use intermediate variables
  ```pluto
  let inner = o.inner
  let v = inner.value  // OK
  ```
- **Root Cause:** Parser at `src/parser/mod.rs:2297` speculatively treats `a.b.c` as qualified enum variant `module.Enum.Variant`
- **Recommended Fix:** Option 1 (parser check for `self`) as quick fix, Option 3 (new AST node `QualifiedAccess`) for general solution

#### 2. 🔴 Errors in Closures Not Supported
- **Status:** 🔴 Critical (no workaround)
- **Effort:** L (2+ weeks, requires pipeline refactor)
- **Impact:** Cannot use `!` operator inside closures
- **Root Cause:** Error inference runs before closure lifting
- **File:** `tests/codegen/BUG_FIXES_SUMMARY.md`, `tests/codegen/_06_error_handling.rs:84`
- **Test:** `#[ignore] // FIXME: Errors in closures not supported - pipeline timing bug`

### P1 - High Priority

#### 3. 🟡 Test Runner Generates Duplicate IDs for Multiple Files
- **Status:** 🟡 Active (workaround: one file per directory)
- **Effort:** S (<3 days)
- **Impact:** Cannot organize tests into multiple files in same directory
- **File:** `feedback/bugs/test-runner-duplicate-ids-multiple-files.md`
- **Example:**
  ```bash
  # tests/lang/fstrings/test1.pluto + test2.pluto
  cargo run -- test tests/lang/fstrings/test1.pluto
  # ERROR: Duplicate definition of identifier: __test_0
  ```
- **Workaround:** Only one test file per directory
- **Recommended Fix:**
  - Option 1: Generate unique IDs with file hash (`__test_<hash>_0`)
  - Option 2: Only compile specified file, not all siblings
  - Option 3: Support directory-based test suites

#### 4. ✅ Trait Method Without `self` Parameter Causes Compiler Panic
- **Status:** ✅ **FIXED** in PR #43 (2026-02-11)
- **Impact:** Invalid trait definitions crashed compiler
- **Fix:** Now shows clear error: "trait method 'X' must have a 'self' parameter"

#### 5. 🟡 Assigning Concrete Class to Trait-Typed Field in Struct Literal Fails
- **Status:** 🟡 Active (workaround: assign after construction)
- **Effort:** M (1 week)
- **Impact:** Cannot directly assign implementing class to trait field in constructor
- **File:** `tests/integration/traits.rs:14963`
- **Workaround:** `let obj = Foo { other_fields... }; obj.trait_field = ConcreteClass { ... }`

#### 6. ✅ Same Trait Listed Twice in Impl List Silently Accepted
- **Status:** ✅ **FIXED** in PR #48 (2026-02-12)
- **Impact:** Duplicate traits were silently accepted, now properly rejected
- **Fix:** Added validation in `check_trait_conformance()` to detect and reject duplicates

---

## ⚠️ Known Limitations (Documented, Low Priority)

### Language Features Not Supported

1. **Empty Array Literals**
   - Cannot infer type of `[]` even with type annotation
   - Tests: 7 ignored tests in `tests/codegen/_12_edge_cases.rs`, `_01_type_representation.rs`
   - Workaround: Use array with at least one element

2. **If-as-Expression**
   - `let x = if cond { a } else { b }` not supported
   - Tests: `tests/codegen/_05_control_flow.rs:174`, `_13_codegen_correctness.rs:487`
   - Workaround: Use statements with mutation

3. **Match-as-Expression**
   - `let x = match y { ... }` not supported
   - Tests: `tests/codegen/_05_control_flow.rs:605`, `:973`
   - Workaround: Use statements with mutation

4. **Scientific Notation in Numeric Literals**
   - `1.7976931348623157e308` not supported
   - Tests: 4 ignored tests in `tests/codegen/_12_edge_cases.rs`, `_01_type_representation.rs`
   - Workaround: Use decimal literals

5. **Binary Literal Syntax**
   - `0b1010` not supported
   - Tests: `tests/codegen/_13_codegen_correctness.rs:256`, `_15_platform_specific.rs:395`
   - Workaround: Use decimal or hex

6. **Field Binding Syntax in Match Arms**
   - `match shape { Circle { radius: r } => ... }` not supported
   - Test: `tests/codegen/_15_platform_specific.rs:466`
   - Workaround: Access fields after match

7. **`?` Operator in `main()`**
   - Using `?` in main causes parser/type errors
   - Tests: `tests/codegen/_11_nullable.rs:139`, `:478`
   - Workaround: Use explicit `if none` checks in main

8. **Fixed-Size Array Syntax**
   - `[Type; size]` not supported
   - Test: `tests/codegen/_07_concurrency.rs:148`
   - Workaround: Use dynamic arrays

9. **Methods on Primitives**
   - `42.to_string()` not supported
   - Test: `tests/codegen/_14_abi_compliance.rs:123`
   - Workaround: Use module functions

10. **Nested Closures**
    - Closure returning closure not supported by closure lifting pass
    - Test: `tests/codegen/_04_function_calls.rs:740`

11. **`\0` Escape Sequence**
    - Null byte escape not supported
    - Test: `tests/codegen/_01_type_representation.rs:355`
    - Supported: `\n`, `\r`, `\t`, `\\`, `\"`

### App/DI Limitations

12. **Apps Cannot Have Regular Fields**
    - Only bracket dependencies and methods allowed
    - Tests: 3 ignored tests in `tests/codegen/_09_dependency_injection.rs`

13. **App `main` Must Return Void**
    - Cannot return exit code from app main
    - Test: `tests/codegen/_09_dependency_injection.rs:261`

14. **Cannot Manually Provide Bracket Dependencies**
    - `Outer[dep] {}` syntax doesn't exist
    - Tests: 2 ignored tests in `tests/codegen/_09_dependency_injection.rs`

### Lexer Issues (Mostly Fixed)

15. **i64::MIN Literal Overflow**
    - `-9223372036854775808` causes lexer overflow
    - Status: See `bugs/lexer-gaps.md` BUG-LEX-009
    - Tests: 3 ignored tests in `tests/codegen/_12_edge_cases.rs`
    - Fixed: Most lexer bugs (BUG-LEX-001 to -008) are now fixed per `bugs/LEXER-SUMMARY.md`

### Concurrency/Channels Limitations (Phase 1)

16. **Channel `close()` Waking Blocked Senders**
    - Known bug: close doesn't wake blocked senders
    - Test: `tests/integration/deterministic.rs:1459`
    - Status: Documented in channel implementation

17. **No `.cancel()` / `.detach()` on Tasks**
    - Phase 1 limitation, Phase 2 feature

18. **No Structured Concurrency**
    - Task groups/scopes not yet implemented
    - See `docs/design/concurrency.md`, `docs/design/rfc-concurrency-v2.md`

19. **No Move Semantics on Spawn**
    - Spawn captures by value (pointer copy for heap types)
    - Shared mutable heap is programmer's responsibility
    - See `docs/design/concurrency.md:319`

20. **GC Suppression While Tasks Active**
    - Unbounded heap growth during long-running tasks
    - 1GB ceiling guardrail (fail-fast abort)
    - See `docs/design/concurrency.md:315`

---

## 🔧 Missing Features / Future Work

### From Open Questions (`docs/design/open-questions.md`)

#### Communication
- [ ] Geographic annotations — syntax for region/locality constraints
- [ ] Service discovery — how do apps find each other?
- [ ] Cross-pod calls — compiler-generated RPC code, serialization format

#### Runtime
- [ ] Configuration format — DI bindings, region constraints, restart policies
- [ ] Supervision strategies — one-for-one, one-for-all, rest-for-one
- [ ] Observability — built-in metrics, tracing, logging hooks
- [ ] Runtime ↔ orchestration interface

#### Dependency Injection
- [ ] Provider registration — DI bindings per environment
- [ ] Lifecycle — singleton vs per-request vs per-process
- [ ] Module ↔ app relationship — can modules contain apps? composition?

#### Concurrency
- [ ] Move semantics on spawn — explicit `move` annotation?
- [ ] Task groups / scopes — structured concurrency with auto-cancellation
- [x] Select / race — waiting on first of multiple tasks/channels (implemented)

#### Contracts
- [ ] Contract inheritance on generics — does `Box<T>` inherit T's invariants?
- [ ] Quantifiers — `forall item in self.items: item.price > 0`
- [ ] Contract testing mode — `@test` with runtime assertions
- [ ] `old()` deep copy semantics — what values can `old()` capture?
- [ ] Protocol composition — can protocols be composed/extended?
- [ ] `@assume` scope — single call, block, or entire function?
- [ ] Gradual adoption — opt-in per module or always enforced?

#### AI-Native Representation
- [ ] Binary format — protobuf, FlatBuffers, Cap'n Proto, custom?
- [ ] Derived data staleness — content hash? version counter?
- [ ] Incremental analysis — partial recompute or full?
- [ ] Cross-project UUIDs — namespace management across libraries
- [ ] SDK language bindings — Python/TS for AI agents?
- [ ] Diff tooling — custom `git diff` for binary `.pluto`?
- [ ] IDE integration — `.pt` sync on save? SDK-powered LSP?
- [ ] Concurrent SDK access — multiple agents, same file (locking? CRDT?)

#### Tooling
- [ ] Standard library scope — core modules (ongoing)
- [ ] Package manager — dependency resolution
- [ ] Formatter / linter — built-in code formatting (`go fmt` style)

### Phased Implementation (In Progress)

#### RPC Implementation (`docs/design/rfc-rpc-implementation.md`)
- **Status:** Phase 1 starting
- **Phase 1:** Wire format + `std.wire` module
- **Phase 2:** `stage` declaration + service model
- **Phase 3:** RPC codegen + HTTP transport
- **Phase 4:** Service discovery + config system
- **Phase 5:** Multi-service binaries

#### String Literals (`docs/design/rfc-string-literals.md`)
- **Phase 1:** ✅ F-strings implemented (`f"Hello {name}"`)
- **Phase 2:** Break regular strings (remove interpolation from `"..."`)
  - [ ] Update all existing Pluto code
  - [ ] Regular strings reject `{...}` with helpful error
  - [ ] Tests for migration

#### Compile-Time Reflection (`docs/design/rfc-compile-time-reflection-and-encodings.md`)
- **Phase 1:** ✅ `TypeInfo` trait + intrinsics for classes and enums
  - Static trait methods: `TypeInfo::type_name<T>()`, `TypeInfo::kind<T>()`
  - Metadata types: `FieldInfo`, `ClassInfo`, `VariantInfo`, `EnumInfo`, `TypeKind`
  - Zero runtime overhead - all generated at compile time
- **Phase 2:** Loop unrolling and compiler transforms
- **Phase 3:** `JsonEncoding` implementation

#### DI Lifecycle (`docs/design/rfc-di-lifecycle.md`)
- **Phase 1:** Scoped services syntax
- **Phase 2:** Scope block typeck
- **Phase 3:** Codegen

#### Deterministic Concurrency Testing (`docs/design/rfc-deterministic-concurrency-testing.md`)
- Waiting for Phase 3 structured concurrency

#### Compiler Metrics (`docs/design/rfc-compiler-metrics.md`)
- **Phase 1:** Pipeline timing metrics
- **Phase 2:** Code characteristic metrics
- **Phase 3:** Error & diagnostic metrics

---

## 📋 Test Coverage Gaps

### Ignored Tests by Category

**Lexer stress tests:**
- 3 ignored in `tests/integration/lexer/stress.rs` — too slow or stack overflow

**Codegen tests:**
- 28 duplicate tests in `tests/codegen/_02_arithmetic.rs` (already covered)
- Various limitation tests (see Known Limitations above)

---

## 🚧 Incomplete/Stub Implementations

1. **HTTP Client (stdlib)**
   - File: `runtime/builtins.c:4423`
   - Note: `// TODO: Implement actual HTTP client with libcurl or sockets`
   - Current: Stub implementation

2. **Regex Escape Sequences (stdlib)**
   - File: `tests/integration/stdlib_tests.rs:128`
   - Note: `// TODO: Full regex escape sequence support (\d, \w, \s) - Phase 2 work`

3. **Installation Docs**
   - File: `book/src/getting-started/installation.md`
   - Note: `TODO: Installation instructions for the Pluto compiler.`

4. **Wire Format Unknown Variant Error**
   - File: `src/marshal.rs:889`
   - Note: `// TODO: When Pluto supports error enums, use WireError.UnknownVariant { type_name, index }`

---

## ✅ Recently Fixed

1. **Duplicate Trait in Impl List** (Bug #6)
   - Fixed in PR #48 (2026-02-12)
   - Added validation to reject duplicate traits in impl list

2. **`?` Operator Crash in Void-Returning Functions**
   - Fixed in commit `ec589633` (2026-02-10)
   - File: `docs/completed/bugs/null-propagate-void-crash.md`

3. **Lexer Bugs (BUG-LEX-001 to -008)**
   - Fixed per `bugs/LEXER-SUMMARY.md`
   - Hex literal validation
   - CRLF line endings
   - Multiple decimal points
   - Span tracking documentation

---

## 📊 Priority Recommendations

### Fix Immediately (Blocking Real Projects)
1. 🔴 **Nested field access bug** (#1) — blocks OOP-style code (Effort: M)
2. 🔴 **Errors in closures** (#2) — blocks functional patterns with error handling (Effort: L)
3. 🟡 **Test runner duplicate IDs** (#3) — blocks multi-file test organization (Effort: S)

### Fix Soon (Quality of Life)
4. ✅ ~~Trait method validation~~ — **FIXED** in PR #43
5. ✅ ~~Duplicate trait impl~~ — **FIXED** in PR #48
6. 🟡 **Trait type coercion** (#5) — struct literal field assignment (Effort: M)

### Features to Implement (See FEATURES.md)
7. **Empty array literals** ([FEATURES.md #1](FEATURES.md)) — P0, high impact (Effort: S)
8. **If/Match as expressions** ([FEATURES.md #2-3](FEATURES.md)) — P0, ergonomics (Effort: M)
9. **HTTP client** ([FEATURES.md #4](FEATURES.md)) — P0, real apps (Effort: M)
10. **RPC implementation** ([FEATURES.md #5, 17-18](FEATURES.md)) — P0/P1, distributed (Effort: L)

### See Also
- **[FEATURES.md](FEATURES.md)** - 49 tracked features with priorities, effort, status
- **[ROADMAP.md](ROADMAP.md)** - Quarterly goals and long-term vision
- **Current Milestone:** v0.2 - Production Foundations (Q2 2026)

---

## 📝 Notes

### Document Organization
This repository uses three main tracking documents:

1. **BUGS_AND_FEATURES.md** (this file) - Known issues, limitations, stubs
2. **[FEATURES.md](FEATURES.md)** - Detailed feature tracker (49 features, priorities, effort, status)
3. **[ROADMAP.md](ROADMAP.md)** - Strategic vision, milestones, quarterly goals

### Data Sources
This document consolidates information from:
- `bugs/` directory - Detailed bug reports
- `docs/design/open-questions.md` - Design questions
- `#[ignore]` test annotations - Skipped tests with reasons
- `TODO`/`FIXME` comments in source - Inline notes
- Design RFCs - Phased implementation plans
- Test failure documentation - Regression tracking

### Maintenance
- Update bug status when fixed (move to "Recently Fixed")
- Add new bugs as discovered (P0/P1/P2/P3 priority)
- Remove limitations when implemented (cross-reference FEATURES.md)
- Review and update monthly

**Last reviewed:** 2026-02-11
**Next review:** 2026-03-11

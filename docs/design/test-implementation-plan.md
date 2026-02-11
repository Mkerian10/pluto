# Test Implementation Plan: Phased Execution

**Created:** 2026-02-11
**Status:** Draft
**Related RFCs:** [Testing Strategy](rfc-testing-strategy.md), [Infrastructure](rfc-test-infrastructure.md), [Core Coverage](rfc-core-coverage.md), [Fuzzing](rfc-fuzzing-stress.md)

## Overview

This document provides a concrete, week-by-week plan to implement the comprehensive testing strategy for the Pluto compiler. The plan is divided into **5 phases** over **10 weeks**, with clear milestones and deliverables.

## Phase Breakdown

### Phase 1: Foundation and Infrastructure (Weeks 1-2)

**Goal:** Set up testing infrastructure to enable systematic testing.

#### Week 1: Property Testing + Compiler API

**Tasks:**
1. Add dependencies to `Cargo.toml`:
   ```toml
   [dev-dependencies]
   proptest = "1.0"
   insta = "1.34"
   criterion = "0.5"
   arbitrary = { version = "1.3", features = ["derive"] }
   ```

2. Create directory structure:
   ```bash
   mkdir -p tests/property
   mkdir -p tests/snapshots
   mkdir -p benches
   mkdir -p fuzz
   ```

3. Implement `TestCompiler` API in `tests/common/mod.rs`:
   ```rust
   pub struct TestCompiler { /* ... */ }
   impl TestCompiler {
       pub fn new(source: &str) -> Self;
       pub fn lex(&self) -> Result<Vec<Token>, LexError>;
       pub fn parse(&self) -> Result<Program, ParseError>;
       pub fn typecheck(&self) -> Result<TypeEnv, TypeError>;
       pub fn compile(&self) -> Result<PathBuf, CompileError>;
       pub fn run(&self) -> Result<TestOutput, RunError>;
   }
   ```

4. Write 5 example property tests in `tests/property/ast.rs`:
   - Spans are monotonic
   - Spans don't overlap
   - Spans are within input bounds
   - All nodes have spans
   - Parse-roundtrip preserves structure

5. Write 5 example snapshot tests in `tests/integration/error_messages.rs`:
   - Type mismatch error
   - Missing return error
   - Undefined variable error
   - Trait not implemented error
   - Match non-exhaustive error

**Deliverables:**
- [ ] Dependencies added
- [ ] Directory structure created
- [ ] `TestCompiler` API implemented and documented
- [ ] 5 property tests written and passing
- [ ] 5 snapshot tests written and passing
- [ ] Documentation in CLAUDE.md updated

**Validation:**
```bash
cargo test --test property
cargo insta test
```

#### Week 2: Fuzzing + Benchmarking

**Tasks:**
1. Install cargo-fuzz:
   ```bash
   cargo install cargo-fuzz
   cargo fuzz init
   ```

2. Implement lexer fuzzer (`fuzz/fuzz_targets/lex.rs`):
   ```rust
   fuzz_target!(|data: &[u8]| {
       if let Ok(s) = std::str::from_utf8(data) {
           let _ = plutoc::lexer::lex(s);
       }
   });
   ```

3. Implement parser fuzzer with Arbitrary for TokenStream (`fuzz/fuzz_targets/parse.rs`)

4. Add corpus seeds:
   ```bash
   cp examples/**/*.pluto fuzz/corpus/lex/
   # Lex them and save tokens for parse corpus
   ```

5. Create benchmarks in `benches/compile_time.rs`:
   - Compile hello world
   - Compile generic function
   - Compile with imports
   - Compile full web service

6. Set up CI for nightly fuzzing (`.github/workflows/fuzz.yml`)

**Deliverables:**
- [ ] cargo-fuzz installed and configured
- [ ] Lexer fuzzer implemented
- [ ] Parser fuzzer implemented
- [ ] Corpus seeds added (20+ files)
- [ ] 4 compile-time benchmarks written
- [ ] Nightly fuzzing CI job created
- [ ] Initial fuzzing run completed (1 hour, no crashes)

**Validation:**
```bash
cargo fuzz run lex -- -max_total_time=60
cargo fuzz run parse -- -max_total_time=60
cargo bench
```

### Phase 2: Core Feature Coverage (Weeks 3-4)

**Goal:** Systematically test all implemented features through parallel agent exploration.

**Strategy:** Deploy multiple agents concurrently to explore different compiler areas. Each agent writes tests, finds bugs, and documents their findings independently. Merge and integrate at week boundaries.

**Parallelization Model:**
- 6 agents working concurrently on different areas
- Each agent has independent test suite and bug tracking
- Weekly sync points to merge findings and resolve conflicts
- Shared bug tracker to avoid duplicate work

## Parallel Agent Workstreams (Week 3-4)

**Coordination:** All agents start simultaneously at Week 3, Day 1. Sync points at end of Week 3 and Week 4.

**IMPORTANT FOR AGENTS:** The user will handle parallelization and coordination. Each agent should work normally on their assigned scope without worrying about other agents. The user will manage branch merging, conflict resolution, and cross-agent synchronization.

---

### Agent 1: Lexer Explorer

**Scope:** Lexer (tokenization) testing and bug fixing

**Branch:** `test-phase2-lexer`

**Responsibilities:**
1. Write lexer edge case tests in `tests/integration/lexer/`:
   - Unicode identifiers (15 tests)
   - Edge tokens (10 tests)
   - Number formats (10 tests)
   - String escapes (10 tests)
   - **Target: 45 tests**

2. Explore edge cases:
   - Fuzz lexer with arbitrary bytes (find crashes)
   - Test UTF-8 boundary conditions
   - Test number overflow/underflow
   - Test comment nesting

3. Bug tracking:
   - File all lexer bugs in `bugs/lexer-agent1.md`
   - Fix P0 bugs immediately
   - Document P1/P2 bugs for later

**Deliverables:**
- [ ] 45+ lexer tests written
- [ ] 90%+ tests passing
- [ ] Lexer bugs documented (estimate: 3-5 bugs)
- [ ] P0 bugs fixed

**Validation:**
```bash
cargo test --test lexer
# Target: 90%+ pass rate, 0 panics
```

---

### Agent 2: Parser Explorer

**Scope:** Parser (AST construction) testing and bug fixing

**Branch:** `test-phase2-parser`

**Responsibilities:**
1. Write parser edge case tests in `tests/integration/parser/`:
   - Precedence and associativity (15 tests)
   - Generics syntax (10 tests)
   - Arrow functions (10 tests)
   - Struct literals (10 tests)
   - **Target: 45 tests**

2. Explore edge cases:
   - Deeply nested expressions (100+ levels)
   - Edge whitespace/newline handling
   - Generics with complex nesting `Map<K, Pair<A, B>>`
   - Error recovery (does parser crash or give good errors?)

3. Property tests:
   - Parse roundtrip: `parse(source).pretty_print()` parses identically
   - Span coverage: All AST nodes have spans

**Deliverables:**
- [ ] 45+ parser tests written
- [ ] 2 property tests written
- [ ] 85%+ tests passing
- [ ] Parser bugs documented (estimate: 5-8 bugs)
- [ ] P0 bugs fixed

**Validation:**
```bash
cargo test --test parser
cargo test --test property::parser
# Target: 85%+ pass rate, 0 panics
```

---

### Agent 3: Typeck Explorer

**Scope:** Type checker testing and bug fixing

**Branch:** `test-phase2-typeck`

**Responsibilities:**
1. Write typeck edge case tests in `tests/integration/typeck/`:
   - Type inference (20 tests)
   - Generics (15 tests)
   - Traits (15 tests)
   - Error propagation (15 tests)
   - Nullable types (10 tests)
   - **Target: 75 tests**

2. Explore edge cases:
   - Generic inference edge cases
   - Trait object edge cases
   - Error type inference across complex call graphs
   - Nullable coercion edge cases

3. Coverage focus:
   - Target 85%+ coverage of `src/typeck/`
   - Run `cargo tarpaulin --lib src/typeck`
   - Document untested paths

**Deliverables:**
- [ ] 75+ typeck tests written
- [ ] 80%+ tests passing
- [ ] Typeck coverage: 85%+
- [ ] Typeck bugs documented (estimate: 10-15 bugs)
- [ ] P0 bugs fixed

**Validation:**
```bash
cargo test --test typeck
cargo tarpaulin --lib -- typeck
# Target: 80%+ pass rate, 85%+ coverage
```

---

### Agent 4: Codegen Explorer

**Scope:** Code generation (Cranelift IR) testing and bug fixing

**Branch:** `test-phase2-codegen`

**Responsibilities:**
1. Write codegen edge case tests in `tests/integration/codegen/`:
   - All PlutoType variants (20 tests)
   - Calling conventions (15 tests)
   - Memory layout (10 tests)
   - **Target: 45 tests**

2. Explore edge cases:
   - Large struct sizes (>100 fields)
   - Deeply nested calls (100+ stack frames)
   - All error handling paths (raise, propagate, catch)
   - Closure calling conventions

3. Critical focus:
   - **Zero panics allowed in codegen**
   - **Zero segfaults in generated binaries**
   - Run generated binaries under valgrind

**Deliverables:**
- [ ] 45+ codegen tests written
- [ ] 85%+ tests passing
- [ ] Codegen coverage: 80%+
- [ ] Codegen bugs documented (estimate: 8-12 bugs)
- [ ] **Zero panics** (critical)

**Validation:**
```bash
cargo test --test codegen
# Zero panics allowed, 85%+ pass rate
```

---

### Agent 5: Runtime Explorer

**Scope:** Runtime builtins (GC, errors, tasks) testing and bug fixing

**Branch:** `test-phase2-runtime`

**Responsibilities:**
1. Write runtime edge case tests in `tests/integration/runtime/`:
   - GC basic cases (15 tests)
   - Error handling (10 tests)
   - Task lifecycle (10 tests)
   - **Target: 35 tests**

2. Explore edge cases:
   - GC stress: allocate 100K objects
   - Concurrent GC: 100 tasks × 10K allocations
   - Error state across threads
   - Task spawn/get edge cases

3. Memory safety focus:
   - Run all tests under valgrind
   - **Zero memory leaks allowed**
   - **Zero data races** (ThreadSanitizer)

**Deliverables:**
- [ ] 35+ runtime tests written
- [ ] 90%+ tests passing
- [ ] Runtime bugs documented (estimate: 5-8 bugs)
- [ ] **Zero valgrind errors**
- [ ] **Zero TSAN warnings**

**Validation:**
```bash
cargo test --test runtime
./scripts/run_valgrind.sh runtime
# 90%+ pass, 0 leaks, 0 races
```

---

### Agent 6: Modules Explorer

**Scope:** Module system testing and bug fixing

**Branch:** `test-phase2-modules`

**Responsibilities:**
1. Write module system tests in `tests/integration/modules/`:
   - Transitive imports (10 tests)
   - Circular import detection (5 tests)
   - Visibility rules (10 tests)
   - **Target: 25 tests**

2. Explore edge cases:
   - Deep import chains (A→B→C→D→E)
   - Diamond dependencies (A→B,C; B,C→D)
   - Name collision resolution
   - Stdlib imports

3. Property tests:
   - Module flatten is deterministic
   - Visibility rules never violated

**Deliverables:**
- [ ] 25+ module tests written
- [ ] 95%+ tests passing
- [ ] Module bugs documented (estimate: 3-5 bugs)
- [ ] 2 property tests written

**Validation:**
```bash
cargo test --test modules
cargo test --test property::modules
# 95%+ pass rate
```

---

## Sync Points

### Week 3 End: Mid-Phase Sync

**All agents stop work, sync findings:**

1. **Bug consolidation:**
   - Merge all `bugs/agent*.md` files
   - Deduplicate bugs (multiple agents may find same bug)
   - Prioritize: P0 → P1 → P2
   - Estimated total: 30-50 unique bugs

2. **Cross-agent bug fixing sprint:**
   - All agents switch to bug fixing
   - Focus on P0 bugs blocking other agents
   - Target: Fix 100% of P0, 50% of P1

3. **Merge branches:**
   - Each agent rebases on master
   - Merge test suites into master
   - Resolve test conflicts

4. **Coverage check:**
   ```bash
   cargo test --all
   cargo tarpaulin --out Html
   # Target: 75%+ coverage, <20 failures
   ```

**Proceed to Week 4 if:**
- ✅ All P0 bugs fixed
- ✅ 75%+ code coverage
- ✅ All branches merged
- ✅ <20 test failures total

---

### Week 4 End: Final Phase Sync

**All agents complete final work:**

1. **Final bug sweep:**
   - Fix remaining P1 bugs (target: 80%+)
   - Document all P2 bugs as known issues
   - Create GitHub issues for all unfixed bugs

2. **Property tests (Agent 2 & 6):**
   - Monomorphize idempotence
   - Closure lift preserves semantics
   - Module flatten preserves names

3. **Coverage push:**
   - Each agent targets 90%+ coverage in their area
   - Combined target: 85%+ overall

4. **Documentation:**
   - Create coverage matrix (`docs/testing/coverage-matrix.csv`)
   - Document all known bugs
   - Update CLAUDE.md

**Final Validation:**
```bash
cargo test --all
cargo test --test property
cargo tarpaulin --out Html
```

**Success Metrics:**
- ✅ 300+ tests written (45+45+75+45+35+25+property)
- ✅ 85%+ line coverage
- ✅ 95%+ test pass rate
- ✅ <10 known bugs (all P2)
- ✅ 0 panics, 0 segfaults, 0 leaks
- ✅ All property tests passing

---

**Phase 2 Checkpoint Summary:**

At end of Week 4, we should have:
- **4 test runs completed** with validation gates between each
- **300+ new tests** across lexer, parser, typeck, codegen, runtime, modules, property
- **85%+ code coverage** (up from ~70% baseline)
- **All individual features tested** in isolation
- **Bug count reduced** from ~30-50 found to <10 remaining (P2 only)
- **Ready for Phase 3:** Feature interaction testing

**Adaptation Policy:**
- If any run finds >20 bugs: Add 2-3 days for bug fixing before next run
- If coverage stuck <80%: Review untested paths, add targeted tests
- If test suite >5 minutes: Parallelize or split into fast/full tiers

### Phase 3: Feature Interaction Testing (Weeks 5-6)

**Goal:** Test all P0 and P1 feature interactions from the N×N matrix.

#### Week 5: P0 Interactions (Critical)

**Tasks:**
1. Identify all P0 matrix cells (estimated 25 cells)

2. Write tests for P0 interactions (`tests/integration/interactions/`):
   - Generics + Closures (5 tests)
   - Generics + Traits (5 tests)
   - Generics + DI (5 tests)
   - Spawn + Errors (5 tests)
   - Spawn + GC (5 tests)
   - Nullable + Errors (5 tests)
   - Contracts + Classes (5 tests)
   - Traits + Generics + DI (5 tests)
   Total: 40 tests

3. Create end-to-end scenario: Web service (`tests/integration/end_to_end/web_service.rs`)
   - Uses: HTTP, JSON, DI, errors, classes, traits
   - ~200 lines of Pluto code
   - Validates: compilation, execution, output

4. Update coverage matrix with interaction test status

**Deliverables:**
- [ ] 40 P0 interaction tests written and passing
- [ ] Web service scenario implemented and passing
- [ ] Coverage matrix updated (all P0 cells marked tested)
- [ ] Integration test documentation updated

**Validation:**
```bash
cargo test --test interactions
cargo test --test end_to_end
```

#### Week 6: P1 Interactions + Scenarios

**Tasks:**
1. Identify all P1 matrix cells (estimated 40 cells)

2. Write tests for P1 interactions:
   - Closures + Errors (5 tests)
   - Enums + Generics (5 tests)
   - Maps + Generics (5 tests)
   - Channels + Spawn (5 tests)
   - Contracts + Traits (5 tests)
   - Nullable + Closures (5 tests)
   - Arrays + Generics (5 tests)
   - Modules + Generics (5 tests)
   Total: 40 tests (covering ~50% of P1 cells)

3. Create end-to-end scenario: Data pipeline (`tests/integration/end_to_end/data_pipeline.rs`)
   - Uses: FS, channels, spawn, generics, collections
   - ~200 lines of Pluto code
   - Validates: concurrency, data flow, correctness

4. Run full test suite, measure time:
   ```bash
   time cargo test
   ```

5. If test suite >5 minutes, parallelize or tier tests (fast/full)

**Deliverables:**
- [ ] 40 P1 interaction tests written and passing
- [ ] Data pipeline scenario implemented and passing
- [ ] Coverage matrix updated (50%+ P1 cells marked tested)
- [ ] Full test suite runs in <5 minutes (or parallelized)
- [ ] Final coverage report: 85%+ line coverage

**Validation:**
```bash
cargo test
cargo tarpaulin
```

**Checkpoint:** At end of Week 6, we should have:
- 400+ total tests
- 85%+ code coverage
- All P0 interactions tested
- 50%+ P1 interactions tested
- 2 real-world scenarios validated

### Phase 4: Fuzzing and Stress (Weeks 7-8)

**Goal:** Discover edge cases via fuzzing and validate correctness under stress.

#### Week 7: Grammar Fuzzing + GC Stress

**Tasks:**
1. Implement grammar-based fuzzer (`fuzz/fuzz_targets/compile.rs`):
   - Define Arbitrary for FuzzProgram
   - Generate syntactically valid Pluto programs
   - Fuzz full compilation pipeline

2. Add oracle: run generated binaries under valgrind
   ```rust
   let output = Command::new("valgrind")
       .args(&["--leak-check=full", binary_path])
       .output()
       .unwrap();
   assert!(!output.stderr.contains("ERROR SUMMARY: "));
   ```

3. Write GC stress tests (`tests/stress/gc_stress.rs`):
   - Allocation storm (100K objects)
   - Large objects (1GB total)
   - Mixed sizes (10K iterations)
   - Concurrent allocation (100 tasks × 10K objects)
   Total: 4 stress tests

4. Set up valgrind integration script (`tests/stress/run_valgrind.sh`)

5. Run fuzzing + stress tests, fix any discovered bugs:
   ```bash
   cargo fuzz run compile -- -max_total_time=7200  # 2 hours
   ./tests/stress/run_valgrind.sh
   ```

**Deliverables:**
- [ ] Grammar-based fuzzer implemented
- [ ] Valgrind oracle added
- [ ] 4 GC stress tests written
- [ ] Valgrind script created
- [ ] 2-hour fuzzing run completed (goal: 0 crashes)
- [ ] All stress tests pass under valgrind (goal: 0 leaks)
- [ ] Any discovered bugs fixed with regression tests

**Validation:**
```bash
cargo fuzz run compile -- -max_total_time=3600
./tests/stress/run_valgrind.sh
```

#### Week 8: Concurrency Stress + Error Stress

**Tasks:**
1. Write concurrency stress tests (`tests/stress/concurrency_stress.rs`):
   - 10K concurrent tasks
   - Concurrent map mutations (100 tasks × 1K ops)
   - Channel 1M messages
   - MPMC channel (10 producers × 5 consumers × 1K msgs)
   Total: 4 stress tests

2. Set up ThreadSanitizer integration script (`tests/stress/run_tsan.sh`)

3. Write error handling stress tests (`tests/stress/error_stress.rs`):
   - Deeply nested propagation (100 levels)
   - Concurrent error isolation (100 tasks)
   Total: 2 stress tests

4. Set up weekly stress test CI job (`.github/workflows/stress.yml`)

5. Run all stress tests, fix any race conditions or deadlocks:
   ```bash
   ./tests/stress/run_concurrency_stress.sh
   ./tests/stress/run_tsan.sh
   ./tests/stress/run_error_stress.sh
   ```

**Deliverables:**
- [ ] 4 concurrency stress tests written and passing
- [ ] ThreadSanitizer script created
- [ ] 2 error stress tests written and passing
- [ ] Weekly stress test CI job created
- [ ] All stress tests pass (no deadlocks, races, or errors)
- [ ] ThreadSanitizer reports no issues

**Validation:**
```bash
./tests/stress/run_all.sh
cargo test --test stress
```

**Checkpoint:** At end of Week 8, we should have:
- 3 fuzzing targets running nightly
- 10 stress tests covering GC, concurrency, and errors
- Valgrind and ThreadSanitizer integration
- 1M+ fuzzing executions without crashes
- 10K tasks, 1GB allocations without leaks

### Phase 5: CI Integration and Documentation (Weeks 9-10)

**Goal:** Automate testing, track metrics, and document the testing strategy.

#### Week 9: CI Automation

**Tasks:**
1. Enhance fuzzing CI (`.github/workflows/fuzz.yml`):
   - Run all 3 fuzz targets (lex, parse, compile)
   - Upload crash artifacts to GitHub Actions
   - Store corpus in git (minimized)
   - Report coverage metrics

2. Create stress test CI job (`.github/workflows/stress.yml`):
   - Weekly schedule (Sunday 4 AM)
   - Run all stress tests
   - Run valgrind and ThreadSanitizer
   - Upload results

3. Add coverage CI job (`.github/workflows/coverage.yml`):
   - Run on every PR
   - Generate coverage report with tarpaulin
   - Upload to Codecov
   - Fail PR if coverage drops >2%

4. Add benchmark CI job (`.github/workflows/bench.yml`):
   - Run on main branch only
   - Compare to previous baseline
   - Fail if >10% regression
   - Upload HTML reports to GitHub Pages

5. Set up corpus management script (`scripts/fuzz_update_corpus.sh`)

**Deliverables:**
- [ ] Fuzzing CI runs nightly (3 targets × 1 hour)
- [ ] Stress CI runs weekly (full suite)
- [ ] Coverage CI runs on every PR (fails if <85%)
- [ ] Benchmark CI runs on main (fails if >10% regression)
- [ ] Corpus management automated
- [ ] All CI jobs passing

**Validation:**
- Manually trigger all CI jobs, verify they pass
- Push a PR, verify coverage check runs

#### Week 10: Metrics + Documentation

**Tasks:**
1. Create testing dashboard (static HTML page):
   - Total test count (unit, integration, property, stress)
   - Code coverage (line, branch)
   - Fuzzing metrics (executions, crashes, corpus size)
   - Benchmark results (compile time trends)
   - Stress test results (pass/fail, duration)

2. Write testing guidelines (`docs/testing/GUIDELINES.md`):
   - When to write unit vs integration tests
   - How to use TestCompiler API
   - How to write property tests
   - How to add fuzz targets
   - How to write stress tests
   - How to update snapshots

3. Update CLAUDE.md with testing infrastructure:
   - Test commands (property, fuzz, stress, coverage)
   - CI job descriptions
   - Testing workflow (local → PR → main)

4. Create regression test template (`tests/integration/regressions/TEMPLATE.md`)

5. Write blog post / announcement about testing strategy (optional)

6. Retrospective: What worked, what didn't, lessons learned

**Deliverables:**
- [ ] Testing dashboard created (`docs/testing/dashboard.html`)
- [ ] Testing guidelines written (`docs/testing/GUIDELINES.md`)
- [ ] CLAUDE.md updated with testing sections
- [ ] Regression test template created
- [ ] Retrospective document written
- [ ] Final metrics: 85%+ coverage, 1M+ fuzzing execs, 0 crashes

**Validation:**
- Review all documentation for accuracy
- Run full test suite one final time:
  ```bash
  cargo test
  cargo test --test property
  cargo fuzz run lex -- -runs=100000
  cargo fuzz run parse -- -runs=100000
  cargo fuzz run compile -- -runs=100000
  ./tests/stress/run_all.sh
  cargo tarpaulin
  cargo bench
  ```

**Checkpoint:** At end of Week 10, we should have:
- Fully automated CI for tests, fuzzing, stress, coverage, benchmarks
- 85%+ code coverage
- 1M+ fuzzing executions without crashes
- Comprehensive documentation
- Testing infrastructure ready for long-term maintenance

## Summary Timeline

| Phase | Weeks | Focus | Deliverables |
|-------|-------|-------|--------------|
| 1 | 1-2 | Infrastructure | Property tests, fuzzing, benchmarks, CI |
| 2 | 3-4 | Core Coverage | 300+ tests, 85% coverage, all features tested |
| 3 | 5-6 | Interactions | 80+ interaction tests, 2 end-to-end scenarios |
| 4 | 7-8 | Fuzzing + Stress | Grammar fuzzing, 10 stress tests, valgrind/TSAN |
| 5 | 9-10 | CI + Docs | Automated CI, metrics dashboard, guidelines |

**Total:** 10 weeks, 500+ new tests, 85%+ coverage, full fuzzing + stress testing, automated CI

## Success Metrics

At the end of Phase 5, we should achieve:

- **Tests:**
  - 500+ integration tests (up from ~200)
  - 20+ property tests
  - 10+ stress tests
  - 3 fuzzing targets

- **Coverage:**
  - 85%+ line coverage
  - 90%+ branch coverage (typeck, codegen)

- **Fuzzing:**
  - 1M+ executions per target
  - 0 crashes
  - 80%+ code coverage (fuzzing-specific)
  - 100+ corpus inputs per target

- **Stress:**
  - 10K concurrent tasks passing
  - 1GB allocations without leaks
  - 1M channel messages without deadlock
  - 0 valgrind errors
  - 0 ThreadSanitizer warnings

- **Automation:**
  - Nightly fuzzing (3 targets × 1 hour)
  - Weekly stress tests
  - Per-PR coverage checks
  - Benchmark regression detection

- **Documentation:**
  - Testing guidelines written
  - CLAUDE.md updated
  - Dashboard created
  - Regression test template

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Fuzzing discovers critical bugs | Budget 2 extra weeks for bug fixes (Week 11-12) |
| Test suite becomes too slow | Parallelize tests, tier into fast/full suites |
| Coverage tools inaccurate | Manual audit of critical paths (typeck, codegen) |
| Stress tests are flaky | Increase timeouts, add retries, investigate root cause |
| CI costs too high | Optimize fuzzing runs, reduce corpus size |

## Next Steps

1. **Review this plan** with the team, adjust timelines as needed
2. **Create GitHub issues** for each week's tasks
3. **Assign owners** for each phase (if team size allows)
4. **Begin Phase 1** immediately (Week 1 tasks)
5. **Weekly check-ins** to track progress and unblock issues

---

**Ready to begin?** Start with [Week 1: Property Testing + Compiler API](#week-1-property-testing--compiler-api)

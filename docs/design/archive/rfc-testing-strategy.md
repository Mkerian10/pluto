# RFC: Comprehensive Testing Strategy for Pluto

**Status:** Draft
**Author:** Development Team
**Created:** 2026-02-11
**Updated:** 2026-02-11

## Executive Summary

This RFC proposes a comprehensive testing strategy for the Pluto compiler and runtime to establish confidence in correctness, prevent regressions, and enable safe refactoring. The strategy addresses three critical gaps:

1. **Systematic coverage** of all language features and edge cases
2. **Infrastructure** for property-based testing, fuzzing, and stress testing
3. **Integration testing** across module boundaries and real-world scenarios

## Motivation

The Pluto compiler has grown rapidly, adding features like generics, concurrency, contracts, nullable types, and RPC. While integration tests exist for individual features, several risks remain:

- **Combinatorial gaps:** Features tested in isolation may interact unexpectedly (e.g., generics + closures + errors)
- **Edge cases:** Corner cases in parser, typeck, and codegen may be untested
- **Refactoring risk:** Major changes (like AST UUIDs, canonical flip) lack regression safety nets
- **Performance unknowns:** No systematic stress testing or benchmarking
- **Concurrency correctness:** GC + spawn interactions need exhaustive validation

This strategy aims to achieve **90%+ confidence in compiler correctness** before the 1.0 release.

## Proposed Strategy

### 1. Test Infrastructure (RFC-TEST-001)

**Goal:** Enable systematic testing with minimal friction.

**Deliverables:**
- Property-based testing framework (proptest or similar)
- Fuzzer infrastructure (libfuzzer or cargo-fuzz)
- Compiler API for programmatic testing
- Snapshot testing for error messages
- Performance regression tracking

**Key Components:**
- `tests/property/` — property-based tests for AST invariants, type system rules
- `fuzz/` — fuzzing targets for lexer, parser, typeck
- `tests/snapshots/` — error message golden files
- `benches/` — criterion benchmarks for compile time and runtime

### 2. Core Language Coverage (RFC-TEST-002)

**Goal:** Systematically test all implemented features and their interactions.

**Deliverables:**
- Exhaustive coverage matrix (feature × feature interactions)
- Integration tests for combinatorial cases
- Edge case tests for each compiler stage
- Regression tests for all historical bugs

**Coverage Areas:**
- **Lexer/Parser:** Unicode, edge tokens, precedence, recovery
- **Typeck:** Inference, generics, traits, errors, nullables, contracts
- **Codegen:** All PlutoType variants, calling conventions, memory layout
- **Runtime:** GC under stress, concurrent allocation, error propagation
- **Module system:** Circular imports, transitive visibility, re-exports
- **Feature interactions:** Generics + closures, spawn + errors, contracts + traits

### 3. Fuzzing and Stress Testing (RFC-TEST-003)

**Goal:** Discover edge cases and validate correctness under extreme conditions.

**Deliverables:**
- Structured fuzzing for parser/typeck (valid-by-construction inputs)
- Grammar-based fuzzing for end-to-end compiler
- Stress tests for GC (allocation storms, concurrent tasks)
- Concurrency stress tests (thousands of spawns, race conditions)
- Memory leak detection (valgrind, sanitizers)

**Fuzzing Targets:**
- Lexer (arbitrary byte streams)
- Parser (token streams with valid structure)
- Typeck (valid ASTs with type errors)
- Codegen (valid typed ASTs)
- Runtime (GC allocation patterns, task spawning patterns)

## Implementation Plan

The strategy will be implemented in **5 phases** over **8-10 weeks**:

### Phase 1: Infrastructure Setup (Week 1-2)
- Add proptest/quickcheck dependency
- Create `tests/property/` structure
- Set up fuzzing infrastructure (cargo-fuzz)
- Add criterion for benchmarking
- Document testing guidelines

### Phase 2: Core Feature Coverage (Week 3-4)
- Audit existing tests, identify gaps
- Write 100+ new integration tests for edge cases
- Create coverage matrix spreadsheet
- Add snapshot tests for error messages
- Property tests for AST transformations

### Phase 3: Feature Interaction Testing (Week 5-6)
- Combinatorial tests (generics × closures × errors, etc.)
- Multi-module integration tests
- Contract + trait interaction tests
- Nullable + error interaction tests
- Real-world scenario tests (web service, data pipeline)

### Phase 4: Fuzzing and Stress (Week 7-8)
- Implement structured fuzzing for parser
- Grammar-based fuzzing for compiler
- GC stress tests (allocation storms)
- Concurrency stress tests (1000+ tasks)
- Memory leak detection CI job

### Phase 5: Continuous Integration (Week 9-10)
- Nightly fuzzing runs
- Performance regression tracking
- Test result dashboards
- Coverage reporting (tarpaulin)
- Documentation and runbooks

## Success Metrics

- **Coverage:** 85%+ line coverage, 90%+ branch coverage
- **Feature matrix:** All N×N feature interactions tested
- **Fuzzing:** 1M+ executions without crashes
- **Stress:** 10K concurrent tasks, 1GB allocations without leaks
- **Regression:** Zero known bugs re-introduced after fixes
- **Performance:** No >10% compile-time regressions between releases

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Fuzzing reveals critical bugs | High | High | Budget 2 weeks for bug fixes |
| Test suite becomes too slow | Medium | Medium | Parallel execution, tiered CI (fast/full) |
| Property tests too complex | Medium | Low | Start simple, iterate on test design |
| Coverage tools inaccurate | Low | Low | Manual audit of critical paths |

## Alternatives Considered

1. **Defer testing until 1.0:** Rejected — technical debt compounds, refactoring becomes risky
2. **Manual testing only:** Rejected — not scalable, misses edge cases
3. **Formal verification:** Deferred — too heavyweight for current stage, revisit post-1.0

## Open Questions

- Should we adopt mutation testing (e.g., cargo-mutants)?
- What's the right balance between unit and integration tests?
- Should we write tests in Pluto itself (dogfooding)?

## Related RFCs

- **RFC-TEST-001:** Test Infrastructure Details
- **RFC-TEST-002:** Core Language Coverage Plan
- **RFC-TEST-003:** Fuzzing and Stress Testing Strategy
- **RFC-AI-NATIVE:** AST UUIDs and canonical representation (depends on test safety)

## References

- [Rust's testing strategy](https://rustc-dev-guide.rust-lang.org/tests/intro.html)
- [SQLite's testing methodology](https://www.sqlite.org/testing.html) — 100% MC/DC coverage
- [Property-Based Testing](https://hypothesis.works/articles/what-is-property-based-testing/) — Hypothesis documentation
- [Compiler fuzzing best practices](https://www.fuzzingbook.org/)

---

**Next Steps:**
1. Review and approve this RFC
2. Create detailed implementation plan with task breakdown
3. Begin Phase 1 infrastructure work
4. Weekly progress reviews during implementation

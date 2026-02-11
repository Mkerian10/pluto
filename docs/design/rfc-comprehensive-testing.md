# RFC: Comprehensive Testing Strategy for Pluto Compiler

**Status:** Draft
**Author:** Testing Initiative
**Date:** 2026-02-11

## Executive Summary

This RFC proposes a systematic approach to achieving near-perfect compiler correctness through comprehensive test coverage. While Pluto currently has strong foundational testing (2000+ integration tests, 288 unit tests, ~42k lines of test code vs ~36k lines of source), there are systematic gaps that prevent us from claiming production-grade reliability.

**Industry Context:** Research shows compiler testing requires multiple complementary approaches—traditional test suites, fuzzing, property-based testing, differential testing, and metamorphic testing. Major compilers like rustc use 10,000+ tests with sophisticated test harnesses ([Rust Testing Guide](https://rustc-dev-guide.rust-lang.org/tests/intro.html)).

## Current State Assessment

### Strengths ✅

1. **Strong test-to-code ratio:** 117% (42k test LOC / 36k source LOC)
2. **Good feature coverage:** Traits (513 tests), Contracts (67 tests), well-tested core features
3. **Decent negative testing:** 315 compile-should-fail tests (~15% of total)
4. **Systematic organization:** Clear separation by feature area
5. **Integration-first approach:** Emphasis on end-to-end behavior over implementation details

### Industry Standard Comparison

| Metric | Pluto | Rustc | Go Compiler | Industry Standard |
|--------|-------|-------|-------------|-------------------|
| Test count | ~2000 | ~15,000+ | ~8,000+ | 5,000+ for production |
| Negative tests | 315 (~15%) | ~30% | ~25% | 20-30% |
| Fuzzing | ❌ None | ✅ Yes | ✅ Yes | Required for security |
| Property tests | ❌ None | Some | Some | Recommended |
| Differential testing | ❌ None | ✅ Yes | N/A | Recommended |
| Benchmarks | ❌ None | ✅ Yes | ✅ Yes | Required for perf |
| Stress tests | ❌ None | ✅ Yes | ✅ Yes | Critical for stability |

**Verdict:** Pluto is at **"good startup compiler"** level (~60-70% of production-grade). Strong foundations, but missing advanced testing techniques required for production deployment.

### Critical Gaps 🔴

Based on [compiler fuzzing research](https://arxiv.org/pdf/2306.06884) and [metamorphic testing approaches](https://dl.acm.org/doi/abs/10.1145/3508035), we're missing:

1. **Edge case explosion testing** - Deeply nested structures, extreme sizes, pathological inputs
2. **Fuzzing infrastructure** - Random valid/invalid program generation
3. **Property-based tests** - Invariant checking across transformations
4. **Stress testing** - Compilation limits, resource exhaustion, OOM handling
5. **Differential testing** - Compare optimization levels, backends (when we add LLVM)
6. **Performance regression tests** - Benchmark suite with alerting
7. **Error message quality tests** - User-facing diagnostic validation
8. **Interaction testing** - Feature combinations (generics + errors + DI + concurrency)
9. **Platform-specific tests** - macOS vs Linux behavioral differences
10. **Security testing** - Malicious inputs, crash resistance, code injection

## Proposed Testing Categories

### 1. Core Correctness Testing (Current: 70% → Target: 95%)

**What we have:** Good positive path coverage for most features.

**What's missing:**

#### A. Edge Cases (Systematic Exploration)

For every language feature, test:
- **Boundary conditions:** Empty collections, zero, max int, unicode edge cases
- **Deep nesting:** 100-level deep expressions, nested generics, recursive types
- **Extreme sizes:** 10,000 element arrays, 100KB strings, 1000-field classes
- **Minimal cases:** Single-element collections, one-char strings, empty functions
- **Type complexity:** Deeply nested generics `Map<string, Array<Map<int, Set<T>>>>`

**Example missing tests:**
```pluto
// Empty edge cases
test "empty array operations" {
    let a: [int] = []
    expect(a.len()).to_equal(0)
    let b = a.map((x: int) => x + 1)  // Should work on empty
    expect(b.len()).to_equal(0)
}

// Deep nesting
test "100-level nested function calls" {
    fn nest100(x: int) int { /* ... */ }
    expect(nest100(42)).to_equal(/* result */)
}

// Extreme sizes
test "10000 element array" {
    let mut arr: [int] = []
    for i in 0..10000 {
        arr = arr + [i]
    }
    expect(arr.len()).to_equal(10000)
}

// Complex generic nesting
test "deeply nested generic types" {
    let x: Map<string, Array<Map<int, Set<string>>>> =
        Map<string, Array<Map<int, Set<string>>>> {}
    // Should compile and run without stack overflow
}
```

#### B. Error Path Coverage (Current: 15% → Target: 30%)

Negative tests should match positive tests 1:1 for type errors, then add:
- **Parse errors:** Every invalid syntax variation
- **Type errors:** Every type mismatch scenario
- **Semantic errors:** Undefined variables, circular deps, invalid casts
- **Contract violations:** Invariant/requires/ensures failures
- **DI errors:** Missing deps, circular deps, scope mismatches
- **Module errors:** Import cycles, visibility violations, missing modules

**Example missing negative tests:**
```rust
#[test]
fn type_error_generic_bound_violation() {
    compile_should_fail_with(
        "fn foo<T: Printable>(x: T) { }\nfn main() { foo(42) }",
        "type `int` does not implement trait `Printable`"
    );
}

#[test]
fn error_nullable_with_error_propagation() {
    compile_should_fail_with(
        "fn foo() int? { let x = bar()!; return x }\nfn bar() int { 42 }",
        "cannot use `!` on non-fallible expression"
    );
}

#[test]
fn di_circular_dependency() {
    compile_should_fail_with(
        "class A[b: B] {}\nclass B[a: A] {}\napp MyApp[a: A] { fn main(self) {} }",
        "circular dependency detected: A -> B -> A"
    );
}
```

#### C. Feature Interaction Testing (New Category)

Test **every combination** of 2-3 features:
- Generics + Errors + DI
- Closures + Traits + Nullable
- Contracts + Concurrency + Errors
- Channels + Generics + Errors
- Modules + Traits + Generics

**Example interaction tests:**
```pluto
test "generic class with DI and error handling" {
    error DBError {}
    class DB {}
    class Repo<T>[db: DB] {
        fn save(self, item: T) T {
            if !valid(item) { raise DBError {} }
            return item
        }
    }
    app MyApp[repo: Repo<int>, db: DB] {
        fn main(self) {
            let x = self.repo.save(42) catch 0
            print(x)
        }
    }
}

test "nullable generic with trait bound and closure" {
    trait Processor {
        fn process(self) int
    }
    fn apply<T: Processor>(opt: T?, f: fn(T) int) int? {
        if opt == none { return none }
        return f(opt?)
    }
    // Full program...
}
```

### 2. Fuzzing Infrastructure (New: 0% → Target: Running continuously)

**Goal:** Find crashes, infinite loops, and memory corruption automatically.

#### A. Grammar-Based Fuzzer

Generate random **valid** Pluto programs using the grammar:
- All syntax variations (if/while/for, match, generics, etc.)
- Random type combinations
- Random nesting depths (1-20 levels)
- Random program sizes (10-1000 lines)

**Implementation approach:**
```rust
// tests/fuzz/grammar_fuzzer.rs
pub struct PlutoGrammarFuzzer {
    max_depth: usize,
    max_statements: usize,
}

impl PlutoGrammarFuzzer {
    pub fn generate_program(&self) -> String {
        // Generate random valid Pluto program
        // Start with a main function, add random statements
        // Ensure type correctness
    }

    pub fn fuzz_compile(&self, iterations: usize) -> Vec<FuzzResult> {
        // Generate N random programs, try to compile each
        // Report: crashes, infinite loops, wrong error messages
    }
}

#[test]
fn fuzz_compiler_stability() {
    let fuzzer = PlutoGrammarFuzzer::new(10, 100);
    let results = fuzzer.fuzz_compile(10_000);
    assert!(results.crashes.is_empty(), "Compiler crashed on valid input");
}
```

**Oracles** (how to detect bugs):
1. **Crash oracle:** Compiler shouldn't panic on any valid input
2. **Time oracle:** Compilation should finish in <10s for small programs
3. **Error oracle:** Invalid programs should produce helpful errors, not crashes

#### B. Mutation-Based Fuzzer

Take valid programs, apply small **invalid** mutations:
- Change types randomly
- Insert/delete tokens
- Swap operands
- Corrupt strings

**Expected behavior:** Graceful error messages, never crashes.

#### C. Property-Based Testing with Proptest

Use [proptest](https://github.com/proptest-rs/proptest) for compiler invariants:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn lexer_roundtrip(input in ".*") {
        // Lex → unlex → lex should be stable
        let tokens = lex(&input);
        let reconstructed = unlex(&tokens);
        let tokens2 = lex(&reconstructed);
        assert_eq!(tokens, tokens2);
    }

    #[test]
    fn type_inference_deterministic(prog in valid_program_strategy()) {
        // Same program should infer same types every time
        let types1 = typecheck(&prog);
        let types2 = typecheck(&prog);
        assert_eq!(types1, types2);
    }

    #[test]
    fn optimization_preserves_semantics(prog in valid_program_strategy()) {
        // Program should have same output at all optimization levels
        let output_o0 = compile_and_run(&prog, OptLevel::O0);
        let output_o2 = compile_and_run(&prog, OptLevel::O2);
        assert_eq!(output_o0, output_o2);
    }
}
```

### 3. Metamorphic Testing (New Category)

Based on [metamorphic testing research](https://dl.acm.org/doi/10.1145/3143561), test **relations between programs**:

#### A. Semantic-Preserving Transformations

Apply transformations that **shouldn't change behavior:**

1. **Variable renaming:** `let x = 5` → `let y = 5`
2. **Expression reordering:** `a + b + c` → `c + b + a` (for commutative ops)
3. **Constant folding:** `2 + 3` → `5`
4. **Dead code insertion:** Add `if false { ... }` blocks
5. **Associativity:** `(a + b) + c` → `a + (b + c)`

**Oracle:** Both programs should produce identical output.

```rust
#[test]
fn metamorphic_variable_renaming() {
    let original = "fn main() { let x = 42\n print(x) }";
    let renamed = "fn main() { let y = 42\n print(y) }";

    assert_eq!(
        compile_and_run_stdout(original),
        compile_and_run_stdout(renamed)
    );
}

#[test]
fn metamorphic_associativity() {
    let v1 = "fn main() { let x = (1 + 2) + 3\n print(x) }";
    let v2 = "fn main() { let x = 1 + (2 + 3)\n print(x) }";

    assert_eq!(
        compile_and_run_stdout(v1),
        compile_and_run_stdout(v2)
    );
}
```

#### B. Differential Testing (When Multiple Backends Exist)

- Compare Cranelift vs future LLVM backend
- Compare optimization levels (-O0 vs -O2)
- Compare different targets (x86_64 vs aarch64)

### 4. Stress and Limit Testing (New Category)

Push the compiler to its **breaking points:**

#### A. Compilation Limits

```pluto
test "1000 function definitions" {
    let mut program = "fn main() { print(f0()) }\n";
    for i in 0..1000 {
        program += "fn f{i}() int { {i} }\n";
    }
    compile_and_run_stdout(program); // Should handle gracefully
}

test "10,000 local variables" {
    let mut program = "fn main() {\n";
    for i in 0..10000 {
        program += "let x{i} = {i}\n";
    }
    program += "print(x9999)\n}";
    compile_and_run_stdout(program);
}

test "deeply nested generics" {
    // Box<Box<Box<Box<Box<Box<T>>>>>>
    // Should compile or give clear error, not stack overflow
}
```

#### B. Resource Exhaustion

```rust
#[test]
fn compiler_oom_handling() {
    // Program that would require >4GB to compile
    // Compiler should error gracefully, not crash
    let huge_array = format!("fn main() {{ let x = [{}] }}",
                             (0..1_000_000).map(|i| i.to_string()).collect::<Vec<_>>().join(","));

    let result = std::panic::catch_unwind(|| compile(&huge_array));
    assert!(result.is_ok(), "Compiler panicked instead of erroring on OOM");
}
```

#### C. Runtime Stress Tests

```pluto
test "spawn 1000 tasks" {
    fn worker(id: int) int { id }
    fn main() {
        let mut tasks: [Task<int>] = []
        for i in 0..1000 {
            tasks = tasks + [spawn worker(i)]
        }
        for task in tasks {
            let _ = task.get()
        }
        print("done")
    }
}

test "1 million element array" {
    fn main() {
        let mut arr: [int] = []
        for i in 0..1_000_000 {
            arr = arr + [i]
        }
        print(arr.len())
    }
}
```

### 5. Error Message Quality Testing (New Category)

Test **user-facing diagnostics** systematically:

```rust
#[test]
fn helpful_error_undefined_variable() {
    compile_should_fail_with(
        "fn main() { print(undefined_var) }",
        "undefined variable `undefined_var`"
    );
}

#[test]
fn helpful_error_type_mismatch() {
    compile_should_fail_with(
        "fn main() { let x: int = \"string\" }",
        "expected type `int`, found `string`"
    );
}

#[test]
fn helpful_error_missing_trait_method() {
    compile_should_fail_with(
        "trait Foo { fn bar(self) int }\nclass X impl Foo {}\nfn main() {}",
        "class `X` does not implement required method `bar` from trait `Foo`"
    );
}

#[test]
fn error_span_accuracy() {
    let result = compile_to_error(
        "fn main() { let x: int = \"string\" }"
    );
    // Assert that span points exactly at "string", not the whole line
    assert_eq!(result.span.start, /* exact char offset of " */);
}
```

**Goal:** Every error should:
1. Clearly state what went wrong
2. Point to the exact location (correct span)
3. Suggest a fix when possible
4. Use friendly, non-jargon language

### 6. Performance and Benchmark Testing (New Category)

Track compilation speed and runtime performance:

#### A. Compilation Speed Benchmarks

```rust
// benches/compile_time.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_compile_hello_world(c: &mut Criterion) {
    let source = r#"fn main() { print("hello") }"#;
    c.bench_function("compile hello world", |b| {
        b.iter(|| compile(black_box(source)))
    });
}

fn bench_compile_large_program(c: &mut Criterion) {
    let source = generate_large_program(1000); // 1000 functions
    c.bench_function("compile 1000 functions", |b| {
        b.iter(|| compile(black_box(&source)))
    });
}

criterion_group!(compile_benches, bench_compile_hello_world, bench_compile_large_program);
criterion_main!(compile_benches);
```

#### B. Runtime Performance Tests

```pluto
// Track that optimizations don't regress performance
test "fibonacci performance" {
    fn fib(n: int) int {
        if n <= 1 { return n }
        return fib(n - 1) + fib(n - 2)
    }

    fn main() {
        let start = std.time.now()
        let result = fib(30)
        let elapsed = std.time.now() - start

        expect(elapsed).to_be_less_than(1000) // < 1 second
        print(result)
    }
}
```

### 7. Platform-Specific Testing (Current: Implicit → Target: Explicit)

Test macOS vs Linux differences:

```rust
#[test]
#[cfg(target_os = "macos")]
fn macos_specific_behavior() {
    // Test macOS-specific runtime behavior
}

#[test]
#[cfg(target_os = "linux")]
fn linux_specific_behavior() {
    // Test Linux-specific runtime behavior
}

#[test]
fn cross_platform_consistency() {
    // Same program should behave identically on both platforms
    let output = compile_and_run_stdout("fn main() { print(42) }");
    assert_eq!(output, "42\n"); // Same on all platforms
}
```

### 8. Security and Robustness Testing (New Category)

Ensure compiler resists malicious inputs:

```rust
#[test]
fn malformed_unicode() {
    // Invalid UTF-8 sequences shouldn't crash compiler
    let malicious = vec![0xFF, 0xFE, 0xFD];
    let result = std::panic::catch_unwind(|| {
        compile(std::str::from_utf8(&malicious).unwrap_or(""))
    });
    assert!(result.is_ok());
}

#[test]
fn code_injection_via_strings() {
    // Ensure string interpolation doesn't allow code injection
    compile_and_run_stdout(r#"
        fn main() {
            let evil = "\" + system(\"rm -rf /\") + \""
            print("safe: {evil}")
        }
    "#);
    // Should print the literal string, not execute anything
}

#[test]
fn stack_overflow_resistance() {
    // Deeply recursive types shouldn't stack overflow compiler
    let recursive = "class A { x: A }";
    compile_should_fail_with(recursive, "recursive type");
    // Should error, not crash
}
```

### 9. Regression Testing (Continuous)

**Every bug fix gets a test:**

```rust
// tests/integration/regressions.rs

#[test]
fn issue_123_generic_trait_method_call() {
    // Regression test for GitHub issue #123
    // Bug: generic trait method calls crashed compiler
    compile_and_run_stdout(r#"
        trait Processor<T> {
            fn process(self, item: T) T
        }
        // ... test case that triggered bug ...
    "#);
}
```

**Policy:** No PR merged without regression test for the bug it fixes.

### 10. Documentation Testing (New Category)

Test all code examples in:
- `README.md`
- `SPEC.md`
- `docs/` directory
- `examples/` directory

```rust
// Similar to Rust's doctest
#[test]
fn test_all_documentation_examples() {
    for example in find_code_blocks_in_docs() {
        let result = compile_and_run(&example);
        assert!(result.success, "Documentation example failed: {}", example.source);
    }
}
```

## Implementation Roadmap

### Phase 1: Fill Core Gaps (2-3 weeks)

**Priority:** High-value, low-effort improvements

1. **Double negative tests** (Week 1)
   - For each positive test, add corresponding negative test
   - Target: 600+ negative tests (from 315)
   - Focus: Type errors, semantic errors

2. **Edge case sweep** (Week 1-2)
   - Empty collections, boundary values, extreme sizes
   - Target: +200 tests
   - Use templated test generation

3. **Feature interaction tests** (Week 2-3)
   - Test 20 most important feature combinations
   - Target: +100 tests

**Deliverable:** Test count: 2000 → 3000+, negative test %: 15% → 25%

### Phase 2: Fuzzing Infrastructure (2-3 weeks)

**Priority:** High-impact, moderate-effort

1. **Grammar-based fuzzer** (Week 1-2)
   - Random valid program generator
   - Run 10,000 programs, ensure no crashes

2. **Mutation-based fuzzer** (Week 2)
   - Random invalid program generator
   - Ensure graceful error handling

3. **Property-based tests with proptest** (Week 3)
   - Add proptest dependency
   - Write 20 property tests for compiler invariants

**Deliverable:** Continuous fuzzing running in CI, 50+ bugs found and fixed

### Phase 3: Advanced Testing (3-4 weeks)

**Priority:** Production-readiness

1. **Metamorphic testing** (Week 1-2)
   - Implement transformation framework
   - Test 10 semantic-preserving transformations

2. **Stress testing** (Week 2)
   - Compilation limits
   - Resource exhaustion handling

3. **Performance benchmarks** (Week 3)
   - Set up Criterion benchmarks
   - Establish baseline metrics
   - Add CI checks for regressions

4. **Error message quality audit** (Week 4)
   - Review all error messages
   - Add tests for error quality
   - Improve spans and suggestions

**Deliverable:** Production-grade testing, <10 bugs per 10k LOC

### Phase 4: Continuous Improvement (Ongoing)

1. **Regression test policy:** Every bug fix requires test
2. **Coverage tracking:** Track line/branch coverage, aim for 85%+
3. **Quarterly security audits:** Fuzzing campaigns, malicious input testing
4. **Community test contributions:** Accept external test PRs

## Success Metrics

| Metric | Current | 6 Months | 12 Months | Industry Standard |
|--------|---------|----------|-----------|-------------------|
| Total tests | 2,000 | 5,000 | 10,000 | 10,000+ |
| Negative tests | 315 (15%) | 1,500 (30%) | 3,000 (30%) | 25-30% |
| Bugs per 10k LOC | ~50 (est) | ~10 | <5 | <5 |
| Fuzzer crashes/day | N/A | 0 | 0 | 0 |
| Test execution time | ~3min | ~10min | ~20min | <30min |
| Code coverage | ~70% (est) | 80% | 85% | 80-90% |
| Bug escape rate | High | Medium | Low | Very low |

## Open Questions

1. **Resource allocation:** How many person-weeks for Phase 1-3?
2. **CI impact:** Longer test suites → slower CI. Acceptable?
3. **Test flakiness:** How to handle non-deterministic tests (concurrency, timing)?
4. **External contributions:** Should we accept community tests via PR?
5. **Differential testing:** Worth implementing before LLVM backend exists?

## Alternatives Considered

### Alt 1: Focus Only on Fuzzing

**Pros:** Automated, finds deep bugs
**Cons:** Misses systematic edge cases, poor error message coverage
**Verdict:** Necessary but insufficient

### Alt 2: Aim for 100% Line Coverage

**Pros:** Comprehensive, measurable
**Cons:** Coverage ≠ correctness, can miss edge cases even with 100% coverage
**Verdict:** Good metric but not sufficient goal

### Alt 3: Minimal Testing (Current Path)

**Pros:** Fast development
**Cons:** Not production-ready, users will find bugs
**Verdict:** Was appropriate for prototype, inadequate for v1.0

## References

### Academic Research
- [A Survey of Modern Compiler Fuzzing](https://arxiv.org/pdf/2306.06884) - Comprehensive overview of fuzzing techniques
- [Metamorphic Testing of Deep Learning Compilers](https://dl.acm.org/doi/abs/10.1145/3508035) - Metamorphic testing approach
- [Metamorphic Testing: A Review](https://dl.acm.org/doi/10.1145/3143561) - Testing the untestable

### Industry Practice
- [Rust Compiler Testing Guide](https://rustc-dev-guide.rust-lang.org/tests/intro.html) - How rustc tests
- [Rust Test Best Practices](https://rustc-dev-guide.rust-lang.org/tests/best-practices.html) - Naming, organization, quality
- [How Rust is Tested](https://brson.github.io/2017/07/10/how-rust-is-tested/) - Overview of Rust's test infrastructure

### Tools and Frameworks
- [The Fuzzing Book](https://www.fuzzingbook.org/) - Fuzzing techniques and implementation
- [Property-Based Testing Overview](https://blog.nelhage.com/post/property-testing-is-fuzzing/) - Relationship to fuzzing

## Conclusion

**Honest Assessment:** Pluto has good testing for a research compiler, but we're at ~65% of production-grade. The gap isn't "random error paths"—it's **systematic missing categories**: fuzzing, property testing, stress tests, error quality, and comprehensive negative testing.

**Path Forward:** Implement Phases 1-3 over 3 months. This will:
1. Find 100+ bugs before users do
2. Prevent regressions
3. Enable confident v1.0 release
4. Establish testing culture for long-term quality

**Cost:** ~8-12 person-weeks upfront, then ~20% of dev time on ongoing test maintenance.

**Benefit:** Production-ready compiler with <5 bugs per 10k LOC, industry-standard reliability.

---

**Next Steps:**
1. Review and approve RFC
2. Prioritize Phase 1 tasks
3. Assign ownership for fuzzing infrastructure
4. Set milestones for each phase

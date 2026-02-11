# RFC-TEST-001: Test Infrastructure

**Status:** Draft
**Author:** Development Team
**Created:** 2026-02-11
**Related:** [RFC: Testing Strategy](rfc-testing-strategy.md)

## Summary

Establishes the foundational testing infrastructure for the Pluto compiler, including property-based testing, fuzzing, snapshot testing, and performance tracking.

## Motivation

Current testing relies on hand-written integration tests in `tests/integration/`. While effective for known scenarios, this approach has gaps:

- **No property-based testing:** Can't easily express invariants like "all valid ASTs type-check or produce a type error"
- **No fuzzing:** Parser/typeck edge cases discovered manually, not systematically
- **No snapshot testing:** Error message changes require manual inspection
- **No performance tracking:** Compile-time regressions detected ad-hoc

This RFC proposes infrastructure to address these gaps with minimal friction for developers.

## Detailed Design

### 1. Property-Based Testing

**Tool:** `proptest` (mature, integrates with cargo test)

**Structure:**
```
tests/
  property/
    ast.rs           # AST invariants (spans, well-formedness)
    typeck.rs        # Type system properties
    codegen.rs       # Codegen correctness properties
    transforms.rs    # Module flatten, closure lift, monomorphize
```

**Example Properties:**
- **AST well-formedness:** All spans non-overlapping, monotonic
- **Type soundness:** Well-typed programs don't get stuck in codegen
- **Transformation idempotence:** Monomorphize twice = monomorphize once
- **Error determinism:** Same input → same error (no HashSet iteration leakage)

**Sample Test:**
```rust
// tests/property/ast.rs
use proptest::prelude::*;

proptest! {
    #[test]
    fn spans_are_monotonic(prog in arb_program()) {
        let spans = collect_all_spans(&prog);
        assert!(spans.windows(2).all(|w| w[0].end <= w[1].start));
    }
}
```

**Generators Needed:**
- `arb_program()` — generates random valid ASTs
- `arb_type()` — generates PlutoType instances
- `arb_expr()` — generates expressions with type constraints

**Dependencies:**
```toml
[dev-dependencies]
proptest = "1.0"
```

### 2. Fuzzing

**Tool:** `cargo-fuzz` (libFuzzer, best Rust support)

**Structure:**
```
fuzz/
  fuzz_targets/
    lex.rs           # Fuzz lexer with arbitrary bytes
    parse.rs         # Fuzz parser with token streams
    typeck.rs        # Fuzz typeck with ASTs
    compile.rs       # End-to-end fuzzing
```

**Fuzzing Strategies:**

#### a) Lexer Fuzzing (Unstructured)
```rust
// fuzz/fuzz_targets/lex.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use plutoc::lexer::lex;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = lex(s); // Should never panic
    }
});
```

#### b) Parser Fuzzing (Structured)
Generate valid token streams, fuzz parser:
```rust
// fuzz/fuzz_targets/parse.rs
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
struct TokenStream {
    tokens: Vec<FuzzToken>,
}

#[derive(Arbitrary, Debug)]
enum FuzzToken {
    Ident(String),
    IntLit(i64),
    Plus, Minus, Star, // ...
}

fuzz_target!(|ts: TokenStream| {
    let tokens = ts.to_pluto_tokens();
    let _ = parse_program(tokens); // Should never panic
});
```

#### c) Typeck Fuzzing (Valid ASTs)
Generate well-formed ASTs, fuzz typeck:
```rust
// fuzz/fuzz_targets/typeck.rs
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
struct FuzzProgram {
    functions: Vec<FuzzFunction>,
    classes: Vec<FuzzClass>,
}

fuzz_target!(|prog: FuzzProgram| {
    let ast = prog.to_ast();
    let _ = typeck(&ast); // Should return Ok or Err, never panic
});
```

**CI Integration:**
- Nightly fuzzing runs (1 hour per target)
- Store corpus in `fuzz/corpus/`
- Regression tests from crashes

### 3. Snapshot Testing

**Tool:** `insta` (popular, ergonomic, good diffs)

**Use Cases:**
- Error message formatting
- Pretty-printed ASTs
- IR output (for debugging)

**Structure:**
```
tests/
  snapshots/
    errors/
      type_mismatch.snap
      missing_return.snap
      ...
    diagnostics/
      unused_variable.snap
      ...
```

**Example Test:**
```rust
// tests/integration/error_messages.rs
use insta::assert_snapshot;

#[test]
fn type_mismatch_error() {
    let source = r#"
        fn foo(x: int) string {
            return x
        }
    "#;

    let err = compile_should_fail(source);
    assert_snapshot!(format_error(&err));
}
```

**Snapshot format:**
```
---
source: tests/integration/error_messages.rs
expression: format_error(&err)
---
Error: Type mismatch
  ┌─ <input>:3:20
  │
3 │             return x
  │                    ^ Expected `string`, found `int`
  │
```

**Review workflow:**
- `cargo insta test` — run tests, detect changes
- `cargo insta review` — interactively approve/reject changes
- Commit approved snapshots with code changes

**Dependencies:**
```toml
[dev-dependencies]
insta = "1.34"
```

### 4. Performance Tracking

**Tool:** `criterion` (statistical benchmarking)

**Structure:**
```
benches/
  compile_time.rs    # Benchmark compile pipeline stages
  runtime.rs         # Benchmark runtime builtins
  stdlib.rs          # Benchmark stdlib functions
```

**Example Benchmark:**
```rust
// benches/compile_time.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use plutoc::compile;

fn bench_compile_hello_world(c: &mut Criterion) {
    let source = r#"
        fn main() {
            print("Hello, world!")
        }
    "#;

    c.bench_function("compile_hello_world", |b| {
        b.iter(|| compile(black_box(source)))
    });
}

criterion_group!(benches, bench_compile_hello_world);
criterion_main!(benches);
```

**CI Integration:**
- Store baseline in `target/criterion/`
- Fail PR if >10% regression
- Generate HTML reports

**Dependencies:**
```toml
[dev-dependencies]
criterion = "0.5"
```

### 5. Compiler Testing API

**Goal:** Make it easy to write tests that exercise specific compiler stages.

**API Design:**
```rust
// tests/common/mod.rs (or src/testing.rs)

pub struct TestCompiler {
    source: String,
    stdlib_path: Option<PathBuf>,
}

impl TestCompiler {
    pub fn new(source: &str) -> Self { /* ... */ }

    pub fn with_stdlib(mut self, path: PathBuf) -> Self { /* ... */ }

    pub fn lex(&self) -> Result<Vec<Token>, LexError> { /* ... */ }

    pub fn parse(&self) -> Result<Program, ParseError> { /* ... */ }

    pub fn typecheck(&self) -> Result<TypeEnv, TypeError> { /* ... */ }

    pub fn codegen(&self) -> Result<Vec<u8>, CodegenError> { /* ... */ }

    pub fn compile(&self) -> Result<PathBuf, CompileError> { /* ... */ }

    pub fn run(&self) -> Result<TestOutput, RunError> { /* ... */ }
}

pub struct TestOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
```

**Usage:**
```rust
#[test]
fn test_type_inference() {
    let result = TestCompiler::new("let x = 42")
        .typecheck()
        .unwrap();

    assert_eq!(result.get_var_type("x"), PlutoType::Int);
}

#[test]
fn test_runtime_output() {
    let output = TestCompiler::new(r#"fn main() { print("hi") }"#)
        .run()
        .unwrap();

    assert_eq!(output.stdout, "hi\n");
    assert_eq!(output.exit_code, 0);
}
```

**Benefits:**
- Test specific stages without full compilation
- Inspect intermediate representations
- Reduce test boilerplate

### 6. Coverage Reporting

**Tool:** `cargo-tarpaulin` (or `cargo-llvm-cov`)

**CI Integration:**
```yaml
# .github/workflows/coverage.yml
- name: Run coverage
  run: |
    cargo tarpaulin --out Lcov --output-dir coverage

- name: Upload to Codecov
  uses: codecov/codecov-action@v3
  with:
    files: coverage/lcov.info
```

**Goals:**
- 85%+ line coverage
- 90%+ branch coverage for typeck/codegen
- Identify untested paths

## Implementation Plan

### Week 1: Foundation
- [ ] Add proptest, insta, criterion to Cargo.toml
- [ ] Create `tests/property/`, `tests/snapshots/` directories
- [ ] Implement `TestCompiler` API
- [ ] Write 5 example property tests (AST invariants)
- [ ] Write 5 example snapshot tests (error messages)

### Week 2: Fuzzing + Benchmarks
- [ ] Install cargo-fuzz: `cargo install cargo-fuzz`
- [ ] Create `fuzz/` directory, implement lexer fuzzer
- [ ] Implement parser fuzzer with Arbitrary for TokenStream
- [ ] Create `benches/` directory, implement compile-time benchmarks
- [ ] Set up CI job for nightly fuzzing (1 hour runs)

### Week 3: CI Integration
- [ ] Add GitHub Actions job for `cargo insta test`
- [ ] Add criterion regression check to PR CI
- [ ] Set up tarpaulin coverage reporting
- [ ] Configure Codecov integration
- [ ] Document testing guidelines in CLAUDE.md

## Testing the Infrastructure

Meta-tests to validate the testing infrastructure itself:

1. **Property test generator coverage:** Ensure generators produce diverse ASTs (collect and analyze generated programs)
2. **Fuzzer effectiveness:** Seed with known crashes, verify they're re-discovered
3. **Snapshot stability:** Run twice, ensure no spurious diffs
4. **Benchmark stability:** Run 10 times, measure variance (<5%)

## Open Questions

1. Should we use `quickcheck` instead of `proptest`? (proptest has better shrinking)
2. Should we write custom Arbitrary impls or use derive? (custom = more control)
3. Should benchmarks run on every PR or just main? (main = less noise)
4. Should we track runtime performance or just compile time? (both, separate jobs)

## Alternatives Considered

- **Manual testing only:** Not scalable, misses edge cases
- **Mutation testing (cargo-mutants):** Deferred to Phase 5, too heavyweight initially
- **Formal verification (why3, Coq):** Too heavyweight for current stage

## Success Criteria

- Developers can write property tests in <10 minutes
- Fuzzing discovers at least 3 new bugs in first week
- Snapshot tests catch unintended error message changes
- Performance regressions detected before merge

---

**Next:** [RFC-TEST-002: Core Coverage Plan](rfc-core-coverage.md)

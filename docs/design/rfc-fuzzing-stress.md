# RFC-TEST-003: Fuzzing and Stress Testing Strategy

**Status:** Draft
**Author:** Development Team
**Created:** 2026-02-11
**Related:** [RFC: Testing Strategy](rfc-testing-strategy.md), [RFC-TEST-001](rfc-test-infrastructure.md), [RFC-TEST-002](rfc-core-coverage.md)

## Summary

Establishes systematic fuzzing and stress testing for the Pluto compiler and runtime to discover edge cases, validate correctness under extreme conditions, and ensure reliability.

## Motivation

While unit and integration tests validate known scenarios, they cannot exhaustively explore the input space. Fuzzing and stress testing address this by:

1. **Fuzzing:** Automatically generates millions of test inputs to discover crashes, hangs, and logic errors
2. **Stress testing:** Validates correctness under resource pressure (memory, concurrency, I/O)
3. **Reliability:** Builds confidence that the compiler won't crash in production

**Key risks mitigated:**
- Parser crashes on malformed input
- Typeck panics on unusual type combinations
- Codegen segfaults on edge-case IR patterns
- GC memory leaks or corruption under concurrent allocation
- Race conditions in spawn/Task implementation

## Detailed Design

### 1. Fuzzing Strategy

#### a) Unstructured Fuzzing (Lexer)

**Goal:** Ensure lexer never panics on arbitrary byte streams.

**Implementation:**
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

**Oracles:**
- No panics
- All errors are `LexError`, not panics
- Spans are valid (start <= end, end <= input.len())

**Corpus seeds:**
- Valid Pluto programs from examples/
- Edge cases: `""`, `"\"`, `"/*"`, `"\u{10FFFF}"`
- Historical crash cases

**Metrics:**
- Code coverage (new branches discovered)
- Crashes per million executions (goal: 0)
- Execution speed (goal: >10K exec/sec)

#### b) Structured Fuzzing (Parser)

**Goal:** Fuzz parser with **valid token streams** to find logic errors, not just syntax handling.

**Token Generator:**
```rust
// fuzz/fuzz_targets/parse.rs
use arbitrary::{Arbitrary, Unstructured};
use plutoc::lexer::Token;

#[derive(Debug)]
struct FuzzTokenStream {
    tokens: Vec<Token>,
}

impl<'a> Arbitrary<'a> for FuzzTokenStream {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = u.int_in_range(0..=100)?;
        let mut tokens = Vec::new();

        for _ in 0..len {
            let tok = match u.int_in_range(0..=20)? {
                0 => Token::Fn,
                1 => Token::Class,
                2 => Token::Let,
                3 => Token::Return,
                4 => Token::If,
                5 => Token::Else,
                6 => Token::While,
                7 => Token::For,
                8 => Token::LParen,
                9 => Token::RParen,
                10 => Token::LBrace,
                11 => Token::RBrace,
                12 => Token::Comma,
                13 => Token::Colon,
                14 => Token::Arrow,
                15 => Token::Plus,
                16 => Token::Minus,
                17 => Token::Star,
                18 => Token::Slash,
                19 => Token::Ident(u.arbitrary::<String>()?),
                20 => Token::IntLit(u.arbitrary::<i64>()?),
                _ => unreachable!(),
            };
            tokens.push(tok);
        }

        Ok(FuzzTokenStream { tokens })
    }
}

fuzz_target!(|ts: FuzzTokenStream| {
    let _ = plutoc::parser::parse_program(&ts.tokens); // No panic
});
```

**Oracles:**
- No panics (parser errors are ok)
- Spans are consistent
- Error recovery doesn't lose tokens

**Corpus seeds:**
- All integration test programs (lexed)
- Minimal examples: `fn foo() {}`, `let x = 1`, etc.

#### c) Grammar-Based Fuzzing (End-to-End)

**Goal:** Generate **syntactically valid Pluto programs** to fuzz typeck and codegen.

**Approach:** Use a Pluto grammar to generate random valid programs.

**Grammar snippet (simplified):**
```
Program    := (Decl)*
Decl       := FuncDecl | ClassDecl | EnumDecl | TraitDecl
FuncDecl   := "fn" Ident "(" Params ")" Type Block
Block      := "{" (Stmt)* "}"
Stmt       := LetStmt | ReturnStmt | IfStmt | ExprStmt
Expr       := Literal | Ident | BinOp | Call | ...
```

**Implementation:**
```rust
// fuzz/fuzz_targets/compile.rs
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
struct FuzzProgram {
    functions: Vec<FuzzFunction>,
    classes: Vec<FuzzClass>,
}

#[derive(Arbitrary, Debug)]
struct FuzzFunction {
    name: String,
    params: Vec<FuzzParam>,
    return_type: FuzzType,
    body: FuzzBlock,
}

// ... (full grammar)

impl FuzzProgram {
    fn to_source(&self) -> String {
        // Pretty-print to Pluto source
    }
}

fuzz_target!(|prog: FuzzProgram| {
    let source = prog.to_source();
    let _ = plutoc::compile(&source); // No panic
});
```

**Oracles:**
- No panics in typeck or codegen
- Type errors are valid (not internal compiler errors)
- Generated binaries execute without segfaults (run under valgrind)

**Advanced:** Use `proptest` to **shrink** failing cases to minimal reproducers.

#### d) Mutation Fuzzing (Coverage-Guided)

**Goal:** Maximize code coverage by mutating inputs based on feedback.

**Tool:** libFuzzer (built into cargo-fuzz)

**Setup:**
```bash
cargo fuzz run lex -- -max_total_time=3600  # 1 hour
cargo fuzz run parse -- -max_total_time=3600
cargo fuzz run compile -- -max_total_time=3600
```

**Coverage tracking:**
- libFuzzer tracks basic block coverage
- Prioritizes inputs that hit new branches
- Stores corpus in `fuzz/corpus/`

**Nightly CI:**
```yaml
# .github/workflows/fuzz.yml
name: Fuzzing
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM daily

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: cargo fuzz run lex -- -max_total_time=3600
      - run: cargo fuzz run parse -- -max_total_time=3600
      - run: cargo fuzz run compile -- -max_total_time=3600
      - name: Upload crashes
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: fuzz-crashes
          path: fuzz/artifacts/
```

### 2. Stress Testing Strategy

#### a) GC Stress Tests

**Goal:** Validate GC correctness under memory pressure.

**Test Cases:**

**Allocation Storm:**
```pluto
test "gc allocation storm" {
    let arrays = []
    for i in 0..100000 {
        arrays.push([i, i+1, i+2])
    }
    expect(arrays.len()).to_equal(100000)
}
```

**Large Objects:**
```pluto
test "gc large objects" {
    let large = []
    for i in 0..1000 {
        large.push("x" * 10000)  // 10KB strings
    }
    expect(large.len()).to_equal(1000)
}
```

**Mixed Sizes:**
```pluto
test "gc mixed sizes" {
    for i in 0..10000 {
        let small = [i]
        let medium = [i; 100]
        let large = [i; 10000]
    }
}
```

**Concurrent Allocation:**
```pluto
test "gc concurrent allocation" {
    let tasks = []
    for i in 0..100 {
        let t = spawn allocate_loop(i)
        tasks.push(t)
    }
    for t in tasks {
        t.get()
    }
}

fn allocate_loop(id: int) {
    for i in 0..1000 {
        let arr = [id, i]
    }
}
```

**Oracles:**
- No memory leaks (run under valgrind)
- No segfaults
- No double-frees
- No use-after-frees
- No data corruption (verify values after GC)

**Valgrind Integration:**
```bash
# tests/stress/run_valgrind.sh
#!/bin/bash
cargo build --release
for test in tests/stress/*.pluto; do
    valgrind --leak-check=full --error-exitcode=1 \
        ./target/release/plutoc run "$test"
done
```

#### b) Concurrency Stress Tests

**Goal:** Validate Task/spawn correctness under high concurrency.

**Test Cases:**

**10K Concurrent Tasks:**
```pluto
test "10k concurrent tasks" {
    let tasks = []
    for i in 0..10000 {
        let t = spawn compute(i)
        tasks.push(t)
    }

    let sum = 0
    for t in tasks {
        sum = sum + t.get()
    }

    expect(sum).to_equal(49995000)  // Sum of 0..9999
}

fn compute(x: int) int {
    return x
}
```

**Concurrent Map Mutations:**
```pluto
test "concurrent map mutations" {
    let m = Map<int, int> {}
    let tasks = []

    for i in 0..100 {
        let t = spawn insert_range(m, i * 100, (i + 1) * 100)
        tasks.push(t)
    }

    for t in tasks {
        t.get()
    }

    expect(m.len()).to_equal(10000)
}

fn insert_range(m: Map<int, int>, start: int, end: int) {
    for i in start..end {
        m[i] = i * 2
    }
}
```

**Task Cancellation (Phase 2):**
```pluto
test "task cancellation" {
    let t = spawn long_running()
    t.cancel()
    // Should not block indefinitely
}

fn long_running() {
    for i in 0..1000000 {
        let x = [i]
    }
}
```

**Oracles:**
- No deadlocks (timeout after 60s)
- No race conditions (run under ThreadSanitizer)
- Correct results (sum, counts match expected)
- No segfaults

**ThreadSanitizer Integration:**
```bash
# tests/stress/run_tsan.sh
#!/bin/bash
RUSTFLAGS="-Z sanitizer=thread" cargo build --release
for test in tests/stress/concurrency_*.pluto; do
    ./target/release/plutoc run "$test"
done
```

#### c) Channel Stress Tests

**Goal:** Validate channel correctness under high throughput.

**Test Cases:**

**1M Messages:**
```pluto
test "channel 1M messages" {
    let (tx, rx) = chan<int>()

    let sender = spawn send_messages(tx, 1000000)
    let receiver = spawn receive_messages(rx, 1000000)

    sender.get()
    let sum = receiver.get()
    expect(sum).to_equal(499999500000)  // Sum of 0..999999
}

fn send_messages(tx: Sender<int>, count: int) {
    for i in 0..count {
        tx.send(i)
    }
    tx.close()
}

fn receive_messages(rx: Receiver<int>, count: int) int {
    let sum = 0
    for msg in rx {
        sum = sum + msg
    }
    return sum
}
```

**Multi-Producer Multi-Consumer:**
```pluto
test "channel MPMC" {
    let (tx, rx) = chan<int>()

    let producers = []
    for i in 0..10 {
        let tx_clone = tx.clone()
        let t = spawn produce(tx_clone, i * 1000, (i + 1) * 1000)
        producers.push(t)
    }

    let consumers = []
    for i in 0..5 {
        let rx_clone = rx.clone()
        let t = spawn consume(rx_clone)
        consumers.push(t)
    }

    for t in producers { t.get() }
    tx.close()

    let total = 0
    for t in consumers {
        total = total + t.get()
    }

    expect(total).to_equal(4995000)  // Sum of 0..9999
}
```

**Oracles:**
- No lost messages
- No duplicate messages
- Correct ordering (for single producer/consumer)
- No deadlocks

#### d) Error Handling Stress Tests

**Goal:** Validate error propagation correctness under nesting and concurrency.

**Test Cases:**

**Deeply Nested Propagation:**
```pluto
error TestError

test "deeply nested propagation" {
    let result = level1()
    expect(result).to_equal(42) catch TestError {
        // Expected
    }
}

fn level1() int raises TestError { return level2()! }
fn level2() int raises TestError { return level3()! }
fn level3() int raises TestError { return level4()! }
fn level4() int raises TestError { return level5()! }
fn level5() int raises TestError {
    raise TestError
    return 42
}
```

**Concurrent Error Isolation:**
```pluto
error TaskError

test "concurrent error isolation" {
    let tasks = []
    for i in 0..100 {
        let t = spawn maybe_error(i)
        tasks.push(t)
    }

    let errors = 0
    let successes = 0

    for t in tasks {
        t.get() catch TaskError {
            errors = errors + 1
        } else {
            successes = successes + 1
        }
    }

    expect(errors).to_equal(50)
    expect(successes).to_equal(50)
}

fn maybe_error(x: int) raises TaskError {
    if x % 2 == 0 {
        raise TaskError
    }
}
```

**Oracles:**
- Errors don't leak across threads
- TLS error state correctly isolated
- No error state corruption

### 3. Tooling and Automation

#### a) Continuous Fuzzing

**Infrastructure:**
- Nightly fuzzing runs (1 hour per target)
- Store corpus in git (small, curated)
- Upload crash artifacts to S3/GH artifacts
- Slack/email notifications on new crashes

**Corpus Management:**
```bash
# scripts/fuzz_update_corpus.sh
#!/bin/bash
cargo fuzz run lex -- -max_total_time=3600
cargo fuzz run parse -- -max_total_time=3600
cargo fuzz run compile -- -max_total_time=3600

# Minimize corpus (remove redundant inputs)
cargo fuzz cmin lex
cargo fuzz cmin parse
cargo fuzz cmin compile

# Commit updated corpus
git add fuzz/corpus/
git commit -m "Update fuzz corpus"
```

#### b) Stress Test CI

```yaml
# .github/workflows/stress.yml
name: Stress Tests
on:
  schedule:
    - cron: '0 4 * * 0'  # Weekly, Sunday 4 AM

jobs:
  stress:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo build --release

      - name: GC stress tests
        run: ./tests/stress/run_gc_stress.sh

      - name: Concurrency stress tests
        run: ./tests/stress/run_concurrency_stress.sh

      - name: Valgrind leak check
        run: ./tests/stress/run_valgrind.sh

      - name: ThreadSanitizer
        run: ./tests/stress/run_tsan.sh
```

#### c) Regression Tests from Fuzzing

When fuzzing discovers a bug:

1. Minimize the failing input: `cargo fuzz tmin <target> <crash-file>`
2. Create a regression test: `tests/integration/regressions/issue_NNN.rs`
3. Add to corpus: `cp <crash-file> fuzz/corpus/<target>/`
4. Verify fix: `cargo fuzz run <target> <crash-file>`

### 4. Metrics and Monitoring

**Key Metrics:**
- **Fuzzing coverage:** % of basic blocks reached
- **Crash rate:** Crashes per million executions
- **Corpus size:** Number of unique inputs
- **Execution speed:** Executions per second
- **Stress test duration:** Time to complete stress suite
- **Memory usage:** Peak RSS during stress tests
- **Leak count:** valgrind-reported leaks

**Dashboard (future):**
- Track metrics over time
- Alert on regressions (coverage drops, new crashes)
- Visualize corpus growth

## Implementation Plan

### Week 1: Fuzzing Setup
- [ ] Install cargo-fuzz: `cargo install cargo-fuzz`
- [ ] Implement lexer fuzzer (unstructured)
- [ ] Implement parser fuzzer (structured, Arbitrary for tokens)
- [ ] Add corpus seeds from examples/
- [ ] Run initial fuzzing (1 hour per target)

### Week 2: Grammar Fuzzing
- [ ] Define Pluto grammar for generation
- [ ] Implement Arbitrary for FuzzProgram
- [ ] Implement end-to-end compiler fuzzer
- [ ] Add oracle: run generated binaries under valgrind
- [ ] Set up nightly CI for fuzzing

### Week 3: GC Stress Tests
- [ ] Write allocation storm tests (100K objects)
- [ ] Write large object tests (10MB allocations)
- [ ] Write concurrent allocation tests (100 tasks)
- [ ] Set up valgrind integration script
- [ ] Run tests under valgrind, fix any leaks

### Week 4: Concurrency Stress Tests
- [ ] Write 10K task test
- [ ] Write concurrent map mutation test
- [ ] Write channel 1M message test
- [ ] Write MPMC channel test
- [ ] Set up ThreadSanitizer integration script

### Week 5: Error + Integration
- [ ] Write deeply nested error propagation test
- [ ] Write concurrent error isolation test
- [ ] Set up weekly stress test CI job
- [ ] Create regression test template
- [ ] Document fuzzing/stress testing in CLAUDE.md

## Success Criteria

- **Fuzzing:**
  - 1M+ executions without panics
  - 80%+ code coverage in lexer/parser/typeck
  - Corpus size: 100+ unique inputs per target
  - Execution speed: >5K exec/sec

- **Stress Testing:**
  - GC handles 1GB allocations without leaks
  - 10K concurrent tasks complete successfully
  - 1M channel messages sent without deadlock
  - All stress tests pass under valgrind and ThreadSanitizer

- **Automation:**
  - Nightly fuzzing runs, reports to Slack
  - Weekly stress test runs
  - Regression tests for all fuzz-discovered bugs

## Open Questions

1. Should we use OSS-Fuzz for continuous fuzzing? (free for open source)
2. Should we fuzz the runtime C code (builtins.c) separately?
3. Should we use AFL++ instead of libFuzzer? (better for parser fuzzing)
4. Should we stress test with larger heaps (10GB+)?

## Alternatives Considered

- **Manual testing only:** Not scalable, misses edge cases
- **Random testing without guidance:** Less efficient than coverage-guided fuzzing
- **Stress testing only on release candidates:** Too late, bugs harder to fix

---

**Next:** [Test Implementation Plan](test-implementation-plan.md)

# RFC-TEST-002: Core Language Coverage Plan

**Status:** Draft
**Author:** Development Team
**Created:** 2026-02-11
**Related:** [RFC: Testing Strategy](rfc-testing-strategy.md), [RFC-TEST-001](rfc-test-infrastructure.md)

## Summary

Systematically tests all implemented Pluto language features and their interactions to achieve 90%+ confidence in compiler correctness.

## Motivation

The Pluto compiler has grown to support 15+ major features (generics, closures, traits, errors, concurrency, contracts, nullable types, etc.). While integration tests exist for each feature in isolation, **combinatorial interactions** are undertested:

- Generics + closures + error propagation
- Spawn + nullable types + contracts
- Traits + generics + DI
- Enums + match + error handling

This RFC proposes a systematic approach to testing all N×N feature interactions and edge cases.

## Coverage Matrix

### Feature List (15 major features)

1. **Core syntax:** functions, let, if/else, while, for, return, break/continue
2. **Types:** int, float, bool, string, void, arrays
3. **Classes:** fields, methods, constructors, `mut self`
4. **Traits:** trait decls, impl, trait objects, vtables
5. **Enums:** unit variants, data variants, match, exhaustiveness
6. **Closures:** capture, arrow syntax, fn types, indirect calls
7. **Generics:** monomorphization, type params, bounds, explicit type args
8. **Errors:** raise, `!`, catch, inferred error-ability
9. **Nullable:** `T?`, `none`, `?` propagation
10. **Contracts:** invariant, requires, ensures, decidable fragment
11. **Concurrency:** spawn, Task<T>, .get(), GC + threads
12. **DI:** app, bracket deps, scoped classes, topological sort
13. **Modules:** import, pub, flatten, transitive imports
14. **Maps/Sets:** Map<K,V>, Set<T>, hash operations
15. **Stdlib:** strings, math, collections, channels, json, http, log, time

### N×N Interaction Matrix

Create a 15×15 matrix where each cell represents a test for the interaction between two features. Priority levels:

- **P0 (Critical):** Core feature interactions, high bug risk
- **P1 (Important):** Common use cases, moderate risk
- **P2 (Nice-to-have):** Rare combinations, low risk

**Example cells:**

| Feature A | Feature B | Priority | Test Status | Test Name |
|-----------|-----------|----------|-------------|-----------|
| Generics | Closures | P0 | ✅ Done | `test_generic_closure_capture` |
| Spawn | Errors | P0 | ✅ Done | `test_spawn_error_propagation` |
| Traits | Generics | P0 | ✅ Done | `test_generic_trait_impl` |
| Nullable | Errors | P1 | ⚠️ Partial | `test_nullable_error_interaction` |
| Contracts | Traits | P1 | ❌ Missing | `test_trait_method_contracts` |
| DI | Generics | P0 | ✅ Done | `test_di_generic_classes` |
| Enums | Match | P0 | ✅ Done | `test_enum_exhaustiveness` |
| Maps | Generics | P0 | ✅ Done | `test_generic_map_keys` |
| Closures | Errors | P1 | ⚠️ Partial | `test_closure_error_capture` |
| Spawn | Contracts | P2 | ❌ Missing | `test_spawn_contract_invariants` |

**Full matrix:** See `docs/testing/coverage-matrix.csv` (to be created).

## Coverage Areas

### 1. Lexer Edge Cases

**Goal:** Ensure tokenizer handles all valid inputs and rejects invalid ones gracefully.

**Test Cases:**
- Unicode identifiers: `let 変数 = 42`, `fn 函数() {}`
- Escaped strings: `"hello\nworld"`, `"quote: \"hi\""`
- Edge tokens: `>>` (two `>` for bitshift vs generics), `...`
- Whitespace handling: tabs, CRLF, mixed
- Integer limits: `9223372036854775807` (I64::MAX), overflows
- Float formats: `1.0`, `.5`, `1e10`, `1.5e-3`
- Comments: `//`, `/* */`, nested `/* /* */ */`
- Invalid UTF-8: reject gracefully with error

**Property Tests:**
- Lex-roundtrip: `lex(source).reconstruct() == source` (whitespace-normalized)
- No panics: `lex(arbitrary_bytes)` never panics

### 2. Parser Edge Cases

**Goal:** Ensure parser handles all syntactically valid programs and produces clear errors for invalid ones.

**Test Cases:**
- Precedence: `1 + 2 * 3 == 7`, `1 < 2 == 3 < 4` (comparison chains invalid)
- Associativity: `1 - 2 - 3 == -4` (left-associative)
- Edge expressions: `f()(x)`, `arr[0][1]`, `obj.method()(x)`
- Empty blocks: `if true {}`, `while false {}`
- Newline handling: function calls across lines, infix ops across lines
- Generics: `Box<Pair<int, int>>`, `f<int, float>()`
- Arrow functions: `(x: int) => x`, `() => { return 42 }`
- Struct literals: `Foo { x: 1, y: 2 }`, `Foo {}` (empty)
- Match arms: single-line, multi-line, nested patterns

**Property Tests:**
- Parse-roundtrip: `parse(source).pretty_print() parses identically`
- Span coverage: All AST nodes have non-overlapping spans

### 3. Typeck Edge Cases

**Goal:** Ensure type checker correctly infers types, enforces constraints, and produces actionable errors.

**Test Cases:**
- Inference: `let x = 42` (int), `let f = (x: int) => x + 1` (fn(int) int)
- Generics: `Box<int>`, `Pair<string, bool>`, inferred vs explicit type args
- Traits: trait object casts, vtable dispatch, trait bounds on generics
- Errors: nested propagation (`f()!.g()!`), catch scopes, unhandled errors
- Nullable: `T?` coercion, `none` inference, `?` in non-nullable return
- Closures: capture inference, nested closures, closures in generics
- Contracts: decidable fragment enforcement, invariant on generic classes
- Enums: match exhaustiveness, data variant field access
- Method resolution: mut self enforcement, overloading prevention
- Forward references: classes referencing later-declared classes (DI)

**Property Tests:**
- Well-typed programs don't panic in codegen
- Type errors are deterministic (no HashMap iteration)
- Monomorphize idempotence: `monomorphize(monomorphize(ast)) == monomorphize(ast)`

### 4. Codegen Edge Cases

**Goal:** Ensure all PlutoType variants and language constructs lower correctly to Cranelift IR.

**Test Cases:**
- All PlutoType variants: Int, Float, Bool, String, Void, Class, Array, Trait, Enum, Fn, Error, Task, Nullable, Map, Set, Range
- Calling conventions: direct calls, method calls, trait calls, closure calls, RPC calls (future)
- Memory layout: class fields, bracket deps first, enum discriminants, closure captures
- GC integration: allocations, reference tracing, tag correctness
- Control flow: if/else, while, for, break/continue, match, early return
- String interpolation: `"x={x}"`, nested expressions
- Array operations: indexing, .len(), bounds checking
- Error handling: raise, propagate, catch, TLS error state
- Spawn: thread creation, task handles, .get() blocking
- Contracts: invariant checks after constructors and mut methods

**Property Tests:**
- Codegen never panics on well-typed ASTs
- Generated binaries execute without segfaults (run under valgrind)

### 5. Runtime Edge Cases

**Goal:** Ensure runtime builtins handle edge cases and stress conditions.

**Test Cases:**
- GC: allocation storms (1M small objects), large objects (10MB), mixed sizes
- Concurrent GC: spawn 100 tasks, each allocating 10K objects
- String ops: empty strings, very long strings (1MB), UTF-8 edge cases
- Array ops: empty arrays, single-element, resizing, bounds violations
- Error state: concurrent errors (different errors in different threads)
- Task lifecycle: spawn, get, get twice (idempotent), task outlives parent
- Channels: blocking send/recv, non-blocking, full/empty, close semantics
- Contracts: invariant violations, requires failures, ensures failures

**Stress Tests:**
- 10K concurrent tasks
- 1GB total allocations without leaks (valgrind)
- 1M channel messages
- 100K map insertions/removals

### 6. Module System Edge Cases

**Goal:** Ensure module resolution, flattening, and visibility work correctly.

**Test Cases:**
- Single-file modules: `import math`, `math.add()`
- Directory modules: `import db`, `db.models.User`
- Transitive imports: A imports B, B imports C, A uses C items
- Circular imports: A imports B, B imports A (should error)
- Visibility: `pub` vs private, access violations
- Name conflicts: same name in multiple modules, prefixing
- Re-exports: `pub import foo` (not yet implemented)
- Stdlib imports: `import std.strings`, `import std.collections`

**Property Tests:**
- Module flatten is deterministic
- Visibility rules never violated

### 7. Feature Interaction Tests

**Goal:** Test all P0 and P1 pairs from the N×N matrix.

**Example Tests:**

#### Generics + Closures
```pluto
class Box<T> {
    value: T

    fn map<U>(self, f: fn(T) U) Box<U> {
        return Box<U> { value: f(self.value) }
    }
}

test "generic closure in method" {
    let b = Box<int> { value: 42 }
    let b2 = b.map((x: int) => x * 2)
    expect(b2.value).to_equal(84)
}
```

#### Spawn + Errors
```pluto
error NetworkError

fn fetch(url: string) string raises NetworkError {
    if url == "" { raise NetworkError }
    return "data"
}

test "spawn with error propagation" {
    let t = spawn fetch("http://example.com")
    let result = t.get()! // propagate error
    expect(result).to_equal("data")
}
```

#### Traits + Generics + DI
```pluto
trait Repo<T> {
    fn save(self, item: T)
}

class UserRepo<T> impl Repo<T> [db: Database] {
    fn save(self, item: T) {
        // ...
    }
}

app MyApp [repo: UserRepo<User>] {
    fn main(self) {
        self.repo.save(User { name: "Alice" })
    }
}
```

#### Nullable + Errors
```pluto
error ParseError

fn parse_int(s: string) int? raises ParseError {
    if s == "bad" { raise ParseError }
    return s.to_int() // returns int?
}

test "nullable with error" {
    let x = parse_int("42")!? // propagate error, then null
    expect(x).to_equal(42)
}
```

#### Contracts + Traits
```pluto
trait Stack<T> {
    requires self.size() >= 0
    fn push(mut self, item: T)
    ensures self.size() == old(self.size()) + 1
}

class VecStack<T> impl Stack<T> {
    items: T[]

    invariant self.items.len() >= 0

    fn size(self) int { return self.items.len() }
    fn push(mut self, item: T) { /* ... */ }
}
```

## Test File Organization

Reorganize `tests/integration/` to reflect coverage areas:

```
tests/
  integration/
    lexer/
      unicode.rs
      edge_tokens.rs
      numbers.rs
    parser/
      precedence.rs
      generics.rs
      edge_cases.rs
    typeck/
      inference.rs
      generics.rs
      traits.rs
      errors.rs
      nullable.rs
    codegen/
      types.rs
      calling_conventions.rs
      memory_layout.rs
    runtime/
      gc_stress.rs
      concurrency_stress.rs
      error_handling.rs
    modules/
      transitive_imports.rs
      visibility.rs
      circular.rs
    interactions/
      generics_closures.rs
      spawn_errors.rs
      traits_generics_di.rs
      nullable_errors.rs
      contracts_traits.rs
    end_to_end/
      web_service.rs       # Real-world scenario
      data_pipeline.rs     # Real-world scenario
```

## Implementation Plan

### Week 1: Audit and Gaps
- [ ] Audit existing tests, map to coverage matrix
- [ ] Identify top 20 missing test cases (prioritize P0)
- [ ] Create `coverage-matrix.csv` spreadsheet
- [ ] Set coverage goals: 85% line, 90% branch

### Week 2: Lexer + Parser
- [ ] Write 15 lexer edge case tests
- [ ] Write 20 parser edge case tests
- [ ] Property test: lex roundtrip
- [ ] Property test: parse spans non-overlapping

### Week 3: Typeck + Codegen
- [ ] Write 25 typeck edge case tests (inference, generics, traits, errors)
- [ ] Write 20 codegen edge case tests (all PlutoType variants)
- [ ] Property test: well-typed programs don't panic in codegen

### Week 4: Runtime + Modules
- [ ] Write 10 runtime edge case tests (GC, errors, tasks)
- [ ] Write 10 module system tests (transitive, circular, visibility)
- [ ] Stress test: 10K concurrent tasks
- [ ] Stress test: 1GB allocations

### Week 5: Feature Interactions
- [ ] Identify all P0 matrix cells (estimated 20-25)
- [ ] Write tests for each P0 interaction
- [ ] Identify P1 cells (estimated 30-40)
- [ ] Write tests for P1 interactions
- [ ] Create end-to-end scenario tests (web service, data pipeline)

### Week 6: Regression and Cleanup
- [ ] Review historical GitHub issues, add regression tests
- [ ] Run coverage report, identify untested paths
- [ ] Write tests for uncovered paths
- [ ] Update CLAUDE.md with new test structure
- [ ] Document coverage matrix in README

## Success Criteria

- **Coverage:** 85%+ line coverage, 90%+ branch coverage
- **Matrix:** All P0 cells tested (100%), 80%+ P1 cells tested
- **Regressions:** All known historical bugs have tests
- **Edge cases:** All identified edge cases tested
- **Real-world:** At least 2 end-to-end scenario tests

## Open Questions

1. Should we auto-generate some interaction tests from the matrix?
2. Should we have a "kitchen sink" test that uses all features together?
3. How do we test codegen correctness beyond "doesn't crash"? (Compare IR to golden files?)
4. Should we test backward compatibility as features evolve?

## Alternatives Considered

- **Random testing only:** Not comprehensive enough for safety-critical features
- **Exhaustive testing:** Infeasible, too many combinations
- **User-reported bugs only:** Reactive, not proactive

---

**Next:** [RFC-TEST-003: Fuzzing and Stress Testing](rfc-fuzzing-stress.md)

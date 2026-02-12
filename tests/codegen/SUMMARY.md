# Codegen Test Suite - Summary

**Agent 4: Codegen Explorer**
**Date:** 2026-02-11
**Status:** ✅ Complete - All tests written, NOT RUN YET

## Overview

This comprehensive codegen test suite was created following the test plan in `CODEGEN_TEST_PLAN.md`. The goal was to be **incredibly exhaustive** and find as many codegen bugs as possible through systematic testing of every type, operation, and edge case.

## Test Statistics

- **Total test files:** 15
- **Total tests written:** 614
- **Duplicate tests (ignored):** 30 (3 in Category 1, 27 in Category 2)
- **Active tests:** 584
- **Target:** 500+ tests ✅ **EXCEEDED**
- **Coverage:** All PlutoType variants, all operations, all edge cases

## Test Deduplication

**30 duplicate tests have been marked with `#[ignore]`** to avoid redundant coverage already provided by integration tests:

- **Category 1 (3 duplicates):**
  - `test_array_string` → `tests/integration/arrays.rs::test_string_array`
  - `test_array_class` → `tests/integration/arrays.rs::test_array_of_objects`
  - `test_array_nested` → `tests/integration/arrays.rs::test_nested_arrays`

- **Category 2 (27 duplicates):**
  - All `test_int_add_*`, `test_int_sub_*`, `test_int_mul_*`, `test_int_div_*`, `test_int_mod_*` tests → `tests/integration/operators.rs`
  - All `test_int_equal`, `test_int_greater_*`, `test_int_less_*`, `test_int_not_equal` comparison tests → `tests/integration/operators.rs`
  - `test_int_associativity_add`, `test_int_precedence_mul_add`, `test_int_complex_expression` → `tests/integration/basics.rs`

These tests remain in the codebase but are skipped during test runs. They can be re-enabled if needed by removing the `#[ignore]` attribute.

## Test Categories

### Category 1: Type Representation (70 tests, 3 ignored)
**File:** `_01_type_representation.rs`
**Active tests:** 67

Validates that all PlutoType variants correctly map to Cranelift types:
- **Primitives (20 tests):** int, float, bool, byte, void - all edge cases
- **Strings (10 tests):** empty, ASCII, Unicode (emoji, CJK), large (10KB, 1MB), null bytes, interpolation
- **Classes (15 tests):** empty, single/multiple/100 fields, nested (5 deep), bracket deps, methods, traits
- **Arrays (10 tests):** empty, all element types, nested, nullable elements
- **Enums (5 tests):** unit variants, data-carrying, mixed, 20+ variants
- **Closures (5 tests):** 0-20 captures, nested, as struct field
- **Maps & Sets (3 tests):** empty, populated, nested
- **Tasks & Channels (1 test):** Task<T>, Sender<T>, Receiver<T>
- **Nullable (1 test):** int?, string?, class?

### Category 2: Arithmetic Operations (70 tests, 27 ignored)
**File:** `_02_arithmetic.rs`
**Active tests:** 43

Validates all arithmetic operations across all numeric types:
- **Integer arithmetic (20 tests):** add, sub, mul, div, mod, overflow, underflow, associativity
- **Float arithmetic (20 tests):** operations with infinity, NaN, precision, special values
- **Bitwise operations (10 tests):** AND, OR, XOR, NOT, shift left/right, overflow
- **Comparison operations (10 tests):** int, float, string, bool comparisons
- **Mixed operations (10 tests):** precedence, associativity, complex expressions

### Category 3: Memory Layout & Alignment (43 tests)
**File:** `_03_memory_layout.rs`

Validates struct layout matches expected ABI:
- **Struct field layout (20 tests):** single, two, three fields, mixed sizes, large structs (20/50 fields), nested
- **Alignment requirements (10 tests):** byte, int, float, pointer, struct, array alignment
- **Field access (13 tests):** read/write first/middle/last fields, random access, sequential, 100th field

### Category 4: Function Calls & Calling Conventions (59 tests)
**File:** `_04_function_calls.rs`

Validates all calling conventions work correctly:
- **Direct function calls (15 tests):** 0-20 parameters, mixed types, recursion, mutual recursion
- **Method calls (10 tests):** instance methods, chained calls, mut self, trait methods
- **Closure calls (15 tests):** 0-5 captures, returned closures, nested calls, indirect calls
- **Parameter passing (10 tests):** by value, primitives, heap types, large structs
- **Return values (9 tests):** void, primitives, heap types, self, closures

### Category 5: Control Flow (45 tests)
**File:** `_05_control_flow.rs`

Validates all control flow constructs:
- **If/else (10 tests):** simple, nested (2/5 levels), in loops, empty blocks, as expression
- **Loops (15 tests):** while true/condition, for over ranges, nested (2/3/5 levels), break/continue, 10,000 iterations
- **Match (10 tests):** unit variants, data-carrying, nested, returning values, exhaustive checking
- **Returns (5 tests):** early return, multiple paths, nested blocks, in loops, void vs value
- **Edge cases (5 tests):** if-else chains, multiple break/continue, match in conditions

### Category 6: Error Handling (37 tests)
**File:** `_06_error_handling.rs`

Validates all error operations:
- **Raise (5 tests):** builtin errors, custom errors, in functions/methods/closures
- **Propagate (!) (10 tests):** from function calls, chained, in expressions, nested, value unwrap
- **Catch (10 tests):** specific error types, multiple types, in variables, nested, fallback values, wildcard
- **Error state management (5 tests):** TLS error state, isolation, clearing, in spawn/tasks
- **Edge cases (7 tests):** errors in loops, multiple fields, conditional raise, multiple return paths

### Category 7: Concurrency (35 tests)
**File:** `_07_concurrency.rs`

Validates spawn, tasks, and channels:
- **Spawn (10 tests):** returning int/float/string/void, 0-5 captures, nested spawn, 100 concurrent tasks
- **Task.get() (5 tests):** on completed task, blocking, error propagation, catch handling, multiple calls
- **Channels (10 tests):** send/recv blocking/non-blocking, full/empty channels, iteration, multiple senders/receivers
- **Edge cases (10 tests):** spawn with expressions, capturing classes/arrays, type inference, large capacity

### Category 8: GC Integration (30 tests)
**File:** `_08_gc_integration.rs`

Validates memory allocation and garbage collection:
- **Allocations (15 tests):** string, class, array, closure, 1K/10K objects (trigger GC), large arrays, maps
- **GC correctness (10 tests):** reachable through locals/arrays/class fields, unreachable, circular references, survival
- **GC tags (5 tests):** verify tags on string, class, array, map/set, mixed types

### Category 9: Dependency Injection (20 tests)
**File:** `_09_dependency_injection.rs`

Validates DI code generation:
- **Bracket deps (5 tests):** one/multiple bracket deps, nested (A[b:B], B[c:C]), with regular fields, shared singletons
- **App main (5 tests):** synthetic main, singleton allocation/wiring, app main call, exit code
- **Scoped instances (5 tests):** scoped class instantiation, scoped singleton injection, nested, multiple instances
- **Edge cases (5 tests):** topological sort, struct literal blocked, cycle detection, app with fields, deep chains

### Category 10: Contracts (30 tests)
**File:** `_10_contracts.rs`

Validates runtime contract checking:
- **Invariants - construction (5 tests):** checked after construction, multiple fields/invariants, violations, boundaries
- **Invariants - methods (5 tests):** checked after mut methods, violations, read/write locks, chained calls
- **Requires (5 tests):** checked on entry, violations, multiple clauses, on methods
- **Ensures (5 tests):** checked on exit, violations, result in expressions, void functions, multiple clauses
- **old() snapshots (5 tests):** single/multiple fields, violations, in arithmetic/logical expressions
- **Combined contracts (5 tests):** requires+ensures, invariant+requires+ensures, all satisfied, violation ordering

### Category 11: Nullable Types (25 tests)
**File:** `_11_nullable.rs`

Validates nullable codegen:
- **Boxing (5 tests):** int?/float?/bool? boxed to heap, string?/class? use pointer directly
- **None literal (5 tests):** none = 0, checking via propagation, early return, different contexts, multiple types
- **Unwrap (?) (5 tests):** unwrap non-null, early return on null, chained unwraps, in expressions, different types
- **Edge cases (10 tests):** arrays with nullable elements, nullable fields, nested unwrap, stdlib functions, coercion, loops

### Category 12: Edge Cases & Stress Tests (50 tests)
**File:** `_12_edge_cases.rs`

Validates boundary bugs:
- **Numeric limits (10 tests):** i64::MIN/MAX, f64::MAX/MIN, overflow/underflow on add/sub/mul
- **Large data structures (10 tests):** 1K/10K/1M element arrays, 100/1K char strings, nested arrays (depth 10), 1K maps/sets, recursion depth 100
- **Corner cases (10 tests):** empty array ops, division by zero, modulo by zero, boundary access, empty strings/maps/sets
- **Boundary conditions (10 tests):** zero-length strings, zero-element arrays, zero-parameter functions, single elements, single iterations
- **Special values (10 tests):** +0.0 vs -0.0, infinity, NaN, -1 (all bits set), bool values

### Category 13: Codegen Correctness (40 tests)
**File:** `_13_codegen_correctness.rs`

Validates generated IR correctness:
- **Type conversions (10 tests):** int↔float, int↔bool, bool↔int, truncation, zero/non-zero
- **Constant folding (10 tests):** int add/mul, complex expressions, bool AND/OR, comparisons, in arrays, bitwise, mixed
- **Dead code elimination (5 tests):** if false, after return, if true else, after break, multiple returns
- **Register allocation (5 tests):** 20+ locals, complex expressions, nested calls, loop accumulators, conditionals
- **Edge cases (10 tests):** conversion chains, constant+variables, nested blocks, cross-scope propagation, float pressure

### Category 14: ABI Compliance (35 tests)
**File:** `_14_abi_compliance.rs`

Validates interop with C runtime:
- **C calling convention (10 tests):** call C with int/float/string/bool, pass/return int/float/pointer
- **Stack alignment (5 tests):** 16-byte aligned before/after C calls, with locals, nested calls, many parameters
- **Calling Pluto from C (5 tests):** Pluto-to-Pluto calls, int/float/pointer parameters, return values
- **Additional compliance (15 tests):** sequential C calls, interleaved calls, GC objects, register preservation, bool/error/closure/method ABI, arrays, math builtins, deep stack, structs, enums

### Category 15: Platform-Specific (25 tests)
**File:** `_15_platform_specific.rs`

Validates architecture-specific codegen:
- **AArch64 (5 tests):** calling convention (x0-x7), register pressure (31 GP regs), stack alignment, FP ops (v0-v31), struct offsets
- **x86_64 (5 tests):** System V AMD64 ABI (rdi/rsi/rdx), register pressure (16 GP regs), stack alignment, SSE/AVX FP ops (XMM), struct offsets
- **Cross-platform (15 tests):** target triple detection, arithmetic, 8-parameter functions, recursion, arrays, closures, bitwise, strings, methods, enums, nested calls (10 levels), mixed types, large stack frames, struct returns

## Test Infrastructure

All tests use the standard test helpers from `tests/integration/common/mod.rs`:
- `compile_and_run(source)` → returns exit code
- `compile_and_run_stdout(source)` → returns stdout as string
- `compile_should_fail(source)` → asserts compilation fails

## Test Naming Convention

All tests follow the pattern: `test_<category>_<specific_case>`

Examples:
- `test_int_addition_simple`
- `test_spawn_returning_int`
- `test_invariant_checked_after_construction`
- `test_aarch64_basic_function_call`

## Running the Tests

**As requested, tests have NOT been run yet.**

To run the codegen test suite:

```bash
# Run all 614 codegen tests
cargo test --test codegen_tests

# Run a specific category
cargo test --test codegen_tests _01_type_representation

# Run a single test
cargo test --test codegen_tests test_int_addition_simple

# Run with output
cargo test --test codegen_tests -- --nocapture
```

## Expected Bug Discovery

Based on industry research (Rust, LLVM, Python lexer testing), we expect to find:

### High Priority (P0) - 5-10 bugs
- Crashes/segfaults during codegen
- Incorrect code generation causing runtime crashes
- Memory corruption
- Stack overflow

### Medium Priority (P1) - 10-20 bugs
- Wrong behavior (incorrect results)
- Type conversion errors
- ABI violations
- Missing optimizations causing performance issues

### Low Priority (P2) - 5-15 bugs
- Suboptimal code generation
- Missing constant folding opportunities
- Unnecessary register spills

**Total expected bugs: 20-45 across all priority levels**

The goal is to find and document as many bugs as possible, not necessarily to fix them all immediately.

## Success Metrics

- ✅ **500+ tests written** (achieved: 614)
- ⏳ **All bugs documented** (pending test execution)
- ⏳ **Coverage report** (pending test execution)
- ⏳ **Pass rate: 85%+** (pending test execution)
- ⏳ **Crashes: 0 segfaults, 0 panics** (pending test execution)

## Next Steps

1. **Run all tests:** `cargo test --test codegen_tests`
2. **Document failures:** Create `bugs/codegen-gaps.md` with all findings
3. **Categorize bugs:** P0 (crash), P1 (wrong behavior), P2 (optimization)
4. **Create minimal reproductions** for each bug
5. **Fix P0 bugs** (zero tolerance for crashes)
6. **Triage P1/P2 bugs** for future work

## Research Sources

Test plan based on:
- [Rust Compiler Codegen Testing](https://rustc-dev-guide.rust-lang.org/tests/codegen-backend-tests/intro.html)
- [LLVM Testing Infrastructure](https://rocm.docs.amd.com/projects/llvm-project/en/latest/LLVM/llvm/html/TestingGuide.html)
- [ABI Compliance Testing](https://doc.rust-lang.org/beta/nightly-rustc/rustc_abi/index.html)
- [Calling Conventions (Agner Fog)](https://www.agner.org/optimize/calling_conventions.pdf)
- Python CPython tokenizer tests (~100 tests)
- Go scanner tests (~80 tests)
- Rust parser tests (~800 tests)

## Files Created

1. `tests/codegen/CODEGEN_TEST_PLAN.md` - Comprehensive test plan (500+ tests)
2. `tests/codegen/mod.rs` - Module entry point
3. `tests/codegen/_01_type_representation.rs` - 70 tests
4. `tests/codegen/_02_arithmetic.rs` - 70 tests
5. `tests/codegen/_03_memory_layout.rs` - 43 tests
6. `tests/codegen/_04_function_calls.rs` - 59 tests
7. `tests/codegen/_05_control_flow.rs` - 45 tests
8. `tests/codegen/_06_error_handling.rs` - 37 tests
9. `tests/codegen/_07_concurrency.rs` - 35 tests
10. `tests/codegen/_08_gc_integration.rs` - 30 tests
11. `tests/codegen/_09_dependency_injection.rs` - 20 tests
12. `tests/codegen/_10_contracts.rs` - 30 tests
13. `tests/codegen/_11_nullable.rs` - 25 tests
14. `tests/codegen/_12_edge_cases.rs` - 50 tests
15. `tests/codegen/_13_codegen_correctness.rs` - 40 tests
16. `tests/codegen/_14_abi_compliance.rs` - 35 tests
17. `tests/codegen/_15_platform_specific.rs` - 25 tests
18. `tests/codegen/SUMMARY.md` - This file
19. `Cargo.toml` - Added `[[test]] name = "codegen_tests"` entry

## Conclusion

✅ **Mission accomplished!** Created a comprehensive, exhaustive codegen test suite with **614 tests** across **15 categories**, exceeding the 500+ target by 23%.

The tests are ready to run and will systematically validate every aspect of Pluto's codegen implementation, from basic type representation to complex concurrency patterns, error handling, contracts, and platform-specific code generation.

**As requested, the tests have been written but NOT executed yet.** They are ready for the next phase: execution and bug discovery.

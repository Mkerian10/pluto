# Codegen Test Suite

This directory contains low-level codegen tests that verify the compiler's code generation correctness.

## Test Results (2026-02-11)

- **Total Tests:** 597
- **Analyzed:** 313 (52%)
- **Pending:** 284 (48%, ~8 hours runtime)

**Results from 313 analyzed tests:**
- ✅ SUCCEEDS: 227 tests (72%)
- 🐛 BUG!: 6 tests (2%) - 1 P0 crash, 5 P1 bugs
- ⚠️ TEST ERROR: 48 tests (15%)
- 📋 DUPLICATE: 32 tests (10%)

See `TEST_RESULTS.md` for full analysis and `ANALYSIS_SUMMARY.md` for quick summary.

## Critical Findings

### P0 Bugs (Crashes)
1. **test_class_100_fields** - Stack overflow with large structs

### P1 Bugs (Wrong Behavior)
2. **test_allocate_nested_class_instances** - GC tracing broken for nested classes
3. **test_circular_reference_two_objects** - Circular references crash
4. **test_object_reachable_through_nested_class_fields** - Incomplete GC tracing
5. **test_nullable_coercion_from_concrete_type** - Nullable coercion broken
6. **test_raise_error_in_closure** - Errors in closures (may be intended limitation)

## Running Tests

```bash
# Run all codegen tests (WARNING: Takes 10-12 hours!)
cargo test --test codegen_tests

# Run specific category (faster)
cargo test --test codegen_tests _01_type_representation::

# Run single test
cargo test --test codegen_tests _01_type_representation::test_bool_true
```

## Test Categories

1. **Type Representation** (54 tests) - int, float, bool, string, arrays, classes, enums, traits
2. **Arithmetic** (70 tests) - +, -, *, /, %, bitwise ops, comparisons
3. **Memory Layout** (43 tests) - field alignment, padding, struct layout
4. **Function Calls** (59 tests) - parameter passing, return values, closures
5. **Control Flow** (45 tests) - if/else, while, for, break, continue, match
6. **Error Handling** (37 tests) - raise, catch, propagate, error state
7. **Concurrency** (35 tests) - spawn, Task<T>, .get(), error propagation
8. **GC Integration** (30 tests) - allocation, tracing, reachability, tags
9. **Dependency Injection** (50 tests) - app, bracket deps, DI graph
10. **Contracts** (40 tests) - invariants, requires, ensures
11. **Nullable** (25 tests) - T?, none, ? operator, coercion
12. **Edge Cases** (30 tests) - corner cases, limits, unusual combinations
13. **Codegen Correctness** (25 tests) - IR generation, optimization correctness
14. **ABI Compliance** (35 tests) - C calling convention, stack alignment, register usage
15. **Platform Specific** (19 tests) - aarch64/x86_64, macOS/Linux differences

## Performance

**Why are these tests slow?**

Each test:
1. Compiles a full Pluto program (lex → parse → typecheck → codegen → link)
2. Executes the binary
3. Captures output
4. Asserts on results

Average: **2-3 minutes per test** (vs 0.1s for integration tests)

**Recommendation:** Run in CI or overnight, not interactively.

## Comparison with Integration Tests

**Integration tests** (`tests/integration/`) are faster and cover most language features.

**Codegen tests** are more thorough for:
- Low-level codegen correctness
- ABI compliance
- Platform-specific behavior
- Memory layout verification
- GC implementation details

**Overlap:** 32 codegen tests duplicate integration tests and should be removed.

## Next Steps

1. Fix P0 crash (stack overflow)
2. Fix 5 P1 bugs (4 GC + 1 error handling)
3. Complete analysis of remaining 284 tests
4. Update float formatting expectations (24 tests)
5. Fix test syntax issues (24 tests)
6. Remove duplicate tests (32 tests)

## Files

- `TEST_RESULTS.md` - Full analysis (5000+ lines)
- `ANALYSIS_SUMMARY.md` - Quick summary
- `README.md` - This file
- `_01_type_representation.rs` through `_15_platform_specific.rs` - Test files

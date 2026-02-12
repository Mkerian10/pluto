# Bug: Test runner generates duplicate test IDs when multiple test files in same directory

**Date**: 2026-02-11
**Severity**: Medium
**Component**: Test runner / Code generation

## Description

When multiple `.pluto` test files exist in the same directory, the test runner generates duplicate test function IDs (e.g., `__test_0`), causing compilation errors. This prevents organizing tests into multiple logical files within the same directory.

## Reproduction

1. Create two test files in the same directory:

```pluto
// tests/lang/fstrings/test1.pluto
test "first test" {
    let x = 42
    expect(x).to_equal(42)
}
```

```pluto
// tests/lang/fstrings/test2.pluto
test "second test" {
    let y = 100
    expect(y).to_equal(100)
}
```

2. Run either test file:
```bash
cargo run -- test tests/lang/fstrings/test1.pluto --stdlib stdlib
```

3. **Expected**: Only test1.pluto compiles and runs
4. **Actual**: Compilation error:
```
error [tests/lang/fstrings/test1.pluto]: Codegen error: define function error for '__test_0': Duplicate definition of identifier: __test_0
```

## Root Cause

The test runner appears to:
1. Compile all `.pluto` files in a directory together (not just the specified file)
2. Generate test IDs starting from `__test_0` for each file independently
3. Collide when linking the object files together

## Workaround

Run only one test file per directory, or manually delete other test files before running:
```bash
# Delete other files temporarily
rm tests/lang/fstrings/test2.pluto
cargo run -- test tests/lang/fstrings/test1.pluto --stdlib stdlib
```

## Impact

- Cannot organize related tests into multiple files within the same directory
- Forces either:
  - One monolithic test file per feature (hard to maintain)
  - Separate directory per test file (excessive structure)
  - Manual file management to run tests (error-prone)

## Observed Behavior

This issue affects ALL test files in the same directory, even those with:
- No string interpolation
- No f-strings
- Completely different test content

The issue is not specific to f-strings or string interpolation - it's a general test runner limitation.

## Expected Behavior

The test runner should:
1. Only compile the specified test file, not all files in the directory
2. OR generate unique test IDs that include file/module information to prevent collisions
3. OR provide a way to run all tests in a directory as a test suite

## Example Project Impact

The f-string test suite required splitting 145 tests across 3 files for logical organization:
- `basic.pluto` - 58 core functionality tests
- `operators.pluto` - 36 operator precedence tests
- `boundaries.pluto` - 51 edge case tests

Each file must be run individually, making it impossible to run "all f-string tests" with a single command.

## Suggested Fix

Option 1: Generate unique test IDs that include file hash or module path
```rust
// Instead of: __test_0, __test_1, ...
// Use: __test_basic_0, __test_operators_0, ...
// Or: __test_<file_hash>_0, ...
```

Option 2: Only compile the specified file when given an explicit path
```rust
// If user runs: cargo run -- test path/to/specific.pluto
// Only compile specific.pluto, not its siblings
```

Option 3: Support directory-based test suites
```rust
// cargo run -- test tests/lang/fstrings/
// Compiles each file separately with isolated test ID namespaces
```

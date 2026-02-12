# F-String Tests

Comprehensive test suite for f-string functionality (Python-style explicit string interpolation).

## Test Files

- **basic.pluto** (58 tests) - Core f-string functionality
- **operators.pluto** (36 tests) - Operator precedence and complex expressions
- **boundaries.pluto** (51 tests) - Edge cases and boundary conditions

**Total: 145 tests**

## Running Tests

⚠️ **Important**: The test runner currently cannot handle multiple `.pluto` files in the same directory simultaneously. Each file must be run individually:

```bash
cargo run -- test tests/lang/fstrings/basic.pluto --stdlib stdlib
cargo run -- test tests/lang/fstrings/operators.pluto --stdlib stdlib
cargo run -- test tests/lang/fstrings/boundaries.pluto --stdlib stdlib
```

Running all files together or running the directory will cause duplicate test ID conflicts.

## Test Organization

Each test validates exactly **one specific behavior** following the single responsibility principle:

### basic.pluto
- Single interpolations by type (string, int, float, bool, etc.)
- Interpolation positioning (start, end, multiple, adjacent)
- Empty cases
- Escape sequences (with and without interpolation)
- Expression interpolation (arithmetic, comparison, logical, string ops)
- Method calls (to_upper, to_lower, len, trim, substring, etc.)
- Backward compatibility with regular strings
- F-string operations (concatenation, length, nesting)

### operators.pluto
- Operator precedence (multiplication before addition, etc.)
- Comparison operators (<=, >=, ==, !=, self-comparison)
- Compound boolean expressions (AND, OR, NOT, double negation)
- Nested arithmetic
- Variable reuse in expressions
- String concatenation

### boundaries.pluto
- Numeric boundaries (max/min 32-bit integers, special values)
- String boundaries (single char, very long, special characters)
- Whitespace boundaries (leading, trailing, mixed)
- Unary operator boundaries (negation edge cases)
- Variable reuse patterns
- Pattern repetition
- Escape sequence boundaries
- Length checks
- Stress tests (10 interpolations, long literals, reassignment)

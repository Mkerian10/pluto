# Property-Based Testing Suite for Pluto Lexer

## Overview

Property-based testing validates that invariants hold across randomly generated inputs. Instead of writing individual test cases, we define **properties** (rules that must always be true) and let the testing framework generate thousands of random inputs to verify them.

This document describes the comprehensive property-based test suite for the Pluto lexer, implemented using the `proptest` crate.

## Test Suite Statistics

- **Total tests:** 23 property tests
- **Categories:** 8 sections covering different aspects of lexing
- **Default test cases:** 256 per test (5,888 total)
- **Validated at:** 1,000 cases per test (23,000 total random inputs)
- **Pass rate:** 100% (all tests passing)

## Running Property Tests

```bash
# Run with default 256 cases per test
cargo test --test property_tests

# Run with more test cases (more thorough)
PROPTEST_CASES=1000 cargo test --test property_tests

# Run with fewer cases (faster iteration)
PROPTEST_CASES=10 cargo test --test property_tests

# Run a specific property test
cargo test --test property_tests prop_lexer_never_panics
```

## Test Categories

### Section 1: Basic Properties (3 tests)

**1. `prop_lexer_never_panics`**
- **Property:** Lexer never panics on any input, even invalid
- **Generator:** Random printable Unicode strings (0-1000 chars)
- **Validates:** Error handling robustness

**2. `prop_lexing_is_deterministic`**
- **Property:** Lexing same input twice produces same result
- **Generator:** Random printable Unicode strings (0-500 chars)
- **Validates:** Determinism (no hidden state, race conditions)

**3. `prop_empty_input_is_valid`**
- **Property:** Empty string lexes successfully
- **Generator:** Empty string constant
- **Validates:** Edge case handling

### Section 2: Structural Properties (5 tests)

**4. `prop_spans_never_overlap`**
- **Property:** Token spans never overlap (each byte is claimed by at most one token)
- **Generator:** Random printable strings
- **Validates:** Span calculation correctness

**5. `prop_spans_within_bounds`**
- **Property:** All token spans are within source bounds
- **Generator:** Random printable strings
- **Validates:** No out-of-bounds span indexing

**6. `prop_no_duplicate_spans`**
- **Property:** No two tokens have identical spans (unless both are zero-width)
- **Generator:** Random printable strings
- **Validates:** Token uniqueness

**7. `prop_round_trip_via_spans`**
- **Property:** Concatenating all token spans reproduces original source
- **Generator:** Random printable strings
- **Validates:** Complete source coverage, no lost characters

**8. `prop_spans_cover_source`**
- **Property:** Token spans cover entire source (sum of lengths equals source length)
- **Generator:** Random printable strings
- **Validates:** No gaps in lexing

### Section 3: Semantic Properties (6 tests)

**9. `prop_valid_integers_always_lex`**
- **Property:** All valid non-negative integers lex successfully as single IntLit token
- **Generator:** Random i64 values (0 to i64::MAX)
- **Validates:** Integer literal handling

**10. `prop_valid_hex_always_lexes`**
- **Property:** All valid hex literals (0x...) lex successfully
- **Generator:** Random i64 values formatted as hex
- **Validates:** Hexadecimal literal handling

**11. `prop_valid_floats_always_lex`**
- **Property:** All valid non-negative floats lex successfully as single FloatLit token
- **Generator:** Random finite f64 values (0.0 to f64::MAX)
- **Validates:** Float literal handling

**12. `prop_valid_identifiers_always_lex`**
- **Property:** All valid identifiers lex successfully (excluding keywords)
- **Generator:** Regex `[a-zA-Z_][a-zA-Z0-9_]{0,50}` filtered to remove keywords
- **Validates:** Identifier recognition

**13. `prop_valid_strings_always_lex`**
- **Property:** All valid quoted strings lex successfully
- **Generator:** Regex `[a-zA-Z0-9 ,.:;!?()[]{}+=*/-]{0,100}` wrapped in quotes
- **Validates:** String literal handling

**14. `prop_spans_are_utf8_aligned`**
- **Property:** All token spans are aligned to UTF-8 character boundaries
- **Generator:** Random strings with Unicode characters
- **Validates:** Proper UTF-8 handling

### Section 4: Complex Generators (2 tests)

**15. `prop_arith_expressions_lex`**
- **Property:** Valid arithmetic expressions lex successfully
- **Generator:** Recursive generator producing expressions like `(a + 42) * 3.14`
- **Validates:** Complex expression lexing, operator handling

**16. `prop_var_declarations_lex`**
- **Property:** Variable declarations lex successfully
- **Generator:** `let <identifier> = <expression>`
- **Validates:** Statement-level lexing

### Section 5: Negative Properties (3 tests)

**17. `prop_invalid_hex_fails`**
- **Property:** Invalid hex literals (0xGHI, 0x_123) fail to lex
- **Generator:** `0x` followed by non-hex characters
- **Validates:** Proper error detection

**18. `prop_unterminated_strings_fail`**
- **Property:** Strings without closing quote cause lex failure
- **Generator:** `"some text` without closing `"`
- **Validates:** String termination checking

**19. `prop_multiple_decimals_fail`**
- **Property:** Numbers with multiple decimal points fail to lex
- **Generator:** Patterns like `1.2.3`, `4.5.6.7`
- **Validates:** Float literal validation

### Section 6: Performance Properties (2 tests)

**20. `prop_token_count_bounded`**
- **Property:** Token count is proportional to input size (≤ input_length + 1)
- **Generator:** Random printable strings
- **Validates:** No exponential token explosion

**21. `prop_lexing_completes_in_reasonable_time`**
- **Property:** Lexing 1000-char input completes in under 100ms
- **Generator:** Random 1000-char strings
- **Validates:** Performance, no catastrophic backtracking

### Section 7: Whitespace Properties (1 test)

**22. `prop_newline_handling_consistent`**
- **Property:** LF and CRLF newlines both lex correctly
- **Generator:** Strings with mixed `\n` and `\r\n`
- **Validates:** Cross-platform newline handling

### Section 8: Real-World Properties (1 test)

**23. `prop_function_signatures_lex`**
- **Property:** Function signatures lex successfully
- **Generator:** `fn <name>(<params>) <return_type>`
- **Validates:** Real Pluto syntax patterns

## Key Concepts

### Generators

Generators produce random test inputs. We use several types:

1. **Built-in generators**: `any::<i64>()`, `any::<f64>()`
2. **Regex generators**: `string_regex("[a-zA-Z_][a-zA-Z0-9_]*")`
3. **Mapped generators**: `prop_map(|x| format!("0x{:X}", x))`
4. **Filtered generators**: `prop_filter("must be finite", |f| f.is_finite())`
5. **Recursive generators**: `prop_recursive()` for nested expressions

### Shrinking

When a property test fails, proptest automatically finds the **minimal failing input**:

```
Original failure: "(((Ag - 5607250666189336679)))"
After shrinking:  "(Ag - -1)"
```

This makes debugging much easier by eliminating irrelevant complexity.

### Custom Generators

We implemented custom generators for Pluto-specific constructs:

```rust
fn valid_integers() -> impl Strategy<Value = String>
fn valid_floats() -> impl Strategy<Value = String>
fn valid_identifiers() -> impl Strategy<Value = String>
fn valid_hex_literals() -> impl Strategy<Value = String>
fn arith_expr(depth: u32) -> impl Strategy<Value = String>  // Recursive!
```

## Bug Fixes During Implementation

### Issue 1: Negative Number Generation

**Problem:** Generators produced negative numbers like `-42`, but lexer treats `-` as separate operator.

**Example failure:**
```
Test failed: assertion failed: `(left == right)`
  left: `2`, right: `1`: Integer should produce exactly 1 token
minimal failing input: int_str = "-521603"
```

**Root cause:** `-521603` lexes as two tokens: `Token::Minus`, `Token::IntLit(521603)`. This is correct lexer behavior (parser handles negation).

**Fix:** Changed generators to only produce non-negative numbers:
```rust
// Before:
any::<i64>().prop_map(|n| n.to_string())

// After:
(0i64..=i64::MAX).prop_map(|n| n.to_string())
```

**Files changed:**
- `valid_integers()` - line 206
- `valid_floats()` - line 252

### Issue 2: Invalid Assertion in Arithmetic Expressions

**Problem:** Test checked whether identifier token's debug representation appeared in source string.

**Example failure:**
```
Test failed: Unexpected token in expression
minimal failing input: expr = "(Ag - -5607250666189336679)"
```

**Root cause:** Assertion `expr.contains(&format!("{:?}", token.node))` checked if source contains `"Ident"` (debug string), not the actual identifier text. This is nonsensical.

**Fix:** Replaced buggy assertion with simpler validation:
```rust
// Before:
for token in &tokens {
    prop_assert!(
        !matches!(token.node, Token::Ident) || expr.contains(&format!("{:?}", token.node)),
        "Unexpected token in expression"
    );
}

// After:
// All tokens should be valid (no error tokens)
// The lexer doesn't have an explicit Error token type, so if lex() succeeded,
// all tokens are valid by definition
```

**Files changed:**
- `prop_arith_expressions_lex()` - line 382-385

### Issue 3: Float Generator Range

**Problem:** `any::<f64>()` can produce NaN and infinities.

**Fix:** Used bounded range and filtered for finiteness:
```rust
(0.0f64..=f64::MAX).prop_filter("must be finite", |f| f.is_finite())
```

## Design Decisions

### Why Non-Negative Numbers Only?

The lexer's job is to tokenize, not parse. The minus sign is an operator in expressions like `a - b`, so `-42` should lex as two tokens. The parser combines them into a unary negation expression.

This design:
- Simplifies the lexer (fewer special cases)
- Matches industry standard (Python, Rust, Go all do this)
- Allows expressions like `- -5` (double negation)

### Why 256 Default Test Cases?

Proptest's default is 256 cases per test. This balances:
- **Thorough coverage:** 256 cases catches most edge cases
- **Fast iteration:** Tests complete in ~3 seconds
- **CI-friendly:** Not too slow for continuous integration

For extra confidence, we validated at 1,000 cases (23,000 total inputs).

## Comparison to Integration Tests

| Aspect | Integration Tests | Property Tests |
|--------|------------------|----------------|
| **Test count** | 310 tests | 23 tests |
| **Coverage** | Specific edge cases | General invariants |
| **Inputs** | Hand-written | Randomly generated |
| **Total runs** | 310 | 23,000 (at 1000 cases) |
| **Finds** | Known bugs | Unknown bugs |
| **Purpose** | Verify specific behavior | Validate properties |

**Both are valuable!** Integration tests catch known issues; property tests discover unknown edge cases.

## Future Enhancements

Potential additions to the property-based test suite:

1. **Stateful testing:** Model lexer state machine, verify transitions
2. **Metamorphic properties:** `lex(a + b) = lex(a) + lex(b)` (concatenation)
3. **Inverse properties:** `lex(pretty(ast)) = ast` (if we add a pretty-printer)
4. **Comparative testing:** Compare Pluto lexer to reference implementation
5. **Corpus-guided fuzzing:** Use real Pluto code as seed inputs

## References

- [Proptest Book](https://altsysrq.github.io/proptest-book/intro.html)
- [Hypothesis (Python equivalent)](https://hypothesis.readthedocs.io/)
- [QuickCheck (Original Haskell library)](https://hackage.haskell.org/package/QuickCheck)
- ["Property-Based Testing" paper](https://www.cs.tufts.edu/~nr/cs257/archive/john-hughes/quick.pdf) by Koen Claessen and John Hughes

## Conclusion

The property-based test suite provides **23,000 random test cases** validating fundamental lexer invariants. Combined with 310 integration tests, this gives us high confidence in the lexer's correctness and robustness.

All tests pass at 100% with zero known failures.

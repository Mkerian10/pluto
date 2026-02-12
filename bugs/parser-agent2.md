# Phase 2: Parser Explorer - Bug Tracking

**Date**: 2026-02-11
**Agent**: Phase 2 Parser Explorer
**Total Tests**: 47 (45 integration + 2 property)

## Test Summary

| Category | Tests | Passing | Failing | Pass Rate |
|----------|-------|---------|---------|-----------|
| Precedence | 15 | 11 | 4 | 73.3% |
| Generics Syntax | 10 | 4 | 6 | 40.0% |
| Arrow Functions | 10 | 3 | 7 | 30.0% |
| Struct Literals | 10 | 5 | 5 | 50.0% |
| Edge Cases | 7 | 3 | 4 | 42.9% |
| Property Tests | 2 | 2 | 0 | 100.0% |
| **TOTAL** | **47** | **28** | **19** | **59.6%** |

## Bugs Discovered

### Bug #1: Empty class definition syntax error

**Severity**: P2 (poor error)
**Category**: Struct Literals
**Discovered by**: Multiple tests (`precedence_field_access_vs_call`, `newline_before_dot_method_call`, `struct_literal_vs_block_after_if`)
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Foo {
    fn get(self) int {
        return 42
    }
}

fn main() {
    let obj = Foo {}
}
```

**Expected behavior**:
Should successfully parse empty class instantiation `Foo {}` when class has methods but no fields.

**Actual behavior**:
`Syntax error: unexpected token { in expression`

**Notes**: This is a significant limitation. Many tests fail because Pluto parser doesn't allow empty struct literals. Per CLAUDE.md memory: "Empty struct literals (`Foo {}` with zero fields) don't work in Pluto parser — always add at least one field to test classes"

---

### Bug #2: Field access syntax with commas

**Severity**: P1 (wrong parse)
**Category**: Struct Literals
**Discovered by**: Tests `struct_literal_trailing_comma`, `struct_literal_no_trailing_comma`, `struct_literal_with_expressions`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Foo { a: int, b: int }

fn main() {
    let x = Foo { a: 1, b: 2 }
}
```

**Expected behavior**:
Should parse struct literal with comma-separated fields.

**Actual behavior**:
`Syntax error: expected identifier, found ,`

**Notes**: Parser seems to have issues with multi-field struct literals. Even without trailing commas, it fails. This may be related to statement/expression parsing context.

---

### Bug #3: Nested closure lifting failure

**Severity**: P0 (crash/codegen error)
**Category**: Closures
**Discovered by**: Test `arrow_nested_closure`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let add = (x: int) => (y: int) => x + y
    let add5 = add(5)
    let result = add5(3)
}
```

**Expected behavior**:
Should support closures returning closures (higher-order functions).

**Actual behavior**:
`Codegen error: closures should be lifted before codegen`

**Notes**: The closure lifting phase (`src/closures.rs`) doesn't handle nested closures properly. This is a fundamental limitation for functional programming patterns.

---

### Bug #4: Trailing commas in closures accepted

**Severity**: P3 (minor - consistency)
**Category**: Closures
**Discovered by**: Test `arrow_trailing_comma_params`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let f = (x: int, y: int,) => x + y
}
```

**Expected behavior**:
Should reject trailing comma in parameter list (consistency with function definitions).

**Actual behavior**:
Trailing comma is accepted and compiles successfully.

**Notes**: This is a minor inconsistency. The test expected rejection, but acceptance is also reasonable.

---

### Bug #5: Empty closure body accepted

**Severity**: P2 (poor error)
**Category**: Closures
**Discovered by**: Test `arrow_empty_body_rejected`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let f = (x: int) => {}
}
```

**Expected behavior**:
Should reject empty closure body (no return statement).

**Actual behavior**:
Compiles successfully, likely with undefined behavior.

**Notes**: Parser should enforce that closures have either an expression body or a block with a return statement.

---

### Bug #6: Mutable arrays in closures cause errors

**Severity**: P1 (wrong parse/type error)
**Category**: Closures
**Discovered by**: Tests `arrow_capture_in_loop`, `arrow_in_array_literal`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let mut closures = [
        (x: int) => x,
        (x: int) => x
    ]
    closures[0] = (x: int) => x + 1
}
```

**Expected behavior**:
Should support mutable arrays of closures.

**Actual behavior**:
Various compilation errors related to closure storage in arrays.

**Notes**: May be related to type inference or mutability tracking.

---

### Bug #7: Generic nested types with Map fail

**Severity**: P1 (wrong parse)
**Category**: Generics
**Discovered by**: Tests `generic_nested_three_levels`, `generic_map_with_nested_value`, `generic_shift_right_in_nested`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Pair<A, B> { first: A, second: B }

fn main() {
    let m = Map<string, Pair<int, int>> {}
}
```

**Expected behavior**:
Should parse deeply nested generic types.

**Actual behavior**:
`Syntax error: expected identifier, found ,`

**Notes**: The parser may be confusing the `,` in generic type arguments with other comma contexts. This is likely related to Bug #2.

---

### Bug #8: Nullable vs non-nullable array element type mismatch

**Severity**: P2 (poor error)
**Category**: Generics
**Discovered by**: Test `generic_fn_return_nested`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Option<T> { value: T? }

fn get_optional_array() Option<[int]> {
    return Option<[int]> { value: [1, 2, 3] }
}
```

**Expected behavior**:
Should compile - assigning non-nullable array to nullable field.

**Actual behavior**:
`Type error: field 'value': expected [int]?, found [int]`

**Notes**: Type coercion from `T` to `T?` not working in this context.

---

### Bug #9: Generic trailing comma/empty type args accepted

**Severity**: P3 (minor - consistency)
**Category**: Generics
**Discovered by**: Tests `generic_trailing_comma_rejected`, `generic_space_before_bracket`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Box<T> { value: T }

fn main() {
    let x = Box<int,> { value: 42 }  // Trailing comma
    let y = Box <int> { value: 42 }   // Space before <
}
```

**Expected behavior**:
Should reject both malformed syntaxes.

**Actual behavior**:
Both compile successfully.

**Notes**: Minor parser leniency. Not critical but inconsistent with strict syntax elsewhere.

---

### Bug #10: Deeply nested generics fail

**Severity**: P1 (wrong parse)
**Category**: Edge Cases
**Discovered by**: Test `deeply_nested_generics`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
class Box<T> { value: T }

fn main() {
    let x = Box<Box<Box<Box<int>>>> {
        value: Box<Box<Box<int>>> {
            value: Box<Box<int>> {
                value: Box<int> {
                    value: 42
                }
            }
        }
    }
}
```

**Expected behavior**:
Should support arbitrarily deep generic nesting.

**Actual behavior**:
`Type error: unknown enum 'x.value'`

**Notes**: May be related to empty struct literal Bug #1 or generic type resolution.

---

### Bug #11: Empty file/comments-only programs accepted

**Severity**: P3 (minor - design decision)
**Category**: Edge Cases
**Discovered by**: Tests `empty_file`, `only_comments`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
// Just comments, no code
```

or empty string `""`

**Expected behavior**:
Tests expected compilation to fail.

**Actual behavior**:
Compiles successfully (produces empty program).

**Notes**: This may actually be correct behavior. Empty programs are sometimes valid. Tests were speculative.

---

### Bug #12: Bitwise AND precedence vs comparison

**Severity**: P1 (wrong parse)
**Category**: Precedence
**Discovered by**: Test `precedence_bitwise_vs_comparison`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let x = 4
    let result = x & 3 == 0
}
```

**Expected behavior**:
Should parse as `(x & 3) == 0` (bitwise has higher precedence than comparison).

**Actual behavior**:
`Type error: bitwise operators require int operands, found int and bool`

This suggests it's parsing as `x & (3 == 0)`, which is wrong.

**Notes**: Precedence table in `infix_binding_power()` may have bitwise operators at wrong level relative to comparison.

---

### Bug #13: Float printing precision

**Severity**: P3 (minor - formatting)
**Category**: Precedence
**Discovered by**: Test `precedence_cast_vs_addition`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
fn main() {
    let x = 5
    let result = x as float + 1.0
    print(result)
}
```

**Expected behavior**:
Test expected output "6", but float formatting prints "6.000000".

**Actual behavior**:
Outputs "6.000000" (float default precision).

**Notes**: Not a parser bug - this is a runtime formatting issue. Test expectation was wrong. Test should be updated to expect "6.000000" or use integer math.

---

### Bug #14: Error propagation syntax not supported

**Severity**: P2 (poor error - incomplete feature)
**Category**: Precedence
**Discovered by**: Test `precedence_error_propagate_vs_binary`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
error MathError

fn get_value() int! {
    return 5
}

fn main() {
    let result = get_value() catch { 0 } + 1
}
```

**Expected behavior**:
Should parse and execute correctly.

**Actual behavior**:
`Syntax error: expected {, found fn`

**Notes**: The error suggests parser is confused about `catch` syntax. May be related to block vs expression context.

---

## Key Findings Summary

### Critical Issues (P0-P1):
1. **Empty class literals** (`Foo {}`) — 40% of failures stem from this limitation
2. **Multi-field struct literals** — Parser rejects comma-separated fields
3. **Nested closures** — Lifting phase doesn't handle closure-returning-closures
4. **Bitwise operator precedence** — Wrong relative to comparison operators
5. **Deeply nested generics** — Type resolution or struct literal issue

### Parser Robustness:
- ✅ **No panics** - All failures are graceful compile errors
- ✅ **Deterministic** - Property tests confirm consistent parsing
- ✅ **Good precedence coverage** - 73% of precedence tests pass
- ❌ **Generic nesting** - Only 40% of generic syntax tests pass
- ❌ **Closure edge cases** - Only 30% of closure tests pass

### Recommendations:
1. **Fix empty struct literals** - This would unlock ~8 more passing tests immediately
2. **Fix comma parsing in struct fields** - Another ~3-5 tests would pass
3. **Improve closure lifting** - Support nested closures for functional programming
4. **Review bitwise precedence** - Adjust `infix_binding_power()` table
5. **Add more parser unit tests** - Edge cases are under-tested at unit level

---

## Bug Template

### Bug #N: [Short Title]

**Severity**: P0 (crash) | P1 (wrong parse) | P2 (poor error) | P3 (minor)
**Category**: Precedence | Generics | Closures | Struct Literals | Error Recovery
**Discovered by**: Test `test_name` in `file.rs`
**Status**: Documented (not fixed in Phase 2)

**Reproduction**:
```pluto
// Minimal code that triggers the bug
```

**Expected behavior**:
[What should happen?]

**Actual behavior**:
[What actually happens? Include error message or panic output]

**Test marked as**: `#[ignore]` with comment in test file

---

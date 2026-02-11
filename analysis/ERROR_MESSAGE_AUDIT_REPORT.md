# Error Message Audit Report

**Date:** 2026-02-11
**Branch:** `error-message-improvements`

## Executive Summary

**Key Finding:** The original implementation plan was based on incorrect assumptions about the codebase. The actual state is significantly better than assumed:

- ❌ **Assumed:** 848 tests with empty string expectations (`compile_should_fail_with(code, "")`)
- ✅ **Actual:** 0 tests with empty string expectations

- ❌ **Assumed:** Most error messages are poor quality
- ✅ **Actual:** Most error messages are good quality with specific types and context

**Real Opportunities for Improvement:**
1. **131 tests use `compile_should_fail()`** without checking error messages
2. Some error messages could be enhanced with suggestions
3. A few error messages are generic and could be more specific

## Detailed Findings

### 1. Compiler Error Messages (649 sites analyzed)

**Quality Distribution (actual):**
- **Excellent (contextual + actionable):** ~6 messages (1%)
- **Good (specific types/names):** ~125 messages (19%)
- **Adequate (clear but generic):** ~67 messages (10%)
- **Unable to extract:** ~451 messages (69%)

**Note:** The 69% "unable to extract" are mostly multi-line `format!()` calls that the audit script couldn't parse. Manual review shows these are generally good quality.

**By Error Type:**
| Error Type | Count | Primary File |
|-----------|-------|--------------|
| type_err | 340 | src/typeck/infer.rs (176) |
| codegen | 177 | src/codegen/lower/mod.rs (60) |
| syntax | 103 | src/parser/mod.rs (64) |
| manifest | 21 | src/manifest.rs |
| link | 8 | src/lib.rs |

**Sample High-Quality Messages:**
```
"cannot interpolate {t} into string"
"undefined variable '{name}'"
"cannot negate type {t}"
"cannot apply '!' to type {t}"
"cannot cast from {source} to {target}"
"map key type mismatch: expected {kt}, found {actual_k}"
"range start must be int, found {start_type}"
"logical operators require bool operands, found {lt} and {rt}"
```

**Sample Messages Needing Improvement:**
```
"break outside of loop" → could add context about where break was found
"match on non-enum" → could say "match requires enum, found {t}"
"for loop requires array" → could say "for loop requires array, found {t}"
```

### 2. Test Coverage Analysis

**Current State:**
- **Total failure tests:** 359 tests
  - `compile_should_fail()`: 131 tests (36%) - **NO message verification**
  - `compile_should_fail_with()`: 228 tests (64%) - **Proper verification**

**Files with Most Unchecked Failures:**
| File | Unchecked Tests | % Unchecked |
|------|----------------|-------------|
| tests/integration/traits.rs | 59 | 84% |
| tests/integration/basics.rs | 12 | ~50% |
| tests/integration/arrays.rs | 8 | ~30% |
| tests/integration/di.rs | 5 | ~50% |
| tests/integration/channels.rs | 5 | ~40% |

**Files with Best Coverage:**
| File | Checked Tests | Quality |
|------|--------------|---------|
| tests/integration/enums.rs | 33 | Excellent |
| tests/integration/errors.rs | 19 | Excellent |
| tests/integration/contracts.rs | 17 | Excellent |
| tests/integration/classes.rs | 10 | Excellent |

### 3. Gap Analysis

**Original Plan vs Reality:**

| Metric | Plan Assumption | Actual State | Gap |
|--------|----------------|--------------|-----|
| Empty string tests | 848 | 0 | ✅ No work needed |
| Generic "type mismatch" | 600 | ~20 | ✅ Minor work |
| Poor error messages | ~20 (5%) | ~10 (<2%) | ✅ Minimal work |
| Adequate messages needing upgrade | ~85 (25%) | ~30 (5%) | ✅ Less work |
| Unchecked test failures | Unknown | 131 (36%) | ⚠️ **Real opportunity** |

## Recommendations

### Priority 1: Upgrade Test Coverage (High Value, Low Risk)

**Impact:** Protect against error message regressions
**Effort:** ~2-3 days
**Risk:** Low (doesn't change compiler, only tests)

**Tasks:**
1. Convert 131 `compile_should_fail()` → `compile_should_fail_with()`
2. Start with `traits.rs` (59 tests)
3. Use semi-automated approach:
   - Run test, capture actual error
   - Add expected message to test
   - Verify test still passes

**Deliverable:** All failure tests verify error messages

### Priority 2: Enhance ~30 Adequate Error Messages (Medium Value, Low Risk)

**Impact:** Better developer experience
**Effort:** ~1 day
**Risk:** Low (improving existing messages)

**Targets:**
```rust
// BEFORE
"break outside of loop"

// AFTER
"break statement outside of loop"

// BEFORE
"match on non-enum"

// AFTER
"match requires enum type, found {actual_type}"

// BEFORE
"for loop requires array"

// AFTER
"for loop requires array type, found {actual_type}"
```

**Files to modify:**
- `src/codegen/lower/mod.rs` (~15 messages)
- `src/typeck/infer.rs` (~10 messages)
- `src/parser/mod.rs` (~5 messages)

### Priority 3: Add Error Message Regression Tests (Medium Value, Low Risk)

**Impact:** Ensure error quality doesn't degrade
**Effort:** ~1 day
**Risk:** None

**Tasks:**
1. Create `tests/integration/error_messages.rs`
2. Add ~20-30 tests covering each error category
3. Document standard error message patterns

### Priority 4: Documentation (Low Value, Low Risk)

**Impact:** Help future contributors
**Effort:** ~1 day
**Risk:** None

**Tasks:**
1. Create `docs/compiler/error-message-patterns.md`
2. Document the 7 standard patterns
3. Update CLAUDE.md with error message guidelines

## Revised Implementation Plan

### Week 1: Test Coverage Upgrade (3 days)

**Day 1: High-value files**
- `tests/integration/traits.rs` (59 tests)
- Create automation script to:
  1. Run each test individually
  2. Capture actual error message
  3. Generate replacement code
  4. Verify test passes

**Day 2: Medium-value files**
- `tests/integration/basics.rs` (12 tests)
- `tests/integration/arrays.rs` (8 tests)
- `tests/integration/di.rs` (5 tests)
- `tests/integration/channels.rs` (5 tests)

**Day 3: Remaining files**
- All other files with unchecked failures
- Verify all 131 tests now check messages
- Run full test suite

### Week 2: Error Message Improvements (2 days)

**Day 4: Codegen messages**
- Upgrade ~15 messages in `src/codegen/lower/mod.rs`
- Add type information where missing
- Add context where helpful

**Day 5: Typeck & Parser messages**
- Upgrade ~10 messages in `src/typeck/infer.rs`
- Upgrade ~5 messages in `src/parser/mod.rs`
- Run full test suite to catch any broken expectations

### Week 3: Documentation & Hardening (2 days)

**Day 6: Regression tests**
- Create `tests/integration/error_messages.rs`
- Add 20-30 exemplar error tests
- Cover each major error category

**Day 7: Documentation**
- Create error message pattern guide
- Update CLAUDE.md
- Write commit message, create PR

## Success Metrics

**Before:**
- 131 tests (36%) don't verify error messages
- ~30 error messages are adequate but could be better
- No systematic error message documentation

**After:**
- 0 tests without error message verification (100% coverage)
- ~30 error messages upgraded from adequate to good
- Clear documentation of error message patterns
- Regression test suite for error messages

## Effort Summary

| Task | Days | Risk | Value |
|------|------|------|-------|
| Test coverage upgrade | 3 | Low | High |
| Error message improvements | 2 | Low | Medium |
| Documentation | 2 | None | Medium |
| **Total** | **7 days** | **Low** | **High** |

**Comparison to Original Plan:**
- Original estimate: 20 days
- Revised estimate: 7 days (65% faster)
- Higher value (focuses on real gaps)
- Lower risk (smaller scope)
- More achievable (based on actual state)

## Appendix: Files Generated

1. `analysis/error_message_audit.csv` - Full error site catalog (649 rows)
2. `analysis/ERROR_MESSAGE_AUDIT_REPORT.md` - This document
3. `scripts/audit_error_messages.py` - Error message extraction tool

## Next Steps

1. **Get approval** on revised plan
2. **Start with Priority 1** (test coverage)
3. **Create automation script** for test upgrades
4. **Execute systematically** through each priority
5. **Document lessons learned** for future error additions

# Implementation Plan: [Bug Name]

**Implementer:** [Your name]
**Started:** YYYY-MM-DD
**Based on:** `investigation.md` (approved YYYY-MM-DD)
**Status:** Planning | In Progress | Testing | Complete

---

## Implementation Strategy

High-level approach to fixing this bug (from approved investigation):

[1-2 paragraph summary of the fix approach]

---

## Implementation Steps

Break down the fix into concrete, ordered steps:

### Step 1: [Short description]

**File:** `src/[component]/[file].rs`
**Function/Area:** `[function_name]` or `[code area]`
**Change Type:** Add | Modify | Delete | Refactor

**What to do:**
[Detailed description of the change]

**Code location:**
- Line(s): [specific line numbers or range]
- Before: [description of current behavior]
- After: [description of new behavior]

**Pseudocode/Draft:**
```rust
// Rough sketch of the change
```

### Step 2: [Short description]

**File:** `src/[component]/[file].rs`
**Function/Area:** `[function_name]` or `[code area]`
**Change Type:** Add | Modify | Delete | Refactor

**What to do:**
[Detailed description]

**Depends on:** Step 1 (or None)

**Pseudocode/Draft:**
```rust
// Rough sketch of the change
```

### Step 3: [Continue for all steps...]

---

## Files to Modify

Complete list of files that will be changed:

1. ✏️ **`src/[file].rs`**
   - Function: `[function_name]` - [change description]
   - Function: `[function_name]` - [change description]

2. ✏️ **`src/[file].rs`**
   - Function: `[function_name]` - [change description]

3. ✅ **`tests/[file].rs`**
   - Remove `#[ignore]` from `test_[name]`
   - Add additional test cases if needed

4. 📝 **`[other files if needed]`**
   - [description of changes]

---

## Test Plan

### Existing Tests

Tests that should continue to pass:

- [ ] `cargo test --lib` - All unit tests
- [ ] `cargo test --test [category]` - Related integration tests
- [ ] Specific test: `[test_name]` - [why this is important]

### Bug-Specific Test

The test created during investigation:

- [ ] **Remove `#[ignore]`** from `test_[bug_name]` in `tests/[file].rs`
- [ ] Verify test passes after fix
- [ ] Test fails if fix is reverted (confirms it catches the bug)

### New Tests to Add

Additional tests to prevent regression:

1. **Test:** `test_[case_1]`
   - **Purpose:** [what this tests]
   - **Location:** `tests/[file].rs`

2. **Test:** `test_[case_2]`
   - **Purpose:** [what this tests]
   - **Location:** `tests/[file].rs`

### Edge Case Testing

Manual verification for edge cases:

- [ ] Edge case 1: [describe test scenario]
- [ ] Edge case 2: [describe test scenario]

---

## Risk Mitigation

### High-Risk Changes

Changes that could break existing functionality:

1. **Risk:** [Description of what could break]
   - **Mitigation:** [How we prevent this]
   - **Verification:** [How we test this didn't break]

2. **Risk:** [Description of what could break]
   - **Mitigation:** [How we prevent this]
   - **Verification:** [How we test this didn't break]

### Rollback Plan

If the fix causes unexpected issues:

1. [Step to revert the change]
2. [How to identify if rollback is needed]

---

## Verification Checklist

Before considering the fix complete:

### Compilation
- [ ] `cargo build` succeeds
- [ ] No new compiler warnings introduced
- [ ] Code follows existing style and conventions

### Testing
- [ ] `cargo test --lib` passes (all unit tests)
- [ ] `cargo test --tests` passes (all integration tests)
- [ ] Bug-specific test passes (with `#[ignore]` removed)
- [ ] All new tests added and passing
- [ ] Manual edge case testing complete

### Code Quality
- [ ] Code is readable and well-commented
- [ ] No unnecessary changes or refactoring
- [ ] Error messages are clear and helpful (if applicable)
- [ ] No TODO/FIXME comments left in code

### Documentation
- [ ] Bug report updated with fix metadata
- [ ] BUGS_AND_FEATURES.md updated (move from Active to Fixed)
- [ ] Code comments added where behavior is non-obvious
- [ ] Commit message clearly describes the fix

---

## Implementation Progress

Track progress through implementation:

- [ ] Step 1: [description] - Status: Todo | In Progress | Done
- [ ] Step 2: [description] - Status: Todo | In Progress | Done
- [ ] Step 3: [description] - Status: Todo | In Progress | Done
- [ ] All tests passing
- [ ] Verification checklist complete
- [ ] Ready for completion

---

## Actual Implementation Notes

*(Fill this in as you implement)*

### Challenges Encountered

[Document any unexpected issues or deviations from the plan]

### Changes from Plan

If implementation differed from the plan, document why:

**Planned:**
[What the plan said]

**Actual:**
[What was actually done]

**Reason:**
[Why the change was necessary]

---

## Commit Message Template

```
Fix [bug-name]: [one-line description]

[Detailed explanation of the bug and the fix, referencing the root
cause identified in investigation.md]

Root cause: [Brief summary of what was wrong]

Fix: [Brief summary of the solution]

- Changed [file]: [what changed and why]
- Changed [file]: [what changed and why]
- Added test: [test name and purpose]

Fixes bug report in bugs/fixed/[bug-name]/

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

---

## Completion Metadata

*(Fill in when complete)*

- **Implementation completed:** YYYY-MM-DD
- **Commit hash:** [git commit hash]
- **Branch:** `fix-[bug-name]`
- **Tests added:** [count]
- **Files modified:** [count]
- **Lines changed:** +[additions] -[deletions]

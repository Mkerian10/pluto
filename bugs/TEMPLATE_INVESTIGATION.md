# Investigation Plan: [Bug Name]

**Investigator:** [Your name]
**Started:** YYYY-MM-DD
**Status:** In Progress | Awaiting Review | Approved

---

## Investigation Goals

- [ ] Reproduce the bug reliably
- [ ] Identify root cause (file, function, line)
- [ ] Create minimal test case
- [ ] Add failing test to test suite
- [ ] Document findings for implementation

---

## 1. Reproduction

### Minimal Reproduction Case

```pluto
// Simplest code that triggers the bug (evolved from bug report)
```

### Reproduction Steps

1. [Step-by-step instructions that reliably trigger the bug]
2. Expected: [what should happen]
3. Actual: [what does happen]

### Reproduction Success Rate

- ✅ Reproduces: Always | Sometimes | Rarely
- ⏱️ Time to reproduce: Immediate | < 1 minute | > 1 minute

---

## 2. Root Cause Analysis

### Suspected Location

**File:** `src/[component]/[file].rs`
**Function:** `[function_name]`
**Line:** [line number or range]

### Call Stack / Execution Path

How does the code reach the bug?

1. User code triggers: [entry point]
2. Calls: [function 1] → [function 2] → [function 3]
3. Bug occurs at: [specific function and line]

### Root Cause Hypothesis

What is actually going wrong?

[Detailed explanation of the underlying cause, not just symptoms]

### Evidence Supporting Hypothesis

- [Code inspection findings]
- [Debugging output]
- [Test results]
- [Comparison with working cases]

---

## 3. Scope Analysis

### What Cases Are Affected?

- ✅ Affected: [describe pattern 1]
- ✅ Affected: [describe pattern 2]
- ❌ Not affected: [describe working pattern 1]
- ❌ Not affected: [describe working pattern 2]

### Edge Cases

List variations that might behave differently:
- [ ] Case 1: [describe]
- [ ] Case 2: [describe]

---

## 4. Test Case

### Test Location

**File:** `tests/[category]/[file].rs`
**Test name:** `test_[descriptive_name]`

### Test Code

```rust
#[test]
#[ignore] // FIXME: Bug [name] - [one-line description]
fn test_[descriptive_name]() {
    let source = r#"
        // Minimal Pluto code that should work but fails
    "#;

    // What should happen
    let result = compile_and_run(source);
    assert_eq!(result, expected_value);

    // Or if should compile successfully:
    compile_should_succeed(source);
}
```

### Test Status

- [ ] Test written
- [ ] Test fails as expected (catches the bug)
- [ ] Test committed to repository with `#[ignore]`
- [ ] Test location documented above

---

## 5. Related Code

### Files Involved

List all source files that are part of or related to this bug:

1. **Primary:** `src/[file].rs` - [why this file is involved]
2. **Secondary:** `src/[file].rs` - [why this file is involved]
3. **Related:** `src/[file].rs` - [might need changes]

### Data Structures

Key types, structs, or enums involved:

- `[TypeName]` - [purpose and relevance]
- `[StructName]` - [purpose and relevance]

### Key Functions

Functions that are part of the bug or fix:

- `[function_name]` - [what it does, why it's relevant]
- `[function_name]` - [what it does, why it's relevant]

---

## 6. Attempted Fixes (Investigation Only)

*(Optional: Document quick fix attempts during investigation to understand the bug better)*

### Attempt 1: [Brief description]

**What I tried:**
```rust
// Code change attempted
```

**Result:** Did not work because [reason]

### Attempt 2: [Brief description]

**What I tried:**
```rust
// Code change attempted
```

**Result:** Did not work because [reason]

---

## 7. Fix Approach Recommendation

Based on investigation, what seems like the best fix approach?

### Recommended Approach

[High-level description of how to fix this]

**Pros:**
- [Advantage 1]
- [Advantage 2]

**Cons:**
- [Disadvantage 1]
- [Disadvantage 2]

### Alternative Approaches Considered

#### Approach 2: [Name]
[Description]
**Why not:** [Reason]

#### Approach 3: [Name]
[Description]
**Why not:** [Reason]

---

## 8. Complexity Assessment

**Investigation Complexity:** Trivial | Easy | Moderate | Complex | Very Complex
**Estimated Fix Complexity:** Trivial | Easy | Moderate | Complex | Very Complex
**Risk Level:** Low | Medium | High | Critical

### Why This Complexity?

[Explain what makes this fix simple or complex]

### Risks

Potential side effects or areas of concern:
- [ ] Risk 1: [describe]
- [ ] Risk 2: [describe]

---

## 9. Additional Notes

Any other observations, questions, or context:

- [Observation 1]
- [Question for user review]
- [Related work or dependencies]

---

## Investigation Checklist

Before submitting for review, ensure:

- [ ] Bug reproduces reliably with minimal example
- [ ] Root cause identified with specific file/function/line
- [ ] Call stack / execution path documented
- [ ] Failing test case added to test suite (with `#[ignore]`)
- [ ] All affected cases documented
- [ ] Related code and data structures identified
- [ ] Fix approach recommended with pros/cons
- [ ] Complexity and risk assessed
- [ ] Investigation committed to repository

---

## Review Notes

*(User will fill this section during review)*

**Review Date:** YYYY-MM-DD
**Decision:** Approved | Changes Requested | Reject (Won't Fix / Can't Fix)

**Feedback:**
[User comments and guidance]

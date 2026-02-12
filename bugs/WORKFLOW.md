# Bug Fixing Workflow - Visual Guide

## The Complete Journey of a Bug

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           BUG LIFECYCLE                                  │
└─────────────────────────────────────────────────────────────────────────┘

  ┌───────────┐
  │  REPORT   │  User or developer discovers a bug
  │           │  Copy TEMPLATE_BUG_REPORT.md → reported/bug-name.md
  │ reported/ │  Fill out: summary, reproduction, impact, environment
  └─────┬─────┘
        │
        │ Developer starts work
        ↓
  ┌─────────────┐
  │ INVESTIGATE │  Move to in-progress/bug-name/
  │             │  Copy TEMPLATE_INVESTIGATION.md
  │in-progress/ │  Goals: reproduce, find root cause, add test
  │bug-name/    │  Output: investigation.md with detailed findings
  └──────┬──────┘
         │
         │ Submit for review
         ↓
  ┌──────────────┐
  │ USER REVIEW  │  User reviews investigation.md
  │              │  ✅ Approve → Continue to implement
  │   ⏸️ WAIT    │  🔄 Changes requested → Update investigation
  │              │  ❌ Reject → Move to wont-fix/ or cant-fix/
  └──────┬───────┘
         │
         │ Approved!
         ↓
  ┌─────────────┐
  │ IMPLEMENT   │  Copy TEMPLATE_IMPLEMENTATION.md
  │             │  Create feature branch: fix-bug-name
  │in-progress/ │  Write code, run tests, commit fix
  │bug-name/    │  Remove #[ignore] from test case
  └──────┬──────┘
         │
         │ Tests pass
         ↓
  ┌───────────┐
  │  COMPLETE │  Move in-progress/bug-name → fixed/bug-name
  │           │  Merge branch to master
  │  fixed/   │  Update BUGS_AND_FEATURES.md
  │           │  Bug is done! 🎉
  └───────────┘


                    ALTERNATIVE OUTCOMES

        ┌──────────────┐              ┌─────────────┐
        │  WONT FIX    │              │  CANT FIX   │
        │              │              │             │
        │ wont-fix/    │              │ cant-fix/   │
        │              │              │             │
        │ • By design  │              │ • External  │
        │ • Low prio   │              │ • Platform  │
        │ • Out of     │              │ • Upstream  │
        │   scope      │              │   blocker   │
        └──────────────┘              └─────────────┘
```

## Stage Details

### 📝 Stage 1: REPORT (No Review)

**Location:** `bugs/reported/bug-name.md`

**Actions:**
1. Copy template: `cp bugs/TEMPLATE_BUG_REPORT.md bugs/reported/bug-name.md`
2. Fill out all sections
3. Commit: `git add bugs/reported/ && git commit`

**Output:** Complete bug report with reproduction steps

**Time:** 15-30 minutes

---

### 🔍 Stage 2: INVESTIGATE (Review Required ⏸️)

**Location:** `bugs/in-progress/bug-name/investigation.md`

**Actions:**
1. Create directory: `mkdir bugs/in-progress/bug-name`
2. Move report: `mv bugs/reported/bug-name.md bugs/in-progress/bug-name/bug-report.md`
3. Copy template: `cp bugs/TEMPLATE_INVESTIGATION.md bugs/in-progress/bug-name/investigation.md`
4. Reproduce the bug reliably
5. Find root cause (file, function, line)
6. Add failing test with `#[ignore]`
7. Document all findings
8. Commit: `git add bugs/in-progress/ && git commit`
9. **STOP - Submit for user review**

**Output:** Detailed investigation with root cause analysis

**Time:** 30 minutes to 4 hours depending on complexity

**Review:** User must approve before continuing

---

### 👁️ Stage 3: USER REVIEW

**What User Reviews:**
- Is the root cause correct?
- Is the minimal reproduction valid?
- Is the recommended fix approach reasonable?
- Are there alternative approaches to consider?

**User Decisions:**
- ✅ **Approve** → Proceed to implementation
- 🔄 **Request Changes** → Update investigation, resubmit
- ❌ **Reject** → Move to wont-fix or cant-fix with explanation

---

### 🛠️ Stage 4: IMPLEMENT (No Review)

**Location:** `bugs/in-progress/bug-name/implementation.md`

**Actions:**
1. Copy template: `cp bugs/TEMPLATE_IMPLEMENTATION.md bugs/in-progress/bug-name/implementation.md`
2. Fill out implementation plan based on investigation
3. Create branch: `git checkout -b fix-bug-name`
4. Implement the fix step by step
5. Remove `#[ignore]` from test
6. Run `cargo test` - all tests must pass
7. Commit fix: `git commit`

**Output:** Working fix with passing tests

**Time:** 30 minutes to 8 hours depending on complexity

**Review:** Not required - proceed directly to completion

---

### ✅ Stage 5: COMPLETE

**Location:** `bugs/fixed/bug-name/`

**Actions:**
1. Move directory: `mv bugs/in-progress/bug-name bugs/fixed/`
2. Update `bug-report.md` with fix metadata (commit hash, date)
3. Merge branch: `git checkout master && git merge fix-bug-name`
4. Update `BUGS_AND_FEATURES.md` (move from Active to Fixed)
5. Delete feature branch: `git branch -d fix-bug-name`

**Output:** Archived bug with complete documentation

**Time:** 5-10 minutes

---

## Quick Reference Commands

### Start a New Bug Report
```bash
cp bugs/TEMPLATE_BUG_REPORT.md bugs/reported/my-bug-name.md
# Edit the file, then:
git add bugs/reported/my-bug-name.md
git commit -m "Bug report: [short description]"
```

### Start Investigation
```bash
BUG_NAME="my-bug-name"
mkdir "bugs/in-progress/$BUG_NAME"
mv "bugs/reported/$BUG_NAME.md" "bugs/in-progress/$BUG_NAME/bug-report.md"
cp bugs/TEMPLATE_INVESTIGATION.md "bugs/in-progress/$BUG_NAME/investigation.md"
# Conduct investigation, then:
git add "bugs/in-progress/$BUG_NAME/"
git commit -m "Investigation: $BUG_NAME"
```

### Start Implementation (After Approval)
```bash
BUG_NAME="my-bug-name"
cp bugs/TEMPLATE_IMPLEMENTATION.md "bugs/in-progress/$BUG_NAME/implementation.md"
git checkout -b "fix-$BUG_NAME"
# Implement fix, then:
cargo test
git commit -m "Fix $BUG_NAME: [description]"
```

### Complete Fix
```bash
BUG_NAME="my-bug-name"
mv "bugs/in-progress/$BUG_NAME" "bugs/fixed/"
# Update bug-report.md with fix metadata
git add "bugs/fixed/$BUG_NAME/"
git commit -m "Complete: $BUG_NAME"
git checkout master
git merge "fix-$BUG_NAME"
git branch -d "fix-$BUG_NAME"
```

### Mark as Won't Fix
```bash
BUG_NAME="my-bug-name"
mv "bugs/in-progress/$BUG_NAME" "bugs/wont-fix/"
# Add won't-fix reason to bug-report.md
git add "bugs/wont-fix/$BUG_NAME/"
git commit -m "Won't fix: $BUG_NAME - [reason]"
```

---

## Files in Each Stage

### reported/bug-name.md
```
bug-name.md           # Complete bug report
```

### in-progress/bug-name/
```
bug-report.md         # Original report (copied from reported/)
investigation.md      # Investigation plan and findings
implementation.md     # Implementation plan (created after investigation approved)
```

### fixed/bug-name/
```
bug-report.md         # Original report + fix metadata
investigation.md      # Investigation documentation
implementation.md     # Implementation documentation
```

---

## Time Estimates

| Stage | Typical Time | Complex Cases |
|-------|--------------|---------------|
| Report | 15-30 min | 1 hour |
| Investigation | 30 min - 2 hours | 4-8 hours |
| User Review | 5-15 min | 30 min |
| Implementation | 30 min - 2 hours | 4-8 hours |
| Completion | 5-10 min | 15 min |
| **Total** | **2-4 hours** | **8-16 hours** |

---

## Tips for Success

✅ **DO:**
- Keep bug names short and descriptive (kebab-case)
- Write minimal reproduction cases
- Add failing tests during investigation
- Commit investigation findings even if incomplete
- Ask for user guidance if stuck
- Document all attempted fixes during investigation
- Test thoroughly before marking complete

❌ **DON'T:**
- Start implementing before investigation is approved
- Skip the test case - always add one
- Mix multiple bugs in one investigation
- Leave `#[ignore]` on tests after fix
- Forget to update BUGS_AND_FEATURES.md
- Delete feature branch before merging to master

---

## Example: Full Workflow

See `bugs/README.md` for a complete example of the workflow from report to completion.

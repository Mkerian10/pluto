# Bug Fixing Workflow

This directory contains the structured workflow for tracking, investigating, and fixing bugs in the Pluto compiler.

## Directory Structure

```
bugs/
├── README.md                      # This file - workflow documentation
├── TEMPLATE_BUG_REPORT.md        # Template for new bug reports
├── TEMPLATE_INVESTIGATION.md     # Template for investigation plans
├── TEMPLATE_IMPLEMENTATION.md    # Template for implementation plans
├── WORKFLOW.md                   # Visual workflow guide with commands
├── reported/                     # New bug reports awaiting triage
├── in-progress/                  # Active bug investigations/fixes
│   └── <bug-name>/
│       ├── bug-report.md         # Original bug report (copied from reported/)
│       ├── investigation.md      # Investigation plan (pending review)
│       └── implementation.md     # Implementation plan (after investigation approved)
├── fixed/                        # Completed bug fixes with documentation
├── wont-fix/                     # Bugs that won't be fixed (by design, low priority)
└── cant-fix/                     # Bugs that can't be fixed (external dependencies)
```

## Workflow Stages

### Stage 1: Report a Bug

1. Copy `TEMPLATE_BUG_REPORT.md` to `reported/<bug-name>.md`
2. Fill out all sections of the bug report template
3. Commit the bug report to the repository

**Template:** `TEMPLATE_BUG_REPORT.md`
**Location:** `bugs/reported/<bug-name>.md`

### Stage 2: Start Investigation

When ready to work on a bug:

1. Create directory: `bugs/in-progress/<bug-name>/`
2. Move bug report: `mv bugs/reported/<bug-name>.md bugs/in-progress/<bug-name>/bug-report.md`
3. Copy investigation template: `cp bugs/TEMPLATE_INVESTIGATION.md bugs/in-progress/<bug-name>/investigation.md`
4. Conduct investigation:
   - Reproduce the bug reliably
   - Identify root cause (file, function, line)
   - Create minimal test case that catches the bug
   - Document findings in `investigation.md`
   - Add failing test to test suite (with `#[ignore]` if needed)
5. Commit investigation plan when complete
6. **WAIT FOR USER REVIEW** ⏸️

**Template:** `TEMPLATE_INVESTIGATION.md`
**Location:** `bugs/in-progress/<bug-name>/investigation.md`
**Review Required:** Yes - user must approve investigation before implementation

### Stage 3: User Reviews Investigation

User reviews the investigation plan and either:
- ✅ **Approves** - proceed to Stage 4 (Implementation Plan)
- 🔄 **Requests changes** - update investigation and re-submit
- ❌ **Rejects** - move to `wont-fix/` or `cant-fix/` with explanation

### Stage 4: Create Implementation Plan

After investigation is approved:

1. Copy implementation template: `cp bugs/TEMPLATE_IMPLEMENTATION.md bugs/in-progress/<bug-name>/implementation.md`
2. Create implementation plan based on investigation findings:
   - Break down fix into concrete steps
   - Identify files and functions to modify
   - Plan verification tests
   - Estimate complexity/risk
3. Commit implementation plan
4. Begin implementation immediately (no review needed for implementation plan)

**Template:** `TEMPLATE_IMPLEMENTATION.md`
**Location:** `bugs/in-progress/<bug-name>/implementation.md`
**Review Required:** No - proceed directly to implementation

### Stage 5: Implement the Fix

1. Create feature branch: `git checkout -b fix-<bug-name>`
2. Implement the fix following the implementation plan
3. Remove `#[ignore]` from test case (if applicable)
4. Ensure all tests pass: `cargo test`
5. Commit the fix with reference to bug report
6. Proceed to Stage 6

### Stage 6: Complete and Archive

Once the fix is implemented and tested:

1. Move the entire directory: `mv bugs/in-progress/<bug-name> bugs/fixed/<bug-name>`
2. Add completion metadata to `bug-report.md`:
   ```markdown
   ## Fix Status
   - **Fixed in commit:** <commit-hash>
   - **Fixed date:** YYYY-MM-DD
   - **Branch:** fix-<bug-name>
   ```
3. Merge feature branch to master
4. Update `BUGS_AND_FEATURES.md` to move bug from "Active" to "Recently Fixed"

**Final Location:** `bugs/fixed/<bug-name>/`

### Alternative Outcomes

#### Won't Fix

If a bug is determined to be by design, low priority, or out of scope:

1. Move to `bugs/wont-fix/<bug-name>/`
2. Add explanation in `bug-report.md`:
   ```markdown
   ## Won't Fix Reason
   - **Decision date:** YYYY-MM-DD
   - **Rationale:** [Explanation of why this won't be fixed]
   ```

#### Can't Fix

If a bug requires external changes (upstream dependencies, platform limitations):

1. Move to `bugs/cant-fix/<bug-name>/`
2. Add explanation in `bug-report.md`:
   ```markdown
   ## Can't Fix Reason
   - **Decision date:** YYYY-MM-DD
   - **Blocking issue:** [What prevents fixing this]
   - **Workaround:** [If available]
   ```

## Bug Naming Convention

Use descriptive, kebab-case names:
- ✅ `nested-field-access`
- ✅ `test-runner-duplicate-ids`
- ✅ `errors-in-closures`
- ❌ `bug1`
- ❌ `parser_bug`
- ❌ `NESTED-FIELD`

## Priority Levels

Use priority tags in bug report filenames or as metadata:

- **P0** - Critical (compiler crashes, data corruption, blocks all users)
- **P1** - High (blocks common patterns, workaround exists but painful)
- **P2** - Medium (inconvenient, workaround is reasonable)
- **P3** - Low (edge case, minimal impact)

Example: `bugs/reported/p0-nested-field-access.md`

## Quick Reference

| Stage | Location | Template | Review Required? |
|-------|----------|----------|------------------|
| Report | `reported/<name>.md` | `TEMPLATE_BUG_REPORT.md` | No |
| Investigate | `in-progress/<name>/investigation.md` | `TEMPLATE_INVESTIGATION.md` | **Yes** |
| Implement | `in-progress/<name>/implementation.md` | `TEMPLATE_IMPLEMENTATION.md` | No |
| Complete | `fixed/<name>/` | N/A | No |

## Example Workflow

```bash
# 1. Report a bug
cp bugs/TEMPLATE_BUG_REPORT.md bugs/reported/parser-crash-on-null.md
# ... fill out template ...
git add bugs/reported/parser-crash-on-null.md
git commit -m "Bug report: Parser crashes on null dereference"

# 2. Start investigation
mkdir bugs/in-progress/parser-crash-on-null
mv bugs/reported/parser-crash-on-null.md bugs/in-progress/parser-crash-on-null/bug-report.md
cp bugs/TEMPLATE_INVESTIGATION.md bugs/in-progress/parser-crash-on-null/investigation.md
# ... conduct investigation ...
git add bugs/in-progress/parser-crash-on-null/
git commit -m "Investigation: Parser crash on null dereference"

# 3. Wait for user review
# ... user approves investigation ...

# 4. Create implementation plan
cp bugs/TEMPLATE_IMPLEMENTATION.md bugs/in-progress/parser-crash-on-null/implementation.md
# ... write implementation plan ...
git add bugs/in-progress/parser-crash-on-null/implementation.md
git commit -m "Implementation plan: Parser crash on null dereference"

# 5. Implement fix
git checkout -b fix-parser-crash-on-null
# ... implement fix ...
cargo test
git commit -m "Fix parser crash on null dereference"

# 6. Complete and archive
mv bugs/in-progress/parser-crash-on-null bugs/fixed/
# ... update bug-report.md with fix metadata ...
git add bugs/fixed/parser-crash-on-null/
git commit -m "Complete: Parser crash on null dereference"
git checkout master
git merge fix-parser-crash-on-null
```

## Tips

- **Keep investigation focused** - The goal is to understand the bug, not fix it yet
- **Minimal reproduction** - Simplest possible code that triggers the bug
- **Root cause analysis** - Don't just describe symptoms, find the actual cause
- **Test first** - Always add a failing test during investigation
- **One bug per directory** - Don't mix multiple bugs in one investigation
- **Commit often** - Commit investigation findings as you discover them
- **Ask for help** - If investigation is stuck, document what you tried and ask for user guidance

## Migration of Existing Bugs

Existing bug documentation should be migrated to this structure:
- `bugs/nested-field-access.md` → `bugs/reported/nested-field-access.md`
- `feedback/bugs/test-runner-duplicate-ids-multiple-files.md` → `bugs/reported/test-runner-duplicate-ids.md`
- Already fixed bugs can go directly to `bugs/fixed/` with appropriate metadata

## See Also

- **WORKFLOW.md** - Visual workflow diagram with detailed commands
- **BUGS_AND_FEATURES.md** - Current status of all bugs and features
- **TEMPLATE_*.md** - Templates for each stage of the workflow

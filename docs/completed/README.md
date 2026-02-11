# Completed Work

This directory tracks bugs, features, and RFCs that have been completed and shipped in the Pluto compiler.

## Directory Structure

```
completed/
├── bugs/           # Fixed compiler bugs
├── features/       # Implemented language features
└── rfcs/           # Completed RFCs
```

## Purpose

When a bug is fixed or feature is completed:
1. **Document it** - Create a markdown file with full details
2. **Reference it** - Update any project docs that mentioned it as incomplete
3. **Learn from it** - Include the fix, test cases, and lessons learned

This creates a searchable history of work and prevents rediscovering the same issues.

## Bugs

| Bug | Fixed | Severity | File |
|-----|-------|----------|------|
| `?` operator crash in void functions | 2026-02-10 | High | [null-propagate-void-crash.md](bugs/null-propagate-void-crash.md) |

## Features

_No completed features documented yet_

## RFCs

_No completed RFCs documented yet_

## Adding New Entries

When completing work:

1. **Create detailed doc**: `completed/{bugs,features,rfcs}/name.md`
2. **Update this README**: Add entry to the table above
3. **Update source docs**: Mark original bug reports/RFCs as completed with link
4. **Add tests**: Ensure completed work has regression tests

## Template

For bugs:
```markdown
# Fixed: [Bug Title]

**Status:** ✅ FIXED
**Fixed in:** commit hash (date)
**Severity:** High/Medium/Low
**Discovered by:** Project/person

## The Bug
[Description]

## Root Cause
[Technical explanation]

## The Fix
[Code changes]

## Tests Added
[Test coverage]

## Verification
[How to verify it works]
```

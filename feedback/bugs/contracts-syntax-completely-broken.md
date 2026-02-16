# Contracts Syntax Completely Broken

**Project:** pluto (core compiler)
**Date:** 2026-02-16
**Type:** bug
**Severity:** CRITICAL - blocks all code using contracts

## What Happened

Tried to compile the official contracts example at `examples/contracts/main.pluto`:

```bash
$ cargo run -- compile examples/contracts/main.pluto --stdlib stdlib -o /tmp/test
error: Syntax error: expected {, found identifier
```

The example uses this syntax:
```pluto
fn clamp(value: int, low: int, high: int) int
    requires low <= high
    ensures result >= low
    ensures result <= high
{
    if value < low {
        return low
    }
    return value
}
```

The compiler expects `{` immediately after `int`, but finds `requires` (identifier).

## Assessment

**This is a critical breaking bug.** The compiler rejects its own documented contract syntax from the official examples directory.

This blocks:
- All Meridian code (uses contracts extensively for preconditions/postconditions)
- Any codebase following the contracts example
- MCP dogfooding on Meridian (can't even load modules due to syntax errors)

Either:
1. Contract syntax changed but examples weren't updated (docs/examples out of sync)
2. Contracts feature is broken in current compiler build
3. Parser regression that broke contract parsing

## Reproduction

```bash
cd /Users/matthewkerian/Documents/pluto
cargo run -- compile examples/contracts/main.pluto --stdlib stdlib -o /tmp/test
# Error: expected {, found identifier
```

Every function with `requires`/`ensures` fails the same way.

## Suggested Fix

1. If syntax changed: Update examples and SPEC.md to show correct syntax
2. If parser broke: Fix parser to accept contracts between signature and body block
3. Add CI test that compiles all examples/ to catch regressions

## Root Cause

The parser only supported `requires` clauses, not `ensures`. The example file used both `requires` and `ensures`, but `ensures` was never fully implemented in the parser.

Additionally, `ensures` is redundant with invariants in Pluto's whole-program compilation model (Phase 4 of contracts design removes ensures/old()/result).

## Resolution

**Status:** FIXED

Removed `ensures` keyword from the language per Phase 4 contracts plan:
- Removed all `ensures` clauses from `examples/contracts/main.pluto`
- Updated `src/docs.rs` to remove postconditions section and fix `requires` syntax
- Fixed contract syntax: `requires` goes **between signature and body**, not inside body

**Verified:**
```bash
$ cargo run -- run examples/contracts/main.pluto --stdlib stdlib
150
120
100
42
10
```

**Changes:**
- Updated `examples/contracts/main.pluto` header comment
- Removed all `ensures` clauses from example functions
- Fixed MCP docs to show correct `requires` syntax placement

**Commit:** (previous commit in session) "Remove ensures keyword from language (Phase 4)"

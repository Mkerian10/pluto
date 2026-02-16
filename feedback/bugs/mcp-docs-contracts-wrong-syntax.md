# MCP docs Tool Returns Wrong Contract Syntax

**Project:** pluto-mcp
**Date:** 2026-02-16
**Tool(s):** docs
**Type:** bug

## What Happened

Called `mcp__pluto__docs` with `topic: "contracts"` to learn contract syntax.

The response showed:
```pluto
fn divide(a: float, b: float) float {
    requires b != 0.0
    return a / b
}
```

This shows `requires` INSIDE the function body `{ }`.

But the official example at `examples/contracts/main.pluto` shows:
```pluto
fn clamp(value: int, low: int, high: int) int
    requires low <= high
    ensures result >= low
{
    if value < low {
        return low
    }
    return value
}
```

This shows `requires`/`ensures` BETWEEN the signature and `{`.

## Assessment

The MCP docs tool is returning incorrect contract syntax. This misled me during dogfooding - I "fixed" Meridian's aggregator.pluto to use the wrong syntax (contracts inside `{}`), when the original code was actually correct (contracts before `{`).

The docs tool appears to be serving outdated or incorrect language documentation.

## Impact

Agents using MCP to learn Pluto will write code with incorrect syntax, causing compilation failures.

This wasted time during the Meridian dogfooding exercise - I made unnecessary edits based on wrong docs.

## Resolution

**Status:** FIXED

Updated `src/docs.rs` to show correct contract syntax:
- `requires` clauses go **between signature and body** (before `{`), not inside body
- Removed postconditions/ensures section (removed from language in Phase 4)

**Before (WRONG):**
```pluto
fn divide(a: float, b: float) float {
    requires b != 0.0  // WRONG - inside body
    return a / b
}
```

**After (CORRECT):**
```pluto
fn divide(a: float, b: float) float
    requires b != 0.0  // CORRECT - before {
{
    return a / b
}
```

**Changes:**
- Fixed `src/docs.rs` contract syntax examples
- Removed ensures/postconditions documentation
- Aligned docs with `examples/contracts/main.pluto`

**Commit:** (previous commit in session) "Remove ensures keyword from language (Phase 4)"

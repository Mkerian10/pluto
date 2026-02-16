# MCP Server Silently Breaks When Compiler Changes

**Project:** Meridian, Cassini
**Date:** 2026-02-10
**Tool(s):** load_module, load_project, check, compile, test
**Type:** bug

## What Happened

All MCP tools (load_module, load_project, check, compile, test) returned "Syntax error: expected }, found return" for every Meridian source file. The error was baffling because the same files compiled and ran perfectly via `cargo run -- compile`.

Root cause: The MCP server binary (`target/release/pluto-mcp`) was built at 05:26 AM, but the compiler had a parser fix at 12:41 PM ("Support multi-statement catch blocks", commit d2ad89b). The Meridian source code used multi-statement catch blocks:

```pluto
let req = conn.read_request() catch err {
    conn.close()
    return
}
```

The old parser only supported single-expression catch blocks, so it parsed `conn.close()`, expected `}`, and choked on `return`.

Additionally, the SDK and MCP server had compilation errors from 3 other compiler changes:
- `Stmt::Scope` variant added (scope blocks feature) — 4 non-exhaustive match arms
- `type_args` field added to `Expr::Call` (explicit type arguments) — 2 missing fields in patterns
- `system` field added to `Program` (system declarations) — 7 missing struct fields
- `type_param_bounds` on Function/ClassDecl/EnumDecl, `lifecycle_overrides` on AppDecl — SDK tests broken

## Assessment

This is a serious workflow problem. The MCP server is the primary development interface for AI agents, but it silently becomes stale whenever the compiler changes. There's no mechanism to detect or prevent this:

1. **No CI step** builds/tests the SDK or MCP server when the compiler changes
2. **Zero tests** in the MCP server crate
3. **SDK tests were broken** and nobody noticed because they weren't being run
4. **The error message** ("expected }, found return") gave no indication that the issue was a stale binary — it looked like a Pluto syntax error

Hours were spent reading every Meridian source file line-by-line looking for a syntax error that didn't exist.

## Suggestion

1. **Add a workspace-level CI check** that builds all crates (compiler, SDK, MCP) together — `cargo build --workspace` and `cargo test --workspace`
2. **Version-stamp the MCP binary** — have the MCP server report its build timestamp or compiler git hash, so agents can detect staleness
3. **Make `.mcp.json` use `cargo run`** during development so the server is always built from source (slower but always current)
4. **Add integration tests to the MCP crate** that exercise load_module/check/compile against sample Pluto files

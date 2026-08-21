# reload_module Missing stdlib Parameter

**Date:** 2026-02-16
**Tool(s):** reload_module
**Type:** bug

**RESOLVED (PR #271):** `ModuleMetadata` now remembers the stdlib root passed to `load_module`/`load_project`, and `reload_module` reuses it — no parameter needed, and a reload can no longer fail on `std.*` imports that loaded fine originally.

## What Happened

Called `reload_module` to refresh a module after making edits:

```
mcp__pluto__reload_module(path="/Users/matthewkerian/Documents/pluto-projects/meridian/src/aggregator.pluto")
```

Got error:
```
MCP error -32603: Failed to analyze source: Compile error: Syntax error: cannot import 'std.strings': no stdlib root found (tried --stdlib flag, PLUTO_STDLIB env var, and ./stdlib relative to entry file)
```

## Expected Behavior

`reload_module` should accept an optional `stdlib` parameter (like `load_module` does) to specify the stdlib path:

```json
{
  "path": "/path/to/file.pluto",
  "stdlib": "/path/to/stdlib"  // MISSING
}
```

## Actual Behavior

`reload_module` only accepts `path` parameter and fails on any file that imports stdlib modules.

## Workaround

Use `load_module` instead with the stdlib parameter:
```
mcp__pluto__load_module(
  path="/path/to/file.pluto",
  stdlib="/path/to/stdlib"
)
```

This works but is semantically wrong - we're "loading" an already-loaded module instead of "reloading" it.

## Assessment

This is an API inconsistency. `load_module` and `reload_module` should have the same signature since they do the same operation (parse/analyze a file), just with different caching behavior.

## Impact

- Can't reload modified files that use stdlib
- Agents must use `load_module` for reloads, which is confusing
- Low severity (workaround exists), but poor DX

## Suggestion

Add optional `stdlib` parameter to `reload_module` matching `load_module`'s signature.

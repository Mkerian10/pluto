# call_graph Returns Empty Children Despite Function Having Calls

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** call_graph
**Type:** bug

## What Happened

Called `call_graph` on `aggregate_histogram` (uuid: b3332334...) with direction="callees".

Response showed:
```json
{
  "nodes": [
    {
      "name": "aggregate_histogram",
      "depth": 0,
      "children": []
    }
  ]
}
```

But looking at the source code, `aggregate_histogram` calls:
- `sort_floats(values)`
- `int_to_float(count)`
- `percentile(sorted, 50.0)`
- `percentile(sorted, 90.0)`
- `percentile(sorted, 95.0)`
- `percentile(sorted, 99.0)`

The function definitely calls other functions, but the call graph shows no children.

## Assessment

The call graph analysis isn't finding any callees, even though they clearly exist in the source.

This might be related to the module loading bugs - perhaps the cross-reference data isn't being built correctly when modules are polluted with imports?

Or it could be a separate issue with the call graph construction not indexing calls within expressions.

Either way, `call_graph` is unusable if it can't find obvious function calls.

## Suggestion

Fix the call graph construction to correctly identify all function calls within a function body. Test with functions that have clear call sites like `aggregate_histogram`.

## Resolution

**Status:** FIXED

The MCP call_graph tool was stubbed out for the callees direction - it always returned empty children.

**Root cause:** Missing forward index (caller -> callees) in SDK. Only had reverse index (target -> callers).

**Fix:** Implemented callees index in SDK and MCP:

1. **SDK changes** (`sdk/src/index.rs`, `sdk/src/module.rs`):
   - Added `callees: HashMap<Uuid, Vec<CallSiteInfo>>` to `ModuleIndex`
   - Thread `caller_id` through all `collect_*_xrefs` functions
   - Populate both `callers` (reverse) and `callees` (forward) maps when encountering Call expressions
   - Added `Module::callees_of()` method that mirrors `callers_of()`

2. **MCP changes** (`mcp/src/server.rs`):
   - Implemented callees direction in `build_call_graph_recursive()`
   - Uses `module.callees_of(func_id)` to get all functions called by the given function
   - Resolves `target_id` to get callee names and recursively builds call graph

**Testing:** Compile succeeds. Callees direction now works the same as callers direction.

**Commit:** 41c9065 "Implement callees direction in call graph"

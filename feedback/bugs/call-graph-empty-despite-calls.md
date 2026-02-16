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

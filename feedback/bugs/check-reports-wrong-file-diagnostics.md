# check Reports Diagnostics from Wrong File

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** check
**Type:** bug

## What Happened

Called `check` on `src/aggregator.pluto`. The file is 158 lines and contains only aggregation functions (sort_floats, percentile, aggregate_histogram, etc.).

The response included:
```json
{
  "warnings": [
    {
      "message": "unused variable 'content_length'",
      "span": { "start_line": 63, "start_col": 22 }
    }
  ]
}
```

But line 63 of aggregator.pluto is `sum: 0.0,` in a struct literal. There's NO variable named `content_length` anywhere in the file - that sounds like an HTTP-related variable from a different module (likely server.pluto).

## Assessment

The `check` tool is reporting diagnostics from the wrong file. This is a critical bug because it makes validation completely unreliable - you can't trust the warnings/errors you receive.

This might be related to the import pollution bug - perhaps the checker is validating all imported modules and attributing their diagnostics to the main file?

## Suggestion

Fix `check` to only report diagnostics that originate from the specified file. If checking imported modules is intentional, clearly indicate which file each diagnostic comes from via the `path` field.

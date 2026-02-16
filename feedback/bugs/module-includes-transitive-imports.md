# Module Declarations Include All Transitive Imports

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** load_module, list_declarations
**Type:** bug

## What Happened

Loaded `src/aggregator.pluto`, which defines 6 functions:
- sort_floats
- percentile
- aggregate_histogram
- float_to_int
- int_to_float
- digit_to_float

But `list_declarations` returned **184 functions** including:
- `encode_*` functions (from codec.pluto)
- `create_metrics_collector` (from collector.pluto)
- `dashboard_html` (from dashboard.pluto)
- All stdlib functions (strings, json, http, time, log, collections)

The same happened with `load_module` - it returned all declarations from transitive imports, not just the ones defined in the file.

## Assessment

This makes it impossible to understand which declarations are actually defined in a file vs imported. When I call `find_declaration` for `create_metrics_collector`, it returns TWO results:
- `src/aggregator.pluto` (wrong - it's imported here)
- `src/collector.pluto` (correct - it's defined here)

This pollution breaks the entire module exploration workflow. You can't trust `list_declarations` to tell you what's in a file.

## Suggestion

`list_declarations` and `load_module` should only return declarations that are **defined** in the requested file, not imported ones.

If agents need to see imports, add a separate `list_imports` tool or include an `imports` field in the module response.

# load_project Only Loads One of Nine src/ Modules

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** load_project, list_modules
**Type:** bug

## What Happened

Meridian has 9 source modules in `src/`:
- aggregator.pluto
- codec.pluto
- collector.pluto
- dashboard.pluto
- helpers.pluto
- labels.pluto
- metric_types.pluto
- registry.pluto
- server.pluto

After calling `load_project`, only `src/aggregator.pluto` appeared in `list_modules`. The other 8 source modules were missing.

The response showed `files_found: 18, files_loaded: 13, files_failed: 5`, but only 7 unique modules in `list_modules` (aggregator + 6 test files).

## Assessment

It appears `load_project` only loads:
1. Files that can be reached by following imports from the test files
2. Files that have no compilation errors

The missing 8 src modules exist and compile successfully (verified with `load_module` on collector.pluto), but weren't loaded by `load_project`.

This makes `load_project` unreliable for exploring a codebase - you can't assume all .pluto files will be loaded.

## Suggestion

`load_project` should load ALL `.pluto` files in the directory tree, regardless of whether they're imported or not. Files with compilation errors can be included in the `errors` array but should still attempt to be loaded.

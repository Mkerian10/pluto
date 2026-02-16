# load_project Returns Duplicate Module Paths

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** load_project
**Type:** bug

## What Happened

Called `load_project` on the Meridian project. The response included a `modules` array that listed the same module path multiple times:

```json
{
  "modules": [
    { "path": ".../src/aggregator.pluto", "declarations": 218 },
    { "path": ".../tests/aggregator/aggregator_test.pluto", "declarations": 218 },
    { "path": ".../src/aggregator.pluto", "declarations": 218 },
    { "path": ".../tests/codec/codec_test.pluto", "declarations": 261 },
    { "path": ".../src/aggregator.pluto", "declarations": 218 },
    ...
  ]
}
```

`src/aggregator.pluto` appears **7 times** in the list with identical declaration counts.

## Assessment

This appears to happen when test files import the same source module. Each test file that imports `src.aggregator` causes the module to be added to the list again instead of being deduplicated.

The `list_modules` tool correctly shows no duplicates, so the bug is specific to the `load_project` response format.

This makes the project summary confusing and misleading - it looks like the project has way more modules than it actually does.

## Suggestion

Deduplicate the `modules` array before returning it. Each unique module path should appear exactly once.

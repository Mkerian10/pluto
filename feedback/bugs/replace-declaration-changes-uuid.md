# replace_declaration Generates New UUID Instead of Preserving

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** replace_declaration
**Type:** bug

## What Happened

Created a function with `add_declaration`:
```json
{ "uuid": "2699173e-d808-45fa-80fe-ceeda9415f57", "name": "hello" }
```

Then called `replace_declaration` to modify the function body. Response:
```json
{ "uuid": "21cce495-a891-4cb4-9ac8-f72a7eacd7d8", "name": "hello" }
```

The UUID changed from `2699173e...` to `21cce495...`.

## Assessment

According to the MCP docs:

> **`replace_declaration`**: Replace an existing declaration by name. The replacement must be the same kind (e.g., a function can only be replaced with a function). **The UUID is preserved.**

But it's not being preserved - it's generating a new UUID.

This breaks:
- Cross-reference stability (callers/usages now point to a stale UUID)
- Call graph analysis
- External tracking of declarations
- The entire "UUIDs are stable" promise

## Suggestion

Fix `replace_declaration` to actually preserve the UUID. The new declaration should inherit the UUID from the old one it's replacing.

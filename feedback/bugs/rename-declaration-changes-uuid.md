# rename_declaration Generates New UUID Instead of Preserving

**Project:** meridian
**Date:** 2026-02-15
**Tool(s):** rename_declaration
**Type:** bug

## What Happened

Had a function with UUID `21cce495-a891-4cb4-9ac8-f72a7eacd7d8` named `hello`.

Called `rename_declaration` to change it from `hello` to `greet`. Response:
```json
{
  "old_name": "hello",
  "new_name": "greet",
  "uuid": "4ed071cf-465c-4dcb-829e-be1bac689d22"
}
```

The UUID changed from `21cce495...` to `4ed071cf...`.

## Assessment

The MCP docs state that UUIDs are the stable identity of declarations and survive renames. The best practices doc says:

> **UUIDs are the stable identity of declarations.** They survive renames, body changes, and reformatting.

But `rename_declaration` is generating a new UUID, which defeats the entire purpose of having stable identifiers.

This is even worse than the `replace_declaration` bug because renaming is explicitly supposed to preserve identity while updating references.

## Suggestion

Fix `rename_declaration` to preserve the UUID. The renamed declaration should keep the exact same UUID it had before.

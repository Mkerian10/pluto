# Import Pollution Resurfaces After rename_declaration

**Date:** 2026-02-16
**Tool(s):** load_module, list_declarations, rename_declaration
**Type:** bug

**RESOLVED (PR #271):** root cause was `reload_module` reloading in sibling-merging mode while `load_module` loads standalone — the reload swapped loading modes, so every transitive import appeared. `ModuleMetadata` now records the original load options and `reload_module` reproduces them exactly. (The rename_declaration tool itself was removed when MCP went read-only.)

## What Happened

After the MCP configuration fix and restart:

1. **Initial state (GOOD):** `list_declarations` on aggregator.pluto correctly showed only 6 functions
2. **After rename_declaration:** Called `rename_declaration` twice (sort_floats → sort_floats_renamed → sort_floats)
3. **Reloaded module:** Called `load_module` again on the same file
4. **Import pollution returned (BAD):** Now showing 81 functions + 23 classes + 1 enum + 1 trait = 106 declarations

## Expected Behavior

`list_declarations` should consistently show only the file's own declarations (6 functions), not transitive imports.

## Actual Behavior

- Pre-restart: Always showed all imports (Bug #3)
- Post-restart, fresh load: Correctly showed 6 functions ✓
- Post-restart, after rename operations: Back to showing all 106 declarations ✗

## Assessment

The import pollution bug is **intermittent** or **triggered by write operations**. Either:
1. `rename_declaration` corrupts internal state, causing subsequent loads to include imports
2. Or there's a caching issue where renamed modules pick up stale import data

## Impact

- Can't trust `list_declarations` after any write operation
- Makes iterative development (rename, check, refine) impossible with MCP tools
- Agents would need to restart MCP server after every edit to get clean declaration lists

## Reproduction

```
1. load_module on fresh aggregator.pluto → shows 6 declarations ✓
2. rename_declaration (any function, any names)
3. load_module again → shows 106 declarations ✗
```

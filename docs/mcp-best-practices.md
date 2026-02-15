# MCP Best Practices

Guidelines for AI agents and developers working with the Pluto MCP server. Following these practices leads to faster, more reliable, and safer editing workflows.

---

## 1. Project Initialization

**Always start with `load_project`**, not `load_module`.

`load_project` does three things that `load_module` doesn't:

1. **Loads all `.pluto` files** in the directory tree, so cross-module searches work immediately
2. **Builds the dependency graph**, detecting circular imports before you hit them during compilation
3. **Establishes the project root** for path safety validation — write tools are blocked without it

```
// Good: loads everything, enables writes, enables cross-module queries
load_project({ path: "/path/to/project", stdlib: "/path/to/pluto/stdlib" })

// Bad: only loads one file, write tools will fail (no project root)
load_module({ path: "/path/to/project/main.pluto" })
```

**Pass the stdlib path** if your project uses any `std.*` imports. Without it, `check`, `compile`, and `run` will fail on imports like `import std.collections`.

---

## 2. Explore Before Editing

**Read the code before changing it.** The MCP server provides structured inspection tools that are faster and more reliable than reading raw source.

**Recommended exploration flow:**

```
1. list_modules           → See what files exist
2. list_declarations      → See what's in each file
3. get_declaration        → Deep-inspect specific items
4. callers_of / usages_of → Understand usage patterns
5. call_graph             → Trace execution paths
```

**Use `find_declaration` for cross-module search.** If you know a function name but not which file it's in, `find_declaration` searches all loaded modules.

**Use `get_declaration` by UUID, not name.** Names can be ambiguous (a function and a class can share a name). UUIDs are unique. After the first lookup by name, use the returned UUID for subsequent queries.

**Use `usages_of` instead of individual xref tools.** `usages_of` combines `callers_of`, `constructors_of`, `enum_usages_of`, and `raise_sites_of` into a single call. Use the individual tools only when you need to filter by usage kind.

---

## 3. Incremental Validation

**Check after every edit**, not just at the end.

```
add_declaration(...)   → check(...)   → fix errors
replace_declaration(...) → check(...) → fix errors
delete_declaration(...)  → check(...) → fix dangling refs
```

Type errors compound. A bad type change in one declaration can cascade into dozens of errors elsewhere. Catching it immediately means fixing one thing instead of untangling ten.

**Use `check` for validation, not `compile`.** `check` runs the type checker without generating a binary — it's faster and gives the same diagnostics. Only use `compile` when you actually need the binary.

**`check` always reads from disk.** Unlike inspection tools (which use the cached AST), `check` re-reads the file from disk. Since write tools write to disk and update the cache simultaneously, this is seamless — but be aware that `check` will pick up any external edits too.

---

## 4. Choosing the Right Write Tool

The MCP server has six write tools, each for a specific operation. Using the right one preserves UUIDs and avoids unnecessary churn.

| Task | Tool | UUID Behavior |
|------|------|---------------|
| Add a new function/class/enum/etc. | `add_declaration` | New UUID assigned |
| Modify a function body or signature | `replace_declaration` | UUID preserved |
| Remove a declaration | `delete_declaration` | Reports dangling refs |
| Change a declaration's name | `rename_declaration` | UUID preserved, intra-file refs updated |
| Add a method to a class | `add_method` | New UUID for method, class UUID unchanged |
| Add a field to a class | `add_field` | New UUID for field, class UUID unchanged |

**Prefer `replace_declaration` over delete + add.** Replacing preserves the UUID, which keeps cross-references, call graphs, and derived analysis data stable. Delete + add generates a new UUID and breaks any external references.

**Prefer `add_method` / `add_field` over `replace_declaration` for the whole class.** Adding a method or field is surgical — it touches only the new addition. Replacing the entire class re-parses everything and risks formatting changes.

**Always check `dangling_refs` after `delete_declaration`.** The response tells you exactly which declarations in the same file reference the deleted one. Fix these before moving on.

---

## 5. File Watching and Staleness

**Use `module_status` to detect external changes.** If another developer (or another tool) modifies a `.pluto` file while your session is active, the in-memory cache becomes stale. `module_status` compares file modification times against load times to detect this.

```
module_status()
// Response: [{ path: "...", is_stale: true, loaded_at: 1707933600 }]
```

**Use `reload_module` to refresh stale modules.** After detecting staleness, reload the specific module. You don't need to reload the entire project.

```
// Good: targeted reload
reload_module({ path: "/path/to/changed_file.pluto" })

// Overkill: reloads everything (slow for large projects)
load_project({ path: "/path/to/project" })
```

**Write tools auto-refresh the cache.** When you use `add_declaration`, `replace_declaration`, etc., the MCP server writes to disk and updates the cache atomically. You don't need to call `reload_module` after your own writes.

**Check staleness before complex operations.** Before a multi-step refactor, run `module_status` to make sure you're working with current data. Editing a stale module risks conflicting with external changes.

---

## 6. Documentation-First Development

**Use `docs` and `stdlib_docs` before writing code.** The MCP server embeds the full Pluto language reference and stdlib API documentation. Query it to get correct syntax, available types, and function signatures.

```
// Check how errors work before implementing error handling
docs({ topic: "errors" })

// Check what std.collections provides before importing it
stdlib_docs({ module: "collections" })
```

**Available language topics:** `types`, `operators`, `statements`, `declarations`, `strings`, `errors`, `closures`, `generics`, `modules`, `contracts`, `concurrency`, `gotchas`

**Available stdlib modules:** `strings`, `math`, `fs`, `json`, `http`, `net`, `socket`, `collections`, `io`, `random`, `time`

**Check the `gotchas` topic.** It covers common pitfalls like newline sensitivity, empty struct literals, and generic syntax ambiguities.

---

## 7. Cross-Module Workflow

When making changes that affect multiple files, follow this order:

### Adding a new declaration used by other files

```
1. add_declaration in the defining module  → check
2. Verify it compiles: check({ path: "defining_module.pluto" })
3. Update importing modules: replace_declaration in each caller → check each
4. Final validation: run or test on the entry point
```

### Renaming a declaration

`rename_declaration` updates references within the same file but **not** across modules. Handle cross-module updates manually:

```
1. usages_of({ uuid: "..." })  → find all cross-module callers
2. rename_declaration in the defining file
3. replace_declaration in each calling file to use the new name
4. check each modified file
```

### Deleting a declaration

```
1. usages_of({ uuid: "..." })  → find all usages
2. Update or remove all cross-module callers first
3. delete_declaration  → check dangling_refs for intra-file issues
4. check the file
```

**Always update callers before deleting the callee.** If you delete first, the callers will have dangling references and `check` will report errors you already knew about.

---

## 8. Safety Practices

### Path Safety

**All write operations are sandboxed** within the project root established by `load_project`. The server validates every write path to prevent:

- Writing outside the project directory
- Directory traversal attacks (`../../../etc/passwd`)
- Symlink escapes

If a write tool returns a path safety error, it means the target path resolves outside the project root. This is a hard block — you cannot bypass it.

### UUID Stability

**UUIDs are the stable identity of declarations.** They survive renames, body changes, and reformatting. Treat UUIDs as the primary key for referencing declarations across tools.

UUIDs are lost when:
- A declaration is deleted and re-added (new UUID assigned)
- A `.pluto` file is recreated from scratch (all UUIDs regenerated)
- The file is synced from `.pt` without UUID hints

Use `pretty_print` with `include_uuid_hints: true` to preserve UUIDs in text format:

```
pretty_print({ path: "...", include_uuid_hints: true })
// Output:
// // @uuid: a1b2c3d4-...
// fn add(a: int, b: int) int {
//     return a + b
// }
```

### Source Preservation

**Write tools modify source text on disk.** They don't just update the AST — they write the actual `.pluto` source file. This means:

- Formatting may change (the pretty-printer normalizes whitespace)
- Comments outside declarations may be affected
- The file is always valid Pluto after a write (parsed before writing)

---

## 9. Performance

### Minimize Redundant Loads

- **Don't call `load_project` repeatedly.** Once loaded, modules stay cached. Use `reload_module` for individual refreshes.
- **Don't call `load_module` on files already loaded by `load_project`.** Check `list_modules` first.

### Use Targeted Queries

- **Use `list_declarations` with `kind` filter** instead of listing everything and filtering client-side.
- **Use `callers_of` instead of `call_graph`** when you only need direct callers (depth 1).
- **Use `get_source` with byte ranges** when you need a specific section of a large file, not the whole thing.

### Batch Logically

If you need to make multiple related changes to the same file, consider:
- Using `replace_declaration` for each changed declaration (rather than replacing the whole file)
- Running `check` once after all changes, not after each one, if the changes are interdependent
- Using `add_declaration` with multiple declarations in a single source string:

```
add_declaration({
  path: "...",
  source: "fn foo() int { return 1 }\n\nfn bar() int { return 2 }"
})
// Both declarations added in one call
```

### Call Graph Depth

- Default depth is 5, maximum is 20
- Start with the default. Only increase depth if you need deeper analysis
- Deep call graphs on large projects can be slow — use `direction: "callers"` to trace upward from a specific function rather than expanding the full callees tree

---

## 10. Common Anti-Patterns

### Writing raw source files directly

Don't bypass the MCP server by writing `.pluto` files directly (e.g., via filesystem tools). The MCP server:
- Parses the source to validate it's correct Pluto
- Assigns UUIDs to new declarations
- Updates the module cache
- Validates path safety

Writing directly skips all of these. If you must write directly, call `reload_module` afterward.

### Ignoring check results

Don't assume a write succeeded just because the tool returned success. The write tool confirms the source was valid Pluto and was written to disk, but it doesn't type-check. A syntactically valid declaration can still have type errors. Always `check` after writes.

### Over-relying on names instead of UUIDs

Names can be ambiguous. Multiple declarations can have the same name (a function `Point` and a class `Point`). After the first lookup, switch to UUIDs:

```
// First lookup: by name
find_declaration({ name: "Point" })
// Returns: [{ uuid: "abc...", kind: "function" }, { uuid: "def...", kind: "class" }]

// Subsequent lookups: by UUID
get_declaration({ path: "...", uuid: "def..." })
```

### Loading the whole project to check one file

`check` reads from disk and doesn't require the file to be loaded. If you just need to validate a single file, you can `check` it directly without loading the entire project (as long as you don't need write tools).

### Forgetting stdlib path

If compilation fails with "unknown module" errors for `std.*` imports, you forgot to pass the `stdlib` parameter. Pass it to `load_project`, `check`, `compile`, `run`, and `test`:

```
check({ path: "...", stdlib: "/path/to/pluto/stdlib" })
```

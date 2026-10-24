# MCP Best Practices

Guidelines for AI agents and developers working with the Pluto MCP server. The server is read-only: it inspects, type-checks, compiles, runs and tests Pluto code, but never writes a source file. Edits happen in your editor or through the `pluto-sdk` crate, and the server is told about them with `reload_module`.

---

## 1. Project Initialization

**Always start with `load_project`**, not `load_module`.

`load_project` does three things that `load_module` doesn't:

1. **Loads all `.pluto` files** in the directory tree, so cross-module searches work immediately
2. **Builds the dependency graph**, detecting circular imports before you hit them during compilation
3. **Establishes the project root**, which later `load_module` / `reload_module` paths are validated against

```
// Good: loads everything, enables cross-module queries
load_project({ path: "/path/to/project", stdlib: "/path/to/pluto/stdlib" })

// Bad: only loads one file; cross-module queries see nothing else
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
edit the file → reload_module(...) → check(...) → fix errors
```

Type errors compound. A bad type change in one declaration can cascade into dozens of errors elsewhere. Catching it immediately means fixing one thing instead of untangling ten.

**Use `check` for validation, not `compile`.** `check` runs the type checker without generating a binary — it's faster and gives the same diagnostics. Only use `compile` when you actually need the binary.

**`check` always reads from disk.** Unlike inspection tools (which use the cached AST), `check` re-reads the file from disk, so it sees your edit even before `reload_module`. The inspection tools do not — reload before querying declarations you just changed.

---

## 4. Editing Happens Outside the Server

The server has no write tools. Make changes in one of two ways:

- **Edit the `.pt` source in your editor.** This is the normal path. Afterwards call `reload_module` on that file (or `load_project` again) so inspection tools see the new declarations.
- **Edit programmatically with `pluto-sdk`.** `Module::open` (or `Module::from_source_file`) → `module.edit()` gives a `ModuleEditor` with `add_from_source`, `replace_from_source`, `delete`, `rename`, `add_method_from_source` and `add_field`; `commit()` yields the new `Module`, and `save` writes it. Declaration UUIDs survive edits made this way; `delete` reports any references it leaves dangling.

**Prefer editing one declaration over rewriting a file.** Whether in an editor or the SDK, touching only the declaration you mean to change keeps the rest of the file's UUIDs, cross-references and derived analysis stable.

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

**Your own edits are external edits.** The server cannot see a file change until you `reload_module` it (or `load_project` again). Make that call right after saving, before any inspection tool.

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

**Stdlib modules with embedded docs:** `strings`, `math`, `fs`, `json`, `http`, `net`, `socket`, `collections`, `io`, `random`, `time`, `compress`, `base64`, `env`, `hash`, `log`, `path`, `regex`, `uuid` (the other modules are documented in the book and in their source)

**Check the `gotchas` topic.** It covers common pitfalls like newline sensitivity, empty struct literals, and generic syntax ambiguities.

---

## 7. Cross-Module Workflow

When making changes that affect multiple files, follow this order:

### Adding a new declaration used by other files

```
1. Add the declaration to the defining module → reload_module → check
2. Update each importing module → reload_module → check each
3. Final validation: run or test on the entry point
```

### Renaming a declaration

A rename touches every module that refers to the name. Let the server find them first:

```
1. usages_of({ uuid: "..." })  → find all callers, across modules
2. Rename in the defining file
3. Update each calling file
4. reload_module + check each modified file
```

### Deleting a declaration

```
1. usages_of({ uuid: "..." })  → find all usages
2. Update or remove all callers first
3. Delete the declaration
4. reload_module + check the file
```

**Always update callers before deleting the callee.** If you delete first, the callers will have dangling references and `check` will report errors you already knew about.

---

## 8. Safety Practices

### Path Safety

Paths passed to `load_module` and `reload_module` are canonicalised and checked against the project root established by `load_project`; a path that resolves outside it (directory traversal, symlink escape) is rejected.

### UUID Stability

**UUIDs are the stable identity of declarations.** They survive renames, body changes, and reformatting. Treat UUIDs as the primary key for referencing declarations across tools.

UUIDs are lost when:
- A declaration is deleted and re-added (new UUID assigned)
- A `.pluto` file is recreated from scratch (all UUIDs regenerated)
- A `.pt` file is re-parsed without UUID hints (`pluto sync` matches by hint, then by name)

Use `pretty_print` with `include_uuid_hints: true` to preserve UUIDs in text format:

```
pretty_print({ path: "...", include_uuid_hints: true })
// Output:
// // @uuid: a1b2c3d4-...
// fn add(a: int, b: int) int {
//     return a + b
// }
```

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

If you need to make multiple related changes to the same file:
- Make them all, then `reload_module` once and `check` once, if the changes are interdependent
- Keep each change scoped to the declaration it concerns rather than rewriting the file

### Call Graph Depth

- Default depth is 5, maximum is 20
- Start with the default. Only increase depth if you need deeper analysis
- Deep call graphs on large projects can be slow — use `direction: "callers"` to trace upward from a specific function rather than expanding the full callees tree

---

## 10. Common Anti-Patterns

### Querying before reloading

The inspection tools answer from the cached AST. After editing a file — by hand or through `pluto-sdk` — call `reload_module` before `list_declarations`, `get_declaration`, `usages_of`, or `call_graph`, or they will describe the old version.

### Ignoring check results

A file that parses is not a file that type-checks. Always `check` after an edit; a syntactically valid declaration can still break callers elsewhere.

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

`check` reads from disk and doesn't require the file to be loaded. If you just need to validate a single file, you can `check` it directly without loading the entire project.

### Forgetting stdlib path

If compilation fails with "unknown module" errors for `std.*` imports, you forgot to pass the `stdlib` parameter. Pass it to `load_project`, `check`, `compile`, `run`, and `test`:

```
check({ path: "...", stdlib: "/path/to/pluto/stdlib" })
```

# Pluto MCP Server

The Pluto MCP (Model Context Protocol) server enables AI agents like Claude Code to work with Pluto projects natively. Instead of reading and writing raw text, the agent uses structured tools to explore, modify, compile, and test Pluto code.

## Setup

### Building

```bash
cd mcp/
cargo build --release
```

The binary is `target/release/pluto-mcp`.

### Claude Code Configuration

Add to your Claude Code MCP settings (`~/.claude/mcp-servers.json` or project-level `.claude/mcp-servers.json`):

```json
{
  "mcpServers": {
    "pluto": {
      "command": "/path/to/pluto-mcp",
      "args": []
    }
  }
}
```

The server communicates over stdio (stdin/stdout) using the MCP protocol.

### Stdlib Path

If your project uses standard library modules (`import std.fs`, `import std.collections`, etc.), you need to provide the stdlib path when loading projects or compiling. The stdlib lives at `stdlib/` in the Pluto repository.

---

## Quick Start

A typical workflow looks like this:

```
1. load_project    → Load all .pluto files in the project
2. list_modules    → See what modules are loaded
3. find_declaration / get_declaration → Explore the code
4. add_declaration / replace_declaration → Make changes
5. check           → Validate with the type checker
6. run / test      → Execute and verify
```

---

## Tools Reference

### Project & Module Loading

#### `load_project`

Scan a project directory and load all `.pluto` source files. This is the recommended way to start a session — it loads everything, builds the dependency graph, detects circular imports, and sets up the project root for path safety.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to the project root directory |
| `stdlib` | string | no | Path to stdlib root (needed for `std.*` imports) |

**Returns:** `ProjectSummary`
```json
{
  "project_root": "/path/to/project",
  "files_found": 5,
  "files_loaded": 5,
  "files_failed": 0,
  "modules": [
    { "path": "/path/to/main.pluto", "declarations": 3 },
    { "path": "/path/to/math.pluto", "declarations": 5 }
  ],
  "errors": [],
  "dependency_graph": {
    "module_count": 2,
    "has_circular_imports": false,
    "modules": [
      { "path": "/path/to/main.pluto", "name": "main", "imports": ["math"] }
    ]
  }
}
```

#### `load_module`

Load a single `.pluto` source file or PLTO binary file. Use `load_project` instead when working with multi-file projects.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to a `.pluto` source or binary file |
| `stdlib` | string | no | Path to stdlib root |

**Returns:** `ModuleSummary`
```json
{
  "path": "/path/to/file.pluto",
  "summary": {
    "functions": 3,
    "classes": 1,
    "enums": 0,
    "traits": 1,
    "errors": 0,
    "app": 0
  },
  "declarations": [
    { "name": "add", "uuid": "a1b2c3d4-...", "kind": "function" },
    { "name": "Point", "uuid": "e5f6a7b8-...", "kind": "class" }
  ]
}
```

#### `list_modules`

List all currently loaded modules with declaration counts.

**Parameters:** None

**Returns:** Array of `ModuleListEntry`
```json
[
  {
    "path": "/path/to/main.pluto",
    "summary": { "functions": 2, "classes": 0, "enums": 1, "traits": 0, "errors": 0, "app": 1 }
  }
]
```

---

### Querying Declarations

#### `list_declarations`

List all declarations in a loaded module, optionally filtered by kind.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the loaded module |
| `kind` | string | no | Filter: `function`, `class`, `enum`, `trait`, `error`, `app` |

**Returns:** Array of `DeclSummary` (name, uuid, kind)

#### `get_declaration`

Deep-inspect a specific declaration. Returns full details including parameters, fields, methods, error sets, contracts, and pretty-printed source.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the loaded module |
| `uuid` | string | no | UUID of the declaration |
| `name` | string | no | Name of the declaration (may be ambiguous) |

At least one of `uuid` or `name` must be provided. If `name` matches multiple declarations, the server returns a disambiguation list.

**Returns:** One of `FunctionDetail`, `ClassDetail`, `EnumDetail`, `TraitDetail`, `ErrorDeclDetail`, or `AppDetail` depending on the declaration kind.

**Example (function):**
```json
{
  "name": "add",
  "uuid": "a1b2c3d4-...",
  "kind": "function",
  "params": [
    { "name": "a", "type": "int", "is_mut": false },
    { "name": "b", "type": "int", "is_mut": false }
  ],
  "return_type": "int",
  "is_fallible": false,
  "error_set": [],
  "signature": {
    "param_types": ["Int"],
    "return_type": "Int",
    "is_fallible": false
  },
  "source": "fn add(a: int, b: int) int {\n    return a + b\n}\n"
}
```

**Example (class):**
```json
{
  "name": "Point",
  "uuid": "e5f6a7b8-...",
  "kind": "class",
  "fields": [
    { "name": "x", "type": "float", "uuid": "..." },
    { "name": "y", "type": "float", "uuid": "..." }
  ],
  "methods": [
    { "name": "distance", "uuid": "..." }
  ],
  "bracket_deps": [],
  "impl_traits": ["Printable"],
  "invariant_count": 0,
  "resolved_fields": [
    { "name": "x", "type": "Float", "is_injected": false }
  ],
  "lifecycle": "Transient",
  "source": "class Point {\n    x: float\n    y: float\n    ...\n}\n"
}
```

#### `find_declaration`

Search for a declaration by name across all loaded modules.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `name` | string | yes | Name to search for |
| `kind` | string | no | Filter: `function`, `class`, `enum`, `trait`, `error`, `app` |

**Returns:** Array of `CrossModuleMatch`
```json
[
  {
    "module_path": "/path/to/math.pluto",
    "uuid": "a1b2c3d4-...",
    "name": "add",
    "kind": "function"
  }
]
```

#### `get_source`

Get raw source text from a loaded module, optionally at a specific byte range.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the loaded module |
| `start` | int | no | Start byte offset (default: 0) |
| `end` | int | no | End byte offset (default: end of file) |

**Returns:** Raw source text as string.

#### `error_set`

Get error handling information for a specific function.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the loaded module |
| `uuid` | string | yes | UUID of the function |

**Returns:**
```json
{
  "function_name": "parse_config",
  "is_fallible": true,
  "error_set": [
    { "name": "FileError", "uuid": "..." },
    { "name": "ParseError", "uuid": "..." }
  ]
}
```

---

### Cross-References

All cross-reference tools search across **all loaded modules**, not just a single file. Load your project with `load_project` first.

#### `callers_of`

Find all call sites that invoke a function.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the function |

**Returns:** Array of `CrossModuleXrefSiteInfo`
```json
[
  {
    "module_path": "/path/to/main.pluto",
    "function_name": "process",
    "function_uuid": "...",
    "span": { "start": 142, "end": 156, "start_line": 8, "start_col": 5, "end_line": 8, "end_col": 19 }
  }
]
```

#### `constructors_of`

Find all sites where a class is constructed (struct literal syntax).

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the class |

**Returns:** Array of `CrossModuleXrefSiteInfo`

#### `enum_usages_of`

Find all sites where an enum or its variants are referenced.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the enum |

**Returns:** Array of `CrossModuleXrefSiteInfo`

#### `raise_sites_of`

Find all sites where an error type is raised.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the error declaration |

**Returns:** Array of `CrossModuleXrefSiteInfo`

#### `usages_of`

Unified cross-reference search. Returns all usages of a declaration in a single query: calls, constructions, enum variant references, and raise sites.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the declaration |

**Returns:** Array of `UnifiedXrefInfo`
```json
[
  {
    "module_path": "/path/to/main.pluto",
    "usage_kind": "call",
    "function_name": "process",
    "function_uuid": "...",
    "span": { "start": 142, "end": 156 }
  },
  {
    "module_path": "/path/to/utils.pluto",
    "usage_kind": "construct",
    "function_name": "create_default",
    "function_uuid": "...",
    "span": { "start": 88, "end": 120 }
  }
]
```

The `usage_kind` field is one of: `"call"`, `"construct"`, `"enum_variant"`, `"raise"`.

#### `call_graph`

Build a call graph starting from a function, traversing across module boundaries.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `uuid` | string | yes | UUID of the root function |
| `max_depth` | int | no | Maximum traversal depth (default: 5, max: 20) |
| `direction` | string | no | `"callers"` or `"callees"` (default: `"callees"`) |

**Returns:**
```json
{
  "root_uuid": "...",
  "root_name": "process_request",
  "direction": "callers",
  "max_depth": 5,
  "nodes": [
    {
      "uuid": "...",
      "name": "process_request",
      "module_path": "/path/to/handlers.pluto",
      "depth": 0,
      "children": [
        {
          "uuid": "...",
          "name": "handle_api",
          "module_path": "/path/to/api.pluto",
          "is_cycle": null
        }
      ]
    }
  ]
}
```

Cycle detection is built in — recursive or mutually-recursive calls are flagged with `"is_cycle": true` instead of recursing infinitely.

---

### Write Tools

Write tools modify source files directly on disk. All paths are validated against the project root (set by `load_project`) to prevent directory traversal. The in-memory module cache is updated to reflect changes.

#### `add_declaration`

Add one or more top-level declarations to a file. Creates the file if it doesn't exist.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path to the `.pluto` source file |
| `source` | string | yes | Pluto source code for the declaration(s) |

**Returns:** Array of `AddDeclResult`
```json
[
  { "uuid": "newly-generated-uuid", "name": "my_function", "kind": "function" }
]
```

**Example source:**
```
fn greet(name: string) string {
    return "hello {name}"
}
```

#### `replace_declaration`

Replace an existing declaration by name. The replacement must be the same kind (e.g., a function can only be replaced with a function). The UUID is preserved.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the `.pluto` source file |
| `name` | string | yes | Name of the declaration to replace |
| `source` | string | yes | Pluto source code for the replacement |

**Returns:** `ReplaceDeclResult`
```json
{ "uuid": "preserved-uuid", "name": "greet", "kind": "function" }
```

#### `delete_declaration`

Delete a declaration by name. Reports any dangling references that result from the deletion.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the `.pluto` source file |
| `name` | string | yes | Name of the declaration to delete |

**Returns:** `DeleteDeclResult`
```json
{
  "deleted_source": "fn old_func() { ... }",
  "dangling_refs": [
    { "kind": "call", "name": "old_func", "span": { "start": 200, "end": 212 } }
  ]
}
```

Always check `dangling_refs` — if non-empty, other declarations in the file reference the deleted one and need to be updated.

#### `rename_declaration`

Rename a declaration. Updates all intra-file references automatically.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the `.pluto` source file |
| `old_name` | string | yes | Current name |
| `new_name` | string | yes | New name |

**Returns:** `RenameDeclResult`
```json
{ "old_name": "calculate", "new_name": "compute", "uuid": "preserved-uuid" }
```

Note: Cross-module references (in other files) are **not** automatically updated. Use `callers_of` to find external callers and update them manually.

#### `add_method`

Add a method to an existing class.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the `.pluto` source file |
| `class_name` | string | yes | Name of the class |
| `source` | string | yes | Method source (must include `self` or `mut self` parameter) |

**Returns:** `AddMethodResult`
```json
{ "uuid": "new-method-uuid", "name": "area" }
```

**Example source:**
```
fn area(self) float {
    return self.width * self.height
}
```

#### `add_field`

Add a field to an existing class.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the `.pluto` source file |
| `class_name` | string | yes | Name of the class |
| `field_name` | string | yes | Name of the new field |
| `field_type` | string | yes | Type string (e.g., `"int"`, `"string"`, `"[float]"`) |

**Returns:** `AddFieldResult`
```json
{ "uuid": "new-field-uuid" }
```

---

### Compile & Execute

#### `check`

Type-check a source file without producing a binary. Returns structured diagnostics.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to the `.pluto` source file |
| `stdlib` | string | no | Path to stdlib root |

**Returns:** `CheckResult`
```json
{
  "success": false,
  "path": "/path/to/file.pluto",
  "errors": [
    {
      "severity": "error",
      "kind": "type",
      "message": "type mismatch: expected int, got string",
      "span": { "start": 45, "end": 52, "start_line": 3, "start_col": 12, "end_line": 3, "end_col": 19 },
      "path": null
    }
  ],
  "warnings": []
}
```

Diagnostic `kind` values: `"syntax"`, `"type"`, `"codegen"`, `"link"`, `"manifest"`.

#### `compile`

Compile a source file to a native binary.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to the `.pluto` source file |
| `output` | string | no | Output path for the binary (defaults to a temp file) |
| `stdlib` | string | no | Path to stdlib root |

**Returns:** `CompileResult`
```json
{
  "success": true,
  "path": "/path/to/file.pluto",
  "output": "/tmp/pluto_binary_abc123",
  "errors": []
}
```

#### `run`

Compile and execute a source file, capturing output.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to the `.pluto` source file |
| `stdlib` | string | no | Path to stdlib root |
| `timeout_ms` | int | no | Execution timeout in milliseconds (default: 10000, max: 60000) |
| `cwd` | string | no | Working directory for execution (default: source file's parent) |

**Returns:** `RunResult`
```json
{
  "success": true,
  "path": "/path/to/file.pluto",
  "compilation_errors": [],
  "stdout": "hello world\n",
  "stderr": "",
  "exit_code": 0,
  "timed_out": false
}
```

If the program exceeds the timeout, `timed_out` is `true` and the process is killed.

#### `test`

Compile in test mode and run the test runner.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Absolute path to the `.pluto` source file containing tests |
| `stdlib` | string | no | Path to stdlib root |
| `timeout_ms` | int | no | Execution timeout (default: 30000, max: 60000) |
| `cwd` | string | no | Working directory |

**Returns:** `TestResult` (same shape as `RunResult`)

Test output appears in `stdout`. A non-zero `exit_code` indicates test failures.

---

### Format & Sync

#### `pretty_print`

Pretty-print a loaded module or a specific declaration as Pluto source text.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the loaded module |
| `uuid` | string | no | UUID of a specific declaration (omit for entire module) |
| `include_uuid_hints` | bool | no | Include `// @uuid: ...` comments (default: false) |

**Returns:** Formatted Pluto source text as string.

With `include_uuid_hints: true`, each declaration is preceded by a UUID comment:
```
// @uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890
fn add(a: int, b: int) int {
    return a + b
}
```

These hints enable stable UUID matching when syncing human edits back via `sync_pt`.

#### `sync_pt`

Sync human edits from a `.pt` text file back to a `.pluto` binary file, preserving UUIDs where declarations match by name.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `pt_path` | string | yes | Path to the `.pt` text file |
| `pluto_path` | string | no | Path to the `.pluto` binary (defaults to same name with `.pluto` extension) |

**Returns:** `SyncResultInfo`
```json
{
  "added": ["new_function"],
  "removed": ["old_function"],
  "modified": ["updated_function"],
  "unchanged": 3
}
```

---

### Module Management

#### `reload_module`

Discard the cached version of a module and reload it from disk. Use this after external edits.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | yes | Path of the module to reload |

**Returns:** `ReloadResult`
```json
{ "path": "/path/to/file.pluto", "reloaded": true, "message": "Reloaded successfully" }
```

#### `module_status`

Show the status of all loaded modules, including whether they've been modified on disk since loading.

**Parameters:** None

**Returns:** Array of `ModuleStatusEntry`
```json
[
  { "path": "/path/to/main.pluto", "is_stale": false, "loaded_at": "2026-02-14T20:00:00Z" },
  { "path": "/path/to/math.pluto", "is_stale": true, "loaded_at": "2026-02-14T19:30:00Z" }
]
```

A module is `is_stale: true` when its file on disk has been modified more recently than when it was loaded.

---

### Documentation

#### `docs`

Get Pluto language reference documentation.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `topic` | string | no | Specific topic (omit for full reference) |

**Available topics:** `types`, `operators`, `statements`, `declarations`, `strings`, `errors`, `closures`, `generics`, `modules`, `contracts`, `concurrency`, `gotchas`

**Returns:** Markdown-formatted documentation.

#### `stdlib_docs`

Get standard library module documentation.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `module` | string | no | Module name (omit to list all modules) |

**Available modules:** `strings`, `math`, `fs`, `json`, `http`, `net`, `socket`, `collections`, `io`, `random`, `time`

**Returns:** Markdown-formatted API documentation with function signatures and descriptions.

---

## Data Types Reference

### SpanInfo

Source location information. Byte offsets are always present; line/column numbers are included when source text is available.

```json
{
  "start": 45,
  "end": 52,
  "start_line": 3,
  "start_col": 12,
  "end_line": 3,
  "end_col": 19
}
```

Line and column numbers are 1-based.

### DiagnosticInfo

Compiler diagnostic (error or warning).

```json
{
  "severity": "error",
  "kind": "type",
  "message": "type mismatch: expected int, got string",
  "span": { "start": 45, "end": 52, "start_line": 3, "start_col": 12 },
  "path": "/path/to/file.pluto"
}
```

- `severity`: `"error"` or `"warning"`
- `kind`: `"syntax"`, `"type"`, `"codegen"`, `"link"`, or `"manifest"`
- `path`: Present for multi-file errors (e.g., sibling file parse failures)

### DeclSummary

Lightweight declaration reference.

```json
{ "name": "add", "uuid": "a1b2c3d4-...", "kind": "function" }
```

Kind values: `"function"`, `"class"`, `"enum"`, `"trait"`, `"error"`, `"app"`

### DanglingRefInfo

Reference left dangling after a deletion.

```json
{ "kind": "call", "name": "deleted_func", "span": { "start": 200, "end": 212 } }
```

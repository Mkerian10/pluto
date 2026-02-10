# RFC: AI-Native Pipeline

> **Status:** RFC (design phase)
>
> **Depends on:** [AI-Native Representation](ai-native-representation.md) (PLTO format, stable UUIDs, derived data model)

## Design Decisions

These are settled and should not be revisited:

| Decision | Choice | Rationale |
|---|---|---|
| **Agent format** | Binary via SDK | Agents interact with `.pluto` binary files through MCP tools backed by the SDK. No raw file I/O. Structured, UUID-stable edits. |
| **Primary consumer** | Claude Code (MCP) | The MCP server is designed for Claude Code first. The SDK is the implementation layer, not a separate product. |
| **MCP scope** | Project-aware | The server understands the full project: multiple modules, imports, cross-file queries. Not a single-file tool. |
| **Binary default** | `.pluto` binary is canonical | New projects use `.pluto` binary as the source of truth from day one. `.pt` text files are generated views for human review, not the canonical format. |

## Motivation

The [AI-Native Representation RFC](ai-native-representation.md) established the vision: `.pluto` becomes a binary canonical format with stable UUIDs and derived analysis data, `.pt` files provide human-readable views, and AI agents interact through an SDK.

That RFC defines the *data model*. This RFC defines the *operational surface* — the concrete tools, APIs, and workflows that let an AI agent read, understand, modify, compile, and test Pluto programs. The centerpiece is an **MCP (Model Context Protocol) server** that exposes Pluto's compiler and SDK as tools an AI agent can call.

The primary consumer is **Claude Code**. The MCP server is designed to be Claude Code's interface to Pluto — the tools are shaped for how an LLM agent reasons about code (structured queries, text-in source writes, explicit validation steps). Other MCP-compatible agents work too, but Claude Code is the design target.

### Why MCP?

MCP (Model Context Protocol) is the standard protocol for AI agent ↔ tool communication. It provides:

- **Tool discovery.** The agent learns what tools are available at connection time.
- **Structured I/O.** Tools accept JSON parameters and return JSON results.
- **Stdio transport.** The server runs as a subprocess, communicating over stdin/stdout. No networking, no auth, no deployment complexity.
- **Ecosystem.** Claude Code speaks MCP natively. One config line and the agent can use all Pluto tools.

### What this enables

Claude Code connected to `pluto-mcp` can:

1. **Explore** a Pluto codebase — list modules, browse declarations, read source
2. **Understand** the code — query types, error sets, call graphs, cross-references
3. **Modify** the code — add functions, edit bodies, rename declarations, delete dead code
4. **Validate** changes — compile, type-check, run tests, read diagnostics
5. **All without leaving the MCP protocol** — no file I/O, no shell commands, no parsing text output

## Architecture

```
┌──────────────┐     stdio (JSON-RPC)     ┌──────────────┐
│   AI Agent   │ ◄──────────────────────► │  pluto-mcp   │
│ (Claude, etc)│                          │  (MCP server) │
└──────────────┘                          └──────┬───────┘
                                                 │
                                    ┌────────────┴────────────┐
                                    │                         │
                              ┌─────▼─────┐            ┌──────▼──────┐
                              │ plutoc-sdk │            │   plutoc    │
                              │ (read/write│            │ (compiler)  │
                              │  .pluto)   │            │             │
                              └─────┬──────┘            └──────┬──────┘
                                    │                          │
                              ┌─────▼──────────────────────────▼──────┐
                              │         .pluto files (PLTO)           │
                              │    [UUIDs] [AST] [Source] [Derived]   │
                              └───────────────────────────────────────┘
                                                 │
                                          plutoc generate-pt
                                                 │
                                          ┌──────▼──────┐
                                          │  .pt files   │
                                          │ (human view) │
                                          └─────────────┘
```

**Three layers:**

| Layer | Crate | Role |
|---|---|---|
| **pluto-mcp** | `mcp/` | MCP server binary. Translates MCP tool calls into SDK/compiler operations. Manages project state. |
| **plutoc-sdk** | `sdk/` | Rust library for reading and writing `.pluto` files. UUID-aware, transactional writes. |
| **plutoc** | `.` (root) | The compiler. Lex, parse, type-check, codegen, link. Also provides `analyze_file()` for enriching with derived data. |

## MCP Server (`pluto-mcp`)

### Transport

Stdio (stdin/stdout). The server is launched as a subprocess by the AI agent's host (Claude Code, IDE, etc.):

```json
{
  "mcpServers": {
    "pluto": {
      "command": "pluto-mcp",
      "args": ["--project", "/path/to/my-project"]
    }
  }
}
```

The `--project` flag sets the working directory. All file paths in tool calls are relative to this root.

### State management

The server is **project-aware** — it understands the full project structure, not just individual files.

- **Project graph.** At startup, the server scans the `--project` directory for `.pluto` files and builds a project-level index: module names, import relationships, public declarations. This enables cross-module queries (`find_declaration` across all modules, `callers_of` across module boundaries).
- **Loaded modules.** Each `.pluto` file that has been accessed is held as a `Module` (from the SDK) with its AST index and derived data. Modules are loaded on first access (lazy), not all at startup.
- **Dirty tracking.** Modules modified through write tools are marked dirty. The agent must explicitly call `save` to flush changes to disk.
- **Module resolution.** The server understands Pluto's import system — when querying cross-references or type-checking, it resolves `import math` to the correct module file(s).

This design means the agent can ask "who calls this function?" and get answers from across the entire project, not just the current file.

## Tools

Tools are organized into five groups: **explore**, **query**, **write**, **compile**, and **format**.

### Explore tools

These tools let the agent discover and browse the project structure.

#### `list_modules`

List all `.pluto` files in the project.

```
Parameters: (none)
Returns: [{ path: string, name: string }]
```

#### `list_declarations`

List all declarations in a module, with optional filtering.

```
Parameters:
  path: string              # module file path (e.g., "math/vectors.pluto")
  kind?: string             # filter: "function" | "class" | "enum" | "trait" | "error" | "app"
  visibility?: string       # filter: "pub" | "private" | "all" (default: "all")

Returns: [{
  id: string,               # UUID
  name: string,
  kind: string,
  is_pub: bool,
  signature?: string,       # human-readable signature (e.g., "fn add(a: int, b: int) int")
}]
```

#### `get_source`

Get the source text of a module or a specific declaration.

```
Parameters:
  path: string              # module file path
  id?: string               # UUID — if provided, returns only that declaration's source

Returns: { source: string }
```

When `id` is provided, the server pretty-prints just that declaration. When omitted, returns the full module source.

### Query tools

These tools let the agent understand the code's semantics — types, errors, relationships.

#### `get_declaration`

Get full details for a declaration by UUID.

```
Parameters:
  id: string                # UUID

Returns: {
  id: string,
  name: string,
  kind: string,
  is_pub: bool,
  module: string,           # which module this lives in
  source: string,           # pretty-printed source text
  signature?: {             # for functions/methods only
    params: [{ name: string, type: string }],
    return_type: string,
    is_fallible: bool,
  },
  error_set?: [{ id: string, name: string }],  # errors this function can raise
  fields?: [{ id: string, name: string, type: string }],  # for classes/enums/errors
  methods?: [{ id: string, name: string, signature: string }],  # for classes
  variants?: [{ id: string, name: string, fields: [...] }],  # for enums
  impl_traits?: [string],   # for classes
  bracket_deps?: [{ name: string, type: string }],  # for classes with DI
}
```

#### `find_declaration`

Find declarations by name across the project.

```
Parameters:
  name: string              # declaration name (exact match)
  kind?: string             # optional kind filter

Returns: [{ id: string, name: string, kind: string, module: string }]
```

#### `callers_of`

Find all call sites that invoke a given function or method.

```
Parameters:
  id: string                # UUID of the target function

Returns: [{
  caller_id: string,        # UUID of the calling function
  caller_name: string,
  module: string,
  source_snippet: string,   # the call expression source text
  span: { start: int, end: int },
}]
```

#### `constructors_of`

Find all sites where a class is constructed via struct literal.

```
Parameters:
  id: string                # UUID of the target class

Returns: [{
  function_id: string,
  function_name: string,
  module: string,
  source_snippet: string,
  span: { start: int, end: int },
}]
```

#### `usages_of`

Find all usages of an enum, error, trait, or class across the project.

```
Parameters:
  id: string                # UUID of the target declaration

Returns: [{
  function_id: string,
  function_name: string,
  module: string,
  usage_kind: string,       # "call" | "construct" | "enum_variant" | "raise" | "impl" | "type_ref"
  source_snippet: string,
  span: { start: int, end: int },
}]
```

This is a unified cross-reference query — it dispatches to `callers_of`, `constructors_of`, `enum_usages_of`, and `raise_sites_of` internally and merges the results.

#### `error_set`

Get the complete error set for a function (all errors it can raise, transitively).

```
Parameters:
  id: string                # UUID of the function

Returns: [{
  id: string,               # error declaration UUID
  name: string,
}]
```

#### `call_graph`

Get the call graph rooted at a function (who it calls, and who they call, etc.).

```
Parameters:
  id: string                # UUID of the root function
  depth?: int               # max depth (default: 3)

Returns: {
  root: string,
  calls: [{
    caller_id: string,
    callee_id: string,
    callee_name: string,
  }]
}
```

### Write tools

These tools let the agent modify the code. All writes are **text-based** — the agent provides Pluto source text, and the server handles parsing, UUID assignment, and AST manipulation internally.

Write tools operate on the in-memory module state. Changes are not flushed to disk until `save` is called.

#### `add_declaration`

Add a new top-level declaration to a module.

```
Parameters:
  path: string              # module file path
  source: string            # Pluto source text for the declaration
                            # e.g., "pub fn cross_product(a: Vector, b: Vector) Vector {\n    ...\n}"
  position?: string         # "before:<uuid>" | "after:<uuid>" | "end" (default: "end")

Returns: {
  id: string,               # UUID assigned to the new declaration
  diagnostics: [...]         # any parse or type errors (may be empty)
}
```

The server:
1. Parses the source text as a standalone declaration
2. Assigns a fresh UUID (and UUIDs to all nested fields, params, methods, variants)
3. Inserts into the module's AST at the requested position
4. Runs type-checking to validate (non-blocking — returns diagnostics)

#### `replace_declaration`

Replace an existing declaration entirely. Preserves the UUID.

```
Parameters:
  id: string                # UUID of the declaration to replace
  source: string            # new Pluto source text

Returns: {
  id: string,               # same UUID (preserved)
  diagnostics: [...]
}
```

The server parses the new source, transplants it into the AST in place of the old declaration, preserving the top-level UUID. Nested UUIDs (params, fields, methods) are matched by name where possible and reassigned otherwise.

#### `rename_declaration`

Rename a declaration. UUID stays the same; all references update automatically.

```
Parameters:
  id: string                # UUID of the declaration
  new_name: string          # new name

Returns: {
  id: string,
  old_name: string,
  new_name: string,
  references_updated: int,  # count of call sites / type refs updated
}
```

Because cross-references are stored by UUID, renaming is cheap — only the name field and the `.pt` view change.

#### `delete_declaration`

Remove a declaration from a module.

```
Parameters:
  id: string                # UUID

Returns: {
  deleted: string,          # name of the deleted declaration
  dangling_references: [{   # references that now point to nothing
    function_id: string,
    function_name: string,
    module: string,
  }]
}
```

#### `add_method`

Add a method to an existing class.

```
Parameters:
  class_id: string          # UUID of the target class
  source: string            # method source (e.g., "fn magnitude(self) float { ... }")
  position?: string         # "before:<uuid>" | "after:<uuid>" | "end"

Returns: { id: string, diagnostics: [...] }
```

#### `add_field`

Add a field to a class or error declaration.

```
Parameters:
  target_id: string         # UUID of the class or error
  name: string              # field name
  type: string              # type expression (e.g., "int", "[string]", "Map<string, int>")
  injected?: bool           # bracket dep? (default: false)

Returns: { id: string, diagnostics: [...] }
```

#### `save`

Flush all dirty modules to disk. Serializes each modified module to PLTO format.

```
Parameters:
  path?: string             # specific module to save, or all dirty modules if omitted

Returns: {
  saved: [string],          # list of paths saved
}
```

### Compile tools

These tools invoke the compiler pipeline.

#### `check`

Type-check the project (no codegen). Returns diagnostics.

```
Parameters:
  path?: string             # specific file, or whole project if omitted

Returns: {
  success: bool,
  diagnostics: [{
    severity: string,       # "error" | "warning"
    message: string,
    file: string,
    span: { start: int, end: int },
    line: int,
    column: int,
  }]
}
```

#### `compile`

Compile the project to a binary.

```
Parameters:
  path: string              # entry file
  output?: string           # output path (default: derived from entry file name)

Returns: {
  success: bool,
  output_path?: string,
  diagnostics: [...]
}
```

#### `run`

Compile and run the project. Captures stdout/stderr.

```
Parameters:
  path: string              # entry file
  args?: [string]           # command-line arguments
  timeout?: int             # max runtime in seconds (default: 30)
  stdin?: string            # stdin input

Returns: {
  success: bool,
  exit_code: int,
  stdout: string,
  stderr: string,
  diagnostics: [...]        # compile errors, if compilation failed
}
```

#### `test`

Compile and run tests.

```
Parameters:
  path: string              # entry file
  filter?: string           # test name filter (substring match)

Returns: {
  success: bool,
  passed: int,
  failed: int,
  results: [{
    name: string,
    passed: bool,
    output?: string,        # stdout from failed tests
  }],
  diagnostics: [...]
}
```

#### `analyze`

Run the full front-end pipeline (lex → parse → module resolve → type-check) and refresh derived data (error sets, resolved signatures). Does **not** generate code.

```
Parameters:
  path: string              # entry file

Returns: {
  success: bool,
  diagnostics: [...],
  stats: {
    functions_analyzed: int,
    error_sets_computed: int,
  }
}
```

After `analyze`, queries like `error_set` and `get_declaration` return fresh derived data.

### Format tools

These tools handle the `.pt` ↔ `.pluto` conversion.

#### `generate_pt`

Generate human-readable `.pt` files from `.pluto` files.

```
Parameters:
  path?: string             # specific module, or all modules if omitted

Returns: {
  generated: [{ pluto_path: string, pt_path: string }]
}
```

#### `sync_pt`

Sync edits from a `.pt` file back into its `.pluto` file. Matches declarations by name/signature to preserve UUIDs.

```
Parameters:
  path: string              # .pt file path

Returns: {
  synced: string,           # .pluto file path
  new_declarations: int,    # count of new declarations (assigned fresh UUIDs)
  updated_declarations: int,
  deleted_declarations: int,
  diagnostics: [...]
}
```

#### `pretty_print`

Pretty-print a declaration or module without writing to disk.

```
Parameters:
  id?: string               # UUID — pretty-print a single declaration
  path?: string             # module — pretty-print the whole module

Returns: { source: string }
```

## SDK Write API (`plutoc-sdk`)

> **Status:** Implemented. See `sdk/src/editor.rs`.

The MCP server's write tools delegate to the SDK's `ModuleEditor` API.

### Principles

- **Text-in, AST-out.** The agent provides Pluto source text. The SDK parses it, assigns UUIDs, and produces AST nodes. This is the right abstraction for LLM agents — they're good at generating text, not at constructing AST structures.
- **Edit-then-commit.** `Module::from_source()` creates an edit-friendly module (parse-only, no compiler transforms). `Module::edit()` consumes the module and returns a `ModuleEditor`. Mutations accumulate in memory until `editor.commit()` produces a new `Module` with regenerated source, re-resolved cross-references, and a rebuilt index.
- **UUID-preserving.** Top-level UUIDs are preserved across replacement. Nested UUIDs (params, fields, methods, variants) are matched by name where possible, with fresh UUIDs assigned to genuinely new items.
- **No type-checking on commit.** The editor operates at the parse level only. Type-checking is a separate step via the MCP `check`/`analyze` tools. This keeps edits fast and allows partially-valid intermediate states during multi-step modifications.

### API

```rust
use plutoc_sdk::{Module, ModuleEditor};

// Create an edit-friendly module from source text
let module = Module::from_source(source)?;
let mut editor = module.edit();

// Add a new function from source text — returns UUID
let fn_id = editor.add_from_source(
    "pub fn cross_product(a: Vector, b: Vector) Vector {\n    ...\n}",
)?;

// Replace a function body (by UUID) — preserves top-level UUID
editor.replace_from_source(
    dot_product_id,
    "pub fn dot_product(a: Vector, b: Vector) float {\n    return a.x * b.x + a.y * b.y + a.z * b.z\n}",
)?;

// Rename — updates declaration name + all reference sites in the AST
editor.rename(dot_product_id, "inner_product")?;

// Delete — returns deleted source + best-effort dangling reference diagnostics
let result = editor.delete(some_old_function_id)?;
println!("Dangling refs: {:?}", result.dangling);

// Add a method to a class (parsed via class-wrapper technique)
editor.add_method_from_source(
    vector_class_id,
    "fn magnitude(self) float {\n    return sqrt(self.x * self.x + self.y * self.y)\n}",
)?;

// Add a field to a class
editor.add_field(vector_class_id, "z", "float")?;

// Commit — pretty-print → re-resolve xrefs → rebuild index → new Module
let module = editor.commit();
assert!(module.source().contains("cross_product"));
```

### Operations

| Method | What it does |
|---|---|
| `add_from_source(source) -> Uuid` | Parse source as a single top-level declaration, append to Program |
| `replace_from_source(id, source)` | Parse replacement (must be same kind), swap into AST, preserve top-level UUID |
| `delete(id) -> DeleteResult` | Remove declaration, return its source + dangling reference list |
| `rename(id, new_name)` | Update declaration name + all reference sites in the AST |
| `add_method_from_source(class_id, source) -> Uuid` | Parse method via class-wrapper technique, add to class |
| `add_field(class_id, name, ty) -> Uuid` | Add a field to a class |
| `commit() -> Module` | Pretty-print → re-resolve xrefs → rebuild index → return new Module |

### Implementation details

**Parsing snippets:** The parser only exposes `parse_program()`, so snippet parsing works by lexing the source and running `parse_program()` with the current program's enum names as context (via `Parser::new_with_enum_context()`). Exactly one declaration must be present.

**Method parsing:** Methods require class body context (for `self` params). The editor wraps method source in a temporary class (`"class __Tmp {\n" + source + "\n}"`), parses it, and extracts the method.

**UUID transplanting on replace:** The replacement declaration gets the old top-level UUID. Nested items (params, fields, methods, variants) are matched by name — matching items preserve their old UUID, genuinely new items keep their fresh UUID.

**Rename walker:** Walks the entire AST updating reference sites. Uses UUID matching for expressions with `target_id`/`enum_id`/`error_id` fields, and name matching for `TypeExpr` nodes. Scoped to top-level declarations only (method rename not supported in v1 due to missing `MethodCall` target_id).

**Dangling references:** `delete()` scans the AST for expressions that still reference the deleted UUID via `target_id`/`enum_id`/`error_id`. This is best-effort — accurate immediately after `from_source()` or `commit()`, but may be stale between mutations.

## CLI Commands

The compiler gets new subcommands that mirror the MCP tools:

```bash
plutoc analyze <file>                # Run front-end + refresh derived data in .pluto
plutoc generate-pt [<file>]          # Generate .pt from .pluto (all files if omitted)
plutoc sync <file.pt>                # Sync .pt edits back to .pluto
plutoc emit-ast <file> -o <out>      # Serialize parsed AST to .plto binary (already exists)
```

These are thin wrappers around the same SDK functions the MCP server uses. `analyze` calls `analyze_file()` + `serialize_program()`. `generate-pt` calls `pretty_print()`. `sync` parses `.pt`, diffs against `.pluto`, and writes updated `.pluto`.

## Agent Workflow

### Example: "Add a cross_product function to math.vectors"

```
Agent                                    pluto-mcp
  │                                          │
  ├─ list_declarations(path: "math/vectors.pluto")
  │                                          │
  │◄─ [{id: "e5f6...", name: "dot_product", kind: "function", ...},
  │     {id: "c9d0...", name: "Vector", kind: "class", ...}]
  │                                          │
  ├─ get_declaration(id: "c9d0...")  // understand Vector's fields
  │                                          │
  │◄─ {name: "Vector", fields: [{name: "x", type: "float"}, ...], ...}
  │                                          │
  ├─ get_declaration(id: "e5f6...")  // understand dot_product's pattern
  │                                          │
  │◄─ {source: "pub fn dot_product(a: Vector, b: Vector) float { ... }"}
  │                                          │
  ├─ add_declaration(
  │     path: "math/vectors.pluto",
  │     source: "pub fn cross_product(a: Vector, b: Vector) Vector {\n    return Vector { x: a.y * b.z - a.z * b.y, y: a.z * b.x - a.x * b.z, z: a.x * b.y - a.y * b.x }\n}",
  │     position: "after:e5f6..."
  │   )
  │                                          │
  │◄─ {id: "a1b2...", diagnostics: []}      // clean — no errors
  │                                          │
  ├─ check()                                 // full project type-check
  │                                          │
  │◄─ {success: true, diagnostics: []}
  │                                          │
  ├─ save()                                  // flush to disk
  │                                          │
  │◄─ {saved: ["math/vectors.pluto"]}
  │                                          │
  ├─ generate_pt(path: "math/vectors.pluto")
  │                                          │
  │◄─ {generated: [{pluto_path: "math/vectors.pluto", pt_path: "math/vectors.pt"}]}
```

### Example: "Rename dot_product to inner_product everywhere"

```
Agent                                    pluto-mcp
  │                                          │
  ├─ find_declaration(name: "dot_product")
  │                                          │
  │◄─ [{id: "e5f6...", module: "math/vectors.pluto"}]
  │                                          │
  ├─ callers_of(id: "e5f6...")
  │                                          │
  │◄─ [{caller_name: "compute_angle", module: "math/angles.pluto", ...},
  │     {caller_name: "main", module: "main.pluto", ...}]
  │                                          │
  ├─ rename_declaration(id: "e5f6...", new_name: "inner_product")
  │                                          │
  │◄─ {id: "e5f6...", old_name: "dot_product", new_name: "inner_product", references_updated: 2}
  │                                          │
  ├─ check()
  │◄─ {success: true}
  │                                          │
  ├─ save()
  │◄─ {saved: ["math/vectors.pluto", "math/angles.pluto", "main.pluto"]}
```

UUID-based references mean the rename is a metadata update — no AST rewriting needed for call sites. The pretty-printer handles rendering the new name.

### Example: "Fix the bug in the login function"

```
Agent                                    pluto-mcp
  │                                          │
  ├─ find_declaration(name: "login")
  │◄─ [{id: "x1y2...", module: "auth.pluto", kind: "function"}]
  │                                          │
  ├─ get_declaration(id: "x1y2...")
  │◄─ {source: "fn login(username: string, password: string) User { ... }", error_set: [...], ...}
  │                                          │
  ├─ callers_of(id: "x1y2...")  // understand usage context
  │◄─ [...]
  │                                          │
  ├─ error_set(id: "x1y2...")   // what errors can this raise?
  │◄─ [{name: "AuthError"}, {name: "NotFound"}]
  │                                          │
  ├─ replace_declaration(id: "x1y2...", source: "fn login(...) User { /* fixed body */ }")
  │◄─ {id: "x1y2...", diagnostics: []}
  │                                          │
  ├─ test(path: "auth.pluto", filter: "login")
  │◄─ {success: true, passed: 3, failed: 0}
  │                                          │
  ├─ save()
```

## Current Implementation Status

> **Last updated:** 2026-02-10

### Foundation (complete)

These components are built, tested, and working:

| Component | Location | Status | Tests | Notes |
|---|---|---|---|---|
| **PLTO binary format** | `src/binary.rs` | Done | 11 | Schema v2, deterministic round-trip, versioned header |
| **UUIDs on AST** | `src/parser/ast.rs` | Done | — | All 10 declaration types: fn, class, enum, variant, trait, trait method, field, param, error, app |
| **Cross-references** | `src/xref.rs` | Done | 10+ | `target_id` on Call, StructLit, ClosureCreate, EnumUnit, EnumData, Raise, MatchArm |
| **Derived data** | `src/derived.rs` | Done | 3 | Error sets + resolved signatures, built from TypeEnv after typeck |
| **Pretty printer** | `src/pretty.rs` | Done | 100+ | 2200 lines, all language constructs, round-trip stable. Per-declaration pretty-print functions for `ModuleEditor`. |
| **SDK read API** | `sdk/src/` | Done | 14 | `Module`, `DeclRef`, UUID/name lookup, xref queries, derived data access |
| **SDK write API** | `sdk/src/editor.rs` | Done | 27 | `ModuleEditor` with add, replace, delete, rename, add_method, add_field, commit |
| **CLI: `emit-ast`** | `src/main.rs` | Done | — | Parse + analyze source → PLTO binary |
| **CLI: `generate-pt`** | `src/main.rs` | Done | — | PLTO binary → pretty-printed `.pt` text |

### MCP server read-only (partially complete)

**Branch:** `mcp-server` (worktree at `../pluto-mcp-server`)

**Crate:** `mcp/` using `rmcp` 0.14 with stdio transport

Six tools implemented and working:

| Tool | What it does |
|---|---|
| `load_module` | Load a `.pluto` file (binary or text, auto-detected), cache in memory |
| `list_declarations` | List declarations in a loaded module, optional kind filter |
| `inspect` | Deep inspection of a declaration by UUID or name (params, fields, methods, source, etc.) |
| `xrefs` | Cross-reference queries: callers, constructors, enum usages, raise sites |
| `errors` | Error set and fallibility for a function |
| `source` | Get source text (full module or byte range) |

**What's missing for Phase 1 completion:**
- **Project awareness.** Currently module-at-a-time (`load_module` by explicit path). Needs: startup project scan, `list_modules` across project, cross-module `find_declaration`, cross-module xref queries, import resolution.
- **Tool name alignment.** Current tool names (`inspect`, `xrefs`, `errors`, `source`) don't match RFC spec (`get_declaration`, `callers_of`, `error_set`, `get_source`). Needs reconciliation.
- **`find_declaration` across project.** Currently can only search within a loaded module.
- **`call_graph` tool.** Not implemented.

### Remaining work

Listed in dependency order. Items higher on the list unblock items lower.

#### 1. Project-awareness in MCP server (Phase 1 gap)

**Priority:** High — makes the read-only server actually useful for Claude Code.

**Work:**
- `ProjectIndex` struct that scans project directory, knows all `.pluto` files
- `list_modules` tool that returns all modules without loading each
- `find_declaration` that searches across all modules
- `callers_of` / `usages_of` that work across module boundaries
- Import resolution (understand `import math` → which file(s))
- `--project` CLI flag on server startup

**Effort:** Medium. Wraps multiple `Module` instances with a project-level index.

#### 2. Compile tools in MCP server (Phase 2)

**Priority:** High — lets the agent validate its work.

**Work:**
- `check` — invoke typeck, return structured diagnostics
- `compile` — full pipeline to native binary
- `run` — compile + execute, capture stdout/stderr with timeout
- `test` — compile + run test framework, parse results
- `analyze` — front-end only, refresh derived data in loaded modules

**Effort:** Medium. The compiler already does all of this (`compile_file()`, `analyze_file()`, test runner). These are MCP wrappers with structured output.

#### ~~3. SDK write API (Phase 3)~~ — DONE

Implemented in `sdk/src/editor.rs` with 27 tests. See "SDK Write API" section above for full details.

**Implemented:** `ModuleEditor` with `add_from_source`, `replace_from_source`, `delete`, `rename`, `add_method_from_source`, `add_field`, `commit`. Parse-only pipeline (`parse_for_editing()`) with no compiler transforms. UUID transplanting on replace. Rename walker with UUID+name matching. Dangling reference detection on delete.

#### 4. MCP write tools (Phase 3)

**Priority:** High — exposes the SDK write API to Claude Code.

**Work:**
- `add_declaration`, `replace_declaration`, `rename_declaration`, `delete_declaration`
- `add_method`, `add_field`
- `save` — flush dirty modules to disk
- Dirty tracking in server state

**Effort:** Small. Thin MCP wrappers over the now-complete `ModuleEditor`.

#### 5. CLI `sync` command (Phase 4)

**Priority:** Medium — the human ↔ AI bridge.

**Work:**
- Parse `.pt` text file into AST
- Load corresponding `.pluto` binary
- Diff the two ASTs: match declarations by name/signature
- Matched declarations: preserve UUID from `.pluto`, take body from `.pt`
- New declarations in `.pt`: assign fresh UUIDs
- Declarations missing from `.pt`: delete from `.pluto`
- Write updated `.pluto` binary

**Effort:** Medium-large. AST diffing and UUID merging is the tricky part.

#### 6. MCP format tools (Phase 4)

**Priority:** Medium.

**Work:**
- `generate_pt` — MCP wrapper over existing CLI command
- `sync_pt` — MCP wrapper over #5
- `pretty_print` — MCP wrapper over existing `pretty_print()`

**Effort:** Small (mostly wiring, but `sync_pt` depends on #5).

#### 7. Cross-module write operations (Phase 5)

**Priority:** Low — future work.

**Work:**
- `move_declaration` across modules (with xref updates)
- Rename that updates references in other modules
- Multi-module transaction model
- Project-wide refactoring tools

**Effort:** Large. Needs multi-module transaction semantics.

## Implementation Phases

## Open Questions

### Resolved

- [x] **Source vs. binary input.** Binary `.pluto` (PLTO) is the canonical format from day one. Text `.pluto` files are supported as a fallback during migration (Phase 1 auto-detects format). A `plutoc migrate` command converts text `.pluto` → binary `.pluto`.
- [x] **Primary consumer.** Claude Code via MCP. SDK is the implementation layer, not a separate product surface.
- [x] **Project scope.** Project-aware — the MCP server understands the full project, resolves imports, supports cross-module queries.

### Open

- [ ] **Concurrent access.** Single MCP server per project for now. If needed later: file locking or advisory lock on project root.
- [ ] **Undo/rollback.** Should the server support undo beyond the transaction boundary? Per-tool undo? Snapshot-based? Or rely on git?
- [ ] **Module creation.** How does an agent create a new module (new `.pluto` file)? A `create_module` tool? What about directory modules?
- [ ] **Partial type-checking.** Can `check` validate a single modified module without re-checking the entire project? How does this interact with whole-program analysis?
- [ ] **Binary stability.** Bincode is the current serialization format. Is it stable enough across Rust compiler versions for committed `.pluto` files? Should we switch to protobuf/flatbuffers before stabilizing?
- [ ] **Large project performance.** How does the MCP server scale to projects with hundreds of modules? The project-level index (built at startup) helps, but incremental updates need design.
- [ ] **MCP resources vs. tools.** Should module source and declaration listings be exposed as MCP resources (passive, subscribable) in addition to tools? Resources enable real-time watching of file changes.
- [ ] **Authentication/sandboxing.** The MCP server has full file system access. Should there be a read-only mode? Project boundary enforcement?

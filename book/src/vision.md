# AI-Native Development

Whole-program compilation means the Pluto compiler builds a complete understanding of your program: resolved types for every expression, inferred error sets for every function, the full call graph, the dependency wiring topology, cross-references between every declaration and its usage sites.

Most compilers throw this knowledge away after code generation. Pluto keeps it.

The compiler's internal representation — the AST, the type information, the analysis results — is a first-class artifact: serializable, queryable, and designed for AI agents to read and write directly. The compiler sees your entire program and understands it deeply. AI agents get access to that same understanding through a structured API.

This chapter covers what is implemented today and where the design is headed.

## What Exists Now

### Binary AST Format (PLTO)

Pluto source files can be compiled to a binary AST format that preserves the full semantic graph:

```
$ plutoc emit-ast main.pt -o main.pluto       # text -> binary AST
$ plutoc generate-pt main.pluto                # binary AST -> text (stdout)
$ plutoc generate-pt main.pluto -o main.pt     # binary AST -> text file
$ plutoc sync main.pt --output main.pluto      # merge text edits back, preserving UUIDs
```

The binary format (`.pluto`) contains the complete parsed AST, the original source text, and derived analysis data. The text format (`.pt`) is standard Pluto syntax that humans read and edit.

### Stable UUIDs

Every declaration in a Pluto program has a stable UUID: functions, classes, enums, traits, methods, fields, parameters, error declarations, enum variants, and app declarations. UUIDs are assigned at creation time and survive renames, moves, and refactors.

This is the foundation. When an AI agent renames a function, the UUID stays the same. Every call site, struct literal, enum usage, and raise site that references that declaration tracks the UUID, not the name string. The name is display text. The UUID is identity.

### Cross-References

The compiler resolves and stores cross-references by UUID:

- **Call sites** -- which functions call which other functions, tracked by the callee's UUID
- **Struct literals** -- where each class is constructed, tracked by the class UUID
- **Enum usages** -- where each enum variant is used, tracked by enum and variant UUIDs
- **Raise sites** -- where each error type is raised, tracked by the error UUID

These are not heuristics or text search. They are exact, compiler-resolved references that survive across renames.

### The SDK (`plutoc-sdk`)

The SDK is a Rust crate for reading and writing Pluto programs as structured data. It is what AI agents (and any tooling) use to interact with Pluto code without parsing text.

**Loading a module:**

```rust
use plutoc_sdk::Module;

// From binary format
let module = Module::from_bytes(&bytes)?;

// From source text (parse without full compilation)
let module = Module::from_source(source)?;

// From a .pluto source file (full front-end pipeline with analysis)
let module = Module::from_source_file("main.pluto")?;
```

**Querying:**

```rust
// List all functions, classes, enums, traits, errors
for f in module.functions() {
    println!("{}: {}", f.name(), f.id());
}

// Look up by name or UUID
let decls = module.find("process_order");
let decl = module.get(some_uuid);

// Cross-references
let callers = module.callers_of(function_uuid);
let constructors = module.constructors_of(class_uuid);
let usages = module.enum_usages_of(enum_uuid);
let raises = module.raise_sites_of(error_uuid);
```

**Editing:**

```rust
let mut editor = module.edit();

// Add declarations
let id = editor.add_from_source("fn greet() {\n    print(\"hello\")\n}\n")?;

// Replace (preserves UUID)
editor.replace_from_source(existing_id, "fn greet() {\n    print(\"goodbye\")\n}\n")?;

// Rename (updates all references)
editor.rename(function_id, "hello")?;

// Add methods and fields to classes
editor.add_method_from_source(class_id, "fn area(self) float {\n    return self.w * self.h\n}\n")?;
editor.add_field(class_id, "z", "float")?;

// Delete (reports dangling references)
let result = editor.delete(function_id)?;
for d in &result.dangling {
    eprintln!("warning: dangling reference to '{}' at {:?}", d.name, d.span);
}

// Commit: re-serializes source, rebuilds index and xrefs
let module = editor.commit();
```

The key property: edits are UUID-stable. Renaming a function updates the declaration's name and every reference site in a single operation. The UUID never changes.

### The MCP Server

The MCP (Model Context Protocol) server exposes the SDK's capabilities as structured tools that AI agents call directly. This is how Claude, and other LLM-based agents, interact with Pluto codebases.

**Read tools:**

| Tool | Purpose |
|------|---------|
| `load_module` | Load and analyze a `.pluto` source file or binary |
| `list_declarations` | List all declarations, optionally filtered by kind |
| `inspect` | Deep inspection of a declaration: params, types, error sets, methods, fields |
| `xrefs` | Cross-reference queries: callers, constructors, enum usages, raise sites |
| `errors` | Error handling info for a function: fallibility and error set |
| `source` | Get source text, optionally at a byte range |

**Write tools:**

| Tool | Purpose |
|------|---------|
| `add_declaration` | Add a function, class, enum, trait, or error |
| `replace_declaration` | Replace a declaration's body (preserves UUID) |
| `delete_declaration` | Remove a declaration (reports dangling refs) |
| `rename_declaration` | Rename with automatic reference updates |
| `add_method` | Add a method to an existing class |
| `add_field` | Add a field to an existing class |

**Build tools:**

| Tool | Purpose |
|------|---------|
| `check` | Type-check without producing a binary |
| `compile` | Compile to a native binary |
| `run` | Compile and execute |
| `test` | Compile in test mode and run tests |

The agent workflow is: load a module, query its structure, understand types and error sets, make a targeted edit, validate with the type checker, iterate.

## The Vision

The tools above are implemented and working. The larger vision takes them further.

### .pluto as Canonical Source

Today, `.pt` text files are the source of truth and `.pluto` binary files are derived. The plan is to invert this: `.pluto` binary becomes canonical, `.pt` text files become generated views for human review.

In this model, a git repository contains `.pluto` binaries (committed, source of truth) and `.pt` text files (generated, for review and code search). The `plutoc sync` command already exists to merge human `.pt` edits back into `.pluto` files, preserving UUIDs where declarations match by name.

### Derived Data

The compiler produces valuable analysis that is currently discarded after each build: resolved types for every expression, inferred error sets, the full call graph, DI wiring topology. The derived data layer stores this analysis in the `.pluto` binary so that agents and tools can query it without re-compiling.

The SDK already surfaces derived data (resolved signatures, class info, enum info, error info) when a module is loaded via `from_source_file`. The plan is to extend this to the full set of compiler analysis.

### The Feedback Loop

The core idea is that the compiler and AI agents form a feedback loop:

1. **Agent writes code** -- using the SDK or MCP server, the agent makes structured edits with stable UUIDs.
2. **Compiler analyzes** -- type checking, error inference, call graph construction. The analysis is stored, not discarded.
3. **Agent reads analysis** -- the agent queries resolved types, error sets, cross-references. It learns what the compiler knows about the code.
4. **Agent writes better code** -- informed by the compiler's analysis, the agent makes its next edit with full knowledge of the type system, error propagation, and dependency graph.

This is not AI replacing developers. It is the compiler's understanding of your program being accessible to every tool in the chain -- AI agents, editors, code review systems, refactoring tools -- through a structured interface with stable identities.

The text file is a view. The semantic graph is the source.

# AI-Native Representation

> **Status:** Active implementation
>
> **Implemented today:** PLTO v3 binary container with freshness tracking, `emit-ast`, `generate-pt`, `sync`, Rust SDK/module APIs.
>
> **In progress:** `pluto analyze` command (separate implementation).
>
> **Still in design:** `pluto init`, signature-only libraries, cross-project UUIDs, incremental analysis.

## Motivation

Pluto is designed for distributed backend systems with whole-program compilation. Today, the compiler reads `.pluto` text files, parses them into an AST, runs type checking and analysis, generates code, and discards all intermediate knowledge. Every compilation starts from scratch.

This is wasteful. The compiler's analysis — type inference, error set computation, DI wiring, call graphs — is expensive and valuable. And as AI agents become the primary authors of Pluto code, the text-file interface becomes a bottleneck: AI writes text, compiler parses it (losing structural information the AI already had), and the analysis results are thrown away rather than fed back to the AI.

The AI-native representation addresses this by making the compiler's internal representation the canonical source format, with human-readable text as a derived view.

## Overview

The system has two file formats:

| | `.pluto` (canonical) | `.pt` (human-readable) |
|---|---|---|
| **Format** | Binary (protobuf/flatbuffers) | Text (Pluto syntax) |
| **Written by** | AI agents (via SDK), compiler (derived data) | `pluto generate-pt` (from .pluto) |
| **Read by** | Compiler, AI agents, SDK | Humans, code review tools, editors |
| **Contains** | Full semantic graph (AST + types + errors + call graph) | Source code in Pluto syntax |
| **Committed to git** | Yes (source of truth) | Yes (derived view for human review) |
| **On conflict** | Always wins | Regenerated from .pluto |

### File Layout

The filesystem structure mirrors today's module system:

```
my-project/
  main.pluto          # binary, canonical
  main.pt             # text, derived view
  math/
    vectors.pluto
    vectors.pt
    matrices.pluto
    matrices.pt
  auth.pluto           # single-file module
  auth.pt
```

`import math` resolves to `math/` directory (or `math.pluto` single file), same as today.

## Data Model

A `.pluto` file contains two distinct layers:

### Authored Layer (written by AI/SDK)

The authored layer is the semantic content that an author (AI or human-via-sync) intentionally created. This is the "source" in the traditional sense.

Contents:
- **Declarations** with stable UUIDs — every function, class, enum, trait, method, and field gets a UUID at creation time that persists across renames, moves, and refactors
- **AST bodies** — the actual code: statements, expressions, control flow
- **Explicit type annotations** — types the author wrote (parameter types, return types, field types)
- **Import declarations** — module dependencies
- **Visibility modifiers** — `pub` markers
- **Error declarations** — named error types
- **App declaration** — DI configuration

Example conceptual structure (not actual binary format):
```
Module {
  id: "a1b2c3d4-...",
  name: "math.vectors",
  schema_version: 1,

  imports: ["math.matrices"],

  declarations: [
    Function {
      id: "e5f6a7b8-...",
      name: "dot_product",
      visibility: Public,
      params: [
        Param { name: "a", type: Class("Vector"), id: "..." },
        Param { name: "b", type: Class("Vector"), id: "..." },
      ],
      return_type: Float,
      body: [
        // AST nodes for the function body
        // Expressions reference other declarations by UUID
      ],
    },
    Class {
      id: "c9d0e1f2-...",
      name: "Vector",
      visibility: Public,
      fields: [
        Field { id: "...", name: "x", type: Float },
        Field { id: "...", name: "y", type: Float },
        Field { id: "...", name: "z", type: Float },
      ],
      methods: [
        Method { id: "...", name: "magnitude", ... },
      ],
      bracket_deps: [],
    },
  ],
}
```

### Derived Layer (written by compiler)

The derived layer is intended to be computed by a planned `pluto analyze` command. It is stored in the `.pluto` file but is always recomputable from the authored layer. If the derived data becomes stale (e.g., after an SDK write that didn't re-analyze), the compiler detects staleness and recomputes.

Contents:
- **Resolved types** — fully resolved type information for every expression
- **Inferred error sets** — which errors each function can raise (computed from the call graph)
- **DI wiring** — the resolved dependency injection graph (topological order, singleton assignments)
- **Call graph** — which functions call which, across module boundaries
- **Cross-module references** — which declarations in this module are used by other modules (useful for impact analysis)
- **Monomorphization instances** — which concrete types were instantiated for generic declarations

The derived layer enables the bidirectional AI/compiler loop:
1. AI writes authored content via SDK
2. AI (or CI) runs planned `pluto analyze` — compiler reads authored content, runs full analysis, writes derived layer back
3. AI reads the enriched `.pluto` for context (types, error sets, dependency info) when planning the next edit

**Normal `pluto compile` / `pluto build` is non-mutating.** It reads `.pluto` files and produces binaries but does not modify the `.pluto` files. Only planned `pluto analyze` would write back derived data. This avoids surprising file changes during builds.

### Canonical AST Contract

The `.pluto` binary format stores the **authored AST shape** — the program as written by the developer or AI agent, with cross-references resolved.

**Pipeline for canonical AST:**
1. Lex source → tokens
2. Parse tokens → raw AST
3. Resolve cross-references → canonical AST with UUIDs

**Canonical AST includes:**
- All user-defined declarations (functions, classes, enums, traits, errors, app)
- AST bodies (statements, expressions) in source form
- Cross-reference target_id/enum_id/variant_id fields populated
- Explicit type annotations from source

**Canonical AST excludes:**
- Monomorphized generics (e.g., `Box__int`, `identity__string`)
- Lifted closures (top-level `__closure_N` functions)
- Desugared spawn expressions (spawn wrapper closures)
- Injected prelude items (TypeInfo trait, reflection types)

**Functions:**
- `pluto::parse_for_editing(source)` — produces canonical AST
- `xref::resolve_cross_refs(program)` — populates UUID cross-references

**Invariant:** Derived data UUIDs must correspond to canonical AST UUIDs.

## Stable UUIDs

Every declaration (function, class, enum, trait, method, field) receives a UUID at creation time.

### Properties

- **Persist across renames.** Renaming `dot_product` to `inner_product` changes the name field but the UUID stays the same. All references (by UUID) remain valid.
- **Persist across moves.** Moving a function from one module to another changes the module but the UUID stays. Cross-module references update their target module but keep the same target UUID.
- **Are globally unique.** No two declarations in any Pluto project share a UUID.
- **Are not content-addressed.** Editing a function's body does not change its UUID. (Content hashing is a separate concern for caching/invalidation.)

### What Gets a UUID

| Entity | Gets UUID | Rationale |
|---|---|---|
| Function | Yes | Top-level declaration |
| Class | Yes | Top-level declaration |
| Enum | Yes | Top-level declaration |
| Enum variant | Yes | Independently referenceable in match arms |
| Trait | Yes | Top-level declaration |
| Method | Yes | Independently callable |
| Field | Yes | Referenced by name in struct literals, DI |
| Parameter | Yes | Part of function signature |
| Error declaration | Yes | Referenced in raise/catch |
| App declaration | Yes | Top-level, one per program |
| Local variable | No | Internal to function body, not referenceable externally |
| Expression | No | Too granular, massive overhead |
| Statement | No | Too granular, massive overhead |

### UUID in Cross-References

When function A calls function B, the call site in A's AST stores B's UUID (not B's name). This means:
- Renaming B does not require updating A's AST
- The SDK and compiler can resolve references by UUID lookup rather than name resolution
- Dangling references (calling a deleted function) are detectable without name-based heuristics

## .pt (Human-Readable) Files

`.pt` files contain standard Pluto syntax — the same language developers write today. They exist for:

1. **Code review.** `.pt` files are committed to git. PRs show diffs in readable Pluto syntax.
2. **Debugging.** When investigating compiler behavior, `.pt` gives a readable view of what the compiler sees.
3. **Human editing.** Developers can edit `.pt` directly when they prefer text editing over AI-mediated changes.

### Generation

```bash
pluto generate-pt            # regenerate all .pt files from .pluto
pluto generate-pt math/      # regenerate .pt for a specific module
```

The generator is a deterministic pretty-printer. Same `.pluto` input always produces the same `.pt` output (modulo formatting version).

### Sync (.pt → .pluto)

```bash
pluto sync                   # sync .pt changes back to .pluto
pluto sync math/vectors.pt   # sync a specific file
```

The sync tool:
1. Parses the modified `.pt` file
2. Diffs the parsed AST against the current `.pluto` file
3. Matches declarations by name + signature to existing UUIDs
4. Preserves UUIDs for unchanged/renamed declarations
5. Assigns new UUIDs to genuinely new declarations
6. Removes declarations that were deleted from `.pt`
7. Writes the updated `.pluto` file
8. Marks derived data as stale (requires planned `pluto analyze` to refresh)

### Conflict Resolution

`.pluto` always wins. If both `.pluto` and `.pt` have been modified:
- Running `pluto generate-pt` overwrites `.pt` with the current `.pluto` state
- Running `pluto sync` after `generate-pt` is a no-op
- CI should enforce that `.pt` matches `.pluto` (fail if out of sync)

## SDK (pluto-sdk)

The SDK is a Rust crate that provides programmatic read/write access to `.pluto` files. It is the primary interface for AI agents.

### Design Principles

- **Type-safe.** Operations are validated at the Rust type level. You cannot create an invalid `.pluto` file through the SDK API.
- **Transactional.** Modifications are staged and committed atomically. No partial writes.
- **UUID-aware.** All operations work with UUIDs. The SDK handles name ↔ UUID resolution.
- **Non-mutating reads.** Reading a `.pluto` file never modifies it.

### API (implemented)

The SDK (`sdk/src/`) provides both read and write access. The read API loads modules from binary PLTO or source text. The write API (`ModuleEditor`) enables text-in/AST-out editing:

```rust
use pluto_sdk::{Module, ModuleEditor, DeclKind};

// Read a module from binary (.pluto) or source text
let module = Module::from_bytes(&bytes)?;      // from PLTO binary
let module = Module::from_source(source)?;     // from text (parse-only, no transforms)

// Query declarations
let funcs = module.find("dot_product");
let decl = module.get(some_uuid)?;
let callers = module.callers_of(some_uuid);

// Edit: Module::edit() consumes the module, returns ModuleEditor
let mut editor = module.edit();

// Add declarations from source text (returns UUID)
let fn_id = editor.add_from_source(
    "pub fn cross_product(a: Vector, b: Vector) Vector {\n    ...\n}"
)?;

// Replace a declaration (preserves top-level UUID)
editor.replace_from_source(dot_product_id,
    "pub fn dot_product(a: Vector, b: Vector) float {\n    return a.x * b.x + a.y * b.y\n}"
)?;

// Rename — updates all reference sites in the AST
editor.rename(dot_product_id, "inner_product")?;

// Delete — reports dangling references
let result = editor.delete(old_fn_id)?;

// Add method/field to a class
editor.add_method_from_source(class_id, "fn magnitude(self) float { ... }")?;
editor.add_field(class_id, "z", "float")?;

// Commit: pretty-print → re-resolve xrefs → rebuild index → new Module
let module = editor.commit();
// UUID unchanged — all references still valid
```

### AI Agent Workflow

```
1. AI receives task: "add a cross_product function to math.vectors"
2. AI reads math/vectors.pluto via SDK
   - Sees existing declarations, their UUIDs, types
   - Reads derived data: what functions call dot_product, what errors propagate
3. AI constructs new function via SDK
   - SDK assigns UUID, validates types
4. AI writes updated .pluto via SDK
5. AI (or CI) runs planned `pluto analyze` to refresh derived data
6. AI runs `pluto generate-pt` to update the human-readable view
7. Both .pluto and .pt are committed to git
```

## Compiler Integration

### Commands

| Command | Reads | Writes | Purpose |
|---|---|---|---|
| `pluto compile` | `.pluto` | binary | Compile to executable. Non-mutating. |
| `pluto analyze` (planned) | `.pluto` | `.pluto` (derived layer) | Enrich with type info, errors, call graph. |
| `pluto generate-pt` | `.pluto` | `.pt` | Generate human-readable view. |
| `pluto sync` | `.pt` + `.pluto` | `.pluto` | Sync human edits back to canonical form. |
| `pluto run` | `.pluto` | binary (temp) | Compile and run. Non-mutating. |
| `pluto test` | `.pluto` | binary (temp) | Compile and run tests. Non-mutating. |
| `pluto migrate` | `.pluto` (old) | `.pluto` (new) | Upgrade schema version. |

### Backward Compatibility

During the transition period, the compiler supports both input formats:
- If a `.pluto` binary file exists, use it
- If only a `.pt` text file exists, parse it (current behavior)
- `pluto init` can convert an existing text-based project to the `.pluto` + `.pt` format

## Libraries and Dependencies

Libraries can be distributed in two forms:

### Full .pluto (open source)

The library ships its complete `.pluto` files including authored and derived layers. Consumers get:
- Full source code (AST bodies)
- Complete type information
- Error sets, call graphs
- Enables whole-program analysis across library boundaries (Pluto's core strength)

### Signature-only .pluto (proprietary)

The library ships stripped `.pluto` files containing only:
- Public declaration signatures (names, types, UUIDs)
- Public error declarations
- Trait definitions
- No function/method bodies
- No private declarations

The compiler can type-check against signature-only libraries but cannot perform cross-boundary analysis (error inference, call graph analysis) into their internals.

## Schema Versioning

Every `.pluto` file contains a schema version number. When the format evolves:

1. The compiler reads the version field first
2. If the version is older than current, it runs the appropriate migration chain (v1→v2→v3...)
3. Migrations are deterministic and lossless — no information is lost
4. After migration, the file is rewritten at the new version
5. `pluto migrate` can batch-migrate an entire project

Migration examples:
- Adding a new field to declarations → default value in migration
- Changing how error sets are represented → transform in migration
- Adding a new declaration type → no migration needed (additive)

## PLTO v3 Binary Format Specification

### Container Structure

Same 20-byte header + 3 sections as v2:

```
[4B magic "PLTO"]
[4B schema version u32 LE]  ← now writes 3
[4B source offset u32 LE]
[4B AST offset u32 LE]
[4B derived offset u32 LE]
[Source section: 4B length u32 LE + UTF-8 bytes]
[AST section: 4B length u32 LE + bincode bytes]
[Derived section: 4B length u32 LE + bincode bytes]
```

### v3 Changes (from v2)

**DerivedInfo now includes DerivedMeta:**
```rust
pub struct DerivedInfo {
    // ... existing 9 fields ...

    #[serde(default)]
    pub meta: Option<DerivedMeta>,
}

pub struct DerivedMeta {
    pub source_hash: String,       // SHA256(source_bytes) hex-encoded
    pub compiler_version: String,  // e.g., "0.1.0"
}
```

**Freshness semantics:**
- `meta = Some(DerivedMeta { hash, version })` → check freshness
- `meta = None` → treat as stale (legacy v2 files)

### Backward Compatibility

- **v3 reader** supports v2 files (synthesizes `meta: None`)
- **v2 reader** rejects v3 files (unsupported version error)
- **Migration**: Run `pluto analyze .` to upgrade all files to v3 (when available)

### Commands

- `pluto emit-ast <file.pt> -o <file.pluto>` — writes v3 with fresh derived data
- `pluto sync <file.pt> <file.pluto>` — writes v3 with stale derived data (meta = None)
- `pluto analyze <file.pluto>` — updates derived data to fresh v3 (in progress, separate implementation)

## Alternatives Considered

### Text files with semantic database (status quo+)

Keep `.pluto` as text files. Build a `.pluto-cache` database (SQLite, etc.) that stores the semantic graph. Compiler updates the database as a side effect.

**Rejected because:** The text file remains the source of truth, so AI agents still need to do text manipulation. The semantic database is a cache, not canonical — it can go stale, get corrupted, or diverge. Two sources of truth is worse than one canonical representation.

### Content-addressed (Unison-style)

Use content hashes as IDs instead of UUIDs. Same code = same ID.

**Rejected because:** IDs change on every edit, which breaks the "stable reference" property. A rename changes the content, changing the hash, breaking all references. UUIDs are better for the AI workflow where declarations are edited frequently.

### Single project database (Smalltalk image)

One binary file for the entire project instead of per-module files.

**Rejected because:** Per-module files are git-friendly (smaller diffs, easier merges, file-level blame), match the existing module system, and are more natural for AI agents working on one module at a time.

### AST only, no derived data

Store only the authored AST in `.pluto`. Derived data lives in a separate cache.

**Rejected because:** The bidirectional AI/compiler loop is the core value proposition. AI needs to read compiler analysis (types, errors, call graphs) to make informed edits. Putting derived data in the canonical file makes it first-class and always accessible, rather than a cache that might not exist.

## Open Questions

- [ ] **Exact binary format.** Protobuf, FlatBuffers, Cap'n Proto, or custom? Needs benchmarking for read/write performance and format stability.
- [ ] **Derived data staleness detection.** How does the compiler know derived data is stale? Hash of authored layer? Timestamp? Version counter?
- [ ] **Incremental analysis.** When one declaration changes, can planned `pluto analyze` update only the affected derived data, or must it recompute everything?
- [ ] **Cross-project UUIDs.** How do UUIDs work across library boundaries? Does a library's UUID namespace conflict with the consumer's?
- [ ] **SDK language bindings.** The SDK is a Rust crate, but AI agents might run in Python or TypeScript. FFI bindings? gRPC service?
- [ ] **Diff tooling.** Custom `git diff` driver for `.pluto` binary files? Or rely entirely on `.pt` diffs for review?
- [ ] **IDE integration.** Do editors work with `.pt` files and sync on save? Or does the SDK power an LSP that works directly with `.pluto`?
- [ ] **Concurrent SDK access.** Can two AI agents edit the same `.pluto` file simultaneously? File locking? OT/CRDT?

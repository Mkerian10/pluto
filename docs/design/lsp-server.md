# RFC: Language Server Protocol (`plutoc lsp`)

**Status:** Proposed
**Priority:** Medium (needed for real editor experience)
**Related:** [unified-binary.md](unified-binary.md)

## Problem

The Zed editor extension already declares a language server:

```toml
[language_servers.plutoc.binary]
path = "plutoc"
arguments = ["lsp"]
```

But `plutoc lsp` doesn't exist. Currently, editor users get syntax highlighting (tree-sitter) but no:
- Diagnostics (error squiggles)
- Go-to-definition
- Hover type information
- Autocompletion
- Error set display
- DI graph visualization

## Design Principle: Reuse the Compiler

The most important architectural decision: **the LSP must use the actual compiler, not a reimplementation.**

Rust learned this the hard way. `rust-analyzer` is a separate reimplementation of much of `rustc`, optimized for incremental analysis. This means:
- Type inference sometimes disagrees between the IDE and the compiler
- Features land in `rustc` months before they work in the IDE
- Two teams maintain two implementations of the same language semantics

Deno got this right: `deno lsp` IS the compiler. Same parser, same type checker, same error messages. No divergence possible.

For Pluto, this means the LSP server calls `plutoc::analyze_file_with_warnings()` (or similar) directly. The MCP server already does this — the LSP would follow the same pattern.

## Architecture

```
┌─────────────────────────────────────┐
│            Editor (Zed, VSCode)     │
└──────────────┬──────────────────────┘
               │ LSP (JSON-RPC over stdio)
               ▼
┌─────────────────────────────────────┐
│         plutoc lsp                  │
│  ┌─────────────────────────────┐    │
│  │  LSP Protocol Handler       │    │
│  │  (textDocument/didOpen,     │    │
│  │   textDocument/diagnostic,  │    │
│  │   textDocument/hover, etc.) │    │
│  └──────────────┬──────────────┘    │
│                 │                    │
│  ┌──────────────▼──────────────┐    │
│  │  Compiler Library (plutoc)  │    │
│  │  - parse()                  │    │
│  │  - analyze_file()           │    │
│  │  - TypeEnv queries          │    │
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
```

The LSP server is a thin protocol adapter over the compiler's existing analysis APIs.

## Capabilities (Phased)

### Phase 1: Diagnostics

The most valuable feature. On every file save:
1. Parse the file
2. Run type-checking via `analyze_file_with_warnings()`
3. Convert `CompileError` / warnings to LSP `Diagnostic` messages
4. Send to editor

This is essentially what the MCP `check` tool does, but triggered automatically on file changes.

### Phase 2: Hover Information

On hover over an identifier, show:
- Variable type (from `TypeEnv`)
- Function signature (params, return type, error set)
- Class fields and methods
- Enum variants

The SDK already has `signature_of()`, `is_fallible()`, `error_set_of()` — these map directly to hover content.

### Phase 3: Go-to-Definition

On ctrl+click, jump to the definition of:
- Functions (local and imported)
- Classes, enums, traits, errors
- Fields and methods

Requires span information, which we already have (`Spanned<T>` on all AST nodes). The SDK's `Module::find()` and cross-reference APIs (`callers_of`, `constructors_of`) provide the data.

### Phase 4: Autocompletion

On typing, suggest:
- Available functions and variables in scope
- Class methods after `.`
- Enum variants after `EnumName.`
- Module members after `module_name.`
- Stdlib modules after `import std.`

This requires access to the `TypeEnv` scope tracking, which the compiler already computes.

### Phase 5: Advanced Features

- Rename symbol (SDK has `ModuleEditor::rename()`)
- Code actions (quick fixes for common errors)
- Inlay hints (inferred types, error sets)
- Workspace symbol search
- Document outline

## Incremental Analysis

The biggest engineering challenge. The compiler today runs the full pipeline on every invocation. For an LSP server, we need:

**Short term:** Re-run the full pipeline on every file change. This is what Deno does for small-to-medium projects. If `analyze_file()` takes <100ms (likely for most Pluto programs), this is fast enough.

**Medium term:** Cache parsed modules and only re-analyze changed files. The build cache RFC covers this infrastructure.

**Long term:** True incremental analysis (salsa-style demand-driven computation). This is what rust-analyzer does. Only consider if the short-term approach proves too slow.

## Shared Infrastructure with MCP

The LSP and MCP servers do similar things with different protocols:

| Capability | MCP Tool | LSP Method |
|-----------|----------|------------|
| Type-check file | `check` | `textDocument/diagnostic` |
| Inspect declaration | `inspect` | `textDocument/hover` |
| Find declaration | `find_declaration` | `textDocument/definition` |
| Cross-references | `xrefs` | `textDocument/references` |
| Rename | `rename_declaration` | `textDocument/rename` |

Consider a shared analysis layer that both the LSP and MCP server use:

```rust
// src/analysis.rs — shared between LSP and MCP
pub struct AnalysisHost {
    modules: HashMap<PathBuf, Module>,
    cache: BuildCache,
}

impl AnalysisHost {
    pub fn check(&self, path: &Path) -> CheckResult { ... }
    pub fn hover(&self, path: &Path, position: Position) -> HoverResult { ... }
    pub fn definition(&self, path: &Path, position: Position) -> Location { ... }
    pub fn references(&self, path: &Path, position: Position) -> Vec<Location> { ... }
}
```

## Implementation Plan

1. **Phase 1:** Diagnostics-only LSP (highest value, simplest)
   - Add `lsp` subcommand to `plutoc`
   - Implement `textDocument/didOpen`, `textDocument/didSave`, `textDocument/publishDiagnostics`
   - Use `analyze_file_with_warnings()` for analysis
   - Depends on: unified binary RFC

2. **Phase 2:** Hover + go-to-definition
   - Map cursor position to AST node (byte offset → Spanned<T>)
   - Query TypeEnv for type information
   - Use span information for definition locations

3. **Phase 3:** Autocompletion
   - Build scope-aware completion from TypeEnv
   - Module member completion from loaded modules

4. **Phase 4:** Refactoring support
   - Rename (delegate to SDK `ModuleEditor::rename()`)
   - Code actions

## Dependencies

- An LSP protocol library (e.g., `tower-lsp` or `lsp-server` crate)
- tokio (already a dependency if MCP is merged in)
- The compiler's `analyze_file()` API

## Open Questions

- **Which LSP crate?** `tower-lsp` (async, tokio-based) vs `lsp-server` (sync, simpler). If we're already using tokio for MCP, `tower-lsp` aligns better.
- **Multi-file support:** How does the LSP handle multi-module projects? Analyze the whole project, or just the open file?
- **Debouncing:** How frequently do we re-analyze on typing? Every keystroke? On save only? After a delay?
- **Memory:** Keeping TypeEnv in memory for responsiveness vs re-computing on demand for simplicity?

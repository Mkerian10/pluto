# RFC: Unified `plutoc` Binary

**Status:** Proposed
**Priority:** Medium
**Related:** [stdlib-embedding.md](stdlib-embedding.md), [lsp-server.md](lsp-server.md)

## Problem

Today, the Pluto toolchain is multiple separate artifacts:

| Artifact | Crate | Binary | How Users Get It |
|----------|-------|--------|-----------------|
| Compiler | `plutoc` | `plutoc` | `cargo build` |
| MCP server | `pluto-mcp` | `pluto-mcp` | `cargo build -p pluto-mcp` |
| SDK | `plutoc-sdk` | (library) | Rust dependency |
| LSP | — | — | Not implemented (Zed config expects `plutoc lsp`) |

Problems:
1. **Version mismatch** — MCP server could be built from a different commit than the compiler
2. **Distribution complexity** — Users need to build and manage multiple binaries
3. **Path configuration** — `.mcp.json` has a hardcoded absolute path to one developer's machine
4. **Duplication** — MCP server reimports the entire compiler as a library anyway

## Prior Art

| Language | Approach | Tools Included |
|----------|---------|----------------|
| Deno | **Single binary, subcommands** | Runtime, LSP, formatter, linter, test runner, bundler, package manager |
| Go | Directory with multiple binaries | `go`, `gofmt`, internal tools (compile, link, asm) |
| Rust | **Separate binaries**, managed by `rustup` | `rustc`, `cargo`, `rustfmt`, `clippy`, `rust-analyzer` (all separate) |
| Zig | **Single binary, subcommands** | Compiler, C/C++ compiler, build system, package manager, test runner |

Deno and Zig prove that "one binary, everything works" is viable and loved by users. Rust's multi-binary approach causes version management pain that `rustup` exists to solve.

## Proposed Solution

### Merge all tools into `plutoc` subcommands

```
plutoc compile <file>        # Compile to binary (exists)
plutoc run <file>            # Compile + execute (exists)
plutoc test <file>           # Test runner (exists)
plutoc check <file>          # Type-check only (new — currently only in MCP)
plutoc watch <subcommand>    # File watcher (exists)

plutoc mcp                   # MCP server over stdio (move from pluto-mcp)
plutoc lsp                   # Language server (future, see lsp-server.md)

plutoc emit-ast <file>       # Serialize to PLTO binary (exists)
plutoc generate-pt <file>    # PLTO binary to text (exists)
plutoc sync <file>           # Sync .pt edits back to .pluto (exists)

plutoc mod init              # Package manager (future, see package-manager.md)
plutoc mod fetch             # Fetch dependencies (future)
plutoc mod update            # Update dependencies (future)
```

### Implementation Plan

#### Phase 1: Merge MCP into `plutoc mcp`

1. Move `mcp/src/{server,serialize,tools,docs}.rs` into `src/mcp/` in the main crate
2. Add `mcp` subcommand to `src/main.rs` clap configuration
3. Add MCP dependencies (`rmcp`, `tokio`, `tempfile`) behind a cargo feature flag:
   ```toml
   [features]
   default = ["mcp"]
   mcp = ["dep:rmcp", "dep:tokio", "dep:tempfile"]
   ```
4. Update `.mcp.json` to use:
   ```json
   {"command": "plutoc", "args": ["mcp"]}
   ```
5. Remove `mcp/` workspace member
6. Remove `pluto-mcp` binary target

The MCP server already calls `plutoc::compile_file_with_stdlib()`, `plutoc::analyze_file_with_warnings()`, etc. — it's already using the compiler as a library. This change just removes the indirection.

#### Phase 2: Add `plutoc check`

Currently type-checking without compilation is only available through the MCP server. Add it as a standalone subcommand:

```
plutoc check <file> [--stdlib <path>]
```

Returns structured diagnostics (errors + warnings) to stdout. Useful for CI, editor integration, and scripting. Uses `analyze_file_with_warnings()` which already exists.

#### Phase 3: Prepare for LSP (see lsp-server.md)

Reserve the `plutoc lsp` subcommand. The Zed extension already expects it:
```toml
[language_servers.plutoc.binary]
path = "plutoc"
arguments = ["lsp"]
```

#### Future: Formatter, Package Manager

These would be additional subcommands added as they're implemented.

### What Happens to the SDK?

`plutoc-sdk` stays as a separate library crate in the workspace. It's consumed by:
- The MCP server code (now inside `plutoc`)
- External tools that want programmatic access to Pluto programs
- The future AI-native representation tooling

The SDK doesn't need to be a subcommand — it's a library API, not a user-facing tool.

### Feature Flag Design

To keep the core compiler lightweight for users who don't need the MCP server:

```toml
[features]
default = ["mcp"]
mcp = ["dep:rmcp", "dep:tokio", "dep:tempfile", "dep:serde", "dep:serde_json"]
```

`cargo install plutoc` gives you everything. `cargo install plutoc --no-default-features` gives you just the compiler.

### Binary Size Considerations

Current sizes:
- `plutoc` (release): ~5MB
- `pluto-mcp` (release): ~8.8MB

The MCP server adds ~3.8MB (mostly tokio + rmcp + serde). With the feature flag, users who don't need MCP can opt out.

## Benefits

1. **No version mismatch** — MCP/LSP always match the compiler exactly
2. **One install** — `cargo install plutoc` and you're done
3. **Portable config** — `.mcp.json` uses `"command": "plutoc"` (finds it on PATH)
4. **Shared code** — MCP docs, stdlib docs, diagnostics formatting all live in one place
5. **Simpler CI** — One binary to build, test, and release

## Migration

1. Implement Phase 1 (merge MCP)
2. Update `.mcp.json` in the repo
3. Update documentation
4. Remove `mcp/Cargo.toml` and `mcp/` directory
5. SDK stays as a workspace member

## Open Questions

- Should the feature flag be `default = ["mcp"]` or opt-in? (Default seems right — most users want the full toolchain)
- Should tokio become a core dependency (needed for LSP too) or stay behind the feature flag?
- When the LSP lands, does it share the same feature flag as MCP or get its own?

# Toolchain Implementation Plan

**Parent RFC:** [rfc-toolchain-architecture.md](rfc-toolchain-architecture.md)
**Status:** Draft
**Scope:** Phased migration from current `plutoc` workspace to unified `pluto` binary

---

## Overview

This document is the "detailed phased implementation plan" referenced in RFC Section 10.2. It breaks the toolchain migration into 7 phases, ordered by dependency, each deliverable as 1-3 pull requests that keep `master` green at every step.

### Dependency Graph

```
Phase 1 (Rename plutoc → pluto) ─── independent
Phase 2 (.deps/ resolution)     ─── independent ──→ Phase 4 (Manifest prep) ──→ Phase 7 (Full removal)
Phase 3 (Toolchain manager)     ─── independent
Phase 5 (CompilerService API)   ─── independent ──→ Phase 6 (Frontend unification)
```

Phases 1, 2, 3, and 5 have no dependencies on each other and can proceed in parallel. Phase 4 requires Phase 2. Phase 6 requires Phase 5. Phase 7 requires Phase 4.

### What This Plan Does NOT Cover

The following items from RFC Section 9 are explicitly deferred:

- **Socket server and internal protocol** (RFC 9.1) — Requires its own protocol design RFC. Phase 5 establishes the `CompilerService` trait; the socket transport is a future layer on top.
- **LSP frontend** — Blocked on socket server. The `pluto serve --lsp` subcommand is a stub until then.
- **Build cache** (RFC 9.5) — Independent project, not on the critical path for toolchain unification.
- **Profiling/coverage/debugging API surfaces** (RFC 9.6) — Coverage CLI already works; API exposure waits for `CompilerService`.
- **`system` declaration design** (RFC 9.2) — Language semantics, not toolchain.
- **Registry design** (RFC 9.4) — Package manager project scope.
- **Windows support** (RFC 9.7) — Not blocking; macOS/Linux only for now.

---

## Phase 1: Rename `plutoc` to `pluto`

**Goal:** The crate, binary, and all references use `pluto` instead of `plutoc`.

**Scope:** Naming only. No behavioral changes.

**Estimated size:** 1 PR.

### Changes

#### `Cargo.toml` (root)
- `name = "plutoc"` → `name = "pluto"`
- All `[[test]]` entries remain unchanged (they reference the crate only via `use pluto::` in source)

#### `sdk/Cargo.toml`
- `name = "pluto-sdk"` → `name = "pluto-sdk"`
- `plutoc = { path = ".." }` → `pluto = { path = ".." }`

#### `mcp/Cargo.toml`
- `pluto-sdk = { path = "../sdk" }` → `pluto-sdk = { path = "../sdk" }`
- `plutoc = { path = ".." }` → `pluto = { path = ".." }`

#### `src/main.rs`
- Line 5: `#[command(name = "plutoc", ...)]` → `#[command(name = "pluto", ...)]`
- All `plutoc::` references in this file (e.g., `pluto::compile_file_with_options`, `plutoc::diagnostics::CompileError`) → `pluto::`
- Error message on line 217: `"use \`pluto compile\`"` → `"use \`pluto compile\`"`
- Coverage hint messages (lines 481, 486): `"pluto test file.pluto --coverage"` → `"pluto test file.pluto --coverage"`

#### `src/lib.rs`
- No changes needed (this file doesn't reference its own crate name)

#### `sdk/src/**/*.rs`
- All `use pluto::` → `use pluto::` (grep for `plutoc::` in `sdk/src/`)

#### `mcp/src/**/*.rs`
- All `use pluto::` and `use pluto_sdk::` → `use pluto::` and `use pluto_sdk::`
- Binary name in `mcp/Cargo.toml` `[[bin]]` stays `pluto-mcp` for now (Phase 6 merges it)

#### `tests/**/*.rs`
- All `use pluto::` → `use pluto::` across all integration and property test files
- The `common/mod.rs` test helper references `plutoc::` for compilation — update those

#### `.github/workflows/build.yml`
- Line 32: `ls -lh target/debug/plutoc` → `ls -lh target/debug/pluto`

#### `.github/workflows/test.yml`
- Line 29: `--exclude plutoc` → `--exclude pluto`

#### `CLAUDE.md`
- All `plutoc` references in CLI examples → `pluto`
- Crate name references updated

### Verification

- `cargo build` produces `target/debug/pluto` binary
- `cargo test` passes (all existing tests)
- `cargo build --workspace` builds all workspace members
- CI workflows reference correct binary name
- `./target/debug/pluto compile examples/hello/main.pluto -o /tmp/hello` works

### Risks

- **Low.** This is a mechanical rename. The `sed`/find-and-replace is straightforward. The only subtlety is that `plutoc` appears in string literals (error messages, hints) that also need updating.
- Downstream consumers of the `plutoc` crate (if any exist outside this workspace) will break. Since Pluto is pre-alpha with no external users, this is acceptable.

---

## Phase 2: `.deps/` Import Resolution

**Goal:** The compiler's import resolution chain gains a `.deps/` lookup step, so that external dependencies placed in `.deps/` by a future package manager are automatically resolved.

**Depends on:** Nothing. Independent of all other phases.

**Estimated size:** 1 PR.

### Changes

#### `src/modules.rs`

The current resolution chain in `resolve_module_imports()` (line 316) and `resolve_modules_inner()` (line 651) is:

1. Check `current_deps` (from `pluto.toml` manifest) — `current_deps.contains_key(first_segment)`
2. Check sibling directory — `dir_path = entry_dir.join(first_segment)`
3. Check sibling file — `file_path = entry_dir.join(format!("{first_segment}.pluto"))`
4. Check stdlib — `std.*` prefix handling

Add a new step between sibling file (2/3) and stdlib (4):

```
3.5. Check .deps/ directory — look for .deps/<name>/ relative to the entry file's directory
```

Implementation:
- After the sibling directory/file check fails and before the stdlib check, look for `entry_dir.join(".deps").join(first_segment)` as a directory
- If found, load it as a directory module via `load_directory_module()`
- Mark the import origin as `ImportOrigin::PackageDep` (reuse existing variant)
- Walk-up behavior (looking for `.deps/` in parent directories) is explicitly deferred to the package manager project, per RFC Section 9.3

The `.deps/` check should come *after* local modules so that a local `auth/` directory always takes precedence over `.deps/auth/`. This matches the principle that local code shadows dependencies.

#### New integration tests

Add tests to `tests/integration/modules.rs`:
- `deps_directory_simple` — create a `.deps/mylib/` with a `pub fn`, import it, verify it works
- `deps_local_shadows_deps` — local `foo/` directory takes precedence over `.deps/foo/`
- `deps_not_found` — importing a module that doesn't exist locally or in `.deps/` still produces a clear error
- `deps_transitive` — `.deps/foo/` can import its own local modules (but not nested `.deps/`)

### Verification

- Existing module tests still pass (no behavioral change for projects without `.deps/`)
- New `.deps/` tests pass
- `cargo test --test modules` green

### Risks

- **Low.** This adds a single directory lookup to the resolution chain. The fallback behavior is unchanged — if `.deps/` doesn't exist, resolution proceeds to stdlib as before.
- Walk-up behavior is intentionally omitted. This means `.deps/` must be a sibling of the importing file's directory. Monorepo layouts (where `.deps/` is at the repo root and importers are in subdirectories) are deferred.

---

## Phase 3: Toolchain Manager

**Goal:** `pluto install`, `pluto use`, and `pluto versions` subcommands. The `pluto` binary can manage multiple compiler versions in `~/.pluto/versions/` and delegate to the active version.

**Depends on:** Nothing. Independent of all other phases. Can proceed in parallel with Phase 1 (rename should land first for naming consistency, but there's no code dependency).

**Estimated size:** 1-2 PRs.

### PR 3a: Version management commands

#### New file: `src/toolchain.rs`

Core module for version management:

```rust
pub fn versions_dir() -> PathBuf       // ~/.pluto/versions/
pub fn active_version_file() -> PathBuf // ~/.pluto/active
pub fn active_version() -> Option<String>
pub fn installed_versions() -> Vec<String>
pub fn install_version(version: &str) -> Result<(), ToolchainError>
pub fn use_version(version: &str) -> Result<(), ToolchainError>
```

Distribution mechanism: download pre-built binaries from GitHub releases (or a future CDN). The exact URL scheme depends on how releases are published. For the initial implementation, `install` can simply download from `https://github.com/<org>/pluto/releases/download/v<version>/pluto-<target>` where `<target>` is `aarch64-apple-darwin` or `x86_64-unknown-linux-gnu`.

#### `src/main.rs`

Add three new subcommands:

```rust
/// Download and cache a compiler version
Install {
    /// Version to install (e.g., "0.2.0" or "latest")
    version: String,
},
/// Set the active compiler version
Use {
    /// Version to activate
    version: String,
},
/// List installed compiler versions
Versions,
```

#### Directory structure

```
~/.pluto/
  active               # Contains version string, e.g., "0.2.0"
  versions/
    0.1.0/
      pluto             # Binary
    0.2.0/
      pluto             # Binary
```

### PR 3b: Auto-delegation on version mismatch

#### `src/main.rs` — early in `main()`

Before parsing subcommands, check if the running binary's version matches the active version:

```rust
fn maybe_delegate() {
    let our_version = env!("CARGO_PKG_VERSION");
    if let Some(active) = toolchain::active_version() {
        if active != our_version {
            let delegate = toolchain::versions_dir().join(&active).join("pluto");
            if delegate.exists() {
                // exec() replaces the current process — no fork overhead
                let err = exec::Command::new(delegate).args(std::env::args_os().skip(1)).exec();
                eprintln!("error: failed to delegate to pluto {active}: {err}");
                std::process::exit(1);
            }
        }
    }
}
```

This runs before `Cli::parse()` so that even unrecognized subcommands (added in newer versions) get delegated correctly.

The `Install`, `Use`, and `Versions` subcommands bypass delegation — they always run on the current binary. This is handled by checking `args[1]` before delegating.

### Verification

- `pluto versions` lists installed versions
- `pluto install <version>` downloads and caches a binary (may need a mock server for tests)
- `pluto use <version>` writes `~/.pluto/active`
- Version mismatch delegation: if `active` != running version and the target binary exists, the process delegates
- All existing tests still pass (the delegation check is a no-op when `~/.pluto/active` doesn't exist)

### Risks

- **Medium.** The download mechanism depends on release infrastructure that doesn't exist yet. The PR can land with the subcommand skeleton and a clear error message ("release downloads not yet configured") while the URL scheme is finalized.
- `exec()` is Unix-only. Windows support would need `CreateProcess`. Since Windows is deferred (RFC 9.7), this is acceptable.
- The `exec` crate (or `std::os::unix::process::CommandExt::exec`) replaces the process. If the delegated binary crashes, the exit code propagates naturally.

---

## Phase 4: Manifest Removal (Prep)

**Goal:** Remove the `Update` subcommand and the manifest/git-cache code from the compiler. The compiler no longer reads `pluto.toml` or fetches git dependencies. Import resolution still works for projects without manifests (which is all real projects, since `.deps/` from Phase 2 replaces manifest-based deps).

**Depends on:** Phase 2 (`.deps/` resolution must be available as the replacement).

**Estimated size:** 1-2 PRs.

### PR 4a: Remove `Update` subcommand and git_cache

#### `src/main.rs`
- Remove the `Update` variant from `Commands` enum (lines 79-83)
- Remove the `Commands::Update { .. }` match arm (lines 467-472)

#### `src/lib.rs`
- Remove `pub fn update_git_deps()` (line 1041)
- Remove `pub mod git_cache;` (line 19)

#### `src/git_cache.rs`
- Delete the entire file

#### `Cargo.toml`
- Do NOT remove the `toml` dependency yet — `src/manifest.rs` still uses it. Full removal is Phase 7.

#### `tests/integration/manifest.rs`
- Remove or update tests that exercise `update_git_deps()` or git dependency fetching
- Keep tests that exercise manifest parsing if any remain useful for Phase 7

### PR 4b: Simplify module resolution to drop `pkg_graph`/`current_deps`

#### `src/manifest.rs`
- Extract `cache_root()` to a standalone utility (it's used by git_cache but the concept of `~/.pluto/cache` may be reused by the build cache). Alternatively, inline it where needed and delete the manifest module entirely.
- Remove `PackageGraph`, `PackageNode`, `DependencyScope` types
- Remove `find_manifest()`, `parse_manifest()`, `resolve_package_graph()`

#### `src/modules.rs`
- Remove `pkg_graph: &PackageGraph` and `current_deps: &DependencyScope` parameters from:
  - `load_directory_module()` (line 168)
  - `resolve_module_path()` (line 265)
  - `resolve_module_imports()` (line 316)
  - `resolve_modules()` (line 630)
  - `resolve_modules_no_siblings()` (line 640)
  - `resolve_modules_inner()` (line 651)
- Remove all `current_deps.contains_key()` / `pkg_graph.deps_for()` branches — these are replaced by the `.deps/` lookup from Phase 2
- The `ImportOrigin::PackageDep` variant stays (`.deps/` imports use it)

#### `src/lib.rs`
- Remove `pub mod manifest;` (line 18)
- All `compile_file*` functions that construct a `PackageGraph::empty()` and pass it to `resolve_modules()` — simplify to call the new parameterless signature
- Remove `pub fn update_git_deps()`

#### All callers of `resolve_modules()`
- Update call sites to drop the `pkg_graph` argument (the function no longer takes it)
- This includes `src/lib.rs`, `mcp/src/`, and `sdk/src/`

### Verification

- `cargo build` succeeds without `src/git_cache.rs` and `src/manifest.rs`
- All existing integration tests pass (no test currently depends on `pluto.toml` for compilation — manifest tests may need removal/update)
- `pluto compile` / `pluto run` / `pluto test` work as before
- Projects with `pluto.toml` files get a clean compilation (the manifest is simply ignored — no warning needed since no real users depend on it)

### Risks

- **Low-Medium.** The `PackageGraph` parameter threads through many functions in `modules.rs`. The refactor is mechanical but touches ~30 call sites. Careful grep-and-replace with incremental compilation checks.
- If any integration test creates a `pluto.toml` for testing purposes, those tests need updating. Check `tests/integration/manifest.rs` specifically.

---

## Phase 5: Server API & `CompilerService`

**Goal:** Define a `CompilerService` trait that abstracts all compiler operations. Implement it as `InProcessServer` (direct library calls, no socket). Refactor the CLI and MCP server to route through `CompilerService` instead of calling library functions directly.

**Depends on:** Nothing. Independent of Phases 1-4. However, the rename from Phase 1 should land first for naming consistency.

**Estimated size:** 2-3 PRs.

**Note:** The socket-based server (RFC Section 3.2) is explicitly deferred. This phase establishes the API boundary; the transport layer is a future project requiring its own protocol design (RFC Section 9.1).

### PR 5a: Define `CompilerService` trait

#### New file: `src/server/mod.rs`

```rust
pub trait CompilerService {
    fn check(&self, path: &Path, stdlib: Option<&Path>) -> CheckResult;
    fn compile(&self, path: &Path, output: &Path, stdlib: Option<&Path>) -> CompileResult;
    fn run(&self, path: &Path, stdlib: Option<&Path>, timeout: Duration) -> RunResult;
    fn test(&self, path: &Path, stdlib: Option<&Path>, opts: TestOptions) -> TestResult;

    fn load_module(&self, path: &Path, stdlib: Option<&Path>) -> LoadResult;
    fn list_declarations(&self, path: &Path, kind: Option<DeclKind>) -> Vec<DeclSummary>;
    fn get_declaration(&self, path: &Path, uuid: Uuid) -> Option<DeclDetail>;
    fn find_declaration(&self, name: &str, kind: Option<DeclKind>) -> Vec<DeclMatch>;

    fn add_declaration(&self, path: &Path, source: &str) -> EditResult;
    fn replace_declaration(&self, path: &Path, name: &str, source: &str) -> EditResult;
    fn delete_declaration(&self, path: &Path, name: &str) -> EditResult;
    fn rename_declaration(&self, path: &Path, old: &str, new: &str) -> EditResult;

    // ... additional methods matching current MCP tool surface
}
```

The trait mirrors the current MCP tool surface (RFC Section 7.4). Each MCP tool maps to exactly one trait method. The types (`CheckResult`, `CompileResult`, etc.) are plain structs — no protocol-specific encoding.

#### New file: `src/server/types.rs`

Result types for all `CompilerService` methods. These are the canonical structured responses — both MCP and CLI format these for their respective outputs.

### PR 5b: `InProcessServer` implementation

#### New file: `src/server/in_process.rs`

```rust
pub struct InProcessServer {
    // Module cache, loaded projects, etc.
    // Initially thin: just delegates to library functions
}

impl CompilerService for InProcessServer {
    fn check(&self, path: &Path, stdlib: Option<&Path>) -> CheckResult {
        // Delegates to pluto::compile_file() or similar
    }
    // ...
}
```

The initial implementation is a thin wrapper around existing library functions. No caching, no persistence, no file watching. These are future enhancements on top of the same trait.

### PR 5c: Route CLI and MCP through `CompilerService`

#### `src/main.rs`
- Create an `InProcessServer` at startup
- Each `Commands` variant calls the corresponding `CompilerService` method instead of calling `pluto::compile_file_with_options()` etc. directly
- Format the structured result for terminal output

#### `mcp/src/server.rs` and `mcp/src/tools.rs`
- Replace direct `plutoc::` / `plutoc_sdk::` calls with `CompilerService` method calls
- The MCP server holds a `Box<dyn CompilerService>` (initially an `InProcessServer`)
- Each MCP tool handler becomes: parse MCP params → call `CompilerService` method → format MCP response

This is the largest refactor in the plan. The MCP server currently embeds significant logic (module loading, caching, cross-reference tracking). That state moves into `InProcessServer`, and the MCP tools become thin translators.

### Verification

- All existing integration tests pass (behavior is unchanged, only the call path is different)
- MCP server still works identically (test via `pluto-mcp` binary or MCP test harness)
- CLI output is identical to before

### Risks

- **Medium-High.** The MCP server currently maintains its own state (loaded modules, module cache). Extracting that into `InProcessServer` requires careful state management. The MCP server's `PlutoMcpServer` struct in `mcp/src/server.rs` has fields like `loaded_modules`, `project_modules`, etc. that need to move.
- The trait surface may need iteration. Start with the current MCP tools as the baseline and refine as needed.
- PR 5c is the largest single PR in the plan. Consider splitting it further: CLI first, then MCP.

---

## Phase 6: Frontend Unification

**Goal:** The MCP server becomes a subcommand of the `pluto` binary (`pluto mcp` or `pluto serve --mcp`). No separate `pluto-mcp` binary.

**Depends on:** Phase 5 (`CompilerService` must exist so the MCP frontend can delegate to it).

**Estimated size:** 1 PR.

### Changes

#### `src/main.rs`

Add subcommands:

```rust
/// Start the MCP server (stdio transport)
Mcp,
/// Start the compiler server (placeholder for socket server)
Serve,
/// Stop a running compiler server
Stop,
```

The `Mcp` subcommand launches the MCP stdio server in the current process. It creates an `InProcessServer` and runs the MCP protocol loop. This is functionally identical to what `pluto-mcp` does today, just embedded in the main binary.

The `Serve` and `Stop` subcommands are stubs that print "not yet implemented — socket server requires protocol design (see RFC Section 9.1)". They exist to reserve the subcommand names and signal intent.

#### `mcp/` crate

Two options (decide at implementation time):

**Option A: Keep `mcp/` as a library, remove the binary.**
- `mcp/Cargo.toml`: remove `[[bin]]` section, keep `[lib]`
- `mcp/src/main.rs`: delete
- `mcp/src/lib.rs`: exports `run_mcp_server(service: &dyn CompilerService)` or similar
- Root `Cargo.toml`: add `pluto-mcp = { path = "mcp" }` to `[dependencies]`
- `src/main.rs`: `Commands::Mcp` calls `pluto_mcp::run_mcp_server()`

**Option B: Inline MCP code into the main crate.**
- Move `mcp/src/` contents into `src/mcp/`
- Remove `mcp/` from workspace
- Adds `rmcp`, `tokio`, `tracing` as dependencies of the main crate

Option A is preferred because it keeps the MCP protocol dependency (`rmcp`, `tokio`) isolated. The main `pluto` binary gains a dependency on the `pluto-mcp` library crate, but does not link `rmcp` or `tokio` unless the `Mcp` subcommand is used. (In practice, Rust links everything statically, so the binary size increases. If this is a concern, the MCP support could be behind a cargo feature flag.)

#### CI workflows

- Update any references to `pluto-mcp` binary to `pluto mcp`
- The `pluto-mcp` binary name can be kept as an alias via a shell script or symlink for backward compatibility during transition

### Verification

- `pluto mcp` starts the MCP server on stdio (test with the existing MCP test harness)
- `pluto serve` prints the "not yet implemented" message
- `pluto stop` prints the "not yet implemented" message
- All existing tests pass
- The standalone `pluto-mcp` binary is no longer produced (or produces a deprecation warning pointing to `pluto mcp`)

### Risks

- **Low-Medium.** The MCP server is already a well-tested standalone binary. Embedding it as a subcommand is primarily a build system change. The main risk is dependency bloat — adding `tokio` and `rmcp` to the main crate's dependency tree increases compile time and binary size.
- Feature-gating MCP support (`--features mcp`) could mitigate binary size but adds build complexity. Decide at implementation time based on measured impact.

---

## Phase 7: Full Manifest Removal

**Goal:** Remove all remaining TOML parsing code and the `toml` dependency. This is the final cleanup after `.deps/` (Phase 2) has been proven as the replacement for manifest-based dependency resolution.

**Depends on:** Phase 4 (manifest prep must be complete).

**Estimated size:** 1 PR.

### Changes

#### `Cargo.toml`
- Remove `toml = "0.8"` from `[dependencies]`
- Remove `serde = { version = "1", features = ["derive"] }` ONLY if no other code uses it (it's likely still needed for `serde_json`, `bincode`, etc.)

#### `src/lib.rs`
- Verify `pub mod manifest;` is already removed (Phase 4)
- Verify no remaining `use crate::manifest::` references

#### `src/modules.rs`
- Verify no remaining `use crate::manifest::` import (line 6 currently imports `DependencyScope, PackageGraph`)
- Verify all `pkg_graph` / `current_deps` parameters are gone (Phase 4)

#### `tests/integration/manifest.rs`
- Delete the file
- Remove the `[[test]] name = "manifest"` entry from `Cargo.toml` (lines 156-157)

#### Remaining cleanup
- Grep for `pluto.toml` across the codebase — remove any remaining references in comments, docs, or error messages
- Update `CLAUDE.md` to remove manifest-related documentation
- Update RFC document to mark `pluto.toml` as removed (not just planned for removal)

### Verification

- `cargo build` succeeds without `toml` in dependencies
- `cargo test` passes
- `grep -r "pluto.toml" src/` returns no results
- `grep -r "manifest" src/` returns no results (other than unrelated uses of the word)

### Risks

- **Very low.** This is pure deletion of code that Phase 4 already disconnected. If anything still references manifest types, compilation will fail immediately with clear errors.

---

## Execution Order Recommendation

The phases can overlap significantly. A recommended timeline:

1. **Phase 1 (Rename)** — Do first. Small, mechanical, unblocks clean naming for everything else.
2. **Phases 2 + 3 + 5a** — Start in parallel after Phase 1 lands. These are independent.
3. **Phase 4** — Start after Phase 2 lands.
4. **Phase 5b + 5c** — Continue after Phase 5a lands.
5. **Phase 6** — Start after Phase 5c lands.
6. **Phase 7** — Start after Phase 4 lands. Can overlap with Phase 6.

Critical path: **Phase 1 → Phase 5a → Phase 5b → Phase 5c → Phase 6**. This is the longest chain and determines the overall timeline for full frontend unification.

The manifest removal path (**Phase 2 → Phase 4 → Phase 7**) is shorter and can complete independently.

---

## Summary Table

| Phase | Description | Depends On | PRs | Risk |
|-------|-------------|------------|-----|------|
| 1 | Rename `plutoc` → `pluto` | — | 1 | Low |
| 2 | `.deps/` import resolution | — | 1 | Low |
| 3 | Toolchain manager (`install`/`use`/`versions`) | — | 1-2 | Medium |
| 4 | Manifest removal (prep) | Phase 2 | 1-2 | Low-Medium |
| 5 | `CompilerService` trait + `InProcessServer` | — | 2-3 | Medium-High |
| 6 | Frontend unification (`pluto mcp`) | Phase 5 | 1 | Low-Medium |
| 7 | Full manifest removal | Phase 4 | 1 | Very Low |

# RFC: Pluto Toolchain Architecture

**Status:** Draft
**Priority:** Critical (foundational for all future tooling)
**Related:** [ai-native-representation.md](ai-native-representation.md), [lsp-server.md](lsp-server.md), [package-manager.md](rfc-package-manager.md), [build-cache.md](build-cache.md), [program-structure.md](program-structure.md), [compilation.md](compilation.md)

## 1. Overview

This RFC defines the architecture of the Pluto toolchain: how the compiler is structured, how it communicates with editors and AI agents, how projects are organized, how dependencies are resolved, and what principles govern all of these decisions.

The scope is the entire developer-facing surface area of Pluto — everything from `pluto compile` to the MCP protocol to the directory layout conventions. It does not cover language semantics, runtime behavior, or the type system. It covers how you interact with the compiler, not what the compiler does with your code.

### Why Now

Pluto currently ships as `plutoc`, a batch-mode CLI binary. The MCP server is a separate binary in the workspace (`mcp/`). The LSP server is unimplemented. There is a `pluto.toml` manifest for dependency management. There is a `pluto-sdk` Rust crate for programmatic AST manipulation.

These pieces were built incrementally as needs arose. They work, but they were not designed together. Before the toolchain grows further — before we ship an LSP, before we formalize the package manager, before we add profiling and debugging — we need a coherent architecture that all future work fits into.

This RFC establishes that architecture. It is prescriptive: it makes decisions, not proposals. Open questions are flagged explicitly in Section 8.

### What Changes

The major shifts from the current state:

1. `plutoc` becomes `pluto` — a single binary for all operations
2. The compiler becomes a persistent server, not a batch process
3. MCP and LSP become thin frontends to the server, not separate binaries
4. `pluto.toml` is eliminated — the filesystem is the only configuration
5. Dependency management becomes a separate internal component (but still `pluto` subcommands)
6. Project kind (app, library, test) is inferred from source code, never declared

---

## 2. Principles

Three principles govern all toolchain decisions. They are ordered by priority — when they conflict, the higher-numbered principle yields to the lower-numbered one.

### 2.1 Source + Filesystem Is Everything

The compiler derives everything it needs from `.pluto` source files and the filesystem structure. There are no configuration files. No manifests. No TOML, JSON, YAML, or any non-Pluto format.

This means:
- No `pluto.toml` (the current manifest is eliminated)
- No `.plutorc`, `.pluto-version`, or project configuration dotfiles
- No `pluto.lock` (lock files are a package manager concern, not a compiler concern)
- Project structure is determined entirely by directory layout and source code contents
- The compiler binary (`pluto`) never parses any format other than Pluto source code

Tooling-managed directories like `.deps/` (dependency cache) and `.pluto-coverage/` (coverage output) are not configuration — they are artifacts produced and consumed by specific `pluto` subcommands. The principle bans configuration files that the compiler reads to understand project structure, not all dot-prefixed directories.

The filesystem IS the project manifest. A directory containing `.pluto` files is a module. A module containing an `app` declaration is an application. A module containing `test` blocks is a test suite. A module containing `pub` declarations is a library. The compiler walks the filesystem to discover structure — it does not read metadata about structure from a separate file.

This principle exists because configuration files create a second source of truth. When the manifest says one thing and the source code says another, which is correct? In Pluto, the source code is always correct because it is the only thing that exists.

External dependency metadata — version requirements, registry locations, dependency graphs — is the package manager's job. The compiler sees only resolved source code on disk. Section 6 covers this separation in detail.

### 2.2 AI-Native

Every compiler capability is a first-class programmatic API surface. AI agents are a primary consumer of the toolchain, not an afterthought.

Concretely:
- Every operation the compiler can perform (compile, run, test, navigate, edit, analyze, profile, measure coverage, debug) is accessible through a structured API, not just a CLI
- Operations return structured data (JSON, typed messages), not human-readable terminal output
- The API is designed for concurrent access by multiple agents working on different parts of a codebase simultaneously
- Navigation and editing APIs operate on the compiler's internal representation (with stable UUIDs on declarations), not on text files with line numbers

The expected workflow for a significant fraction of Pluto projects: an AI agent authors all code, runs all tests, and iterates on the implementation. No human writes or runs anything. The toolchain must support this workflow as well as it supports a human typing `pluto run main.pluto` in a terminal.

This does not mean the human workflow is degraded. The CLI is the same CLI. The APIs exist in addition to the CLI, not instead of it. But when there is a design tension between making something pretty in a terminal and making something precise in a structured API, the API wins.

### 2.3 Eager Performance

The compiler does work upfront so that operations feel instant. Aggressive caching, warm processes, and precomputation are the default.

This principle motivates the server-first architecture (Section 3). A persistent compiler process can:
- Keep parsed and type-checked modules in memory
- Invalidate incrementally when files change
- Precompute analysis that clients haven't requested yet
- Amortize startup costs across many operations

The goal is that common operations — type-checking a file after an edit, running tests, navigating to a definition — take single-digit milliseconds from the client's perspective. The server pays the cost of loading and analyzing the project once; subsequent operations are lookups into cached state.

This principle also motivates the eventual build cache (Section 8.5): content-addressable caching of compilation artifacts, potentially shared across projects and machines.

---

## 3. Compiler Model

### 3.1 Single Binary: `pluto`

One binary. One command. Everything.

```
pluto compile <file> [-o <output>]   # Compile to native binary
pluto run <file>                     # Compile and execute
pluto test <file>                    # Compile and run tests
pluto serve                          # Start the compiler server explicitly
pluto stop                           # Stop the running server
pluto install <version>              # Download and cache a compiler version
pluto use <version>                  # Set the active compiler version
pluto versions                       # List installed compiler versions
```

The current binary is named `plutoc`. It becomes `pluto`. The `c` suffix implies "just a compiler" — but Pluto's binary is more than a compiler. It is the entire toolchain: compiler, test runner, server, version manager.

There is no `pluto-server`, `pluto-mcp`, `pluto-lsp`, or `pluto-pkg` binary. The user installs one thing, and that one thing does everything. Subcommands, not separate binaries. This includes dependency management — the package manager is a set of `pluto` subcommands (e.g., `pluto add`, `pluto update`), not a separate binary. When this RFC refers to the "package manager" as a "separate tool" or "separate concern," it means a distinct internal component with its own logic and responsibilities, not a separate binary on PATH.

### 3.2 Server-First Architecture

The compiler runs as a persistent server process. All operations go through the server. The CLI, MCP frontend, and LSP frontend are all thin clients of the same server.

#### Server Lifecycle

The server starts on first invocation of any `pluto` command. If you run `pluto compile main.pluto` and no server is running, the CLI starts one. If a server is already running, the CLI connects to it. There is one server per user per machine.

The server runs indefinitely. It is designed to be lightweight — a Pluto server with no loaded projects consumes minimal resources. There is no idle timeout. The server stays running until explicitly stopped or until the machine shuts down.

`pluto stop` performs a graceful shutdown. This is needed for two situations: upgrading the compiler (the new version needs to replace the old server), and debugging (when the server itself is misbehaving). Normal users should never need `pluto stop`.

Auto-restart on version mismatch: if the CLI detects that the running server is a different compiler version than the CLI binary, it gracefully restarts the server. The new server version replaces the old one transparently. This handles the common case of `pluto install 0.3.0 && pluto use 0.3.0` followed by `pluto compile` — the old server shuts down and the new one starts automatically.

#### Communication

The server listens on a Unix domain socket at `~/.pluto/server.sock`. All three frontends (CLI, MCP, LSP) communicate with the server over this socket using the same internal protocol.

The choice of Unix domain socket over TCP is deliberate:
- No port conflicts with other services
- No network exposure (the compiler server is not accessible from other machines)
- No TLS overhead
- Filesystem permissions handle authentication naturally (the socket is owned by the user)
- Faster than TCP loopback for high-frequency operations

Windows support is an open question (Section 8.7).

#### Architecture

```
pluto-lib (core compiler library — Rust crate)
  │
  ├── Server process (holds all state, caches, loaded modules)
  │     │
  │     ├── Internal protocol (Unix socket, structured messages)
  │     │     │
  │     │     ├── CLI frontend (thin socket client, translates CLI args)
  │     │     ├── MCP frontend (thin stdio translator, ~50 lines)
  │     │     └── LSP frontend (thin stdio translator, ~50 lines)
  │     │
  │     ├── Module cache (parsed + type-checked modules, invalidated on file change)
  │     ├── Build cache (compiled artifacts, content-addressed) [future]
  │     ├── File watcher (detects changes, triggers invalidation)
  │     └── Concurrent request handling (multiple agents/editors simultaneously)
  │
  └── Direct library API (for embedding, testing, SDK)
```

The critical point: MCP and LSP are not reimplementations of compiler logic. They are protocol translators. The MCP frontend reads JSON-RPC over stdin, translates each request into a server protocol message, sends it over the Unix socket, receives the response, and writes JSON-RPC to stdout. That is all it does. The LSP frontend does the same thing with the LSP protocol. Both are approximately 50 lines of glue code.

All real work — parsing, type checking, code generation, navigation, editing, analysis — happens in the server process. The server exposes a capability-based API (compile, run, test, navigate, edit, analyze, etc.), and the frontends map protocol-specific requests to that API.

This design has three consequences:
1. MCP, LSP, and CLI always behave identically because they call the same code
2. Adding a new frontend (e.g., a web-based IDE protocol) is trivial — just another translator
3. The server's in-memory state (loaded modules, type environments) is shared across all frontends

### 3.3 Toolchain Manager

`pluto` includes a built-in version manager. The user does not install `plutoenv`, `plutovm`, or any separate version management tool. `pluto` manages itself.

```
pluto install 0.2.0          # Download compiler version 0.2.0 to ~/.pluto/versions/0.2.0/
pluto install latest          # Download the latest release
pluto use 0.2.0              # Set 0.2.0 as the active version
pluto versions               # List installed versions, mark active
```

Compiler binaries are cached at `~/.pluto/versions/<version>/pluto`. The active version is recorded at `~/.pluto/active`. When the user runs any `pluto` command, the binary checks whether it matches the active version. If not, it delegates to the active version's binary.

The `pluto` binary itself is the only thing on the user's PATH. It acts as both the toolchain manager and the compiler. When the active version matches the running binary, commands execute directly. When they differ, the running binary exec's the active version's binary, forwarding all arguments.

#### No `.pluto-version` Files

There are no per-project version pinning files. The compiler does not look for `.pluto-version`, `.tool-versions`, or any similar dotfile. The active compiler version is a machine-global setting, not a per-project setting.

If version pinning per project becomes necessary in the future, it would be expressed in Pluto source code (e.g., a `min_version` declaration), not in a separate configuration file. This is consistent with Principle 2.1: source + filesystem is everything.

Compiler compatibility checking is the compiler's job. If a source file uses features from a newer language version, the compiler reports an error. The compiler knows what it supports — it does not need an external file to tell it.

### 3.4 Server State Model

The server maintains the following in-memory state:

**Module cache.** Every module the server has loaded is cached in memory: source text, parsed AST, type environment, error diagnostics, and derived analysis data (call graph, error sets, cross-references). Modules are keyed by their canonical filesystem path.

**File watcher.** The server watches loaded directories for filesystem changes. When a `.pluto` file changes on disk, the server invalidates the affected module and any modules that depend on it. The next request that touches an invalidated module triggers a re-parse and re-check.

**No project state.** The server does not have a concept of "the current project." It loads modules on demand as clients request them. Multiple agents can work on different projects simultaneously — they simply reference different paths. There is no `pluto init`, no project registration, no workspace configuration.

**Concurrency.** The server handles concurrent requests from multiple clients. Two MCP agents editing different files in the same project, an LSP server providing diagnostics, and a CLI user running tests — all simultaneously, all against the same server. The server's internal locking is per-module, not global: concurrent operations on different modules do not block each other.

---

## 4. Module System

### 4.1 Directory = Module

A directory containing `.pluto` files is a module. All `.pluto` files in a directory are auto-combined into a single module. This is the Go model.

```
my-project/
  main.pluto          # entry point
  auth/
    handler.pluto     # \
    middleware.pluto   #  } together these form the "auth" module
    tokens.pluto      # /
  db/
    connection.pluto   # \
    queries.pluto      #  } together these form the "db" module
    migrations.pluto   # /
```

`import auth` imports the `auth` module, which is the combination of all `.pluto` files in the `auth/` directory. There is no way to import `auth/handler.pluto` individually — the file is not a module. The directory is.

Files within a module can be organized however the developer wants. Split by feature, by type, by alphabetical order — it does not matter. The compiler sees one flat namespace per directory.

### 4.2 No File-Level Imports Within a Module

You cannot import individual files within a multi-file module. There is no `import auth.handler` syntax. `import auth` imports the entire `auth/` directory as a single module.

A single `.pluto` file can also serve as a module: `import math` resolves to `math.pluto` if no `math/` directory exists. This is the degenerate case — a module that happens to be one file. It is still imported as a module, not as a file. The distinction matters: if `math.pluto` later grows into a `math/` directory with multiple files, all importers continue to write `import math` with no changes.

Files within a multi-file module are an organizational convenience. Moving a function from `handler.pluto` to `middleware.pluto` within the `auth/` directory is a no-op from the module system's perspective — no imports change, no visibility changes, no API changes.

### 4.3 Visibility

Within a module, everything sees everything. All functions, classes, traits, enums, and errors defined in any file within the same directory are visible to all other files in that directory. No `pub` needed within a module.

`pub` controls external visibility only. A declaration marked `pub` is visible to other modules that import this module. A declaration without `pub` is module-private.

```
// auth/tokens.pluto
fn validate_signature(token: string) bool {    // private to auth module
    // ...
}

pub fn verify(token: string) bool {            // visible to importers of auth
    return validate_signature(token)
}
```

This matches Go's exported/unexported model (uppercase vs. lowercase in Go, `pub` vs. no-`pub` in Pluto). The reasoning is the same: the module boundary is the meaningful API boundary, not the file boundary.

### 4.4 Duplicate Names = Compile Error

If two files in the same directory define a function, class, trait, enum, or error with the same name, that is a compile error. There is no implicit merging, no last-file-wins behavior, no partial class definitions.

```
// auth/handler.pluto
fn process_request(r: Request) Response { ... }

// auth/middleware.pluto
fn process_request(r: Request) Response { ... }    // ERROR: duplicate definition
```

The error message reports both file locations and the conflicting name. This rule applies to all top-level declarations. Method names within different classes do not conflict (they are scoped to their class).

### 4.5 Import Resolution

Import resolution follows this chain:

1. **Sibling directory** — `import auth` looks for `auth/` relative to the importing file's directory
2. **Sibling file** — if no directory, looks for `auth.pluto` relative to the importing file's directory
3. **Standard library** — `import std.math` resolves to the stdlib's `math/` module

This is the currently implemented resolution chain. The full resolution chain — including external dependencies from `.deps/`, walk-up behavior for monorepo layouts, and registry-sourced packages — is deferred to the package manager project (Section 6). The compiler's resolution logic will be extended at that point; for now, it handles local and stdlib imports only.

### 4.6 Current Implementation

The current module system in `src/modules.rs` implements the flatten-before-typeck design: imported modules are parsed, their `pub` declarations are collected, and those declarations are merged into the importing module's namespace with prefixed names (e.g., `math.add`, `auth.verify`). Type checking and code generation see a flat namespace with dotted names.

This design carries forward unchanged. The module system's semantics are stable — what changes in this RFC is the discovery mechanism (how modules are found on disk), not the flattening mechanism (how modules are merged into a program).

---

## 5. Project Kinds

Pluto projects do not declare their kind in a configuration file. The compiler infers project kind from the source code.

### 5.1 App

A module containing an `app` declaration (or a `stage` subclass declaration like `daemon`, `http_server`, etc.) is an application. It compiles to a native binary. The compiler generates a `main()` entry point that performs dependency injection wiring, allocates singletons, and calls the app's `main` method (or the stage's lifecycle methods).

```
app OrderService[db: Database, cache: RedisCache] {
    fn main(self) {
        // ...
    }
}
```

Detection: the compiler scans top-level declarations for `app` or any stage-derived declaration. If found, the module is an application.

A module may contain at most one app/stage declaration. Multiple app declarations in the same module is a compile error. The `system` declaration (Section 5.4) composes multiple apps into a distributed system — it does not put multiple apps in one module.

### 5.2 Test

A module containing `test` blocks is a test suite. It compiles to a test runner binary that discovers and executes all test blocks.

```
test "order processing handles empty cart" {
    let processor = OrderProcessor { ... }
    expect(processor.handle(empty_cart)).to_equal(EmptyCartError)
}
```

Detection: the compiler scans for `test` declarations. If found, the module is a test suite.

Test modules can coexist with app or library code. A file containing both functions and test blocks compiles as a test suite when invoked with `pluto test`, and the test blocks are stripped when invoked with `pluto compile` or `pluto run`. This is already how the current compiler works (see `run_frontend()` in `src/lib.rs`: test functions are retained in test mode and stripped otherwise).

### 5.3 Library

Any module with `pub` declarations is importable as a library. There is no `lib` declaration, no library manifest, no special project structure. If your module has `pub fn add(a: int, b: int) int`, other modules can `import` it and call `add`.

Libraries are the implicit default. If a module is not an app and not a test suite, it is a library (or an internal module — there is no meaningful distinction from the compiler's perspective).

### 5.4 System (Future)

The `system` declaration defines the topology of a distributed application: which stages exist, how they communicate, what their deployment constraints are. This is Layer 3 of the program structure model (see `docs/design/program-structure.md`).

```
system OrderPlatform {
    api: OrderApi              // an http_server stage
    worker: OrderWorker        // a daemon stage
    reporter: DailyReport      // a scheduled_job stage
}
```

The `system` construct is explicitly undesigned in this RFC. The current implementation supports parsing and basic compilation of system declarations (`src/stages.rs`, `Commands::Compile` system file handling in `src/main.rs`), but the full distributed topology semantics — inter-stage communication, deployment annotations, geographic placement — are open research. A Datalog-inspired constraint language for expressing topology rules is one direction under consideration.

What this RFC establishes about `system`:
- It will be a top-level declaration in Pluto source code, not a configuration file
- It will be inferred from source (the presence of `system` makes the module a system definition)
- It will compose existing stages, not replace them
- The full design is a separate RFC

### 5.5 No Manifest Determines Project Kind

This point is worth emphasizing because it is unusual. Most languages have a project manifest that declares "this is a binary" or "this is a library" (`Cargo.toml`'s `[[bin]]` vs `[lib]`, `package.json`'s `main` vs `bin`). Pluto has no such manifest.

The compiler looks at the source code. If it sees `app`, it builds a binary. If it sees `test`, it builds a test runner. If it sees `pub` declarations, it is importable. The source code is self-describing.

This eliminates an entire class of errors: the manifest says "library" but the code has a `main`, the manifest says "binary" but there is no entry point, the manifest lists a file that does not exist. In Pluto, the code is the specification.

---

## 6. Dependency Management

### 6.1 Separation of Concerns

The compiler resolves imports from the filesystem. The package manager puts code on the filesystem. These are two different concerns within the `pluto` binary, with a clean internal boundary between them.

```
┌──────────────────────────────────────────┐
│  Package Manager (future, separate tool) │
│                                          │
│  - Reads dependency specs (from where?)  │
│  - Fetches code from registries/git/etc  │
│  - Resolves versions                     │
│  - Writes source to .deps/ directory     │
│  - Manages lock file                     │
│  - Manages .deps/                        │
└──────────────────┬───────────────────────┘
                   │ Writes .pluto files to disk
                   ▼
┌──────────────────────────────────────────┐
│  Compiler (pluto)                        │
│                                          │
│  - Reads .pluto files from filesystem    │
│  - Resolves imports to directories       │
│  - Knows nothing about git, URLs,        │
│    registries, versions, or packages     │
│  - Sees .deps/ as just another directory │
└──────────────────────────────────────────┘
```

The compiler has zero knowledge of git, URLs, registries, version numbers, or package metadata. It resolves `import foo` to a directory on disk. Whether that directory got there because a human created it, because a package manager fetched it, or because an AI agent synthesized it — the compiler does not know and does not care.

This is a deliberate design choice with significant implications:

**The compiler never fetches code.** Running `pluto compile main.pluto` never triggers a network request. If an import cannot be resolved to a local directory, compilation fails with an error. This makes compilation hermetic and reproducible — the same files on disk always produce the same result.

**The compiler never parses non-Pluto formats.** The `pluto` binary does not contain a TOML parser, a JSON parser, or a YAML parser (serde_json is used internally for coverage data, but not for project configuration). It reads `.pluto` source files and nothing else.

**Version resolution is not the compiler's problem.** If `foo` version 1.2 and `foo` version 1.3 both satisfy the dependency constraints, the package manager decides which one to use. The compiler sees whichever version the package manager placed on disk.

### 6.2 The `.deps/` Directory

External dependencies are placed in a `.deps/` directory by the package manager. The compiler's import resolution chain will be extended to look in `.deps/` after checking sibling directories and before checking the stdlib:

1. Sibling directory — `import foo` looks for `foo/` relative to the importing file
2. Sibling file — `import foo` looks for `foo.pluto` relative to the importing file
3. Dependencies directory — `import foo` looks for `.deps/foo/`
4. Standard library — `import std.math` resolves to the stdlib

The exact resolution chain — including walk-up behavior for monorepo layouts (looking for `.deps/` in parent directories) and handling of transitive dependencies — is deferred to the package manager project. The compiler will implement whatever resolution logic the package manager design requires.

### 6.3 Current State: `pluto.toml`

The current codebase has a `pluto.toml` manifest system (`src/manifest.rs`) that supports path dependencies and git dependencies. The `plutoc update` command fetches git dependencies. This system works but violates Principle 2.1 (source + filesystem is everything).

The migration plan:
1. The `pluto.toml` manifest is removed from the compiler
2. The `plutoc update` (now `pluto update`) command is removed from the compiler
3. Dependency specification and fetching moves to dedicated `pluto` subcommands (`pluto add`, `pluto update`)
4. The compiler gains `.deps/` resolution in its import chain
5. The package manager subcommands write resolved dependencies to `.deps/`

The package manager's own configuration format (how users specify dependencies) is an open question for the package manager project. It could be a TOML file that the package manager reads, dependency declarations in Pluto source code, or something else entirely. The compiler is not involved in this decision.

### 6.4 Default Registry

There will be a language-default registry for Pluto packages. The details — centralized vs. federated, trust model, immutability guarantees, API design — are deferred to the package manager project. What this RFC establishes:

- A default registry will exist (users should not need to configure a registry to use packages)
- The registry is a package manager concern, not a compiler concern
- Per-import registry overrides may be supported in the future (expressed in source code, not config files)

---

## 7. MCP Integration

### 7.1 Architecture: Thin Frontend

The MCP server is a thin stdio-to-socket translator. It accepts MCP protocol messages (JSON-RPC over stdin/stdout, as editors and AI agents expect), translates each request into the compiler server's internal protocol, sends it over the Unix socket, receives the response, and writes the MCP response to stdout.

```
┌──────────────┐     stdio      ┌──────────────────────┐
│  AI Agent    │ ──────────────▶│  MCP Frontend        │
│  (Claude,    │◀───────────────│  (~50 lines of Rust) │
│   Cursor,    │                └──────────┬───────────┘
│   etc.)      │                           │ Unix socket
└──────────────┘                           ▼
                                ┌──────────────────────┐
                                │  Compiler Server     │
                                │  (all real logic)    │
                                └──────────────────────┘
```

The MCP frontend contains no compiler logic. It does not parse Pluto source code, does not maintain module state, does not cache anything. It is a protocol adapter. When the server restarts (e.g., due to a version upgrade), the MCP frontend reconnects automatically.

The current MCP server (`mcp/src/`) is a standalone binary that embeds compiler logic directly. In the target architecture, it becomes a thin wrapper that delegates everything to the compiler server.

### 7.2 Path-Based, No Sessions

Every MCP tool call specifies paths. There are no session IDs, no context objects, no "current project" state in the MCP protocol.

```json
{
    "tool": "check",
    "params": {
        "path": "/Users/dev/myproject/main.pluto",
        "stdlib": "/Users/dev/.pluto/stdlib"
    }
}
```

The server manages state internally — it caches loaded modules, maintains file watchers, keeps type environments warm. But the client does not need to manage this state. Every request is self-contained: "check this file," "run this file," "find usages of this declaration in this module."

This design enables concurrent multi-agent workflows naturally. Two AI agents working on different parts of a codebase simply pass different paths. They do not interfere with each other's sessions because there are no sessions. The server handles concurrency internally through per-module locking.

### 7.3 Operating Within the Import Graph

The MCP server does not operate on isolated files. When a module is loaded, the server resolves its imports, loads all transitive dependencies, and operates within the full resolved import graph.

Loading `/Users/dev/myproject/main.pluto` may trigger loading of `auth/`, `db/`, `std.math`, and any other imports. Subsequent queries — "find all callers of this function," "what is the error set of this function?" — search the entire loaded graph, not just the single file.

This is consistent with Pluto's whole-program compilation model. The compiler needs the full program to do meaningful analysis (error set inference, DI validation, call graph construction). The MCP server exposes this analysis to AI agents.

### 7.4 Current API Surface

The current MCP server exposes these capabilities (implemented in `mcp/src/tools.rs` and `mcp/src/server.rs`):

**Navigation:**
- `load_module` — load and analyze a `.pluto` file, return declaration summary
- `load_project` — scan a directory, load all `.pluto` files
- `list_modules` — list currently loaded modules
- `list_declarations` — list declarations in a module, optionally filtered by kind
- `get_declaration` — deep inspection of a declaration (params, types, error sets, methods, fields, source)
- `find_declaration` — search for a declaration by name across all loaded modules
- `callers_of` — find all call sites of a function
- `usages_of` — find all usages of a declaration (calls, constructions, enum usages, raise sites)
- `constructors_of` — find all struct literal construction sites
- `raise_sites_of` — find all sites where an error is raised
- `enum_usages_of` — find all usages of an enum variant
- `call_graph` — build caller/callee graph from a function
- `error_set` — get error handling info for a function

**Editing:**
- `add_declaration` — add a new top-level declaration
- `replace_declaration` — replace a declaration with new source (must be same kind)
- `delete_declaration` — remove a declaration, report dangling references
- `rename_declaration` — rename and update all references within file
- `add_field` — add a field to a class
- `add_method` — add a method to a class

**Building and running:**
- `check` — type-check a file, return structured diagnostics
- `compile` — compile to native binary
- `run` — compile and execute, capture stdout/stderr
- `test` — compile in test mode and execute test runner

**Source access:**
- `get_source` — get source text, optionally at a byte range
- `pretty_print` — pretty-print a module or declaration as Pluto source
- `module_status` — check which modules are stale (modified on disk since load)
- `reload_module` — reload from disk, discarding cache
- `sync_pt` — sync human edits from `.pt` text back to `.pluto` binary

**Documentation:**
- `docs` — Pluto language reference (types, operators, statements, etc.)
- `stdlib_docs` — stdlib module documentation (function signatures and descriptions)

### 7.5 Future API Surfaces

The following capabilities are planned but not yet implemented. Each will be exposed through the same MCP protocol as new tool definitions:

**Profiling.** Compile with profiling instrumentation, run, collect profile data, and return structured profiling results (hot functions, call counts, timing). The server API returns structured data; the MCP frontend translates it to the MCP response format.

**Code coverage.** The current `--coverage` flag on `pluto run` and `pluto test` writes coverage data to `.pluto-coverage/`. The MCP API will expose this: instrument a run, collect data, return structured coverage results (line-level, function-level, branch-level). LCOV output is already implemented; the MCP layer adds structured access.

**Debugging.** Compile with debug info, launch with a debug adapter, expose breakpoints/stepping/variable inspection through the MCP protocol. The debug adapter protocol (DAP) may be a separate frontend to the server, similar to MCP and LSP.

**Build cache.** Expose cache status, cache hits/misses, and cache management operations through the API. Allow agents to query whether a module needs recompilation or can be served from cache.

---

## 8. Deliberate Simplifications

### 8.1 Git-Unaware

The compiler and server know nothing about git. They do not read `.git/`, do not check branch names, do not look at commit history, do not use git hashes for caching.

This is a deliberate simplification that avoids enormous complexity:
- Branch switching = files changed on disk = the server's file watcher detects changes and invalidates affected modules
- Git worktrees = separate directories = naturally isolated in the server (different paths, different cached modules)
- Rebasing, merging, cherry-picking = more files changed on disk = more invalidation

The server's file watcher (using `notify` crate, already implemented in `src/watch.rs`) detects all filesystem changes regardless of their cause. Whether a file changed because the user edited it, because git checkout replaced it, or because an AI agent wrote it through the SDK — the server sees a changed file and invalidates its cache.

This means the compiler cannot do git-aware optimizations like "only recheck files that changed since the last commit." That is an acceptable tradeoff. Content-based invalidation (file hash changed = invalidate) is simpler, more general, and correct in all cases.

### 8.2 One Server Per User

There is exactly one compiler server per user per machine. Not one per project, not one per workspace, not one per terminal session.

This means:
- All projects on the machine share the same server process
- Module caches from different projects coexist in the same server
- Memory usage grows with the number of loaded projects (but modules are loaded on demand, not eagerly)

The alternative — one server per project — would require the user to manage server lifetimes, would waste memory for common dependencies (the stdlib would be loaded N times), and would complicate the CLI (which project's server should `pluto compile` connect to?).

The one-server model is simpler and works well for the expected usage pattern: a developer (or AI agent) works on 1-3 projects concurrently, with substantial overlap in dependencies. The server's memory usage is bounded by the number of distinct modules loaded, and modules are evicted based on access patterns when memory pressure is high.

---

## 9. Open Questions

These are explicitly unresolved areas. Each is flagged for a future design effort.

### 9.1 Server Protocol Design

The internal protocol between frontends and the compiler server is not specified in this RFC. Open questions:
- Message format (protobuf, flatbuffers, msgpack, custom binary, JSON)
- Request/response multiplexing (one connection per client? shared connection pool?)
- Streaming responses for long-running operations (compilation progress, test output)
- Error handling and retransmission semantics
- Protocol versioning (server and client at different protocol versions)

### 9.2 `system` Declaration Design

The distributed topology layer is undesigned. Open questions:
- How are inter-stage communication channels declared?
- What deployment constraints can be expressed? (co-location, geographic, resource)
- Is a Datalog-inspired constraint language the right model?
- How does the `system` interact with the runtime (process supervision, crash recovery)?
- Can systems compose (system of systems)?

### 9.3 Import Resolution Chain

The full import resolution chain is deferred to the package manager project. Open questions:
- Walk-up behavior: does the compiler look for `.deps/` in parent directories? How far up?
- Transitive dependencies: does `foo` in `.deps/` have its own `.deps/`? How is the resolution chain constructed for nested dependencies?
- Name conflicts: what happens when a local module and a dependency have the same name?
- Aliased imports: `import foo as bar` — needed for conflict resolution?

### 9.4 Registry Design

The package registry is deferred to the package manager project. Open questions:
- Centralized (crates.io model) vs. federated (Go proxy model) vs. content-addressed (Zig model)
- Trust model: who can publish? Can packages be yanked?
- Immutability guarantees: once published, can a version be changed?
- Namespace management: flat (crates.io) vs. scoped (@org/pkg) vs. URL-based (Go)
- Discoverability: search, categories, documentation hosting

### 9.5 Build Cache Design

The build cache is deferred to a future design effort. Open questions:
- Content-addressable? (hash of source + dependencies + compiler version = cache key)
- Shared across projects? (same dependency at same version = same cached artifact)
- Shared across machines? (remote cache for CI pipelines)
- What is cached? (parsed ASTs? type environments? compiled objects? linked binaries?)
- Eviction policy: LRU? Size-bounded? Manual cleanup?

### 9.6 Profiling/Coverage/Debugging Integration

The integration of profiling, code coverage, and debugging into the server API is deferred. Open questions:
- Profiling: CPU profiling only, or also memory/allocation profiling?
- Coverage: how does coverage interact with monomorphization? (generic function instantiated 5 times — is each tracked separately?)
- Debugging: embed a debug adapter, or delegate to an external debugger (lldb)?
- All three: how do instrumented builds interact with the build cache? (instrumented and non-instrumented artifacts need separate cache entries)

### 9.7 Windows Support

The server architecture assumes Unix domain sockets. Windows does not have Unix domain sockets (Windows 10 1803+ has partial support via the AF_UNIX address family, but it is not universally available). Open questions:
- Named pipes? (Windows-native, well-supported, but different API)
- TCP loopback with a random port? (works everywhere, but requires port management and has security implications)
- Conditional compilation with platform-specific socket backends?

This is not blocking — Pluto currently targets macOS and Linux. Windows support is a future concern.

---

## 10. Migration Path

### 10.1 Context

Pluto is a pre-alpha language. There are no production users. Breaking changes to user programs, CLI interfaces, and project structure are acceptable.

The constraint is the compiler repository itself: `master` must remain green at every incremental step. Every commit must pass all tests. The migration is implemented as a series of backward-compatible (or cleanly breaking) changes, not a single large rewrite.

### 10.2 Key Migration Steps

The following are the major milestones. A detailed phased implementation plan will be a separate follow-up document.

**`plutoc` to `pluto` rename.** The binary changes from `plutoc` to `pluto`. The crate name in `Cargo.toml` changes from `plutoc` to `pluto`. All subcommands remain the same. This is a clean break — the old name stops working. CI, documentation, and examples are updated in the same commit.

**Server architecture.** The compiler library (`pluto-lib`) gains a server mode. The server process is extracted from the library. The CLI is refactored to connect to the server instead of calling the library directly. Initially, the server is in-process (library calls, no socket) to validate the API surface. Then it moves to a separate process with socket communication.

**Frontend extraction.** The MCP server (`mcp/`) is refactored from a standalone binary embedding compiler logic to a thin stdio-to-socket translator. The LSP server is implemented as a similar thin translator. Both are compiled into the `pluto` binary as subcommands (`pluto serve --mcp`, `pluto serve --lsp`, or similar).

**`pluto.toml` removal.** The manifest system (`src/manifest.rs`, `src/git_cache.rs`) is extracted from the compiler into the separate package manager tool. The compiler's import resolution is simplified to filesystem-only. The `update` subcommand is removed from the compiler CLI.

**Toolchain manager.** The `install`, `use`, and `versions` subcommands are added to the `pluto` binary. The version management directory structure (`~/.pluto/versions/`) is established. The auto-delegation mechanism (CLI binary exec's the active version) is implemented.

**`.deps/` resolution.** The compiler's import resolution chain is extended to include `.deps/` as a resolution target. This is the integration point where the package manager's output becomes visible to the compiler.

### 10.3 Incremental Compatibility

During the migration, the compiler maintains backward compatibility where possible:
- If `pluto.toml` exists but the package manager is not installed, the compiler warns but does not error
- Import resolution falls back to the current behavior (sibling directory/file, stdlib) when `.deps/` does not exist
- The server architecture is opt-in initially (`pluto serve` starts the server, CLI commands work both with and without a running server)

The goal is that existing Pluto programs continue to compile at every step of the migration. Features are added, then old features are deprecated, then deprecated features are removed.

---

## 11. Directory Layout Reference

### 11.1 User Home

```
~/.pluto/
  server.sock              # Unix domain socket for compiler server
  active                   # Symlink or file pointing to active version
  versions/
    0.1.0/
      pluto                # Compiler binary for version 0.1.0
    0.2.0/
      pluto                # Compiler binary for version 0.2.0
  cache/                   # Build cache (future)
```

### 11.2 Project Layout

```
my-project/
  main.pluto               # Entry point (contains app declaration)
  auth/
    handler.pluto           # \
    middleware.pluto         #  } "auth" module
    tokens.pluto            # /
  db/
    connection.pluto        # \
    queries.pluto           #  } "db" module
  models/
    order.pluto             # \
    user.pluto              #  } "models" module
  .deps/                    # Managed by package manager (not compiler)
    some_library/
      lib.pluto
    another_library/
      utils/
        helpers.pluto
```

No config files in the project root. No manifest. No lock file (that is inside `.deps/` or managed by the package manager tool). The project is entirely described by its `.pluto` files and directory structure.

### 11.3 Standard Library

```
stdlib/
  prelude.pluto            # Auto-imported into every program
  math/
    *.pluto                # std.math module
  strings/
    *.pluto                # std.strings module
  collections/
    *.pluto                # std.collections module
  json/
    *.pluto                # std.json module
  http/
    *.pluto                # std.http module
  fs/
    *.pluto                # std.fs module
  net/
    *.pluto                # std.net module
  io/
    *.pluto                # std.io module
  time/
    *.pluto                # std.time module
  log/
    *.pluto                # std.log module
  random/
    *.pluto                # std.random module
  regex/
    *.pluto                # std.regex module
  uuid/
    *.pluto                # std.uuid module
  base64/
    *.pluto                # std.base64 module
  env/
    *.pluto                # std.env module
  path/
    *.pluto                # std.path module
  socket/
    *.pluto                # std.socket module
  wire/
    *.pluto                # std.wire module
  rpc/
    *.pluto                # std.rpc module
```

The stdlib follows the same directory = module convention. Each subdirectory is a module importable via `import std.<name>`. The stdlib is distributed with the compiler binary (embedded or co-located).

---

## 12. Comparison With Existing Toolchains

### 12.1 Go

Go's toolchain shares several design choices with Pluto's: single binary (`go`), directory = package, visibility by convention (`pub` in Pluto, uppercase in Go), no config for project kind.

Where Pluto differs:
- Go has `go.mod` and `go.sum` — Pluto has no compiler-level manifest
- Go's toolchain fetches dependencies — Pluto's compiler does not touch the network
- Go has no persistent server architecture (gopls is separate from the compiler)
- Go has no MCP/AI-native API surface

### 12.2 Deno

Deno's approach — the runtime IS the toolchain (`deno run`, `deno test`, `deno lint`, `deno fmt`) — is the closest precedent for Pluto's single-binary model.

Where Pluto differs:
- Deno uses URL-based imports — Pluto uses filesystem-only imports
- Deno embeds V8 — Pluto compiles to native code
- Deno has `deno.json` configuration — Pluto has no configuration files
- Deno's LSP is `deno lsp` (same binary) — Pluto follows the same pattern

### 12.3 Zig

Zig's content-addressed dependencies and minimal tooling philosophy align with Pluto's principles.

Where Pluto differs:
- Zig has `build.zig.zon` — Pluto has no manifest
- Zig has no persistent server — Pluto's server-first architecture is a significant departure
- Zig does not have an AI-native API surface
- Zig's build system is in Zig — Pluto has no build system DSL (the compiler handles everything)

### 12.4 Rust (Cargo + rust-analyzer)

Rust's toolchain is the anti-pattern that Pluto's design explicitly avoids:
- Two implementations of language semantics (rustc + rust-analyzer)
- Heavy configuration (`Cargo.toml` is 50-100 lines in real projects)
- Compiler and package manager tightly coupled (Cargo invokes rustc)
- No persistent server in the compiler (rust-analyzer is a separate long-running process)

Pluto's single-server-multiple-frontend model eliminates the rustc/rust-analyzer divergence problem entirely. There is one implementation of the type checker, one implementation of error inference, one implementation of name resolution. MCP, LSP, and CLI all use it.

---

## 13. Summary of Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Binary name | `pluto` (not `plutoc`) | Single binary = single name, not "just a compiler" |
| Number of binaries | One | Users install one thing |
| Server architecture | Persistent, one per user | Eager performance, shared caches |
| Server communication | Unix domain socket | No port conflicts, no network exposure |
| MCP/LSP architecture | Thin frontends to server | No reimplementation, no divergence |
| Configuration files | None | Source + filesystem is everything |
| Project kind | Inferred from source | No manifest needed |
| Module granularity | Directory | Files are organizational, directories are semantic |
| Visibility model | `pub` for external, everything visible within module | Go model — module is the API boundary |
| Duplicate names | Compile error | No implicit merging or last-file-wins |
| Dependency fetching | Not the compiler's job | Package manager is separate |
| Git awareness | None | Filesystem changes are filesystem changes |
| Version management | Built into `pluto` | No separate version manager tool |
| Import resolution | Local filesystem, then stdlib | Extended with `.deps/` when package manager ships |
| Registry | Future, default exists | Package manager project scope |
| Build cache | Future, content-addressed | Eager performance principle |
| Windows sockets | Open question | Not blocking — macOS/Linux only for now |

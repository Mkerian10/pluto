# RFC: Global Content-Addressed Build Cache

**Status:** Proposed
**Priority:** Medium
**Related:** [package-manager.md](package-manager.md), [stdlib-embedding.md](stdlib-embedding.md)

## Problem

Today, the only caching Pluto does is `.pluto-cache/runtime.o` in the current working directory. This means:

1. **The C runtime is recompiled** if you build from a different directory
2. **No caching of compiled Pluto code** — every `plutoc compile` runs the full pipeline
3. **Branch switching is expensive** — nothing is shared between branches (no cache at all)
4. **Worktrees waste work** — `../pluto-ast-uuids` and `../pluto-reflection` each compile the same runtime independently
5. **CI recompiles everything** every run

## Prior Art

### Go: Global, Content-Addressed (the gold standard)

- **Location:** `$GOCACHE` (default `~/.cache/go-build`)
- **Key strategy:** `ActionID = hash(source + compiler_version + flags + deps)` — content-based, NOT timestamp-based
- **Branch switching:** Free — same source on different branch = same hash = cache hit
- **Remote cache:** Go 1.24's `GOCACHEPROG` protocol — pluggable external cache via JSON over stdin/stdout

### Zig: Content-Addressed, Global + Local

- **Global:** `~/.cache/zig/o/{hash}` — artifacts keyed by content hash
- **Local:** `./zig-cache/` — project-specific working files
- **Natural deduplication:** Identical inputs from different projects produce the same key

### Cargo: Project-Local (the cautionary tale)

- **Location:** `./target/` per project
- **Branch switching:** Painful — half the cache invalidates
- **Multi-project:** No sharing between projects
- **Known problem:** A 2025 Rust project goal is to rework the layout to fix this

## Proposed Design

### Cache Location

```
~/.pluto/cache/
  objects/          # Compiled object files, keyed by content hash
    {hash}.o
  runtime/          # Compiled C runtime, keyed by runtime source hash + cc version
    {hash}.o
  stdlib/           # Parsed/type-checked stdlib modules (future)
    {hash}.bin
  tmp/              # Temporary files during compilation
```

Default: `~/.pluto/cache/`. Override: `$PLUTO_CACHE` environment variable.

### Cache Key Strategy

Content-addressed hashing. The cache key for a compilation unit is:

```
key = SHA256(
    source_text,          # The .pluto source being compiled
    compiler_version,     # plutoc version string (e.g., "0.1.0+abc123")
    target_triple,        # e.g., "aarch64-apple-darwin"
    optimization_level,   # debug vs release (future)
    dependency_hashes,    # hashes of all imported modules (transitive)
)
```

This means:
- **Same source, different branch** → cache hit (same hash)
- **Same source, different compiler version** → cache miss (correct, compiler may generate different code)
- **Same source, different dependency** → cache miss (correct, semantics may differ)
- **Same source, different directory** → cache hit (path is NOT part of the key)

### What Gets Cached

#### Phase 1: Runtime Object

Replace the current `.pluto-cache/runtime.o` with a global cache entry:

```
Key: SHA256(gc.c + threading.c + builtins.c + builtins.h + cc_version + target + flags)
Value: runtime.o (the linked relocatable object)
```

This alone fixes the "recompile runtime from every directory" problem.

#### Phase 2: Compiled Object Files

Cache the Cranelift-generated object bytes for each compilation unit:

```
Key: SHA256(source + compiler_version + target + dep_hashes)
Value: object bytes (.o)
```

For single-file programs, this caches the entire compilation. For multi-module programs, each module can be cached independently.

#### Phase 3: Parsed/Type-Checked Modules (Future)

Cache the result of parsing + type-checking for stdlib and library modules:

```
Key: SHA256(source + compiler_version)
Value: serialized TypeEnv + Program (the analyzed module)
```

This would make `import std.json` near-instantaneous on subsequent builds.

### Cache Operations

```rust
pub struct BuildCache {
    root: PathBuf,  // ~/.pluto/cache/
}

impl BuildCache {
    /// Look up a cached artifact by content hash.
    fn get(&self, key: &[u8; 32]) -> Option<Vec<u8>>;

    /// Store an artifact in the cache.
    fn put(&self, key: &[u8; 32], value: &[u8]) -> Result<()>;

    /// Compute cache key for a compilation unit.
    fn compilation_key(
        source: &str,
        compiler_version: &str,
        target: &str,
        dep_hashes: &[&[u8; 32]],
    ) -> [u8; 32];
}
```

### Concurrency Safety

Multiple `plutoc` processes (parallel builds, worktrees, CI) may access the cache simultaneously:

- **Atomic writes:** Write to `tmp/` first, then rename into `objects/`. Renames are atomic on POSIX.
- **Duplicate work is OK:** Two processes computing the same key concurrently will both write the same content. The rename ensures the final file is always valid.
- **No locks needed:** Content-addressed storage is naturally idempotent — writing the same hash twice produces the same result.

This is the same approach Go uses: "They will coordinate using operating system file locks and may duplicate effort but will not corrupt the cache."

### Cache Cleaning

```
plutoc cache clean              # Remove all cached artifacts
plutoc cache clean --older 30d  # Remove artifacts older than 30 days
plutoc cache info               # Show cache size and statistics
```

### Future: Remote Cache Protocol

Inspired by Go 1.24's `GOCACHEPROG`, allow plugging in a remote cache:

```
PLUTO_CACHE_PROG=my-remote-cache plutoc compile main.pluto
```

The external program receives JSON messages on stdin:
```json
{"op": "get", "key": "abc123..."}
{"op": "put", "key": "abc123...", "size": 12345}
{"op": "close"}
```

This lets organizations plug in S3, GCS, or shared NFS caches without modifying the compiler.

## Implementation Plan

1. **Phase 1:** Global runtime cache (`~/.pluto/cache/runtime/`)
   - Replace `.pluto-cache/runtime.o` with content-addressed global entry
   - Key: hash of C sources + cc version + target + flags
   - Immediate benefit: no re-compilation across directories/worktrees

2. **Phase 2:** Compiled object cache (`~/.pluto/cache/objects/`)
   - Cache Cranelift output by content hash
   - Requires computing dependency hashes for cache key
   - Enables incremental-feeling builds

3. **Phase 3:** Module analysis cache
   - Cache parsed + type-checked stdlib modules
   - Requires serializing TypeEnv (non-trivial)

4. **Phase 4:** Remote cache protocol
   - Design JSON protocol
   - Implement stdin/stdout bridge

## Open Questions

- Should the cache be opt-out (`--no-cache`) or opt-in (`--cache`)? Opt-out (on by default) seems right.
- How does the test framework interact? `plutoc test --no-cache` already exists for test randomization — should it also skip the build cache?
- Should `plutoc cache` be a subcommand group, or just flags on `plutoc compile`?
- What's the eviction policy? LRU? Age-based? Size limit? Go uses no automatic eviction (requires `go clean -cache`).

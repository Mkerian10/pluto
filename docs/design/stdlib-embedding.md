# RFC: Embed Standard Library in the Compiler Binary

**Status:** Proposed
**Priority:** High (prerequisite for distribution)
**Related:** [build-cache.md](build-cache.md), [unified-binary.md](unified-binary.md)

## Problem

The Pluto stdlib lives on the filesystem (`stdlib/` directory) and requires one of:
- `--stdlib <path>` CLI flag
- `PLUTO_STDLIB` environment variable
- A sibling `stdlib/` directory next to the source file

If someone receives a standalone `plutoc` binary and writes `import std.json`, compilation fails. The prelude and C runtime are already embedded via `include_str!()` — the stdlib is the odd one out.

## Prior Art

| Language | Stdlib Distribution | Notes |
|----------|-------------------|-------|
| Go | Source shipped in tarball, compiled on demand | Switched from precompiled (Go 1.19) to source-only (Go 1.20), reducing size from 140MB to 60MB |
| Rust | Precompiled `.rlib` files per target | Requires `rustup target add` for cross-compilation |
| Zig | Source subsets embedded in binary | 130MB of libc headers compressed into ~50MB binary via deduplication |
| Deno | Remote packages (JSR), fetched on demand | Stdlib not embedded, but runtime is self-contained |

## Proposed Solution

### Embed stdlib sources via `include_str!()`

```rust
// src/stdlib_embedded.rs (generated or manual)
pub struct EmbeddedStdlib {
    pub modules: &'static [(&'static str, &'static str)],  // (name, source)
}

pub static STDLIB: EmbeddedStdlib = EmbeddedStdlib {
    modules: &[
        ("std.strings", include_str!("../stdlib/strings/strings.pluto")),
        ("std.math", include_str!("../stdlib/math/math.pluto")),
        ("std.json", include_str!("../stdlib/json/json.pluto")),
        ("std.collections", include_str!("../stdlib/collections/collections.pluto")),
        ("std.fs", include_str!("../stdlib/fs/fs.pluto")),
        ("std.http", include_str!("../stdlib/http/http.pluto")),
        ("std.io", include_str!("../stdlib/io/io.pluto")),
        ("std.log", include_str!("../stdlib/log/log.pluto")),
        ("std.net", include_str!("../stdlib/net/net.pluto")),
        ("std.socket", include_str!("../stdlib/socket/socket.pluto")),
        ("std.time", include_str!("../stdlib/time/time.pluto")),
        ("std.env", include_str!("../stdlib/env/env.pluto")),
        ("std.path", include_str!("../stdlib/path/path.pluto")),
        ("std.random", include_str!("../stdlib/random/random.pluto")),
        ("std.regex", include_str!("../stdlib/regex/regex.pluto")),
        ("std.base64", include_str!("../stdlib/base64/base64.pluto")),
        ("std.uuid", include_str!("../stdlib/uuid/uuid.pluto")),
        ("std.wire", include_str!("../stdlib/wire/wire.pluto")),
        ("std.rpc", include_str!("../stdlib/rpc/rpc.pluto")),
    ],
};
```

### Resolution order (updated)

When resolving `import std.strings`:

1. **Filesystem override** — Check `--stdlib` / `PLUTO_STDLIB` / sibling `stdlib/` (allows local development and custom stdlib versions)
2. **Embedded sources** — Fall back to the compiled-in stdlib
3. **Error** — Module not found

This means:
- **Users** get a self-contained binary that just works
- **Developers** can override with local files during stdlib development
- **Package manager** (future) could provide versioned stdlib overrides

### Changes to `src/modules.rs`

The module resolver currently calls filesystem APIs to find stdlib modules. The change:

```rust
// Before: only filesystem lookup
fn resolve_stdlib_module(name: &str, stdlib_root: Option<&Path>) -> Result<String, CompileError> {
    // ... filesystem lookup ...
}

// After: filesystem first, then embedded fallback
fn resolve_stdlib_module(name: &str, stdlib_root: Option<&Path>) -> Result<String, CompileError> {
    // Try filesystem first (for overrides / local dev)
    if let Some(root) = stdlib_root {
        if let Ok(source) = try_filesystem_stdlib(name, root) {
            return Ok(source);
        }
    }

    // Fall back to embedded
    if let Some(source) = stdlib_embedded::STDLIB.get(name) {
        return Ok(source.to_string());
    }

    Err(CompileError::ModuleNotFound(name.to_string()))
}
```

## Size Impact

Current stdlib total: ~50KB of `.pluto` source text (rough estimate). This is negligible compared to the 8.8MB `pluto-mcp` binary or the embedded C runtime. No compression needed.

## Testing

- Existing stdlib integration tests should pass without `--stdlib` flag
- Add a test that compiles with no filesystem stdlib available (verify embedded fallback works)
- Add a test that filesystem override takes precedence over embedded

## Migration

1. Implement `src/stdlib_embedded.rs` with embedded sources
2. Update `src/modules.rs` to check embedded before erroring
3. Remove the hard requirement for `--stdlib` / `PLUTO_STDLIB` / sibling directory
4. Keep filesystem override working for development
5. Update CLI help text and documentation

## Open Questions

- Should we also embed stdlib for the SDK's `Module::from_source_file_with_stdlib()`? (Probably yes, for the same reasons)
- Should there be a `--no-embedded-stdlib` flag to force filesystem-only resolution? (Probably not needed)
- When the package manager lands, how do versioned stdlib overrides interact with embedding? (Defer to package manager RFC)

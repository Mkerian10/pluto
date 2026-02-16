# Compiler Reports False "app and main" Conflict

**Date:** 2026-02-16
**Component:** Compiler (also surfaced via MCP compile/load_module tools)
**Type:** bug

## What Happened

Attempted to compile `test_minimal.pluto`:

```pluto
import std.collections

class Registry {
    data: Map<string, int>

    fn add(mut self, key: string, value: int) {
        self.data[key] = value
    }
}

fn main() {
    let data: Map<string, int> = Map<string, int> {}
    let mut reg = Registry { data: data }
    reg.add("test", 42)
}
```

Called:
```
mcp__pluto__compile(
  path="/Users/matthewkerian/Documents/pluto-projects/meridian/test_minimal.pluto",
  stdlib="/Users/matthewkerian/Documents/pluto/stdlib"
)
```

Got error:
```json
{
  "success": false,
  "errors": [{
    "message": "cannot have both an app declaration and a top-level main function",
    "span": {"start": 27, "end": 320},
    "severity": "error"
  }]
}
```

Same error with `load_module`.

## Expected Behavior

File should compile successfully - it has only a `fn main()` and no `app` declaration.

## Actual Behavior

Compiler rejects it claiming there's both an `app` and a `main()`, when the file clearly only has `main()`.

## Assessment

**CONFIRMED: This is a compiler bug, not an MCP bug.**

Tested with raw compiler:
```bash
cargo run -- compile test_minimal.pluto --stdlib stdlib -o /tmp/test_minimal
# Result: Same error - "cannot have both an app declaration and a top-level main function"
```

The file clearly has only `fn main()` and no `app` declaration. The compiler is incorrectly detecting a non-existent app declaration.

The error span (27-320) covers most of the file, suggesting the compiler is confused about the overall structure rather than a specific declaration.

## Impact

Can't compile basic executable files with `main()` entry points. This blocks testing simple Pluto programs.

## Reproduction

Any file with `fn main()` and no `app` declaration triggers this error. Even minimal examples fail.

## Root Cause

The meridian directory contains multiple `.pluto` files:
- `main.pluto` (with `app Meridian { fn main(self) { ... } }`)
- `test_minimal.pluto` (with `fn main() { ... }`)

The module system's sibling file auto-merging behavior merges all `.pluto` files in the same directory into a single module. When compiling `test_minimal.pluto`, it loads `main.pluto` as a sibling, sees both an `app` declaration and a top-level `fn main()`, and correctly rejects the program.

This is working-as-designed behavior for the module system (sibling files are meant to be merged), but conflicts with the common pattern of keeping test files alongside application code.

## Resolution

**Status:** FIXED

Added `--standalone` flag to the compile command that disables sibling file merging, allowing files to be compiled in isolation:

```bash
pluto compile test_minimal.pluto --stdlib stdlib -o test_minimal --standalone
```

The flag is also available through the MCP `compile` tool via the `CompileOptions.standalone` field.

**Changes:**
- Added `standalone: bool` field to `CompileOptions` in `src/server/types.rs`
- Added `--standalone` CLI flag to compile command in `src/main.rs`
- Updated `compile_file_with_options()` signature to accept standalone parameter
- Threaded standalone flag through `InProcessServer::compile()`
- Added integration test `standalone_compilation_skips_siblings` in `tests/integration/modules.rs`

**Commit:** d319d6e "Add --standalone flag to fix false app+main conflict"

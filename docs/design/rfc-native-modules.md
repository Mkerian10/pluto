# RFC: Native Library Modules

**Status:** Draft — direction from design discussion (2026-10-02, the #371 pivot); awaiting owner review
**Author:** Design discussion
**Date:** 2026-10-02
**Related:** [epistemics.md](epistemics.md) (trust boundaries, the assumption surface), [rfc-properties.md](rfc-properties.md) (`assume` on externs — phase 5, shipped), [rfc-distributed-safety.md](rfc-distributed-safety.md) (third-party services open question), issue #371 (compression — the motivating client), issue #372 (the pure-Pluto throughput data)

## Thesis

A Pluto module may own **native C sources**, vendored inside its directory,
compiled and linked into the program alongside the embedded runtime. `extern fn`
already binds C symbols — the stdlib has always been "a Pluto wrapper over C."
The missing concept is exactly one: today externs can only reach the embedded
runtime; there is no way for a *library* to bring its own C. This RFC adds that
concept and nothing else.

The pattern it enables is the `-sys` wrapper library (cgo, Rust `-sys` crates):
`std.compress` vendors lz4 and miniz in its module tree behind a Pluto API;
future needs (crypto, sqlite) follow the same mechanism instead of each being a
runtime negotiation. The runtime stays minimal; you pay for a dependency only by
importing its module; and the vendored C sits **readable in the module tree** —
this is still the source-level-libraries pillar, extended one language down.

Why not pure Pluto, why not runtime vendoring (the #371 options): measured
pure-Pluto throughput (~126 MB/s crc32c via #372) is dogfood-grade, not
production-grade, and vendoring into the runtime taxes every binary and every
`builtins.c` edit with codec compile time. Module-owned native code dissolves
both: production-speed C, paid only by importers, cached per module.

## What exists today (audit)

- `extern fn` declarations with automatic symbol resolution at link time; the
  callable type surface is the extern whitelist in `src/typeck/register.rs`
  (recently extended to `bytes` by the #368 work).
- `src/lib.rs::link()` compiles the embedded runtime `.c` files with `cc`,
  caches by content hash, links everything with `ld -r`.
- Two defects found by the #371 investigation, fixed by this design: the
  runtime compiles with **no `-O` flag at all**, and a **single cache key**
  would make any vendored code re-pay compilation on every runtime edit.

## Design

### Declaration: the `native` block

One per module, in the module's own source — visible where the module is read,
in keeping with "the declaration surface is the source":

```pluto
// stdlib/compress/compress.pt
native {
    sources = ["vendor/lz4.c", "vendor/lz4frame.c", "vendor/miniz.c"]
}

extern fn LZ4F_compressFrameBound(src_size: int, prefs: int) int
extern fn mz_compressBound(len: int) int
// ... hand-declared, honest signatures over the whitelist types
```

- `sources` are paths relative to the module directory; conventionally under
  `vendor/`, each dependency with its `LICENSE` and a provenance comment
  (upstream version/commit, how to re-vendor).
- **Declarative only.** A sources list, not a build script: no commands, no
  code generation, no configure step. If a dependency needs one, it is amalgamated
  upstream or it is not vendorable. (This is a founding constraint of the
  feature: the build stays `cc` over listed files, auditable at a glance.)
- No header parsing / bindgen: extern fns are hand-declared. Small, honest,
  reviewable — and the declarations are the natural home for `assume` clauses.
- An optional `link = ["z"]` names system libraries. Discouraged and visibly
  weaker (see Trust below): the #371 investigation measured that only `-lz`
  resolves out-of-box on macOS; system linking trades self-containment and
  byte-reproducibility for convenience. Permitted because refusing it entirely
  just pushes people to worse workarounds — but it is marked, not silent.

### Compilation and caching

- Module native sources compile exactly like runtime files (`cc -c`), **keyed
  by their own content hash per module** — a `builtins.c` edit never recompiles
  a codec, and vice versa. Objects fold into the final link.
- **`-O2` for all C** — module sources and the embedded runtime alike. The
  missing-optimization defect is fixed here as part of the plumbing change.
- `PLUTO_TEST_MODE`: module C is compiled under the same define and must build
  in both modes (same rule the runtime lives by). Pure-compute libraries
  (codecs, hashes) satisfy this trivially; a module that needs threads must
  guard like the runtime does.
- Cross-compilation: module C goes through the same target `cc` as the runtime;
  no new mechanism, same constraints.

### The boundary type surface

Unchanged from today's extern rules: the whitelist (`int`, `float`, `bool`,
`byte`, `bytes`, `string`, `void`) crosses; entities, closures, traits,
fn-values, generics do not. Wrapper modules do their marshaling in Pluto on the
near side of the boundary. (If a future wrapper genuinely needs out-params or
structs, that is a separate extern-ABI RFC — deliberately not smuggled in here.)

### Trust: the boundary is visible, not safe

C can corrupt anything; no annotation changes that, and this RFC does not
pretend otherwise. What Pluto adds — uniquely — is **attribution**:

- Extern fns in native modules may carry `assume <property>(...)` clauses
  (shipped, rfc-properties.md phase 5) for semantic claims.
- Every native module contributes to the **assumption surface**: `pluto
  analyze` lists each native trust point — module, vendored-source content
  hash, declared symbols, and any system-linked libraries (marked distinctly,
  since their code is not even content-hashed). A deployment can enumerate
  every line of foreign code it trusts and every claim it assumed, with owners.

In epistemic terms: a vendored C dependency is testimony from code the compiler
cannot see. The language's job is not to make the testimony true — it is to
make sure nobody forgets it was testimony.

## First client and acceptance test

`std.compress`, per the #371 investigation's measurements: vendored lz4 frame
suite (BSD-2, ~10.5k lines) and miniz (MIT, ~9.3k lines) behind a Pluto API
(one-shot + streaming per the investigation's sketch), with gzip + lz4 as the
honest v1 codec set. Acceptance: a program importing `std.compress` round-trips
data through both codecs; a program *not* importing it carries zero codec code;
`pluto analyze` lists the module's trust entry; editing `builtins.c` does not
recompile the codecs (cache-key test). zstd remains the deferred decision it
was in #371.

Pure-Pluto codecs remain something a library author *may* write (and lz4 would
be a fine dogfood exercise) — they are no longer the policy.

## Owner decisions

1. **Declaration surface**: the in-language `native` block (recommended) vs a
   sidecar manifest file. The block keeps the declaration where the module is
   read; a manifest would be the first non-Pluto source of program meaning.
2. **System linking**: allow-with-marking (recommended) vs forbid in v1.
3. **Scope of the `-O2` fix**: fold into phase 1 here (recommended — it's the
   same lines of `src/lib.rs`) vs ship separately first.
4. **Availability**: stdlib-only at first, or user modules immediately?
   Recommended: same mechanism for user modules from day one — that is the
   point of making it a module feature rather than a runtime feature; the
   assumption surface is the audit tool either way.
5. **C only** (recommended) vs C++ someday: `cc` as C compiler only; C++ would
   drag in runtime/ABI questions this RFC deliberately avoids (snappy's C port
   exists precisely because everyone makes this same choice).

## Phasing

1. **The mechanism**: `native` block parsing (module-level; flattening carries
   it; SCHEMA bump), per-module compile + cache key + link plumbing, `-O2`
   everywhere, both-modes enforcement, a toy test module in the test suite
   exercising cache behavior and both compile modes.
2. **`std.compress`**: vendored lz4 + miniz, the streaming API, the acceptance
   tests above, examples.
3. **The analyze surface**: native trust entries in DerivedInfo/`pluto analyze`
   alongside the assumed-properties report.

Each phase is independently shippable; phase 1 alone unblocks any future
wrapper library.

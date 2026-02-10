# RFC: Iterative Development for Pluto

> **Status:** Proposal
> **Implementation status:** Not started

## Summary

Add three iterative development modes to `plutoc`:

1. **`plutoc watch`** — File watcher that recompiles and reruns on save
2. **`plutoc watch test`** — File watcher that reruns tests on save
3. **`plutoc repl`** — Interactive expression evaluator

These share a common foundation: the Pluto compiler is already fast enough (~170ms for trivial programs, ~250ms for test suites) that recompiling on every change is viable without any incremental compilation infrastructure.

## Motivation

Today, the Pluto edit-compile-test cycle is:

```
1. Edit file in editor
2. Switch to terminal
3. Type `plutoc run main.pluto --stdlib stdlib`
4. Read output
5. Switch back to editor
6. Repeat
```

This is 6 steps and ~2-3 seconds of human overhead per iteration, even though compilation itself takes <300ms. The goal is to eliminate steps 2-4 entirely.

**Watch mode** makes the feedback loop: `save file → see output` (zero manual steps).
**Watch test** makes the test loop: `save file → see test results` (zero manual steps).
**REPL** makes the exploration loop: `type expression → see result` (no files at all).

## Feasibility

### Measured compile times

Using the release binary on warm cache (macOS, Apple Silicon):

| Scenario | Wall-clock time |
|---|---|
| `plutoc run` — trivial program | **~170ms** |
| `plutoc run` — closures + functions | **~300ms** |
| `plutoc test` — 4 test cases | **~250ms** |
| `plutoc run` — generics + stdlib | **~500ms** |

These are end-to-end: lex → parse → typeck → codegen → link → exec. The runtime object (`builtins.c`) is cached per-process, so subsequent compiles within a single `plutoc watch` process skip the `cc builtins.c` step entirely.

### What makes this easy

- **The compiler is already fast.** 170-300ms is below human perception threshold for "instant."
- **Runtime object caching exists.** `cached_runtime_object()` uses `OnceLock` — compiled once, reused for all subsequent compilations in the same process.
- **`compile_file_with_stdlib()` is self-contained.** Takes a file path, produces a binary. No global state. Can be called repeatedly.
- **File watching is a solved problem.** The `notify` crate (Rust's standard file watcher) handles FSEvents/inotify/kqueue across platforms.

### What we don't need

- **No incremental compilation.** Full recompile at 170ms is fast enough. Incremental would add complexity for marginal gain.
- **No JIT.** Fork/exec per iteration adds ~10ms overhead. Negligible.
- **No persistent heap.** Each run starts fresh. This is actually a feature — deterministic, no stale state.

---

## Design: `plutoc watch`

### Usage

```bash
plutoc watch run main.pluto --stdlib stdlib     # Watch + run
plutoc watch test main.pluto --stdlib stdlib    # Watch + test
```

### Behavior

1. Compile and run/test the target file immediately on startup
2. Watch the target file **and all imported modules** for changes
3. On any change, recompile and rerun
4. Display clear output with visual separator between runs

### Output format

```
─── Compiling main.pluto ──────────────────────────
hello world

─── Watching for changes... ───────────────────────

─── Recompiling (main.pluto changed) ─────────────
hello world!

─── Watching for changes... ───────────────────────
```

For test mode:

```
─── Testing main.pluto ────────────────────────────
test addition works ... ok
test negative numbers ... ok
test edge cases ... FAILED
  expected: 0
  got: -1

2 passed, 1 failed

─── Watching for changes... ───────────────────────
```

### Watch targets

The watcher monitors:
- The entry file itself
- All files discovered during module resolution (imports)
- The `pluto.toml` manifest if present
- The stdlib directory if `--stdlib` is passed

When a module resolution discovers new imports (because the user added an `import` statement), the watcher updates its file set on the next successful compile.

### Debouncing

File saves often trigger multiple FSEvents in rapid succession (editor write → rename, auto-formatter, etc.). The watcher debounces with a 100ms window: wait 100ms after the last change event before recompiling.

### Error handling

Compile errors are printed but don't crash the watcher. The watcher continues monitoring and recompiles on the next save:

```
─── Recompiling (main.pluto changed) ─────────────
error: type mismatch at line 12: expected int, found string

─── Watching for changes... ───────────────────────
```

Runtime errors (nonzero exit code, crash) are reported similarly:

```
─── Recompiling (main.pluto changed) ─────────────
Invariant violation: BankAccount.balance >= 0
Process exited with code 1

─── Watching for changes... ───────────────────────
```

### Terminal clearing

By default, clear the terminal between runs (like `cargo watch -c`). `--no-clear` flag to disable.

### Process management

If the previous run is still executing when a new change arrives:
1. Kill the running subprocess (SIGTERM, then SIGKILL after 1s)
2. Recompile and start fresh

This handles long-running programs (e.g., HTTP servers) gracefully.

---

## Design: `plutoc repl`

### Usage

```bash
plutoc repl                       # Start REPL
plutoc repl --stdlib stdlib       # Start REPL with stdlib
```

### Architecture: Compile-per-iteration

The REPL maintains a growing session buffer. Each iteration compiles the **entire accumulated source** and executes it.

```
User input → Classify → Accumulate into session → Assemble program → Compile → Execute → Print
```

### Session state

```
SessionState {
    imports: Vec<String>,
    declarations: Vec<String>,    // fn, class, trait, enum, error
    main_body: Vec<String>,       // let bindings, statements
}
```

Assembled into:

```
{imports}

{declarations}

fn main() {
    {main_body}
    print("__REPL_MARKER__")
    {new_statements}
}
```

Only output after `__REPL_MARKER__` is shown to the user, so previous `print()` calls don't repeat.

### Input classification

| Input starts with | Classification |
|---|---|
| `import` | Import |
| `fn`, `class`, `enum`, `error`, `trait` | Declaration |
| `let`, `print`, `if`, `while`, `for`, assignment | Statement (goes in main_body) |
| Everything else | Bare expression → wrapped in `print(...)` |

The REPL uses the lexer to peek at the first token for classification. No partial parsing needed.

### Multi-line input

If input has unmatched `{`, `[`, `(`, or ends with `=>`, prompt for continuation:

```
pluto> fn fib(n: int) int {
  ...>   if n <= 1 { return n }
  ...>   return fib(n - 1) + fib(n - 2)
  ...> }
pluto> fib(10)
55
```

### Redefinition

Defining `fn foo()` twice replaces the first definition. The REPL tracks declarations by name.

### Error recovery

Failed compilations don't add to the session. The user can retry:

```
pluto> let x = 42
pluto> x + "oops"
error: type mismatch
pluto> x + 1
43
```

### Commands

| Command | Effect |
|---|---|
| `:q` | Exit |
| `:reset` | Clear session |
| `:source` | Show assembled program |
| `:type <expr>` | Show inferred type (typeck only, no codegen) |
| `:undo` | Remove last input |

### Limitations (acceptable for v1)

- No `app`/DI — the REPL owns `fn main()`
- No persistent mutable state across iterations (each is a fresh process)
- No `spawn` persistence across iterations
- Performance degrades with very large sessions (100+ declarations)

---

## Design: `plutoc watch test` specifics

### Selective test execution

When watching a project with many test files, rerunning all tests on every save is wasteful. The watcher can be targeted:

```bash
# Watch a specific test file
plutoc watch test tests/math_test.pluto --stdlib stdlib

# Watch the whole project (reruns all tests)
plutoc watch test . --stdlib stdlib
```

### Test output formatting

The watcher shows a summary on each run:

```
─── Testing (math.pluto changed) ──────────────────
test addition ... ok
test subtraction ... ok
test division by zero ... ok

3 passed, 0 failed (0.24s)

─── Watching for changes... ───────────────────────
```

On failure, show full diagnostics:

```
─── Testing (math.pluto changed) ──────────────────
test addition ... ok
test subtraction ... FAILED
  Expected: 5
  Got: 4
  at test "subtraction", line 14
test division by zero ... ok

2 passed, 1 failed (0.25s)

─── Watching for changes... ───────────────────────
```

### Pass/fail sound (optional)

`--bell` flag to ring the terminal bell on test failure. Small thing, surprisingly useful.

---

## Implementation

### Dependencies

One new dependency: `notify` (file watching crate). Mature, cross-platform (FSEvents on macOS, inotify on Linux).

### Architecture

```
src/
  watch.rs          // File watcher + debounce + recompile loop
  repl.rs           // REPL input loop + session state + assembly
  main.rs           // New subcommands: watch, repl
```

### Shared infrastructure

Both `watch` and `repl` use the same core loop:

```rust
loop {
    let source = get_source();  // watch: read file, repl: assemble session
    match compile_and_run(&source) {
        Ok(output) => display_output(output),
        Err(e) => display_error(e),
    }
    wait_for_trigger();  // watch: FSEvent, repl: user input
}
```

The difference is where the source comes from and what triggers the next iteration.

### Estimate

| Component | LOC | Notes |
|---|---|---|
| `watch.rs` — file watcher + debounce | ~200 | `notify` crate, event loop, process management |
| `watch.rs` — import tracking | ~100 | Parse imports from successful compile to update watch set |
| `watch.rs` — output formatting | ~100 | Separators, clearing, timing |
| `repl.rs` — input loop | ~150 | Readline, multi-line detection, commands |
| `repl.rs` — session state + assembly | ~150 | Classification, accumulation, marker filtering |
| `main.rs` — subcommand wiring | ~50 | clap commands |
| **Total** | **~750 LOC** | |

### Phasing

**Phase 1: `plutoc watch run`** — Highest value, simplest implementation. File watcher + recompile on change. ~300 LOC.

**Phase 2: `plutoc watch test`** — Same watcher, different compile path. ~100 LOC incremental.

**Phase 3: `plutoc repl`** — Session management + input classification. ~350 LOC incremental.

---

## What this does NOT include

These are explicitly out of scope and would be separate RFCs:

1. **Incremental compilation.** Full recompile is fast enough. Module-level caching would add complexity without meaningful UX improvement at current project sizes.

2. **Hot reload / code patching.** Replacing code in a running process without restart. Requires JIT or dynamic linking. Much harder, different use case (long-running servers).

3. **JIT backend.** `cranelift-jit` for in-process execution. Would require upgrading Cranelift from 0.116 → 0.128+. Only needed if fork/exec overhead becomes a bottleneck (it isn't at 170ms).

4. **Distributed test runner.** Parallelizing tests across machines. Separate concern, separate RFC.

5. **IDE-integrated test runner.** The LSP could report test results. Worth doing, but orthogonal to this RFC.

## Decision

**Feasible and high-value.** The compiler is fast enough that a simple file watcher gives near-instant feedback without any architectural changes. ~750 LOC total, phased delivery starting with watch mode.

The key insight is that Pluto's compilation speed makes "dumb" recompilation a viable strategy. We don't need incremental compilation, JIT, or caching — we just need to trigger the existing pipeline on file save.

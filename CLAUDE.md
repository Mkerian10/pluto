# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                        # Build the compiler
cargo test                         # Run all tests (unit + integration)
cargo test --lib                   # Run unit tests only
cargo test --tests                 # Run all integration tests only
cargo test --lib <test_name>       # Run a single unit test
cargo test --test errors           # Run one integration test file
cargo test --test errors <name>    # Run a single test in a file
cargo run -- compile <file.pt> -o <output>  # Compile a .pt source file
cargo run -- run <file.pt>      # Compile and immediately run
```

## Compiler Pipeline

Defined in `src/lib.rs::compile_file()` (file-based with module resolution) and `compile()` (single-source-string). The full pipeline has 17 stages, orchestrated by `run_frontend()`:

1. **Lex** (`src/lexer/`) — logos-based tokenizer, produces `Vec<Spanned<Token>>`
2. **Parse** (`src/parser/`) — recursive descent + Pratt parsing for expressions, produces `Program` AST
3. **Module resolve** (`src/modules.rs`) — discovers imported modules, parses them, validates visibility
4. **Module flatten** (`src/modules.rs`) — merges imported items into the main program with prefixed names (e.g., `math.add`)
5. **Prelude inject** (`src/prelude.rs`) — prepends prelude classes, traits, enums into the program
6. **Stage flatten** (`src/stages.rs`) — flattens `stage` declaration hierarchy
7. **Ambient desugar** (`src/ambient.rs`) — desugars `uses` clauses into hidden injected fields
8. **Spawn desugar** (`src/spawn.rs`) — transforms `spawn func(args)` into `Expr::Spawn { call: Closure { ... } }`
9. **Contract validation** (`src/contracts.rs`) — validates decidable fragment for invariants/requires/ensures
10. **Marshal phase A** (`src/marshal.rs`) — generates marshaler stubs before typeck
11. **Type check** (`src/typeck/`) — multi-pass: registers declarations (traits, enums, app, errors, classes, functions), checks bodies, infers error sets, enforces error handling. Returns `TypeEnv`
12. **Reflection generation** (`src/reflection.rs`) — generates `TypeInfo` intrinsic implementations
13. **Monomorphize** (`src/monomorphize.rs`) — generates concrete copies of generic functions/classes/enums
14. **Marshal phase B** (`src/marshal.rs`) — generates marshalers for monomorphized types
15. **Trait conformance** (`src/typeck/`) — validates all trait implementations post-monomorphize
16. **Serializable validation** (`src/typeck/serializable.rs`) — validates types marked serializable
17. **Closure lift** (`src/closures.rs`) — transforms `Expr::Closure` into top-level functions + `Expr::ClosureCreate`
18. **Cross-reference resolution** (`src/xref.rs`) — resolves stable UUIDs for declarations
19. **Codegen** (`src/codegen/`) — lowers AST to Cranelift IR, produces object bytes
20. **Link** (`src/lib.rs::link()`) — compiles runtime C files with `cc`, links into final binary

## Key Architecture Notes

**Spanned types** — All AST nodes are wrapped in `Spanned<T>` (defined in `src/span.rs`), carrying a `Span { start, end }` byte offset. Access the inner value with `.node` and source location with `.span`.

**Type system** — `PlutoType` enum in `src/typeck/types.rs`: `Int` (I64), `Float` (F64), `Bool` (I8), `Byte`, `Bytes`, `String` (heap-allocated), `Void`, `Class(name)`, `Array(element_type)`, `Trait(name)`, `Enum(name)`, `Fn(params, return_type)`, `Map(key, value)`, `Set(element)`, `Range`, `Error`, `TypeParam(name)`, `Task(inner)`, `Sender(inner)`, `Receiver(inner)`, `Stream(inner)`, `GenericInstance(kind, name, args)`, `Nullable(inner)`. Nominal typing by default, structural for traits (via vtables).

**Enums** — Unit variants (`Color::Red`) and data-carrying variants (`Shape::Circle { radius: float }`). `match` with exhaustiveness checking.

**Closures** — Arrow syntax `(x: int) => x + 1`. Capture by value. Represented as heap-allocated `[fn_ptr, captures...]`. Lifted to top-level functions by `src/closures.rs` after monomorphization (stage 17).

**Error handling** — `error` declarations, `raise` to throw, `!` postfix to propagate, `catch` (shorthand or wildcard) to handle. Compiler infers error-ability via fixed-point analysis and enforces handling at call sites.

**App + DI** — `app` declaration with `fn main(self)`. Classes use bracket deps `class Foo[dep: Type]` for injection. Compile-time topological sort wires singletons. Codegen synthesizes `main()` that allocates and connects all dependencies.

**Modules** — `import math` with `pub` visibility. Flatten-before-typeck design: imported items get prefixed names (e.g., `math.add`). Supports directory modules, single-file modules, and hierarchical imports.

**Dual file formats** — The compiler accepts both `.pt` (human-readable text source) and `.pluto` (binary AST with UUIDs). All entry points auto-detect format via magic bytes. Module resolution tries `.pluto` first, falls back to `.pt`. Stdlib and examples use `.pt`. When both `name.pluto` and `name.pt` exist, `.pluto` is preferred.

**String interpolation** — `"hello {name}"` with arbitrary expressions inside `{}`.

**Codegen target** — Auto-detected via `host_target_triple()` in `src/codegen/mod.rs` and `src/toolchain.rs`. Supports aarch64/x86_64 on macOS/Linux.

**Linking with C runtime** — The compiler embeds C runtime modules via `include_str!()` and compiles them with `cc` at link time. Core files linked into every binary:
- `runtime/builtins.c` — Core runtime (strings, arrays, I/O, maps, sets, math, errors)
- `runtime/threading.c` — Concurrency primitives (tasks, channels, select)
- `runtime/gc/marksweep.c` or `runtime/gc/noop.c` — GC backend (selectable)
- `runtime/builtins.h` — Shared declarations and GC tags
- `runtime/coverage.c` — Coverage instrumentation (when enabled)

Additional split-out runtime files exist but are compiled into `builtins.c` or linked separately: `arrays.c`, `bytes.c`, `collections.c`, `core.c`, `errors.c`, `fs.c`, `http.c`, `io.c`, `math.c`, `net.c`, `strings.c`, `test.c`, `time.c`.

Each `.c` file is compiled to a `.o` file, then linked with `ld -r` into a single relocatable `runtime.o`, which is finally linked with the Cranelift-generated object code.

**AI-native representation (planned)** — Future direction where `.pluto` becomes a binary canonical representation (full semantic graph with stable UUIDs per declaration) and `.pt` files provide human-readable text views. AI agents write `.pluto` via an SDK (`pluto-sdk`), the compiler enriches `.pluto` with derived analysis data on demand (`pluto analyze`), and `pluto sync` converts human `.pt` edits back to `.pluto`. See `docs/design/ai-native-representation.md` for the full RFC.

**CompilerService** — Protocol-agnostic trait in `src/server/mod.rs` covering module management, declaration inspection, cross-references, compilation/execution, analysis, and documentation. MCP is read-only — all editing operations have been removed. `InProcessServer` in `src/server/in_process.rs` caches `(Program, String, DerivedInfo)` per module and delegates to compiler primitives. Both CLI (`src/main.rs` compile command) and MCP (`mcp/src/server.rs` — docs, stdlib_docs, check, compile, run, test) route through it. See `src/server/types.rs` for all result types.

**Stdlib modules** — 19 modules in `stdlib/`: `std.base64`, `std.collections`, `std.env`, `std.fs`, `std.http`, `std.io`, `std.json`, `std.log`, `std.math`, `std.net`, `std.path`, `std.random`, `std.regex`, `std.rpc`, `std.socket`, `std.strings`, `std.time`, `std.uuid`, `std.wire`. Plus `stdlib/prelude.pluto` (auto-imported into every program).

**No semicolons** — Pluto uses newline-based statement termination. Newlines are lexed as `Token::Newline` and the parser consumes them at statement boundaries while skipping them inside expressions.

## Cranelift API Quirks (v0.116)

- Use `Variable::from_u32()`, not `Variable::new()` (doesn't exist in this version)
- `InstBuilder` trait must be imported to use `builder.ins().*` methods
- `FunctionBuilder::finalize()` takes ownership (pass by value, not `&mut`)

## AST Walker Policy

The compiler has two patterns for walking the AST:

1. **Visitor pattern** (`src/visit.rs`) — for analysis/collection/rewriting passes where >50% of match arms are pure structural recursion. Use `Visitor` for read-only passes, `VisitMut` for in-place rewriting.

2. **Manual `match` blocks** — for core compiler passes where >50% of each arm is domain-specific logic (type checking, code generation, pretty printing, error analysis). These walkers should use **exhaustive matching** (no `_ => {}` catch-all) to ensure new AST variants are handled explicitly.

**When adding a new AST variant:**
- Add the variant to `walk_expr`/`walk_stmt`/`walk_type_expr` in `src/visit.rs` (if it has children)
- Update the core manual walkers: `infer_expr`, `check_stmt`, `lower_expr`, `lower_stmt`, `emit_expr`, `emit_stmt`
- All visitor-based passes automatically handle the new variant

**When writing a new walker:**
- If >50% pure recursion → use `Visitor`/`VisitMut`
- If >50% custom logic → manual `match` with exhaustive matching (no catch-all)
- Never use `_ => {}` catch-all on AST enum matches — list no-op variants explicitly

See `docs/design/visitor-phase4-assessment.md` for the full analysis of which walkers use which pattern.

## Test Infrastructure

**Unit tests** — inline `#[cfg(test)]` modules in `src/lexer/mod.rs`, `src/parser/mod.rs`, and `src/typeck/mod.rs`.

**Integration tests** — Split by feature in `tests/integration/`. Shared helpers in `tests/integration/common/mod.rs`:
- `compile_and_run(source) -> i32` — compiles source string and returns exit code
- `compile_and_run_stdout(source) -> String` — compiles and captures stdout
- `compile_should_fail(source)` — asserts compilation produces an error
- `compile_should_fail_with(source, msg)` — asserts compilation fails with specific message

Test files (68 files): `ai_native_e2e`, `analyze`, `arrays`, `arrow_functions`, `basics`, `bytes`, `chained_field_access`, `channels`, `classes`, `closures`, `concurrency`, `contracts`, `control_flow`, `control_flow_extended`, `coverage`, `deterministic`, `di`, `di_lifecycle`, `edge_cases`, `enums`, `error_messages`, `error_recovery`, `error_snapshots`, `errors`, `expression_complexity`, `fs`, `fstrings`, `gc`, `generators`, `generics`, `generics_syntax`, `http_tests`, `if_expression_errors`, `io`, `json_tests`, `lexer_tests`, `literal_parsing`, `manifest`, `maps`, `marshaling`, `match_expression_errors`, `modules`, `mutability`, `nullable`, `numeric`, `operators`, `precedence`, `precedence_extended`, `prelude`, `rpc`, `scope_blocks`, `sets`, `stages`, `statement_boundaries`, `stdlib_tests`, `strings`, `struct_literals`, `sync`, `system`, `testing`, `toolchain`, `traits`, `type_syntax`, `uuid_base64`, `visit`, `warnings`, `wire`.

**Module tests** — `tests/integration/modules.rs`. Multi-file test helpers:
- `run_project(files) -> String` — writes multiple files to temp dir, compiles entry, returns stdout
- `compile_project_should_fail(files)` — asserts multi-file compilation fails

**Property tests** — `tests/property/` directory. Run with `cargo test --test ast`.
Use `proptest` to verify compiler invariants (spans, determinism, no panics).

**Snapshot tests** — `tests/integration/error_snapshots.rs`. Run with `cargo test error_snapshots`.
Use `insta` to validate error messages. Review changes with `cargo insta review`.

**Fuzzing** — `fuzz/` directory. Requires `cargo install cargo-fuzz`. Run lexer fuzzer: `cargo fuzz run lex`. Run parser fuzzer: `cargo fuzz run parse`.
Store crash cases in `fuzz/artifacts/`. Fuzzing discovers edge cases through random input generation.

**Compiler benchmarks** — `benches/` directory. Run with `cargo bench`.
Measures compilation speed using criterion. Baseline results stored in `target/criterion/`.
These measure how fast the **compiler** runs (lex, parse, typecheck, codegen).

**Runtime benchmarks** — `benchmarks/` directory. Run with `./benchmarks/run_benchmarks.sh`.
Measures execution speed of Pluto programs (how fast compiled code runs).
Already exists, documented in `benchmarks/BENCHMARKS.md`.

**TestCompiler API** — Programmatic access to compiler stages. Import from `tests/integration/common`:
```rust
use common::TestCompiler;

let tc = TestCompiler::new("fn main() {}");
let tokens = tc.lex().unwrap();
let ast = tc.parse().unwrap();
let output = tc.run().unwrap();
```

**Pre-commit hook** — A git pre-commit hook runs `cargo test` before every commit. All tests must pass for a commit to succeed.

**Running the full test suite** — Do NOT run `cargo test` (all tests) locally when verifying a feature branch. Instead, push the branch, create a PR, and let CI run the full test suite. Running all tests locally is slow and CI is the source of truth. You may run individual test files locally for quick iteration (e.g., `cargo test --test generators`).

## Protected Feature Branches

The following branches must NOT be merged into `master` until explicitly authorized by the user. Do not rebase them onto master, do not merge them, do not fast-forward them. They are long-lived feature branches with in-progress work:

- **`ast-uuids`** — Phase 1 of AI-native representation (stable UUIDs on AST nodes). Worktree at `../pluto-ast-uuids`.
- **`canonical-flip`** — Making `.pluto` binary the canonical source format (AI-native representation). Worktree at `../pluto-canonical-flip`. Long-lived branch — do NOT merge until fully migrated.

## Git Workflow

**Master must always be green** — The `master` branch must always build and pass all tests. Never commit incomplete work, partial features, or broken code directly to `master`. All work happens on feature branches; `master` only receives completed, tested merges.

**Never work directly on master** — Always create a feature branch or worktree before making changes. Even small fixes get a branch. The only commits on `master` should be merge commits from completed feature branches.

**Commit regularly on your branch** — Commit after completing each logical unit of work (a feature, a fix, a refactor). Do not accumulate large uncommitted changes. Intermediate commits on feature branches don't need to be green, but the final merge to master must be.

**Use worktrees for parallel work** — When multiple agents or tasks are running concurrently, use git worktrees to avoid conflicts. Use a naming convention that identifies **you** (your session) so other agents know whose worktree is whose:
```bash
# Include your feature name in both the directory and branch name
git worktree add ../pluto-<feature-name> -b <feature-name>
# Example: git worktree add ../pluto-for-loops -b for-loops
# Example: git worktree add ../pluto-trait-handle -b trait-handle

# Do your work in the worktree directory
# ... work in ../pluto-<feature-name> ...

# When done: rebase onto master, merge, then clean up YOUR worktree and branch
git worktree remove ../pluto-<feature-name>
git branch -d <feature-name>
```

**Worktree rules:**
- **Only clean up your own worktrees.** Run `git worktree list` to see all active worktrees. If a worktree/branch isn't yours, leave it alone — another agent may be actively using it.
- **Never force-delete worktrees with uncommitted changes** unless you created them. Use `git worktree list` and check before removing.
- **Don't delete branches that are checked out in other worktrees.** Git will error if you try — respect that error and leave the branch alone.

**Branch per feature** — Each feature or task should be on its own branch. Merge to `master` when complete and tests pass.

**Rebase onto master before merging** — Always resolve conflicts on your feature branch, never on master. Rebase your branch onto the latest master, fix any conflicts there, and verify tests pass. Then do a fast-forward merge to master. This keeps master's history clean and ensures conflicts are never resolved in a half-broken state on master.

**Write examples for new features** — When adding a new user-facing feature (new stdlib module, new language construct, etc.), write an example in `examples/<name>/main.pt` and add it to `examples/README.md` before rebasing and merging. Examples should be self-contained and demonstrate the feature's key capabilities. For stdlib features, include `--stdlib stdlib` in the run instructions.

**Merge checklist** — Before merging to `master`, verify ALL of the following:
1. `cargo test` passes on your branch (all unit, integration, and module tests)
2. If the feature is user-facing, an example exists in `examples/` and `examples/README.md` is updated
3. Rebase onto latest `master` and resolve any conflicts **on your branch**
4. `cargo test` passes again after conflict resolution on your branch
5. Fast-forward merge to `master` (should be clean, no conflicts)

```bash
# On your feature branch:
git fetch origin                  # Get latest
git rebase master                 # Rebase onto master, resolve conflicts HERE
# Fix any conflicts, then: git add <files> && git rebase --continue
cargo test                        # MUST pass on your branch after rebase

# Then merge to master (fast-forward, no conflicts):
git checkout master
git pull                          # Get latest from other agents
git merge <feature-name>          # Fast-forward merge (no conflicts)
cargo test                        # Sanity check — should pass
```

**Pull when starting work** — Before beginning any new task, pull the latest `master` to start from the most up-to-date code:
```bash
git pull                          # On master, before creating a worktree/branch
```

**Before starting work** — Check `git status` and `git worktree list` to understand the current state. If there are uncommitted changes from another agent, coordinate or use a worktree. Do not touch other agents' worktrees or branches.

**Pull request workflow** — After creating a PR:
1. Check CI status with `gh pr checks <pr-number>`
2. Once CI is green (all checks passing), automatically open the PR in the browser with `open <pr-url>`
3. Do not open the PR before CI passes - wait for green status first

**PR titles and descriptions** — Write commit messages, PR titles, and PR descriptions in a natural, human-like style:
- Don't mention Claude, AI, or that you're an assistant
- Don't use phrases like "generated by AI" or "AI-assisted"
- Write as if a human developer did the work
- Use natural language: "Add property tests" not "This PR adds property tests"
- Be direct and professional, not overly formal or robotic
- Remove the "🤖 Generated with Claude Code" footer from PR descriptions

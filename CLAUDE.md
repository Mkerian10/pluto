# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                        # Build the compiler
cargo test                         # Run all tests (unit + integration)
cargo test --lib                   # Run unit tests only
cargo test --test integration      # Run integration tests only
cargo test --lib <test_name>       # Run a single unit test
cargo test --test integration <test_name>  # Run a single integration test
cargo run -- compile <file.pluto> -o <output>  # Compile a .pluto file
cargo run -- run <file.pluto>      # Compile and immediately run
```

## Compiler Pipeline

Defined in `src/lib.rs::compile()`. Six stages:

1. **Lex** (`src/lexer/`) — logos-based tokenizer, produces `Vec<Spanned<Token>>`
2. **Parse** (`src/parser/`) — recursive descent + Pratt parsing for expressions, produces `Program` AST
3. **Type check** (`src/typeck/`) — two-pass: first registers all declarations, then checks bodies. Returns `TypeEnv`
4. **Codegen** (`src/codegen/`) — lowers AST to Cranelift IR, produces object bytes
5. **Link** (`src/lib.rs::link()`) — compiles `runtime/builtins.c` with `cc`, links both `.o` files into final binary
6. **Cleanup** — removes temp `.o` files

## Key Architecture Notes

**Spanned types** — All AST nodes are wrapped in `Spanned<T>` (defined in `src/span.rs`), carrying a `Span { start, end }` byte offset. Access the inner value with `.node` and source location with `.span`.

**Type system** — `PlutoType` enum in `src/typeck/types.rs`: `Int` (I64), `Float` (F64), `Bool` (I8), `String` (heap-allocated), `Void`, `Class(name)`, `Array(element_type)`, `Trait(name)`. Nominal typing by default, structural for traits (via vtables).

**Codegen target** — Hardcoded to `aarch64-apple-darwin` in `src/codegen/mod.rs`. Needs platform detection in the future.

**Linking with C runtime** — The compiler embeds `runtime/builtins.c` via `include_str!()` and compiles it with `cc` at link time. This provides `print`, memory allocation, string ops, and array ops.

**No semicolons** — Pluto uses newline-based statement termination. Newlines are lexed as `Token::Newline` and the parser consumes them at statement boundaries while skipping them inside expressions.

## Cranelift API Quirks (v0.116)

- Use `Variable::from_u32()`, not `Variable::new()` (doesn't exist in this version)
- `InstBuilder` trait must be imported to use `builder.ins().*` methods
- `FunctionBuilder::finalize()` takes ownership (pass by value, not `&mut`)

## Test Infrastructure

**Unit tests** — inline `#[cfg(test)]` modules in `src/lexer/mod.rs`, `src/parser/mod.rs`, and `src/typeck/mod.rs`.

**Integration tests** — `tests/integration/basic.rs` (registered via `[[test]]` in Cargo.toml). Three helper functions:
- `compile_and_run(source) -> i32` — compiles source string and returns exit code
- `compile_and_run_stdout(source) -> String` — compiles and captures stdout
- `compile_should_fail(source)` — asserts compilation produces an error

**Module tests** — `tests/integration/modules.rs`. Multi-file test helpers:
- `run_project(files) -> String` — writes multiple files to temp dir, compiles entry, returns stdout
- `compile_project_should_fail(files)` — asserts multi-file compilation fails

**Pre-commit hook** — A git pre-commit hook runs `cargo test` before every commit. All tests must pass for a commit to succeed.

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

# When done: merge to master, then clean up YOUR worktree and branch
git worktree remove ../pluto-<feature-name>
git branch -d <feature-name>
```

**Worktree rules:**
- **Only clean up your own worktrees.** Run `git worktree list` to see all active worktrees. If a worktree/branch isn't yours, leave it alone — another agent may be actively using it.
- **Never force-delete worktrees with uncommitted changes** unless you created them. Use `git worktree list` and check before removing.
- **Don't delete branches that are checked out in other worktrees.** Git will error if you try — respect that error and leave the branch alone.

**Branch per feature** — Each feature or task should be on its own branch. Merge to `master` when complete and tests pass.

**Merge checklist** — Before merging to `master`, verify ALL of the following:
1. `cargo test` passes on your branch (all unit, integration, and module tests)
2. Pull latest `master` and resolve any conflicts
3. `cargo test` passes again after conflict resolution
4. Only then commit the merge to `master`

```bash
git checkout master
git pull                          # Get latest from other agents
git merge <feature-name>          # Merge your branch
# Resolve any conflicts
cargo test                        # MUST pass before committing
git commit                        # Only if tests pass
```

**Pull when starting work** — Before beginning any new task, pull the latest `master` to start from the most up-to-date code:
```bash
git pull                          # On master, before creating a worktree/branch
```

**Before starting work** — Check `git status` and `git worktree list` to understand the current state. If there are uncommitted changes from another agent, coordinate or use a worktree. Do not touch other agents' worktrees or branches.

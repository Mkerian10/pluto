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

**Commit regularly** — Commit after completing each logical unit of work (a feature, a fix, a refactor). Do not accumulate large uncommitted changes.

**Use worktrees for parallel work** — When multiple agents or tasks are running concurrently, use git worktrees to avoid conflicts:
```bash
git worktree add ../pluto-<feature-name> -b <feature-name>  # Create worktree + branch
# ... do work in ../pluto-<feature-name> ...
git worktree remove ../pluto-<feature-name>                  # Clean up when done
```

**Branch per feature** — Each feature or task should be on its own branch. Merge to `master` when complete and tests pass.

**Before starting work** — Check `git status` to ensure a clean working tree. If there are uncommitted changes from another agent, coordinate or use a worktree.

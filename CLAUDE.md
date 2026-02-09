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
cargo run -- compile <file.pluto> -o <output>  # Compile a .pluto file
cargo run -- run <file.pluto>      # Compile and immediately run
```

## Compiler Pipeline

Defined in `src/lib.rs::compile_file()` (file-based with module resolution) and `compile()` (single-source-string). Eight stages:

1. **Lex** (`src/lexer/`) — logos-based tokenizer, produces `Vec<Spanned<Token>>`
2. **Parse** (`src/parser/`) — recursive descent + Pratt parsing for expressions, produces `Program` AST
3. **Module resolve** (`src/modules.rs`) — discovers imported modules, parses them, validates visibility
4. **Module flatten** (`src/modules.rs`) — merges imported items into the main program with prefixed names (e.g., `math.add`)
5. **Closure lift** (`src/closures.rs`) — transforms `Expr::Closure` into top-level functions + `Expr::ClosureCreate`
6. **Type check** (`src/typeck/`) — multi-pass: registers declarations (traits, enums, app, errors, classes, functions), checks bodies, infers error sets, enforces error handling. Returns `TypeEnv`
7. **Codegen** (`src/codegen/`) — lowers AST to Cranelift IR, produces object bytes
8. **Link** (`src/lib.rs::link()`) — compiles `runtime/builtins.c` with `cc`, links both `.o` files into final binary

## Key Architecture Notes

**Spanned types** — All AST nodes are wrapped in `Spanned<T>` (defined in `src/span.rs`), carrying a `Span { start, end }` byte offset. Access the inner value with `.node` and source location with `.span`.

**Type system** — `PlutoType` enum in `src/typeck/types.rs`: `Int` (I64), `Float` (F64), `Bool` (I8), `String` (heap-allocated), `Void`, `Class(name)`, `Array(element_type)`, `Trait(name)`, `Enum(name)`, `Fn(params, return_type)`, `Error`. Nominal typing by default, structural for traits (via vtables).

**Enums** — Unit variants (`Color::Red`) and data-carrying variants (`Shape::Circle { radius: float }`). `match` with exhaustiveness checking.

**Closures** — Arrow syntax `(x: int) => x + 1`. Capture by value. Represented as heap-allocated `[fn_ptr, captures...]`. Lifted to top-level functions by `src/closures.rs` before typeck.

**Error handling** — `error` declarations, `raise` to throw, `!` postfix to propagate, `catch` (shorthand or wildcard) to handle. Compiler infers error-ability via fixed-point analysis and enforces handling at call sites.

**App + DI** — `app` declaration with `fn main(self)`. Classes use bracket deps `class Foo[dep: Type]` for injection. Compile-time topological sort wires singletons. Codegen synthesizes `main()` that allocates and connects all dependencies.

**Modules** — `import math` with `pub` visibility. Flatten-before-typeck design: imported items get prefixed names (e.g., `math.add`). Supports directory modules, single-file modules, and hierarchical imports.

**String interpolation** — `"hello {name}"` with arbitrary expressions inside `{}`.

**Codegen target** — Hardcoded to `aarch64-apple-darwin` in `src/codegen/mod.rs`. Needs platform detection in the future.

**Linking with C runtime** — The compiler embeds `runtime/builtins.c` via `include_str!()` and compiles it with `cc` at link time. This provides `print`, memory allocation, string ops, array ops, and error handling runtime (`pluto_get_error`, `pluto_set_error`, `pluto_clear_error`).

**No semicolons** — Pluto uses newline-based statement termination. Newlines are lexed as `Token::Newline` and the parser consumes them at statement boundaries while skipping them inside expressions.

## Cranelift API Quirks (v0.116)

- Use `Variable::from_u32()`, not `Variable::new()` (doesn't exist in this version)
- `InstBuilder` trait must be imported to use `builder.ins().*` methods
- `FunctionBuilder::finalize()` takes ownership (pass by value, not `&mut`)

## Test Infrastructure

**Unit tests** — inline `#[cfg(test)]` modules in `src/lexer/mod.rs`, `src/parser/mod.rs`, and `src/typeck/mod.rs`.

**Integration tests** — Split by feature in `tests/integration/`. Shared helpers in `tests/integration/common/mod.rs`:
- `compile_and_run(source) -> i32` — compiles source string and returns exit code
- `compile_and_run_stdout(source) -> String` — compiles and captures stdout
- `compile_should_fail(source)` — asserts compilation produces an error
- `compile_should_fail_with(source, msg)` — asserts compilation fails with specific message

Test files: `basics`, `operators`, `control_flow`, `strings`, `arrays`, `classes`, `traits`, `enums`, `closures`, `di`, `errors`.

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

**Merge checklist** — Before merging to `master`, verify ALL of the following:
1. `cargo test` passes on your branch (all unit, integration, and module tests)
2. Rebase onto latest `master` and resolve any conflicts **on your branch**
3. `cargo test` passes again after conflict resolution on your branch
4. Fast-forward merge to `master` (should be clean, no conflicts)

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

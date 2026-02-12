# Checklist: Adding a New AST Variant

When adding a new variant to `Expr`, `Stmt`, or `TypeExpr`, follow this comprehensive checklist to ensure all necessary updates are made.

## 1. Update walk functions (`src/visit.rs`)

- [ ] Add the new variant to the appropriate walk function:
  - `walk_expr` for new `Expr` variants
  - `walk_stmt` for new `Stmt` variants
  - `walk_type_expr` for new `TypeExpr` variants
- [ ] Visit all child nodes (expressions, statements, type expressions, blocks)
- [ ] Add the variant to `walk_expr_mut` / `walk_stmt_mut` / `walk_type_expr_mut` for mutable traversal
- [ ] Add a unit test in `src/visit.rs::tests` that verifies all children are visited correctly

**Why:** The visitor pattern's walk functions are the single source of truth for structural recursion. All visitor-based passes depend on these being correct. Missing a child node here propagates bugs to every visitor.

## 2. Update core manual walkers

Update these files with exhaustive matching (no `_ => {}` catch-all):

### Type checking (`src/typeck/`)
- [ ] `src/typeck/infer.rs::infer_expr` — type inference for expressions
- [ ] `src/typeck/infer.rs::infer_type_for_expr` — alternative type inference
- [ ] `src/typeck/check.rs::check_expr` — type checking for expressions
- [ ] `src/typeck/check.rs::check_stmt` — type checking for statements

### Code generation (`src/codegen/`)
- [ ] `src/codegen/lower/mod.rs::lower_expr` — lower expression to Cranelift IR
- [ ] `src/codegen/lower/mod.rs::lower_stmt` — lower statement to Cranelift IR

### Pretty printing (`src/pretty.rs`)
- [ ] `src/pretty.rs::emit_expr` — emit source code for expressions
- [ ] `src/pretty.rs::emit_stmt` — emit source code for statements
- [ ] `src/pretty.rs::emit_type_expr` — emit source code for type expressions

### Error analysis (`src/typeck/errors.rs`)
- [ ] `src/typeck/errors.rs::collect_expr_effects` — collect error effects from expressions
- [ ] `src/typeck/errors.rs::collect_stmt_effects` — collect error effects from statements
- [ ] `src/typeck/errors.rs::enforce_expr` — enforce error handling in expressions
- [ ] `src/typeck/errors.rs::enforce_stmt` — enforce error handling in statements

**Why:** These are the core compiler passes. The compiler will produce exhaustive match warnings if you forget to update them, but it's better to be systematic.

## 3. Add tests

- [ ] **Parser test:** Add unit test in `src/parser/mod.rs::tests` if the variant involves new syntax
- [ ] **Integration test:** Add end-to-end test in `tests/integration/` exercising the new variant:
  - Test that it parses correctly
  - Test that it type-checks correctly
  - Test that it codegen works and produces expected output
  - Test error cases (if applicable)
- [ ] **Visitor test:** If the variant has child nodes, verify that visitor-based passes handle it correctly

**Example test structure:**
```rust
#[test]
fn test_new_variant() {
    let source = r#"
        fn main() {
            // Use the new variant here
        }
    "#;
    let output = compile_and_run(source);
    assert_eq!(output, "expected output");
}
```

## 4. Update documentation (if user-facing)

- [ ] `SPEC.md` — Add syntax and semantics to language specification
- [ ] `CLAUDE.md` — Add compiler architecture notes if the variant has special handling
- [ ] `examples/` — Add example demonstrating the new feature (if it's a new language construct)
- [ ] `examples/README.md` — Document how to run the example

**Why:** User-facing features need documentation for adoption. Internal features need architecture notes for maintainability.

## 5. Special considerations

### If the variant contains child expressions/statements/types:
- [ ] Verify `walk_expr` / `walk_stmt` / `walk_type_expr` visits ALL children
- [ ] Consider whether visitor-based passes need special handling (e.g., spawn opacity, error boundaries)

### If the variant creates a new scope:
- [ ] Update scope tracking in closures (`src/closures.rs`)
- [ ] Update ambient scope rewriting (`src/ambient.rs`) if applicable
- [ ] Document scope semantics in `SPEC.md`

### If the variant interacts with errors:
- [ ] Update error collection and enforcement in `src/typeck/errors.rs`
- [ ] Add tests for error propagation (`!`) and handling (`catch`)
- [ ] Document error semantics

### If the variant interacts with concurrency:
- [ ] Update spawn desugaring (`src/spawn.rs`) if applicable
- [ ] Update task origin tracking in `src/typeck/errors.rs`
- [ ] Consider race conditions and memory safety

### If the variant has special codegen requirements:
- [ ] Update type size calculations if it's a heap type
- [ ] Update GC tracing if it contains heap references
- [ ] Add runtime support in `runtime/builtins.c` if needed

## 6. CI verification

- [ ] `cargo test` passes — all unit and integration tests
- [ ] `cargo build` produces no warnings (especially exhaustive match warnings)
- [ ] `cargo clippy` produces no new warnings
- [ ] All CI checks pass on your PR

## 7. Review checklist

Before submitting your PR, verify:

- [ ] All match arms are exhaustive (no `_ => {}` catch-all on AST enums)
- [ ] All child nodes are visited in walk functions
- [ ] All core compiler passes handle the variant
- [ ] Tests exercise the happy path and error cases
- [ ] Documentation is updated (if user-facing)
- [ ] Example code demonstrates the feature (if user-facing)

---

## Quick Reference

**Files that MUST be updated for new AST variants:**

1. `src/visit.rs` — walk functions
2. `src/typeck/infer.rs` — type inference
3. `src/typeck/check.rs` — type checking
4. `src/codegen/lower/mod.rs` — code generation
5. `src/pretty.rs` — pretty printing
6. `src/typeck/errors.rs` — error analysis (if expression/statement)

**Files that MAY need updates depending on semantics:**

- `src/closures.rs` — closure lifting
- `src/spawn.rs` — spawn desugaring
- `src/ambient.rs` — ambient scope rewriting
- `src/monomorphize.rs` — generic instantiation
- `runtime/builtins.c` — runtime support

---

## Example: Adding `Expr::TryExpr`

Let's walk through adding a new `Expr::TryExpr { expr, handlers }` variant:

1. **Define AST node** in `src/parser/ast.rs`:
   ```rust
   TryExpr {
       expr: Box<Spanned<Expr>>,
       handlers: Vec<TryHandler>,
   }
   ```

2. **Update `walk_expr`** in `src/visit.rs`:
   ```rust
   Expr::TryExpr { expr, handlers } => {
       v.visit_expr(expr);
       for handler in handlers {
           v.visit_expr(&handler.body);
       }
   }
   ```

3. **Add test** in `src/visit.rs::tests`:
   ```rust
   #[test]
   fn test_walk_expr_visits_try_handlers() {
       let try_expr = dummy(Expr::TryExpr {
           expr: Box::new(dummy(Expr::Call { ... })),
           handlers: vec![
               TryHandler { body: dummy(Expr::IntLit(1)) },
               TryHandler { body: dummy(Expr::IntLit(2)) },
           ],
       });

       let mut collector = ExprCollector::default();
       collector.visit_expr(&try_expr);

       assert!(collector.visited.contains("TryExpr"));
       assert!(collector.visited.contains("Call"));
       assert!(collector.visited.contains("IntLit"));
   }
   ```

4. **Update core walkers** — type checking, codegen, pretty printing, error analysis

5. **Add integration test** in `tests/integration/`:
   ```rust
   #[test]
   fn test_try_expr() {
       let source = r#"
           fn main() {
               let x = try risky_call() {
                   on NetworkError => 0
                   on TimeoutError => 1
               }
               print(x)
           }
       "#;
       assert_eq!(compile_and_run(source), "0\n");
   }
   ```

6. **Update documentation** — `SPEC.md`, examples, etc.

---

**Last updated:** 2026-02-12
**Related:** `docs/design/rfc-visitor-pattern.md`, `docs/design/visitor-phase4-assessment.md`

# Decision Tree: Visitor Pattern vs. Manual Walkers

**Purpose:** This document provides a clear decision-making process for choosing between the visitor pattern and manual `match` blocks when writing AST walkers in the Pluto compiler.

**Related Documents:**
- [Visitor Pattern RFC](rfc-visitor-pattern.md) — Full rationale and design
- [Phase 4 Assessment](visitor-phase4-assessment.md) — Detailed analysis of core walkers
- [Post-Migration Plan](visitor-post-migration-plan.md) — Follow-up work and best practices
- [AST Variant Checklist](../checklists/add-ast-variant.md) — Steps for adding new AST nodes

---

## Quick Answer

**Use the visitor pattern when:**
- Your walker is primarily structural recursion (<50% custom logic per arm)
- You're collecting simple data (identifiers, references, counts)
- You want to reuse traversal logic across multiple passes

**Use manual `match` blocks when:**
- Your walker has complex domain-specific logic (>50% custom per arm)
- You're implementing core compiler passes (type checking, code generation)
- The logic varies significantly between different AST variants

---

## Decision Tree

```
                    ┌─────────────────────────────────┐
                    │ Writing a new AST walker?       │
                    └────────────┬────────────────────┘
                                 │
                    ┌────────────▼────────────────────┐
                    │ Does it modify the AST?         │
                    └────┬────────────────────┬───────┘
                         │ Yes                │ No
                         │                    │
                ┌────────▼────────┐  ┌────────▼────────┐
                │ Manual match    │  │ Continue below  │
                │ (VisitMut TBD)  │  │                 │
                └─────────────────┘  └────────┬────────┘
                                              │
                    ┌─────────────────────────▼──────────────────┐
                    │ What type of task are you implementing?    │
                    └────┬──────────────────┬─────────────┬──────┘
                         │                  │             │
         ┌───────────────▼──────┐  ┌────────▼──────┐  ┌──▼─────────────┐
         │ Core compiler pass   │  │ Data          │  │ Analysis       │
         │ (typeck, codegen)    │  │ collection    │  │ or validation  │
         └───────────┬──────────┘  └────────┬──────┘  └──┬─────────────┘
                     │                      │             │
                     │                      │             │
            ┌────────▼──────────┐  ┌────────▼──────┐     │
            │ Manual match      │  │ Visitor       │     │
            │                   │  │ pattern       │     │
            └───────────────────┘  └───────────────┘     │
                                                          │
                                   ┌──────────────────────▼──────────┐
                                   │ Estimate custom logic %:        │
                                   │ Count arms with >3 lines of     │
                                   │ non-recursive logic             │
                                   └────────────┬────────────────────┘
                                                │
                                   ┌────────────▼─────────────────────┐
                                   │ >50% custom logic per arm?       │
                                   └────┬──────────────────┬──────────┘
                                        │ Yes               │ No
                                        │                   │
                           ┌────────────▼──────────┐  ┌─────▼───────────────┐
                           │ Manual match          │  │ Visitor pattern     │
                           │ (exhaustive, no _=>)  │  │                     │
                           └───────────────────────┘  └─────────────────────┘
```

---

## Decision Point Details

### 1. Does it modify the AST?

**Question:** Will your walker create a new or modified AST as output?

- **Yes → Manual match** (for now)
  - `VisitMut` is not yet implemented in Pluto
  - AST transformations need precise control over reconstruction
  - Examples: monomorphization, closure lifting, module flattening

- **No → Continue to next decision**
  - Your walker is read-only (traverses but doesn't modify)
  - Examples: type checking, data collection, validation

---

### 2. What type of task?

#### Core Compiler Pass (typeck, codegen, pretty printing)

**Use manual `match` blocks.**

**Rationale:**
- These passes have complex, variant-specific logic
- Type checking and code generation vary significantly per AST node
- Domain coupling makes visitor abstraction unhelpful
- Exhaustive matching provides clear documentation of behavior

**Examples:**
- `infer_expr` — each variant computes a different type
- `lower_stmt` — each variant emits different Cranelift IR
- `check_stmt` — each variant performs different validation
- `emit_expr` — each variant has different pretty-print format

**Current implementations:** All use manual exhaustive `match` blocks (no `_ =>`).

---

#### Data Collection

**Use the visitor pattern.**

**Rationale:**
- Collecting data is primarily structural recursion
- Same logic applies across most/all variants
- Visitor eliminates boilerplate traversal code

**Examples:**
- `collect_spawn_closure_names` — finds spawn expressions
- `collect_free_vars` — finds captured variables in closures
- `collect_deps_from_expr` — finds DI dependencies
- `collect_idents_in_stmt` — finds identifier references

**Pattern:**
```rust
struct MyCollector {
    data: Vec<String>,
}

impl Visitor for MyCollector {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        if let Expr::SomeVariant { field, .. } = &expr.node {
            self.data.push(field.clone());
        }
        walk_expr(self, expr); // Automatic recursion
    }
}
```

---

#### Analysis or Validation

**Estimate custom logic percentage.**

**Method:**
1. Sketch out the match arms for your walker
2. Count arms where >3 lines are NOT just calling `walk_*` or other walkers
3. Calculate: `(custom_arms / total_arms) * 100`

**Examples:**

- **<50% custom (use visitor):**
  ```rust
  // PropagateDetector — finds error propagation operators
  match &expr.node {
      Expr::Propagate { .. } => { self.found = true; }  // 1 custom arm
      _ => {}  // 28 other arms (pure recursion)
  }
  ```
  - Only 1/29 arms has custom logic → 3% → **Use visitor**

- **>50% custom (use manual match):**
  ```rust
  // collect_expr_effects — analyzes fallibility
  match &expr.node {
      Expr::Call { name, .. } => {
          // Custom: resolve function, check fallibility
          if let Some(sig) = env.get_function(name) {
              if !sig.error_set.is_empty() {
                  effects.errors.extend(&sig.error_set);
              }
          }
          // Then recurse into args
      }
      Expr::MethodCall { object, method, .. } => {
          // Custom: resolve method via vtable/class
          let obj_ty = infer_expr(object, env)?;
          if let Some(method_sig) = resolve_method(&obj_ty, method, env) {
              effects.errors.extend(&method_sig.error_set);
          }
          // Then recurse
      }
      // ... 25+ more arms, each with unique analysis logic
  }
  ```
  - 20+/29 arms have custom logic → 70% → **Use manual match**

---

### 3. Core Compiler Pass Heuristic

If your walker is part of:
- Type checking (`src/typeck/`) — **manual match**
- Code generation (`src/codegen/`) — **manual match**
- Pretty printing (`src/pretty.rs`) — **manual match**
- Error analysis (`src/typeck/errors.rs` — `collect_*_effects`, `enforce_*`) — **manual match**

These domains have >50% custom logic by definition.

---

## Examples by Category

### ✅ Use Visitor Pattern

| Walker | File | Logic |
|--------|------|-------|
| `PropagateDetector` | typeck/errors.rs | Finds `!` operators (1/29 arms custom) |
| `FreeVarCollector` | typeck/closures.rs | Collects captured variables |
| `SpawnClosureCollector` | codegen/mod.rs | Finds spawn expressions |
| `IdentCollector` | typeck/check.rs | Collects identifier names |
| `DependencyCollector` | derived.rs | Collects DI dependencies |
| `SelfMutationChecker` | typeck/check.rs | Detects mutations to `self` |

**Common pattern:** One or two variants need special handling, rest are pure recursion.

---

### ❌ Use Manual Match

| Walker | File | Custom Logic % |
|--------|------|----------------|
| `infer_expr` | typeck/infer.rs | 85% (each variant computes different type) |
| `check_stmt` | typeck/check.rs | 80% (each variant validates differently) |
| `lower_expr` | codegen/lower/mod.rs | 95% (each variant emits different IR) |
| `lower_stmt` | codegen/lower/mod.rs | 95% (each variant emits different IR) |
| `emit_expr` | pretty.rs | 90% (each variant formats differently) |
| `emit_stmt` | pretty.rs | 90% (each variant formats differently) |
| `collect_expr_effects` | typeck/errors.rs | 70% (resolves functions/methods per variant) |
| `enforce_expr` | typeck/errors.rs | 75% (validates error handling per variant) |

**Common pattern:** Each variant has unique domain logic that can't be abstracted.

---

## When in Doubt

**Default to manual `match` first**, then refactor to visitor if you notice:
- Most arms are identical (just calling `walk_*`)
- You're duplicating the same recursion pattern
- The walker is simple data collection

**Prefer manual `match` for:**
- Anything in `typeck/`, `codegen/`, or `pretty.rs`
- Passes that make decisions based on AST structure
- Logic that varies significantly per variant

**Prefer visitor pattern for:**
- Finding specific patterns in the AST
- Collecting lists of nodes/names/references
- Validating simple invariants
- Passes where most variants are no-ops

---

## Anti-Patterns to Avoid

### ❌ Don't use visitor with mostly empty visit methods

```rust
// BAD: Most visit methods do nothing
impl Visitor for MyVisitor {
    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        // Only care about one variant, but have to handle all via walk
        if let Expr::Call { .. } = &expr.node {
            // do something
        }
        walk_expr(self, expr);
    }
}
```

**Better:** Just use a targeted manual match on the specific case.

---

### ❌ Don't use manual match for pure data collection

```rust
// BAD: Lots of boilerplate recursion
fn collect_idents(expr: &Expr, idents: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name) => { idents.insert(name.clone()); }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_idents(lhs, idents);   // Boilerplate
            collect_idents(rhs, idents);   // Boilerplate
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_idents(arg, idents);  // Boilerplate
            }
        }
        // ... 26 more arms, all just recursing
    }
}
```

**Better:** Use visitor pattern to eliminate recursion boilerplate.

---

## Rule of Thumb

**Visitor pattern = DRY for recursion**

If you're writing the same recursion code in multiple arms, use the visitor.

If each arm does something fundamentally different, use manual `match`.

---

## Checklist

Before implementing a new walker, answer these questions:

- [ ] Does it modify the AST? (Yes → manual match)
- [ ] Is it a core compiler pass? (Yes → manual match)
- [ ] Is it simple data collection? (Yes → visitor)
- [ ] Does >50% of arms have custom logic? (Yes → manual match, No → visitor)
- [ ] Will it have catch-all patterns (`_ => {}`)? (No! Use exhaustive matching)

---

## Further Reading

- **Visitor Pattern RFC** ([rfc-visitor-pattern.md](rfc-visitor-pattern.md))
  - Full motivation and design decisions
  - Performance analysis (monomorphization = zero overhead)
  - Migration phases 0-3 summary

- **Phase 4 Assessment** ([visitor-phase4-assessment.md](visitor-phase4-assessment.md))
  - Detailed analysis of 13 core walkers
  - Methodology for estimating custom logic percentage
  - Decision rationale for keeping manual matches

- **Post-Migration Plan** ([visitor-post-migration-plan.md](visitor-post-migration-plan.md))
  - Best practices and guidelines
  - Common patterns and utilities
  - CI enforcement and testing strategies

- **AST Variant Checklist** ([../checklists/add-ast-variant.md](../checklists/add-ast-variant.md))
  - Step-by-step guide for adding new AST nodes
  - Ensures both visitor and manual walkers are updated

---

## Summary

The visitor pattern is a **tool for eliminating boilerplate recursion**, not a universal solution. Use it when recursion dominates your walker. Use manual `match` when domain logic dominates.

**Key insight:** The decision is about code ratio, not code quality. Both approaches are valid—choose based on which one makes your specific walker clearer and more maintainable.

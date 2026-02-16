# ScopeTracker Utility - Usage Examples

This document demonstrates how to use the `ScopeTracker` utility from `src/visit/scope_tracker.rs` to simplify scope management in AST visitors and compiler passes.

## Overview

Many compiler passes need to track scope-local state (variable bindings, types, etc.) with proper nesting semantics. The `ScopeTracker` utility provides a clean abstraction over the common pattern of maintaining a stack of scopes.

**Key features:**
- Generic over value type (works with any `T`)
- Automatic scope hierarchy (innermost to outermost lookup)
- Depth tracking for multi-level analysis
- Shadowing detection
- Mutable and immutable lookups

## API Quick Reference

| Method | Purpose |
|--------|---------|
| `new()` | Create empty tracker (call `push_scope()` before use) |
| `with_initial_scope()` | Create tracker with one scope already pushed |
| `push_scope()` | Enter a new scope (block, function, etc.) |
| `pop_scope()` | Exit current scope, returning its contents |
| `insert(name, value)` | Add binding to current scope |
| `insert_shadowing(name, value)` | Add binding, return previous value if shadowed |
| `lookup(name)` | Find binding in any scope (innermost first) |
| `lookup_mut(name)` | Find binding mutably |
| `lookup_with_depth(name)` | Find binding + scope depth (0 = outermost) |
| `contains(name)` | Check if binding exists in any scope |
| `contains_in_current(name)` | Check if binding exists in current scope only |
| `current_scope()` | Get reference to current scope's HashMap |
| `depth()` | Get number of active scopes |

## Before & After Examples

### Example 1: Simple Variable Tracking

**Before (manual scope stack):**

```rust
struct VariableTracker {
    scopes: Vec<HashMap<String, PlutoType>>,
}

impl VariableTracker {
    fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn insert(&mut self, name: String, ty: PlutoType) {
        self.scopes.last_mut().unwrap().insert(name, ty);
    }

    fn lookup(&self, name: &str) -> Option<&PlutoType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}

// Usage
let mut tracker = VariableTracker::new();
tracker.push_scope();
tracker.insert("x".to_string(), PlutoType::Int);
let ty = tracker.lookup("x");
tracker.pop_scope();
```

**After (with ScopeTracker):**

```rust
use crate::visit::scope_tracker::ScopeTracker;

// Usage
let mut tracker = ScopeTracker::<PlutoType>::with_initial_scope();
tracker.insert("x".to_string(), PlutoType::Int);
let ty = tracker.lookup("x");
tracker.pop_scope();
```

**Benefit**: Eliminates ~20 lines of boilerplate, clearer intent

---

### Example 2: TypeEnv Refactoring (Conceptual)

**Current TypeEnv pattern** (simplified from `src/typeck/env.rs`):

```rust
pub struct TypeEnv {
    scopes: Vec<HashMap<String, PlutoType>>,
    // ... other fields
}

impl TypeEnv {
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        // Also push to other parallel stacks...
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
        // Also pop from other parallel stacks...
    }

    pub fn lookup(&self, name: &str) -> Option<&PlutoType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    pub fn lookup_with_depth(&self, name: &str) -> Option<(&PlutoType, usize)> {
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(ty) = scope.get(name) {
                return Some((ty, i));
            }
        }
        None
    }
}
```

**Potential refactoring with ScopeTracker:**

```rust
use crate::visit::scope_tracker::ScopeTracker;

pub struct TypeEnv {
    variables: ScopeTracker<PlutoType>,
    task_origins: ScopeTracker<String>,
    immutable_vars: ScopeTracker<()>,  // Set represented as ScopeTracker<()>
    // ... other fields
}

impl TypeEnv {
    pub fn push_scope(&mut self) {
        self.variables.push_scope();
        self.task_origins.push_scope();
        self.immutable_vars.push_scope();
    }

    pub fn pop_scope(&mut self) {
        self.variables.pop_scope();
        self.task_origins.pop_scope();
        self.immutable_vars.pop_scope();
    }

    pub fn lookup(&self, name: &str) -> Option<&PlutoType> {
        self.variables.lookup(name)
    }

    pub fn lookup_with_depth(&self, name: &str) -> Option<(&PlutoType, usize)> {
        self.variables.lookup_with_depth(name)
    }
}
```

**Benefit**: Each scope stack is clearly named and self-documenting, reduces code duplication

---

### Example 3: Closure Capture Analysis

**Pattern (from closure lifting):**

```rust
use crate::visit::scope_tracker::ScopeTracker;
use crate::visit::{Visitor, walk_expr};

struct CaptureAnalyzer {
    locals: ScopeTracker<PlutoType>,
    captures: Vec<(String, PlutoType)>,
}

impl CaptureAnalyzer {
    fn new() -> Self {
        Self {
            locals: ScopeTracker::with_initial_scope(),
            captures: Vec::new(),
        }
    }
}

impl Visitor for CaptureAnalyzer {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        match &stmt.node {
            Stmt::Let { name, .. } => {
                // Add to current scope
                let ty = /* infer type */;
                self.locals.insert(name.node.clone(), ty);
            }
            Stmt::If { .. } | Stmt::While { .. } | Stmt::For { .. } => {
                self.locals.push_scope();
                walk_stmt(self, stmt);
                self.locals.pop_scope();
                return;
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &Spanned<Expr>) {
        match &expr.node {
            Expr::Ident(name) => {
                // Check if identifier is local or captured
                if let Some((ty, depth)) = self.locals.lookup_with_depth(name) {
                    if depth < self.locals.depth() - 1 {
                        // Captured from outer scope
                        self.captures.push((name.clone(), ty.clone()));
                    }
                }
            }
            Expr::Closure { .. } => {
                self.locals.push_scope();
                walk_expr(self, expr);
                self.locals.pop_scope();
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}
```

**Benefit**: Clean scope management for closure analysis without manual stack handling

---

### Example 4: Shadowing Detection

**Use case:** Warn when a variable shadows another in the same scope

```rust
use crate::visit::scope_tracker::ScopeTracker;

let mut vars = ScopeTracker::<PlutoType>::with_initial_scope();

// First declaration
vars.insert("x".to_string(), PlutoType::Int);

// Later in same scope - detect shadowing
if let Some(prev_ty) = vars.insert_shadowing("x".to_string(), PlutoType::Float) {
    eprintln!("Warning: variable 'x' shadows previous binding of type {:?}", prev_ty);
}
```

---

### Example 5: Multi-Level Scope Analysis

**Use case:** Track which scope level a variable was declared in

```rust
use crate::visit::scope_tracker::ScopeTracker;

let mut vars = ScopeTracker::<PlutoType>::new();

// Global scope (depth 0)
vars.push_scope();
vars.insert("global".to_string(), PlutoType::Int);

// Function scope (depth 1)
vars.push_scope();
vars.insert("local".to_string(), PlutoType::Float);

// Block scope (depth 2)
vars.push_scope();
vars.insert("block_var".to_string(), PlutoType::Bool);

// Analyze variable origins
match vars.lookup_with_depth("global") {
    Some((ty, 0)) => println!("global is a global variable"),
    Some((ty, 1)) => println!("global is a function parameter/local"),
    Some((ty, depth)) => println!("global is from nested block at depth {}", depth),
    None => println!("global not found"),
}
```

---

## When to Use ScopeTracker

### ✅ Good Use Cases

1. **Simple variable binding tracking** - Just need to store name → type mappings
2. **Closure analysis** - Track which variables are local vs captured
3. **Scope-aware visitors** - Any visitor that needs to understand nesting
4. **Shadowing detection** - Check if names are redefined in same scope
5. **Multi-level analysis** - Need to know which scope level a binding came from

### ❌ Not Ideal For

1. **Complex state per scope** - If you need multiple HashMaps per scope level, manual stack may be clearer
2. **Non-variable state** - Loop labels, break/continue targets (not name-based lookups)
3. **Global-only state** - If there's no actual nesting, just use a HashMap

---

## Integration with Visitor Pattern

ScopeTracker works naturally with the visitor pattern for scope-aware traversal:

```rust
use crate::visit::{Visitor, walk_stmt};
use crate::visit::scope_tracker::ScopeTracker;

struct MyVisitor {
    scopes: ScopeTracker<MyData>,
}

impl Visitor for MyVisitor {
    fn visit_stmt(&mut self, stmt: &Spanned<Stmt>) {
        match &stmt.node {
            Stmt::Let { name, .. } => {
                self.scopes.insert(name.node.clone(), /* data */);
            }
            // Enter new scope for block statements
            Stmt::If { then_block, else_block, .. } => {
                self.scopes.push_scope();
                // Visit then block
                for stmt in &then_block.node.stmts {
                    self.visit_stmt(stmt);
                }
                self.scopes.pop_scope();

                if let Some(else_blk) = else_block {
                    self.scopes.push_scope();
                    for stmt in &else_blk.node.stmts {
                        self.visit_stmt(stmt);
                    }
                    self.scopes.pop_scope();
                }
                return; // Skip walk_stmt
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }
}
```

---

## Performance Notes

- **Zero-cost for flat scopes**: If you only use one scope level, it's just a `Vec<HashMap>`
- **Efficient lookup**: Reverse iteration is cache-friendly for typical nesting depth (1-5 levels)
- **No allocation overhead**: ScopeTracker reuses the same Vec, only HashMaps are allocated per scope

---

## Comparison to Manual Implementation

| Feature | Manual Stack | ScopeTracker |
|---------|-------------|--------------|
| Lines of code | ~20-30 | ~1-5 |
| Depth tracking | Manual | Built-in |
| Shadowing detection | Manual | `insert_shadowing()` |
| Mutable lookup | Manual | `lookup_mut()` |
| Current scope access | Manual | `current_scope()` / `current_scope_mut()` |
| Type safety | Easy to mix up stacks | Generic type enforces correctness |
| Documentation | Requires comments | Self-documenting API |

---

## Future Enhancements

Possible additions based on demand:

- **Scoped sets**: `ScopeTracker<()>` pattern is common, could have a dedicated `ScopeSet<T>` type
- **Parallel stacks**: Helper for managing multiple aligned scope stacks (like TypeEnv's scopes + task_spawn_scopes)
- **Scope metadata**: Attach metadata to each scope level (e.g., loop depth, function name)
- **Visitor integration**: Auto-push/pop based on AST structure

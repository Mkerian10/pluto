# Bug: Parser Misinterprets Nested Field Access as Qualified Enum Variant

## Summary
The parser incorrectly treats nested field access patterns (e.g., `self.registry.gauges`, `obj.inner.value`) as qualified enum variant access (e.g., `module.Enum.Variant`), causing type errors.

## Reproduction
```pluto
class Inner {
    value: int
}

class Outer {
    inner: Inner
}

fn main() {
    let o = Outer { inner: Inner { value: 42 } }
    let v = o.inner.value  // ERROR: unknown enum 'o.inner'
}
```

Error message:
```
error: unknown enum 'o.inner'
```

## Root Cause

### Parser Speculation (src/parser/mod.rs:2297)
When the parser encounters a pattern like `a.b.c`, it speculatively treats it as a qualified enum variant:

```rust
} else if matches!(&lhs.node, Expr::FieldAccess { object, .. } if matches!(&object.node, Expr::Ident(_))) {
    // Possible qualified enum: module.Enum.Variant or module.Enum.Variant { fields }
    let (module_name, enum_local) = match &lhs.node {
        Expr::FieldAccess { object, field } => {
            match &object.node {
                Expr::Ident(n) => (n.clone(), field.node.clone()),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };
    let qualified_enum_name = format!("{}.{}", module_name, enum_local);
    // Creates EnumUnit speculatively
```

This matches ANY `ident.field.name` pattern without distinguishing between:
- Module qualified enums: `colors.Color.Red`
- Nested field access: `obj.inner.value`
- Self field access: `self.registry.gauges`

### Type Checker Behavior (src/typeck/infer.rs:866-921)
The type checker's `infer_enum_unit()` immediately errors when the enum lookup fails, without attempting to reinterpret the expression as nested field access:

```rust
let ei = env.enums.get(&enum_name.node).ok_or_else(|| {
    CompileError::type_err(
        format!("unknown enum '{}'", enum_name.node),
        enum_name.span,
    )
})?.clone();
```

### Execution Flow
For `self.registry.gauges`:
1. Parser sees `self` (lexed as `Token::SelfVal`, converted to `Ident("self")`)
2. Parses `.registry` → `FieldAccess { object: Ident("self"), field: "registry" }`
3. Sees `.gauges` and matches the condition at line 2297
4. Treats it as qualified enum: `module="self"`, `enum="registry"`
5. Creates `EnumUnit { enum_name: "self.registry", variant: "gauges" }`
6. Type checker tries to look up enum "self.registry"
7. Fails with error: "unknown enum 'self.registry'"

## Impact
- **Blocks meridian metrics client library** - uses `self.registry.gauges`, `self.registry.merge_counter()`
- Affects any code with nested field access of depth > 1
- Common pattern in OOP-style code

## Rejected Solutions
1. **Naming heuristics** - Use uppercase/lowercase to distinguish enums from fields
   - Rejected: Fragile, not reliable, poor developer experience

## Potential Solutions

### Option 1: Parser-based distinction
Check if the first identifier is `Token::SelfVal` or other non-module identifiers before speculating enum variant:

```rust
// In parser, before line 2297 condition:
if let Expr::FieldAccess { object, .. } = &lhs.node {
    if let Expr::Ident(name) = &object.node {
        // Don't speculate if it's 'self' or other non-module idents
        if name == "self" {
            // Parse as nested field access, not enum
        }
    }
}
```

**Pros:** Fixes the common case (self), minimal changes
**Cons:** Doesn't solve the general problem (obj.field.field)

### Option 2: Type checker fallback
If enum lookup fails, try to reinterpret as nested field access:

```rust
// In infer_enum_unit():
match env.enums.get(&enum_name.node) {
    Some(ei) => { /* normal enum handling */ }
    None => {
        // Attempt to parse as nested field access instead
        // Re-tokenize and re-parse the expression
    }
}
```

**Pros:** Handles all cases, backward compatible
**Cons:** More complex, requires expression reinterpretation

### Option 3: Lookahead in parser
Defer the enum vs field decision until more context is available:

```rust
// Don't immediately create EnumUnit
// Create an ambiguous node that typechecker resolves
Expr::QualifiedAccess {
    segments: vec!["self", "registry", "gauges"]
}
```

**Pros:** Clean separation of concerns
**Cons:** Requires new AST node type, typechecker changes

## Recommended Solution
**Option 1** as a quick fix for the `self` case, followed by **Option 3** for the general solution in a future release.

## Workaround for meridian
Refactor code to use intermediate variables to avoid nested field access:

```pluto
// Instead of:
let count = self.registry.gauges.len()

// Use:
let gauges = self.registry.gauges
let count = gauges.len()
```

Or flatten the structure:
```pluto
// Instead of:
class Collector {
    registry: Registry
}

// Use direct fields:
class Collector {
    counters: Map<string, Counter>
    gauges: Map<string, Gauge>
    histograms: Map<string, Histogram>
}
```

## Test Cases
See `/tmp/test_nested.pluto` and `/tmp/test_enum_bug.pluto` for minimal reproductions.

## References
- Parser speculation code: `src/parser/mod.rs:2297-2355`
- Type checker error: `src/typeck/infer.rs:901-906`
- Rejected fix branch: `fix-nested-field-access` (uses naming heuristics)

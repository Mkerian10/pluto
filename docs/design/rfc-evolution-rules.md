# RFC: Compile-Time Evolution Rules

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-14
**Depends on:** `rfc-schema.md`, `rfc-storage.md`, `rfc-migration.md`

## Summary

A compile-time rule engine that reacts to structural diffs between schema snapshots and current source, enforcing policies about what changes are safe, what changes require migration hints, and what changes are forbidden. Evolution rules are the policy layer on top of the migration system — they determine *what's allowed*, while the migration planner determines *how to execute*.

## Motivation

The migration system (`rfc-migration.md`) can compute diffs and generate migration plans. But it needs a policy layer that answers:

- Is this change safe to deploy automatically?
- Does this change require a `from` clause?
- Should this change be blocked entirely?
- Does this change need a two-phase deployment?

Without rules, every schema change would either be allowed silently (dangerous) or require manual review (slow). Rules automate the safety policy.

## Design

### Built-In Rules

The compiler ships with a set of built-in evolution rules. These are always active and cannot be disabled:

#### Field Rules

| Rule | Trigger | Severity | Description |
|------|---------|----------|-------------|
| `required-field-needs-default` | Add non-optional field to existing schema | Error | Adding a required field without a `from` clause means existing data has no value for this field. Must provide `from => default_value`. |
| `type-change-needs-from` | Change field type | Error | Changing a field's type without a `from` clause means no transformation is specified. Must provide `from old_name: OldType => expr`. |
| `field-removal-warning` | Remove field from stored schema | Warning | Removing a field from a schema bound to storage deletes data. Acknowledged by proceeding with the deployment. |
| `rename-needs-from` | Field name change detected (heuristic) | Info | If a field is removed and a new field of similar type is added, suggest a `from` clause to preserve data. |

#### Enum Rules

| Rule | Trigger | Severity | Description |
|------|---------|----------|-------------|
| `variant-removal-blocked` | Remove variant from stored enum | Error | Existing data may reference this variant. Must handle existing data first (migrate to a different variant or add a `from` clause on fields using this enum). |
| `variant-addition-safe` | Add variant to enum | OK | Backward compatible — existing data doesn't reference the new variant. |

#### Storage Rules

| Rule | Trigger | Severity | Description |
|------|---------|----------|-------------|
| `storage-removal-blocked` | Remove storage declaration | Error | Deleting storage means deleting all data. Requires explicit `--allow-data-loss` flag. |
| `index-removal-warning` | Remove index from storage | Warning | May impact query performance. |
| `unique-index-on-existing` | Add unique index to populated storage | Warning | May fail if existing data has duplicates. |

#### Schema Rules

| Rule | Trigger | Severity | Description |
|------|---------|----------|-------------|
| `conditional-discriminator-change` | Change a field's discriminator condition | Error | Changes which data shapes are valid. Existing data may not satisfy new conditions. |
| `spread-removal` | Remove a spread from a schema | Warning | Removes fields that came from the spread. Same as individual field removal for each spread field. |

### Rule Evaluation

Rules are evaluated during the migration planning phase:

```
Source Code  →  Parse  →  Type Check  →  Migration Planner
                                              ↓
                          Snapshot  →  Structural Diff
                                              ↓
                                    Evolution Rules  →  Migration Plan
                                              ↓                ↓
                                    Errors/Warnings     SQL/Operations
```

1. The compiler parses and type-checks the current source
2. The migration planner loads the snapshot and computes the structural diff
3. Each diff entry is evaluated against the evolution rules
4. Rule violations produce errors (blocking) or warnings (informational)
5. If no blocking errors, the migration plan is generated

### Rule Output

Rules produce three kinds of output:

**Errors** — Block compilation. The developer must fix the issue (add a `from` clause, provide a default, etc.) before the migration plan is generated.

```
error[E0401]: adding required field 'created_at' to stored schema 'User' without default
  --> src/models.pluto:15:5
   |
15 |     created_at: int
   |     ^^^^^^^^^^^ no `from` clause or default value
   |
   = help: add a default: `created_at: int from => 0`
   = help: or make it optional: `created_at: int?`
```

**Warnings** — Allow compilation but surface potential issues.

```
warning[W0201]: removing field 'legacy_id' from stored schema 'User'
  --> src/models.pluto:8:5
   |
   = note: this will delete the 'legacy_id' column from the 'users' table
   = note: data in this column will be permanently lost
```

**Info** — Suggestions, not problems.

```
info: field 'name' removed and 'full_name' (same type) added to schema 'User'
  --> src/models.pluto:10:5
   |
   = help: if this is a rename, add: `full_name: string from name: string => name`
```

### Wire Compatibility Rules

When schemas are used in cross-process communication (stage pub methods), additional rules apply:

| Rule | Trigger | Severity | Description |
|------|---------|----------|-------------|
| `wire-breaking-change` | Incompatible schema change on a stage pub method parameter or return type | Error | The sender and receiver must agree on the schema. Breaking changes require all services to be updated simultaneously. |
| `wire-additive-ok` | Add optional field to a wire schema | OK | Old receivers ignore new fields. New receivers handle missing fields via `none`. |
| `wire-removal-two-phase` | Remove field from a wire schema | Warning | Requires two-phase deployment: receiver stops reading the field first, then sender stops sending it. |

### Interaction with `from` Clauses

`from` clauses are the primary mechanism for satisfying evolution rules:

- `required-field-needs-default` → satisfied by `from => default`
- `type-change-needs-from` → satisfied by `from old: OldType => transform(old)`
- `rename-needs-from` → satisfied by `from old_name: Type => old_name`

The rule engine checks for matching `from` clauses before emitting errors. If a `from` clause resolves the issue, no error is produced.

### Strict vs. Permissive Mode

The compiler supports two evolution modes:

**Strict (default):** All rules enforced. Breaking changes require `from` clauses. Data loss requires explicit flags. This is the safe default for production.

**Permissive (`--allow-breaking`):** Warnings instead of errors for breaking changes. Useful during early development when schemas are in flux and there's no production data to protect.

```bash
# Strict (production)
plutoc migrate --snapshot prod.snapshot

# Permissive (development)
plutoc migrate --snapshot dev.snapshot --allow-breaking
```

### Custom Rules (Future)

A future extension allows projects to define custom evolution rules:

```
evolution rule no_float_to_int {
    match FieldTypeChanged(schema, field, Float, Int) {
        error "converting float to int loses precision — use a decimal type instead"
    }
}

evolution rule require_audit_fields {
    match SchemaAdded(name) where has_storage(name) {
        require field(name, "created_at", Int)
        require field(name, "updated_at", Int)
        error "stored schemas must have created_at and updated_at fields"
    }
}
```

Custom rules are declared in source files and participate in the same evaluation pipeline as built-in rules. This is future work — not needed for v1.

## Implementation

### Rule Engine

The rule engine is a simple pattern matcher over diff entries:

```rust
struct EvolutionRule {
    name: &'static str,
    severity: Severity,
    matches: fn(&DiffEntry, &EvolutionContext) -> bool,
    message: fn(&DiffEntry, &EvolutionContext) -> String,
}

enum Severity {
    Error,
    Warning,
    Info,
}

struct EvolutionContext {
    snapshot: Snapshot,
    current: CurrentState,
    storage_bindings: HashMap<String, StorageDecl>,
    from_clauses: HashMap<(String, String), Vec<FromClause>>,
}
```

### Evaluation

```rust
fn evaluate_rules(diff: &[DiffEntry], ctx: &EvolutionContext) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for entry in diff {
        for rule in BUILT_IN_RULES {
            if (rule.matches)(entry, ctx) {
                // Check if a from clause resolves this
                if !resolved_by_from_clause(entry, ctx) {
                    diagnostics.push(Diagnostic {
                        severity: rule.severity,
                        message: (rule.message)(entry, ctx),
                        rule_name: rule.name,
                    });
                }
            }
        }
    }
    diagnostics
}
```

### Integration Points

The rule engine runs as part of the migration planner:

1. `src/migration.rs` computes the structural diff
2. `src/evolution.rs` evaluates rules against the diff
3. Blocking errors prevent migration plan generation
4. Warnings and info are included in the migration plan output
5. The CLI presents diagnostics in the standard compiler diagnostic format

## Examples

### Example 1: Safe Addition

```
// Snapshot: User { id, name, email }
// Current:  User { id, name, email, avatar_url: string? }

// Rule evaluation:
// - FieldAdded(User, avatar_url, string?) → optional field → OK
// Migration plan: ALTER TABLE users ADD COLUMN avatar_url TEXT;
```

### Example 2: Blocked Removal

```
// Snapshot: User { id, name, email, legacy_id }
// Current:  User { id, name, email }

// Rule evaluation:
// - FieldRemoved(User, legacy_id, string) → stored schema → Warning
// Migration plan generated with warning:
//   ALTER TABLE users DROP COLUMN legacy_id;
//   -- WARNING: data in 'legacy_id' column will be permanently lost
```

### Example 3: Missing `from` Clause

```
// Snapshot: Order { id, total: float }
// Current:  Order { id, total_cents: int }

// Rule evaluation:
// - FieldRemoved(Order, total, float) → stored schema → Warning
// - FieldAdded(Order, total_cents, int) → required field without from → Error!

// Compiler error:
// error[E0401]: adding required field 'total_cents' without default
//   = help: add `total_cents: int from total: float => int(total * 100.0)`
```

### Example 4: Fixed with `from`

```
// Current: Order { id, total_cents: int from total: float => int(total * 100.0) }

// Rule evaluation:
// - FieldRemoved(Order, total, float) → from clause matches → resolved
// - FieldAdded(Order, total_cents, int) → from clause provides transformation → OK
// Migration plan:
//   ALTER TABLE orders ADD COLUMN total_cents INTEGER;
//   UPDATE orders SET total_cents = CAST(total * 100 AS INTEGER);
//   ALTER TABLE orders DROP COLUMN total;
```

## Open Questions

- [ ] **Rule precedence.** When multiple rules match the same diff entry, how do they compose? Currently: all matching rules produce diagnostics. If any is an Error, the entry blocks. Could add explicit precedence if needed.
- [ ] **Custom rule syntax.** The custom rule syntax shown above is speculative. Should it be Pluto code, a DSL, or configuration? Deferred to v2.
- [ ] **Per-environment rules.** Should different environments (dev, staging, prod) have different rule strictness? Currently handled by `--allow-breaking` flag. Could be more granular.
- [ ] **Schema ownership.** In a multi-team system, who "owns" a schema? Can team A change a schema that team B uses? This is an organizational question that rules could enforce (e.g., "schema changes to shared schemas require explicit approval").

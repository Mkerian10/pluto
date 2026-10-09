# RFC: Module Model & Name Resolution

**Status:** Draft (2026-10-08) — core model agreed in design discussion; migration details open (see §8)
**Author:** Design discussion
**Date:** 2026-10-08
**Related:** [rfc-module-semantics.md](rfc-module-semantics.md) (supersedes its point-fix framing of #413/#437), [rfc-native-modules.md](rfc-native-modules.md), [ai-native-representation.md](ai-native-representation.md) (UUIDs vs DefIds), `docs/v1-vision.md` (ceremony-free structure pillar); issues #390, #391, #401, #437, #484, #502, #503, #504; in-flight branch `module-canonical-identity`

## 1. The problem

The module system is the weakest load-bearing part of the language. It shows
up as two things that are really one thing:

- **A bug cluster.** Seven open issues (#390, #391, #401, #437, #484, #502,
  #503, #504) that look unrelated but share a single root. The worst, #502, is
  a *silent wrong answer*: linking an otherwise-unused module changes what a
  program computes, with no error and no warning. Its sibling #503 rejects a
  type-correct program based only on what one field is *named*. #504 loses
  nullable narrowing in codegen once the module graph is large enough.
- **A smell.** Real projects don't use subdirectories. That is not a style
  tic — the current model punishes hierarchy (see §2), so code converges on
  flat layouts, and an agent optimizing for "compiles and does what I wrote"
  does the same.

These are the same gap seen from two ends: **Pluto has no name-resolution
phase.** It has a *renaming* phase wearing one's clothes.

## 2. Root cause (what exists today)

Modules are resolved by **string rewriting over a flattened single
namespace**, before typeck:

1. `flatten_modules` (`src/modules.rs`) merges every module's declarations into
   one `Program`, renaming each by string: `add` → `math.add`
   (`prefix_name`, literally `format!("{}.{}", module, name)`).
2. `ModuleRewriter` then walks each module body and **string-rewrites
   references** to match. Its core (paraphrased):
   ```rust
   Expr::Call { name, .. } =>
       if self.module_prog.functions.iter().any(|f| f.node.name.node == name.node) {
           name.node = prefix_name(self.module_name, &name.node);
       }
   ```

Every defect in the cluster lives in that mechanism:

- **Not scope-aware.** "Is there a module function with this string?" is a
  global membership test, not a scope walk. It cannot distinguish a local, a
  parameter, or a shadowing binding from a top-level reference. A local `at`
  and a `fn at` are indistinguishable. → #390, #502.
- **Resolves against the *flattened* set,** whose contents depend on the import
  graph. The same source resolves differently depending on what else is linked.
  → #502 verbatim.
- **A hand-maintained match over AST shapes.** Every construct (`Call`,
  `StructLit`, `EnumUnit`, `Match`, `Raise`, `Catch`, state clauses, provides,
  satisfies, …) needs its own arm; a new construct silently mis-resolves until
  someone adds one. → #484 (missing arm), #504 (facts keyed by `(file, span)`
  survive but with the wrong identity).
- **Identity is a string.** Nominal types, typed-`catch` matching, and xref all
  key off the prefixed string. Reach one declaration two ways → two strings →
  split type. → #391, #437.

And the module *model* underneath it is thin. A **directory is one module**:
`load_directory_program` merges all `.pt` files in a directory into one flat
`Program`. Files in a directory are not submodules; they are fragments of one
module, concatenated. `collect_source_files` does not recurse, so
subdirectories are neither merged nor importable as children — they are
invisible. There is **no nesting and no per-file boundary**, which is why
hierarchy is inexpressible and why two files in one directory silently share a
namespace (#401).

The two layers:

- **Model** (language design): what a module *is*.
- **Name resolution** (implementation): the pass that makes the model true.

A resolver resolves names *according to a model*. The model has to be pinned
first, or the machinery has nothing principled to implement.

## 3. The model (agreed)

### 3.1 Module = directory

A directory is a module; its `.pt` files are merged (Go's package model).
Within a package everything is visible to everything — no intra-package import
or qualification ceremony. This is the ergonomic win and it is kept
deliberately: split a large module into files by topic, move a function between
files, and nothing else changes.

### 3.2 Subdirectory = child module

A subdirectory is a *child module*, named `parent.child` from the outside
(`db/` is `db`, `db/catalog/` is `db.catalog`). Hierarchy comes from
directories nesting. A module's **canonical directory path is its identity** —
reaching the same directory two ways is one module, one set of declarations,
one nominal identity (this is what closes #391/#437 at the root). The resolver
already keys modules by canonical path (`Resolver::ids_by_path`); today that
identity is discarded at flatten time. This RFC keeps it all the way down.

### 3.3 Visibility: `pub` is the package boundary

Unmarked declarations are **package-private** (visible across the package's own
files, invisible outside it). `pub` makes a declaration reachable from outside
the package. One boundary, not a gradient. No `pub(crate)`-style intermediate
levels in this RFC (see §9 Non-goals).

### 3.4 Referencing: the qualified name *is* the reference (model "C")

**First-party code is never imported.** A declaration in another package of the
same project is referenced by its canonical qualified name at the use site:

```pluto
db.catalog.lookup(key)
```

- The qualified path is resolvable iff the target is `pub` — `pub` still fully
  gates reach; "no import" removes *reachability ceremony*, not *visibility
  control*.
- `import` is reserved for **external dependencies** (another `pluto.toml`
  package). It brings that dependency's root name into scope.
- An optional `as` alias may be introduced purely for brevity
  (`db.catalog as cat`), never for reachability. (Alias scope — file or
  block — is open, see §8.)

Rationale: canonical path is already the module identity, so the name you
*write* is the identity is the reference — there is no separate import binding
that can drift out of sync with what it names. It is maximally ceremony-free
(a stated v1 pillar), and `pub` remains the single visibility boundary. The
cost — losing a hand-maintained per-file dependency list — is accepted: the
compiler/LSP derives the dependency set trivially, and "let the tool derive it"
is on-brand for the AI-native direction.

**Relative imports are rejected.** `import catalog` relative to the current
directory would make a name mean different things depending on the file's
position — the same context-dependent-name disease this RFC exists to kill,
moved up to the import line.

### 3.5 Duplicate definition across sibling files is an error

Two files in one package defining the same name (`fn helper` in both
`catalog.pt` and `wal.pt`) is a genuine conflict and must be a clear,
located error naming both sites — not a silent merge. → closes #401.

## 4. Name resolution (the implementation)

Introduce a real **resolver** pass that runs after module discovery and before
typeck. It walks scopes and binds every identifier to a `Res` — a resolved
target:

- a `DefId` (a specific top-level declaration, identified by `(module
  canonical path, declaration)`),
- a local slot (a specific binding introduced in this body),
- or a type parameter.

After resolution, **no downstream phase looks a name up by string.** Typeck,
monomorphization, marshal, xref, and codegen consume `Res`/`DefId`. Flattening
is demoted from *the mechanism* to, at most, a `DefId`-keyed lowering detail —
or removed.

### 4.0 `Res`/`DefId` and the stable UUID are the same idea at two maturities

This is not a new identity scheme competing with the AI-native UUIDs — it is the
missing half of them. `ai-native-representation.md` already specifies that a
call site should store the callee's **UUID, not its name** ("resolve references
by UUID lookup rather than name resolution", §*UUID in Cross-References*). Today
that is not what happens: UUIDs are minted fresh (`Uuid::new_v4()`) at parse,
and `xref::resolve_cross_refs` builds a **name-keyed** `DeclIndex` over the
*flattened* program and stamps the matched declaration's UUID onto each
reference. So cross-references are *derived from* the broken string resolution —
xref is **inside** this bug cluster's blast radius, not above it. If string
resolution picks the wrong declaration, it stamps the wrong UUID.

The resolver inverts that. The clean split (rustc's `DefId` / `DefPathHash`):

- **`DefId`** — a session-local, interned, cheap handle the resolver assigns to
  every declaration *within one compilation*. `Res` carries it; typeck,
  monomorphize, marshal, codegen key on it. Fast, not portable.
- **UUID** — the **durable, portable** identity: stable across renames/moves,
  globally unique, persisted in `.pluto`. The `DefPathHash` analogue.
- **Bridge** — the resolver maintains a `DefId ↔ UUID` table. References resolve
  to `DefId` during the compile; serialization lowers `DefId → UUID`. The
  name-keyed `DeclIndex` is **retired**: cross-references now come straight out
  of the resolver, which means they are resolved by *identity* and are immune to
  the cluster by construction.

The existing "mint at parse, match by name+signature on `.pluto` sync" machinery
is unaffected — UUIDs keep their durable role. What changes is that *name* stops
being the resolution key anywhere in the compiler. This makes the AI-native
UUID-cross-reference guarantee **true for the first time**, rather than true
only when string resolution happened to be right.

### 4.1 The resolution rule (precise)

For an unqualified identifier, resolve against this scope chain, inner first,
with inner bindings shadowing outer:

1. locals and parameters of the enclosing block/function (lexical);
2. the **current package** — all declarations across all its files (§3.1);
3. a `pub` declaration of an **ancestor/other package** named by a qualified
   path at the use site (§3.4);
4. an imported **external dependency** root (§3.4).

A qualified path `a.b.c` resolves by walking the module tree from a root
(current package root for first-party, dependency root for external), and
succeeds only if each boundary it crosses is permitted by `pub`.

This chain is the whole fix: step 1 existing and being *checked first* is what
stops a local from colliding with a module function (#390/#502); steps 2–4
being resolved against the module *tree by identity*, not a flattened string
bag, is what stops "depends what's linked" (#502) and split identity
(#391/#437).

## 5. What this fixes

| Issue | Cause under today's model | Fixed by |
|-------|---------------------------|----------|
| #502 | local vs flattened fn name; flat bag depends on link set | §4.1 steps 1–2, identity-keyed tree |
| #390 | local shadows flattened fn name | §4.1 step 1 precedence |
| #391 | diamond import → two prefixed strings → split type | §3.2 canonical-path identity |
| #437 | transitive same-named classes collide | §3.2 identity + §3.3 boundary |
| #484 | missing rewriter arm for sibling variant | §4 resolver replaces per-arm rewriting |
| #503 | field name collides across enum types in flat namespace | §3.2 identity + §4 DefId fields |
| #504 | narrowing facts keyed by `(file, span)`, wrong identity in big graph | §4 Res/DefId identity through codegen |
| #401 | sibling files silently merge | §3.5 duplicate-definition error |

## 6. Pipeline changes

Today (CLAUDE.md steps 3–4): *module resolve* → *module flatten* (string
prefix + `ModuleRewriter`), then everything downstream sees one flat `Program`
of strings.

Proposed:

1. **Module discovery** — build the module *tree* keyed by canonical directory
   path (packages and their child packages). Largely the existing `Resolver`,
   minus the discard of identity.
2. **Resolve** — assign `Res`/`DefId` to every identifier per §4. Duplicate and
   visibility errors surface here with real locations.
3. Typeck / reflection / monomorphize / marshal / xref / codegen consume
   `DefId`. Symbol/type identity, catch matching, and flow-fact keys are all
   `DefId`-based, not string-based.

Staging is open (§8): a conservative intermediate keeps a flattened `Program`
but keyed by `DefId` (prefix strings become cosmetic) before fully threading
`DefId` through every phase.

## 7. Backward compatibility

The common case is unaffected: a project that is a single directory of `.pt`
files is one package, everything already sees everything, and no `import`
between first-party files was ever written. Such projects compile unchanged.

The change bites only where today's behavior *relied on the defect* — e.g.
cross-package references that currently work by flat-namespace accident must
become qualified (`db.catalog.X`), and names that collided silently now error.
Both are the intended corrections.

## 8. Recommendations and open questions

### Recommended (investigated; open to override)

- **`DefId` ↔ UUID = the same identity at two maturities** (fully argued in
  §4.0). `DefId` is the session-local handle, the UUID is its durable name, the
  resolver owns the table between them, and the name-keyed `DeclIndex` is
  retired. This is not a separate scheme from the AI-native UUIDs — it is what
  finally makes "cross-references resolve by UUID, not name" actually hold.

- **Staging: B then C, with C committed.** Land it in two milestones rather than
  one big-bang:
  - *Milestone 1 (B):* introduce the resolver + `DefId`, but keep emitting a
    flattened `Program` whose names are **derived from `DefId`**
    (guaranteed-unique mangling). Downstream string-keyed phases keep working
    unchanged, yet can no longer collide — this alone closes the entire bug
    cluster with minimal blast radius.
  - *Milestone 2 (C):* thread `DefId` through typeck / monomorphize / marshal /
    codegen / xref, retire the name-keyed `DeclIndex`, route UUID lowering
    through the resolver table.

  C is **not optional long-term**: the AI-native pillar requires UUID-based
  resolution (§4.0), which requires the resolver threaded through — i.e. C. B is
  the safe landing, C is the destination.

### Still open (owner decisions)

- **Stdlib migration** — the 23 stdlib modules + `prelude.pt`. Do they become
  packages under the new tree unchanged? How does prelude injection
  (`src/prelude.rs`) interact with package-private vs `pub`? Does the stdlib
  ABI handshake (`STDLIB_ABI_VERSION`) need a bump?
- **External dependency namespacing** — how a `pluto.toml` dependency's root
  name enters scope; collision between a dependency root and a first-party
  top-level package name; version identity.
- **Alias scope** — is `as` a file-level or block-level binding? Can it rename
  a first-party package for brevity even though first-party needs no `import`?
- **Qualified-name / local-name ambiguity** — if a package is named `foo` and a
  local is named `foo`, the §4.1 chain says local wins; confirm that is the
  wanted rule and that a qualified `foo.bar` is unambiguous regardless.
- **Whole-program cost** — resolution is whole-project; confirm acceptable
  against incremental-compile goals (note: resolution is strictly cheaper than
  today's clone-heavy flatten-and-rewrite).

## 9. Non-goals

- A visibility gradient (`pub(package)`, `pub(super)`, friend modules). One
  boundary (`pub`) for now.
- Changing the within-package "everything sees everything" convenience.
- Macro hygiene (Pluto has no macros).

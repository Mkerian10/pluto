# RFC: Module Semantics — Privacy, Provenance, and the Library Path

**Status:** Draft — awaiting owner review; field-privacy default and the entity-access resolution were ratified in design discussion (2026-10-04)
**Author:** Design discussion
**Date:** 2026-10-04
**Related:** [rfc-boundaries.md](rfc-boundaries.md) (protocols are modules — the thesis this RFC makes true), [rfc-verification.md](rfc-verification.md) (STRICT discharge — the solver this RFC leans on), [epistemics.md](epistemics.md), [rfc-objects.md](rfc-objects.md) (entities), issues #421, #413, #437, #440

## The problem

The module system is about to become the load-bearing construct of the
protocol thesis: *the protocol is the module* — authority theorems via
`satisfies`, client conformance by typechecking against exported types. That
plan currently rests on four defects and two never-decided semantics:

- **#421** — imported generic classes cannot be instantiated at all (a
  grammar hole), and the accident is the only thing preventing forgery of
  `fs.File<Read, Open>` from outside std.fs. Worse: non-generic evidence is
  forgeable *today* — `blob.WriteGrant { token: auth.epoch_now() }` compiles
  and the fence accepts it. The proven theorem ("no unfenced write")
  survives; the stronger informal claim ("only grantees write") does not.
- **#413 / #437** — directory-module sibling imports fail resolution;
  transitively-imported same-named classes collide in flattening.
- **#440** — external entity field reads take no lock (a formal data race
  with observable mid-method cross-field states); container fields read raw
  hand out live references that mutate serialized state past the lock.
- Undecided: field visibility, re-exports.

One RFC, so the library wave builds on decided ground instead of accidents.

## Doctrine: three mechanisms, three jurisdictions

A design discussion considered and **rejected** a `sealed`/`evidence` class
marker: contracts will be common on ordinary data types, and the normal case
for a contract-carrying type is that outsiders *do* construct and mutate it —
with the solver standing guard, not a wall. Hiding and correctness are
separate concerns, and conflating them was the error. The decomposition:

- **The solver guards what values can be** — at every construction and write
  site, in every module (STRICT discharge is whole-program), and, validated
  with proven coverage, at decode.
- **Field privacy guards where values come from** — opt-in, per field, where
  an author wants it; provenance for evidence types falls out as a corollary.
- **Fencing guards what values may do** — judged by the authority at the
  point of effect, as always. No mechanism pretends to do another's job.
- **Entities speak only in messages** — with a measured carve-out for scalar
  observation (below).

## 1. Field privacy: `priv`

```pluto
pub class WriteGrant {
    priv token: int
}
```

- **Default public.** Most Pluto classes are wire-shaped data whose fields
  are the API; a private-by-default rule would break every existing class
  and fight the language's low-ceremony grain. The marker is on the rare
  hidden field.
- Outside the declaring module, a `priv` field cannot be read or written,
  and — since a struct literal must name every field — **any `priv` field
  makes external literal construction impossible**. Construction then flows
  through the module's own functions, which is what provenance means.
- Inside the module: full rights. Copying, passing, comparing, matching on
  the value as a whole: unchanged everywhere (privacy controls provenance,
  not usage).
- **Privacy is not secrecy.** A `priv` field still crosses the wire in a
  wire-shaped type (decode is the sanctioned constructor — §4), and its
  bytes are visible to the peer. `priv` is about who may *create and alter*
  field-states in-process, nothing more.
- Provenance remains a history property no invariant can express ("this
  token was minted by `grant_write`") — `priv` is how a module makes it a
  fact. With `token` private, the live forgery and the tamper
  (`g.token = epoch_now()`) both become compile errors, and std.blob's
  informal claim upgrades to the real one.

## 2. External instantiation is proof-gated, not walled (#421)

The grammar learns `mod.Class<T> { ... }`. Nothing else is new: discharge
already runs whole-program at every construction and write site, and
generic-class contracts are template-proven under skolems (#363). External
instantiation of contract-carrying types is simply *allowed and proven*,
same as internal.

Two hard requirements ride along:

- **Sequencing: `priv` lands with or before the grammar fix** — the parser
  hole is currently the only thing between the world and a forged
  `File<Read, Open>`. The evidence fields in std.fs and std.blob are marked
  `priv` in the same change, so the door never stands open.
- **A cross-module discharge battery.** Cross-module is where this
  machinery's latent bugs have lived (the #382 flattening fixes, #384's
  wire probe). The fix ships with external-instantiation discharge tests:
  external construction that violates an invariant rejects with the proof
  diagnostic; external mutation sites discharge; generic external
  instantiation goes through the skolem path; monomorphized ownership is
  determined structurally from the base name, never by re-parsing mangled
  strings (the #412 lesson).

## 3. Entity access: writes never, scalars locked, structures by message

External **writes** to entity fields are already rejected (#427/#432, zero
collateral). This RFC completes the story, incorporating the steelman run
against full methods-only access:

- **External reads of scalar-shaped fields are allowed, lowered through the
  read lock.** Scalar-shaped: `int`, `float`, `bool`, `byte`, `string`,
  unit-variant enums — types that cannot smuggle mutation. This preserves
  the dominant idiom (`server.port`, `c.count`) without hand-written getter
  ceremony, and fixes #440's tearing/ordering race at the cost of an
  uncontended rdlock.
- **External reads of reference-shaped fields are rejected** — arrays,
  maps, sets, `bytes`, class-typed, and nullables of those. The raw read
  *is* the escape hatch (`e.arr[0] = 1`, `e.cfg.x = 1` mutate serialized
  state past the lock); the honest interface is a method returning a
  deliberate copy. Closes #440's remaining holes structurally.
- **Cross-field consistency is a method, by doctrine.** No field-level
  mechanism can provide it; a snapshot method holds the lock across the
  whole observation. Stated in the entity chapter, enforced by nothing —
  because single-field reads are individually honest.
- **External literal construction of entities stays legal.** The line is
  principled, not arbitrary: construction happens before the value is
  shared — a single-threaded moment, race-free by happens-before; field
  access happens *during* sharing and races with serialized methods.

Acceptance input before the enforcement lands: a read-site sweep measuring
reference-field read collateral in stdlib/examples/tests (scalar reads, the
common case, are untouched).

## 4. Decode: the sanctioned constructor, with a coverage theorem

Wire decode constructs values — including `priv`-fielded ones — in foreign
processes; that is its job. The trust it restores is predicate-grade and
must be *provably delivered*:

- Every declared single-state invariant on a boundary-crossing type is
  validated at decode (the one sanctioned runtime check). All decode paths
  funnel through one structural validation point, so a new path cannot skip
  it — the systematic fix for the silent-skip family (#384, #426), which
  has so far been killed one instance at a time.
- A sealed-or-invariant-bearing type crossing a boundary whose decode path
  the compiler cannot instrument is a compile error.
- `pluto analyze` gains the **perception report** — every boundary-crossing
  type, the invariants checked at its decode, the typed error on violation —
  beside the assumption surface and the arithmetic residue.
- The residue is stated, not papered over: decode restores predicates,
  never provenance. Provenance is re-established only at the authority
  (the fence), which is why the fence is an exact-match check at the point
  of effect and always was.

## 5. Cycles: rejection is doctrine

Module import cycles are rejected ("circular import detected") — confirmed
for directory and single-file modules alike. This RFC ratifies the behavior
as doctrine rather than accident: **library layering is a DAG, enforced.**
The flatten-with-prefixes model effectively requires it, and for protocol
libraries it enforces honest layering (`wal → {fs, hash}`,
`receipts → wal`, `broker → {wal, blob}`); a cycle between protocol layers
is a design smell the compiler catches for free. One fix rides along: the
rejection currently reports as a "Codegen error" though it fires during
resolution — it becomes a resolution-phase diagnostic with the import chain
in the message.

## 6. Resolution defects

- **#413** — sibling imports between directory modules
  (`locks/locks.pt` importing `marks/marks.pt`) fail with "cannot find
  module." Pure resolution gap, reproduces with no typestates involved.
  Fixed with pins; multi-file libraries are the normal shape.
- **#437** — two modules both declaring `pub class Foo` import fine
  directly but collide ("already declared") when arriving transitively
  through a middle module; prefixing loses a layer somewhere. Fixed with
  pins, including the single-letter-module oddity noted in the issue.

## 7. Re-exports

`std.wal` will raise `fs.SyncError`; without re-export, every consumer must
`import std.fs` to write the catch — leaking the dependency and doubling
the import list. Proposal: a minimal named re-export form —

```pluto
pub import fs.SyncError
```

— making the item part of this module's surface under its name
(`wal.SyncError`), one item per line, no globs, no renaming in v1.
Flattening already carries prefixed items; re-export is an aliasing entry
in the module's export table, not a copy. Deliberately small: globs and
renames can come later if real libraries demand them.

## Owner decisions

1. **`priv` spelling and default** — `priv` on fields, default public.
   *Ratified in discussion; recorded here.*
2. **Entity access shape** — writes never / scalars locked / structures by
   message / consistency by doctrine. *Landed via steelman; ratify here.*
3. **Scalar set** — is `string` in the scalar-read set? Recommended yes
   (immutable); `bytes` stays out (mutable).
4. **Re-exports** — minimal named form now (recommended) vs defer entirely.
5. **Error-attribution fix scope** — fold the resolution-phase reporting fix
   into the wave (recommended) vs separate.

## Phasing

1. **`priv` + the #421 grammar fix + evidence-field migration + the
   cross-module discharge battery** — one change, so the forgery door never
   opens. Closes #421.
2. **Entity access rules** (after the read-site sweep) — closes #440.
3. **Decode coverage + the perception report.**
4. **Resolution fixes** — #413, #437, cycle-error attribution.
5. **Re-exports.**

Each phase independently shippable. The hardening wave implements against
the ratified text; the distributed-library wave (std.wal first) follows on
decided ground.

# RFC: Conversions and Number Types

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-10-03
**Related:** [rfc-verification.md](rfc-verification.md), [contracts.md](contracts.md), [v1-vision.md](../v1-vision.md) (Static Verification); issues #395, #398, #416, #441, #442

## Summary

1. **Remove `as`.** Every conversion is a named method. Conversions that can
   lose information return a nullable: `f.to_int()` is `int?`, `x.to_byte()` is
   `byte?`.
2. **Proof-narrowed conversions.** When flow facts prove a conversion is in
   range, its result narrows from `T?` to `T` — the same mechanism as nullable
   narrowing. Guards and `assert` are how a programmer explains; no annotation
   language.
3. **Width types.** `int(8/16/32/64)`, `uint(8/16/32)`, `float(32)` are
   storage shapes: packed in arrays, widened to `int`/`float` on read, written
   through checked (proof-narrowed) conversions. All arithmetic stays `int`;
   `byte` becomes `uint(8)`.
4. **Collection facts.** Element facts live in element types (`[uint(8)]`);
   length facts become ghost int fields of invariant-bearing classes, made
   sound by closing three aliasing doors. This answers most of #398 without a
   runtime-checked tier.
5. **Rejected:** range (refinement) types, user-defined operator overloading,
   downcasting from traits to classes, user-defined conversions.

The common thread: operators and conversions have **fixed, compiler-defined
meanings** that the verification engine can reason about. Expressiveness comes
from *declaring* properties of types, never from user code behind an operator.

## Motivation

### `as` silently produces wrong values

`as` is a closed set of six built-in conversions. Three of them lose
information without saying so:

| Cast | Today | Silent loss |
|---|---|---|
| `float as int` | `fcvt_to_sint_sat` | NaN → 0, ±inf / out-of-range saturate (#442) |
| `int as byte` | low 8 bits | `300 as byte == 44` |
| `int as float` | round to nearest | ints above 2^53 lose precision (undocumented) |
| `int as bool` | `!= 0` | (deliberate, C idiom) |
| `bool as int`, `byte as int` | widen | none |

#416 established the doctrine that **defects trap and conditions raise**:
arithmetic overflow traps, and #441 extended this to out-of-range shift
amounts. Casts are the last numeric operation that silently fabricates values.

### Conversion has three inconsistent spellings

`as` (six casts), primitive methods (`string.to_int()` returns `int?`;
`float.to_int()` saturates), and stdlib functions (`strings.parse_int` raises
`ParseError`). The same method name `to_int` has different failure semantics
on different receivers.

### `as` has no other job

Pluto has no class inheritance. Upcasts (class → trait, `T` → `T?`, `fn` →
fallible `fn`) are implicit coercions, and downcasts do not exist — recovering a
concrete type is done with enums + `match`, nullable narrowing, or typed
`catch`. `as` is purely numeric representation change, which a method spells
just as well, with the failure mode in the name.

### Invariants cannot talk about collections (#398)

Class invariants are restricted to linear arithmetic over the class's own int
fields. `invariant self.segments.len() > 0` is rejected as outside the provable
fragment, so the most common real invariants ("never empty", "every element in
range", "append-only increasing") live in comments.

## Design

### 1. Remove `as`

| Today | Replacement | Failure behavior |
|---|---|---|
| `byte as int` | `b.to_int()` | total |
| `bool as int` | `b.to_int()` | total |
| `int as float` | `x.to_float()` (exists) | total; rounds to nearest above 2^53 (documented) |
| `int as bool` | `x != 0` | removed — the comparison says it better |
| `float as int` | `f.to_int()` → `int?` | `none` for NaN, ±inf, out of range; truncates toward zero |
| `int as byte` | `x.to_byte()` → `byte?` | `none` outside 0..255 |
| (deliberate truncation) | `x.low_byte()` | low 8 bits, by name |

Rounding is explicit and composes: `f.round().to_int()`, `f.floor().to_int()`.

`as` stops being a keyword. Migration: ~80 uses in stdlib + examples (44
`as byte`, 31 `as int`, 3 `as float`, 2 `as bool`) and ~213 in tests. The
`byte as int` cases are mechanical; each `int as byte` site gets a deliberate
choice between `to_byte()` and `low_byte()` — a stdlib survey found none of the
16 current sites relies on truncation (all are masked, constant, or known
in-range).

`string.to_int()` already returns `int?` (and as of #448 overflow yields `none`
rather than a clamped value), so the two `to_int` methods converge.

### 2. Proof-narrowed conversions

A conversion that can fail is declared to return `T?`. When the flow-fact
engine (`src/typeck/facts.rs`) proves the input is in range at the call site,
the result is narrowed to `T`:

```pluto
fn put(buf: bytes, x: int) {
    if x >= 0 && x <= 255 {
        buf.push(x.to_byte())       // narrowed: byte
    }
}

fn put2(buf: bytes, x: int) {
    assert x >= 0 && x <= 255       // runtime abort if false; fact afterward
    buf.push(x.to_byte())           // narrowed: byte
}

buf.push(61.to_byte())              // constant: byte
```

This is not a new kind of typing. Pluto already gives one expression different
types depending on flow facts: nullable narrowing (`if x != none { /* x: T */
}`). Proof narrowing is the same mechanism with a richer fact domain. Existing
rules carry over:

- A redundant fallback on a narrowed conversion (`x.to_byte() ?? 0` where the
  range is proven) is legal, as redundant `?` on a narrowed variable is today.
  It may warn ("fallback is unreachable"), mirroring the existing degenerate
  condition warnings.
- **The programmer explains with control flow.** Guards (`if`, early
  `return`/`continue`/`break`) and `assert` are the explanation language.
  There are no hints, tactics, or loop-invariant annotations.
- `assert` is the "I know this fits; trap if I'm wrong" form. No unwrap-or-trap
  operator is added.

#### Diagnostics are part of the design

When a conversion stays `T?` and is used where `T` is required, the error must
point at the **conversion**, state what is known, and suggest the missing fact:

```
error: `x.to_byte()` may be none
  x is known >= 0 but has no upper bound
  help: add a guard (`if x > 255 { ... }`) or `assert x <= 255`
```

Without this, narrowing feels arbitrary; with it, it teaches.

#### Stability

Whether a program type-checks must not depend on prover cleverness that
varies between compiler versions. The provable fragment is specified (as
rfc-verification.md already does for invariants), and prover changes may only
**accept more** programs, never fewer.

#### Fact engine work required

- `assert` becomes a fact source in the flow engine. Today it feeds invariant
  discharge (`discharge.rs`) but the flow engine that drives overflow/shift
  elision only applies kill rules to it.
- Interval rules for `x & c` (`[0, c]` for constant `c >= 0`) and `x % c`
  (`[0, c-1]` for non-negative `x`, constant `c > 0`), so the existing
  masking idioms are proven.
- Width-typed values (`byte`/`uint(8)`, elements read from `bytes` or a
  packed array) carry their width's range as a fact.
- Loop entry currently drops all facts (`facts.rs`, phase 1). Not required for
  this RFC, but it is the largest practical limit on narrowing.

### 3. Width types (storage, not arithmetic)

```pluto
let frame: [int(8)] = decode(buf)    // packed: 1 byte per element
let port: uint(16) = 8080            // literal checked at compile time
```

A width type is a **storage shape** for an int, not a second arithmetic.
Widths follow what the hardware does natively: `int(8/16/32/64)` signed,
`uint(8/16/32)` unsigned. There is no `uint(64)`: it cannot widen into `int`
(i64), and a 64-bit unsigned domain would fork every arithmetic rule.

- **All arithmetic is `int`.** Reading a width-typed location widens to `int`
  and yields the width's range as a flow fact for free. Writing into one is a
  conversion (`x.to_uint8()` → `uint(8)?`), proof-narrowed exactly as §2. No
  mixed-width arithmetic or promotion rules exist because no width arithmetic
  exists.
- **Literals** check at compile time: `let p: uint(16) = 8080` is fine,
  `70000` is a compile error.
- `byte` becomes `uint(8)` (current name kept); `bytes` stays the packed
  `[uint(8)]`.
- **Arrays pack:** `[int(16)]` stores 2 bytes per element. Today every array
  element occupies 8 bytes regardless of type (`__pluto_array_new` allocates
  `cap * 8`), so a `[byte]` wastes 8× the cache of `bytes`.
- `float(32)` is the same move for floats (widen on read, round on store;
  `float` = `float(64)`). Store semantics are an open question.
- Java's wart (`b1 + b2` is an int that needs a cast to store back) is
  answered by narrowing: when facts prove the sum fits, the store narrows
  silently; when they cannot, bits were about to be lost and the program must
  say which way (`to_uint8()` handling, or `low_byte()`).

Arbitrary-range invariants (0..100) get no type; they are use-site facts via
guards and `assert`, which §2 already handles. The first draft's range types
are recorded under Rejected alternatives.

#### Performance: why storage-only costs nothing

A scalar `i8 + i8` costs exactly what `i64 + i64` costs on a 64-bit core —
per-width scalar arithmetic is semantic surface with no speed payoff. The
wins are **cache density** (packing, above) and **SIMD** (one NEON/SSE2
instruction adds 16 packed bytes), and SIMD is reached through bulk
operations over packed memory, not through scalar types:

- **Now:** bulk `bytes`/array operations in the C runtime (#393 — slice,
  copy, fill, compare, search, and fixed-width codecs like `read_u32_le`).
  `cc -O2` auto-vectorizes these today; zero Cranelift work.
- **Deferred:** compiler-emitted Cranelift SIMD for operations the compiler
  itself generates (bytes/array `==`, marshaling copies). Cranelift supports
  128-bit vectors; punted until profiles justify it.
- **Out of scope:** auto-vectorizing user loops. Cranelift does no
  auto-vectorization, and growing one is an LLVM-sized project.
- SIMD arithmetic never traps (vector adds wrap), so a loop is vectorizable
  only where overflow checks are **elided by proof** (#444/#452) — the trap
  doctrine and SIMD coexist through the existing elision gate.

### 4. Collection facts

#### Element facts live in element types

"Every element of `xs` is in 0..255" is not tracked about the collection — it
is the element type `[uint(8)]`, and the type system already enforces it at
every write through every alias. Width types generalize this: a port list is
exactly `[uint(16)]`, whose elements carry 0..65535 on every read while every
write is a proven or checked conversion. Aliasing is irrelevant because every
reference to the array has the same element type. Ranges that are not width
boundaries (0..100) are not carried by element types; they remain use-site
facts.

One rule is needed: `[uint(8)]` does not coerce to a mutable `[int]`
(otherwise a push of 70000 through the wider view breaks it — Java's
`ArrayStoreException` problem). Converting between them copies.

#### Length facts become ghost int fields

Inside an invariant-bearing class, a collection field's `len()` is modeled as
a ghost int field updated by known deltas:

| Operation | Effect |
|---|---|
| `push` | `len + 1` |
| `pop`, `remove` | `len - 1`, requires `len > 0` |
| `xs[i] = v` | unchanged |
| `clear` | `0` |
| `self.xs = e` | `len(e)` |

This is linear arithmetic — the fragment the invariant prover already
discharges. `invariant self.segments.len() > 0` then works: a `pop()` must be
preceded by a proof that `len > 1`, or be followed by a `push`.

#### Closing the aliasing doors

This is sound only if the collection cannot be mutated behind the class's
back. Today it can, three ways (verified on master 2026-10-02):

```pluto
let arr = [1, 2]
let mut lg = Log { segs: arr }
arr.push(3)            // 1. construction alias        → lg.segs.len() == 3
let s = lg.segs
s.push(4)              // 2. field-read alias          → 4
grow(lg.segs)          // 3. fn grow(xs: [int]) { xs.push(99) }  → 5
```

For collection fields mentioned by an invariant:

1. **Construction:** the argument must be a fresh expression (literal or call
   result), or is copied in.
2. **Read-out:** the field does not escape the class's methods; reads from
   outside yield a copy (or are rejected — open question).
3. **Calls:** passing the field to a non-`mut` parameter is safe **if #395 is
   decided strictly** — i.e. mutating methods (`push`, `pop`, ...) require a
   `mut` binding, making non-`mut` parameters read-only. Then only `mut`
   parameter passing needs to be ruled out. Without strict #395, door 3 needs
   escape analysis.

This RFC therefore recommends deciding #395 strictly.

#### Ordering facts: append-only first

"Base offsets strictly increasing" relates adjacent elements. In general this
needs quantified reasoning, but the real uses are append-only, which reduces to
one more ghost term: track `last()` like `len`, and `push(v)` requires
`v > self.xs.last()`. Index assignment into an ordering-constrained field is
rejected.

#### Relationship to #398

#398 asks for a runtime-checked invariant tier because collection facts are
inexpressible. The settled verification direction is STRICT (no runtime
invariant checks except wire/marshal decode). With element facts in types and
length/last as ghost fields, the common collection invariants become
**provable**, not merely checkable. #398 shrinks to "possibly a checked tier for
arbitrary ordering facts," which can be decided separately.

### 5. Future: the number-type family

Range types are one member of a family of *declared* number types whose
operators the compiler defines once and the prover understands:

```pluto
wrapping(uint(32))    // modular arithmetic for hash/checksum loops; subsumes wrapping_*
saturating(uint(8))   // clamps at the width bounds
fixed<2>              // decimal: int scaled by 100
```

- **Wrapping** types replace the `wrapping_add`/`wrapping_sub`/`wrapping_mul`
  builtins and are closed under arithmetic.
- **Fixed-point** covers most of what BigDecimal is used for (money, rates) as
  integer arithmetic: overflow traps and full provability, no allocation.
  Multiplication changes scale and division needs an explicit rounding mode.
- **Arbitrary precision** (`integer`) needs a heap representation and a runtime
  library — a new base type, not a refinement. It is the one type where codegen
  matches the prover's mathematical-integer model exactly.

This section is direction, not proposal; it exists to check that width types
do not paint the family into a corner.

## Rejected alternatives

### User-defined operator overloading

The restricted, trait-based form (Rust, Swift, Kotlin) is the modern
mainstream and is rarely regretted for math types. It is rejected for Pluto
because Pluto's design makes consequential things visible — `mut`, `!`/`catch`,
`at` — and an overloaded operator is a hidden call that, in Pluto, could:

- **raise** — error inference would need `a + b!` or an exemption from visible
  fallibility;
- **lock, block, or cross a boundary** — an entity method takes its
  per-instance lock and may be placed remotely;
- **redefine `==`** — whose meaning (structural for values, identity for
  entities) underpins the value/entity split and map/set keys;
- **become opaque to the prover**, which reasons about `+` as integer
  arithmetic.

The felt cost (Java `BigDecimal` method chains) is addressed by declared number
types (§5): operators keep fixed, compiler-defined meanings parameterized by
declared properties. **Revisit if** a numeric domain arises that the
number-type family cannot express and method-chain spelling becomes a real
burden in shipped code; even then, prefer built-in support for that domain
over opening overloading to users.

### Range (refinement) types

The first draft of this RFC proposed `type Port = int where 0 <= it <= 65535`.
Rejected as a construct: refinement typing arriving through a side door — a
new declaration form, a `where` grammar under immediate pressure to grow
(congruences, disjunctions, predicates), and a per-type closure story. Width
types keep the two motivations with real weight (generalizing `byte`; packed
storage); the remainder — element facts for arbitrary ranges like 0..100 —
did not justify the construct and stays expressed by guards and `assert` at
use sites.

### Downcasting (`trait as Class`)

Breaks the trait abstraction (callers branch on concrete types; adding an
implementation stops being safe) and requires runtime type information vtables
do not carry. Code that needs the concrete type models it as an enum.

### User-defined conversions

Consistent with the wire doctrine (schema-level only, no user encode hooks):
conversions between built-in types are compiler-defined; conversions involving
user types are ordinary functions.

### Keeping `as` with trap semantics

Considered first: `as` asserts fit and traps; `to_*` methods are the checked
path. Rejected in favor of removal because `as` would then be a second spelling
whose only distinction is the failure mode, and `assert` + proof narrowing
covers the asserting use without it.

### Saturation as defined semantics (#442 option)

Zero cost and matches Rust's `as`, but NaN silently becoming `0` is exactly the
fabricated-value defect #416 set out to eliminate.

## Phasing

1. **Fact engine prerequisites:** `assert` as a flow-fact source; `&` / `%`
   interval rules; facts from `byte`-typed values.
2. **Conversion methods + narrowing:** `to_byte()`, `low_byte()`,
   `float.to_int()` → `int?`, `bool.to_int()`, `byte.to_int()`; proof narrowing
   and the conversion diagnostic. Fixes #442. Migrate `std.json`
   `get_int()` to the checked form.
3. **Remove `as`:** migrate stdlib, examples, tests; deprecation error with a
   fix-it pointing at the replacement method.
4. **#395 strict** (prerequisite for 6).
5. **Width types:** spelling decision, literals, `to_*` conversions,
   widen-on-read facts, `byte` re-expressed as `uint(8)`, packed array layout
   (GC + marshal interaction).
6. **#393:** offset-based bulk `bytes`/array operations and fixed-width
   codecs in the C runtime. Independent of every other phase and the largest
   near-term performance item — can land first.
7. **Collection facts:** ghost `len`/`last`, aliasing rules, invariant
   discharge over them. Resolves most of #398.
8. **Number-type family** (§5): separate RFC.

## Open questions

1. **Read-out of invariant-bearing collection fields** from outside the class:
   copy, or compile error?
2. **`low_byte()` naming** — or `wrapping_to_byte()`, to match the future
   wrapping family?
3. **Spelling** — `int(8)` reads as a parameterized type and invites
   `int(12)` and `fn f<N>(x: int(N))`; if widths are a closed set, plain
   nominal names (`i8`-style) avoid implying a generics dimension that does
   not exist.
4. **Do width types map onto wire-format widths directly?** Decoding would
   validate the width (consistent with existing wire decode validation).
4b. **`float(32)` store semantics** — f64→f32 rounds to nearest; a finite
   value beyond f32 range becomes ±inf. Acceptable (infinity is a float
   value), or a defect?
5. **`std.json` `get_int()`** on a non-integral or out-of-range number: raise a
   JSON error, or return `int?`?
6. **Specifying the provable fragment** for narrowing: one shared definition
   with invariant discharge, or a separate (smaller) one?
7. **Zero-copy views.** #393's operations are offset-based
   (`read_u32_le(buf, off)`, `copy(dst, doff, src, soff, n)`), which covers
   zero-copy *reads* with no new type — under strict #395 a non-`mut` `bytes`
   parameter plus offsets is a compiler-enforced read-only view. Is a
   Go-style slice *type* (a shared mutable view into a backing buffer) ever
   wanted? It would reopen every aliasing door this RFC closes and break
   spawn/channel copy semantics (#429 is that bug today), so the default
   answer is no; a read-only view type is the fallback if profiles show
   copying dominating after #393.

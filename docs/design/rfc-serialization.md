# RFC: Schemas, Encoders, and the Serialization Campaign

**Status:** Draft — awaiting owner review; intended as the sole language workstream once accepted
**Author:** Design discussion, 2026-10-10 through 2026-10-12
**Date:** 2026-10-12
**Supersedes:** [rfc-schema.md](rfc-schema.md) (the February `schema` construct), [rfc-wire-format.md](rfc-wire-format.md) (the type-coded binary format)
**Narrows:** [rfc-migration.md](rfc-migration.md), [rfc-evolution-rules.md](rfc-evolution-rules.md) (kept for the rule catalogue; the snapshot and version model change)
**Related:** [distributed-model.md](distributed-model.md) (logical placement over physical execution — the principle this RFC applies to data), [epistemics.md](epistemics.md) (perception, testimony, closure), rfc-boundaries.md on PR #403 (closure, not trust; the deployment plan as composition certificate), [rfc-properties.md](rfc-properties.md) (claims, not instructions), [ai-native-representation.md](ai-native-representation.md) (stable UUIDs — the identity this RFC rides on), [rfc-number-types.md](rfc-number-types.md) (width types, punted; reopened narrowly here as claims)

## Summary

Pluto gets a serialization story that only a whole-program, deployment-aware,
proof-carrying language can have. It rests on four ideas:

1. **Two record kinds for two worlds.** `class` is the closed-world record:
   every producer is under this compiler, so a value either exists whole or not
   at all. `schema` is the open-world record: its producer is not under this
   compiler, so fields may be absent, defaulted, unknown, or in flight in
   several versions at once, as a permanent condition rather than a deploy
   window. Tolerance lives in `schema` and nowhere else.
2. **Layouts are never written; they are products.** A layout is a shape times
   an encoder. Shapes are classes and schemas. Encoders are format
   interpreters over a fixed, closed data model that know nothing about any
   particular type. The compiler generates the walk between them. One encoder
   per format, never one per type.
3. **The format between Pluto processes is the deployment plan's choice.** The
   program never names it. A fused boundary serializes nothing; a network
   boundary gets the compiler's compact encoding. Only a foreign boundary
   names an encoder, because the world dictates those bytes.
4. **Decode is perception, and identity rides on UUIDs.** The decode site is
   the one runtime validation point: invariants re-checked, required-ness
   enforced, `priv` coverage proven. Evolution across time uses the stable
   field UUIDs the canonical `.pluto` format already mints, so renames and
   reorders are free, field numbers are never hand-assigned, and a rolling
   deploy is a plan that names its predecessor build.

Everything above uses machinery that already exists: the perception surface,
the facts engine, whole-program error inference, the interface hash, the
canonical `.pluto` format, the deterministic harness. This RFC points that
machinery at bytes.

## Findings that motivated this

Surveyed 2026-10-10 across the compiler and the four dogfood systems in the
sibling directories (Styx, a Kafka-class log; Hydra, a CockroachDB-class SQL
store; Kerberos, a SQLite reimplementation; Charon, a Flink-class stream
processor).

- **Three problems wear one name.** (A) Pluto talking to Pluto across `at`,
  `serve`, cross-domain channels, and Pluto-owned durable state. (B) Pluto
  talking to the world: Kafka's protocol, pgwire, SQLite records, external
  JSON. (C) Shape over time: rolling deploys, stored data written by older
  builds. The February RFCs addressed only (C), in isolation, and never shipped.
- **The dogfoods route around the compiler.** All four hand-write every codec.
  The same `codec.pt` has been copied four times and is drifting. Compiler
  marshaling failed in every real multi-module program it met: Styx hit the
  qualified-symbol bug (#499); Hydra got an internal error on
  `at h { begin(rts) }` with a plain value class (`__wire_encode_hlc.Timestamp`
  missing). Both adopted "only bytes and ints cross `at`."
- **Real requirements no RFC mentions.** Byte relay as a design value (payloads
  pass through untouched). Signed sentinels versus unsigned writers (a `-1`
  read as u32 spun a Kafka reader four billion times). Order-preserving key
  encodings. Fields stamped in place outside a CRC. Byte-exact external
  protocols.
- **The February-era projects drowned on the JSON side**: `j.get("x").get_string()`
  boilerplate, hand-written enum↔string helpers, brace-interpolation bugs.
- **What ships today** is the right skeleton with the wrong body:
  `src/marshal.rs` already generates a per-type walker that drives an
  `Encoder` trait — serde's factoring — but it is internal, boundary-only,
  routes through a `WireValue` tree before reaching JSON text, and the RPC
  frame is a second, newline-delimited text format wrapped around it. The
  decoder is strict on field count; the interface hash makes every change
  breaking; there is no evolution model.
- **A premise of rfc-schema.md dissolved.** It was written before the
  value/entity split. Its motivations (value semantics, no identity, structural
  equality, purity) are now what `class` is. What survives is the time
  dimension, relocated here, and the open-world kind, which is the reason
  `schema` is still a kind.

## Doctrine

These are the principles every phase below is checked against.

**Closed world, open world.** The boundary doctrine splits the world by
closure, not trust. A `class` is closed: the compiler sees every constructor
and write. A `schema` is open: the producer is foreign. This distinction is a
world assumption, which is absolute, not a placement, which is plan-relative —
which is why `schema` is a kind and "crosses a boundary" is not.

**Logical shape, physical spelling.** The distributed model separates logical
placement from physical execution. Applied to data: the class or schema is the
logical shape; the encoder supplies the physical spelling; the plan chooses
which encoder each Pluto-to-Pluto boundary uses. One shape, many spellings.
Protobuf's mistake is conflating the two in one file.

**Claims, not instructions.** Per-field attributes are welcome when they state
something true about the data independent of any single encoder, the compiler
can enforce, prove, diff, or plan with them, and they do not name code. `priv`,
`from`, PII, retention, width are claims. `encode_with = fn` is an
instruction and is refused. This is the properties doctrine (judgmental only,
generative never) applied to fields.

**A class may be spelled but never asked to tolerate.** A class decodes
exactly: all fields required, unknown fields rejected, no defaults. The moment
tolerance is wanted, write a schema. No knobs on class decode, ever, because
each knob is a piece of the open world leaking back into the closed one.

**Decode is perception.** Every entry of testimony into typed data goes through
one door, where invariants are re-checked, required-ness is enforced, `priv`
coverage is proven, cycles are refused. There is no post-hoc validation step.
Admission from schema to class is ordinary code; the door already did the
checking.

**Byte relay is first-class.** `bytes` crosses every boundary as one opaque
blob. A decoded value re-emits what it did not read from the original bytes.
Nothing is ever re-encoded that was not changed.

## Design

### The data model

The closed set of shapes. Every class, schema, and enum lowers to these; every
encoder interprets only these. This is the serialization data model, kept in
one place in the compiler (`src/serial/model.rs`, new) and mirrored by the
`Encoder`/`Decoder` traits.

| Shape | Lowered from | Notes |
|---|---|---|
| `unit` | `void` | |
| `bool` | `bool` | |
| `int` | `int`, `byte` | carries range facts (see width claims) |
| `float` | `float` | |
| `string` | `string` | UTF-8 |
| `bytes` | `bytes` | one blob, never element-wise |
| `nullable(T)` | `T?` | |
| `array(T)` | `[T]` | |
| `map(K, V)` | `Map<K, V>` | encoders decide key spelling; no more flat-array hack |
| `set(T)` | `Set<T>` | |
| `record` | `class`, `schema` | ordered fields, each with name + UUID + claims |
| `variant` | `enum` | unit and data variants; index, name, UUID |
| `handle` | `object` | entity identity triple; never the entity's state |

Thirteen shapes. Closures, tasks, channels, traits, streams, ranges, errors are
not shapes and never cross. The existing `wire_transferable` predicate becomes
"lowers to the data model" and moves fully into typeck.

### Encoders and decoders

Two traits in a new `std.encode` module (name open; `std.wire` is retired).

```pluto
pub trait Encoder {
    fn unit(mut self)
    fn bool(mut self, v: bool)
    fn int(mut self, v: int, f: FieldCtx)
    fn float(mut self, v: float)
    fn string(mut self, v: string)
    fn bytes(mut self, v: bytes)
    fn null(mut self)
    fn begin_array(mut self, n: int)      fn end_array(mut self)
    fn begin_map(mut self, n: int)        fn end_map(mut self)
    fn begin_record(mut self, r: RecordCtx)
    fn field(mut self, f: FieldCtx)       // name, uuid, index, claims, range
    fn end_record(mut self)
    fn variant(mut self, v: VariantCtx)   // enum name, variant name, index, arity
    fn end_variant(mut self)
    fn handle(mut self, h: EntityHandle)
    fn unknown(mut self, u: Unknown)      // re-emit a preserved unknown field
}

pub trait Decoder {
    // pull-based; mirrors Encoder; every method is fallible (DecodeError)
    // plus: fn skip(mut self)              — skip the current value without materializing
    //       fn begin_record_tolerant(...) — schema mode: returns a FieldCursor that
    //                                       yields known fields by uuid/name and
    //                                       captures unknowns
}
```

`FieldCtx`, `RecordCtx`, `VariantCtx` are compiler-populated values carrying the
field's name, UUID, index, declared claims, and proven integer range. They are
what lets an encoder make policy from facts (varint for a bounded count,
fixed-width for a `width.u64` claim) without the schema ever naming a spelling.

**Walkers.** For every shape-bearing type `T` the compiler generates

```pluto
fn __encode_T<E: Encoder>(v: T, mut e: E)
fn __decode_T<D: Decoder>(mut d: D) T        // fallible
```

monomorphized per (type, encoder) via the existing generic-function machinery.
No `WireValue` tree; encoders write to `bytes` directly. The cycle guard
(#425) stays in the walker prologue.

**The compiler's own encoder: `PlutoBinary`.** Positional, no per-element type
codes, little-endian fixed ints for full-range values and varints for
range-bounded ones, length-prefixed strings and blobs, variant index as varint.
Every frame carries the **layout hash** of its root type (see Time). Written in
Pluto in the stdlib as an ordinary `Encoder` — the construct dogfooded by its
own transport. The RPC frame (`token\narg\narg`) and its text scalars are
replaced by one `PlutoBinary` envelope: request = (method token, layout hash,
args record); response = (status, payload or error record). Error responses
encode the full error value through the same path, retiring the "exactly one
error type or `__unknown`" limitation.

**JSON.** `std.json` gains `JsonEncoder`/`JsonDecoder` conforming to the
traits, plus the user-facing surface:

```pluto
let s = json.encode(order)                 // any class, schema, or enum
let o = json.decode<Order>(s)!             // class: strict; schema: tolerant
```

The DOM (`Json`) stays for genuinely dynamic data. Enum representation in JSON
(externally tagged, internally tagged with a key, adjacently tagged, untagged)
is a parameter of `JsonEncoder`, not a claim on the enum (decision D4). So is
the key naming convention (snake_case as written, or camelCase): a field's
external name is a property of the format family, not of the field, and no
per-field rename syntax is reserved. A true one-off rename, if one ever shows
up, fits the claims doctrine as `satisfies encode.name(self.f, "externalName")`
without a keyword.

**User encoders.** A foreign format is one `impl Encoder` + one `impl Decoder`,
written once per format. Styx's Kafka protocol: i16 string lengths, `-1` as
null, i32 array counts, big-endian. Charon's element frame: u8 variant index,
uvarint for bounded ints, u64 for `width.u64` claims. Hydra's order-preserving
key encoding: an encoder with `satisfies verify.ordered(...)`. Kerberos's
record format: varint-tagged. Each replaces a hand-written per-type reader and
writer with a per-format interpreter the compiler drives.

**`std.codec`.** The four copies of the dogfoods' `codec.pt` become one stdlib
module with contracts: `put_u16be(mut b: bytes, v: int) requires 0 <= v < 65536`,
`put_i32be` with the signed range, `uvarint`/`svarint`/zigzag, a `Reader` whose
`invariant self.pos <= len(self.buf)` makes a short read a typed `CodecError`.
Encoders are written on top of it. The facts engine discharges the `requires`
at call sites, which turns the Styx `-1` sentinel bug into a compile error.

### Schema: the open-world record

```pluto
schema CreateOrder {
    customer_email: string                      // required
    items: [LineItem]                           // required
    note: string?                               // optional: absent and null both read as none
    currency: string = "USD"                    // defaulted at decode; expression must be pure
    satisfies privacy.pii(self.customer_email)  // a claim the compiler checks (phase 4)
}

schema LineItem {
    sku: string
    quantity: int
    invariant self.quantity > 0                 // checked at the door
}

schema UpdateOrder partial of CreateOrder       // every field tri-state: absent | null | value
```

**Semantics.**

- A schema is a value: structural equality, deep copy on spawn, no identity.
  Methods, generics, invariants, `priv`, trait impls, `satisfies` all allowed.
  The February RFC's purity rule for methods is dropped; purity is required
  only of default and `from` expressions, which run at decode or migration time.
- **Closure.** Fields may be primitives, enums whose payloads are closed,
  containers, schemas, and classes (a class inside a schema is a strict
  sub-shape — decision D1). Entities cross as handles. Anything else is an
  error at the schema declaration.
- **Presence.** Required fields are enforced at decode: a missing required
  field is a typed `DecodeError` naming the field. Inside the program
  `req.customer_email` is a plain `string`. `T?` reads absent and null as
  `none`. Defaulted fields are filled at decode. In `partial` mode every field
  is tri-state and narrows flow-sensitively:

  ```pluto
  if u.note is present { o.note = u.note }     // u.note: string? inside the branch
  ```

  `present` is a new flow-fact kind in `src/typeck/facts.rs`, structurally the
  same as nullable narrowing. A partial schema is also the natural output of
  a partial parse (see Streaming), so one concept serves PATCH bodies and
  incremental decode.
- **Unknown fields.** Captured by the decoder into a compiler-owned hidden
  bag, re-emitted by an encoder of the same family on encode, never visible as
  a user field (so the in-band `__variant`/`__bytes` collision of today's JSON
  mapping cannot recur). The bag is encoder-family-scoped: unknowns captured
  by `JsonDecoder` are re-emitted by `JsonEncoder` and dropped by any other
  encoder (decision D2).
- **Admission** is ordinary code: a function from schema to class. The decode
  already enforced presence, so admission reads plain fields and constructs
  the class under the usual invariant proof. The decode coverage theorem
  (`check_decode_coverage`) extends to schemas: an invariant- or `priv`-bearing
  schema at any decode site has a validator by construction.
- **Spread** (`...Timestamped`) is deferred (decision D8).
- **Conditional fields** (`when`) from the February RFC are dropped: that is a
  data-carrying enum whose JSON spelling flattens the payload, which is an
  encoder parameter.

**Compiler changes.** Lexer keyword `schema`; parser → `SchemaDecl` (fields
with presence mode, default expr, claims, `from` clauses); `PlutoType::Schema`;
typeck registration, closure check, presence narrowing, tolerant decode
lowering; SCHEMA_VERSION bump in `src/binary.rs`; SDK/MCP serialization of the
new declaration; `pluto analyze` lists schemas in the perception surface.

### Classes through encoders

A class is spelled for a foreign reader only through an explicitly chosen
encoder at an explicit site:

```pluto
let s = json.encode(order)                  // Order is a class: exact spelling
let o = json.decode<Order>(s)!              // strict: all fields, no unknowns, no defaults
```

That site is the written commitment. `pluto analyze` lists every class that
reaches a foreign encoder, and a change to such a class produces a diagnostic
naming the site. For Pluto-to-Pluto boundaries nothing is written: classes
cross `at`/`serve` under the plan's encoder, as today, minus the bugs.

### Field claims and width claims

Claims attach to fields via `satisfies` (library-defined) or a bare marker
(compiler-owned, like `priv`). The three-question test for admitting a new
claim: true of the data independent of any encoder; enforceable, provable,
diffable, or plannable by the compiler; names no code.

- **`std.width`** — `satisfies width.u64(self.ts)` etc. Treated by the facts
  engine as an invariant (`0 <= self.ts < 2^64`, overflow-trapped per #443)
  and surfaced to encoders via `FieldCtx.range`. This is how Charon's `value`
  (varint) and `ts` (fixed u64) are told apart without the schema naming a
  spelling and without reopening width *types* (decision D3).
- **`std.privacy.pii`** — a flow claim: the field's value may not reach a sink
  the property names (log sinks, `print`, a foreign encoder not cleared) and
  may not cross a boundary to a domain the plan has not cleared. Discharged by
  a new flow atom in the kernel (phase 4).
- **`std.retention.cascade`** — an obligation on the storage story; parked
  until storage is a construct, listed so the claim form is designed with it
  in mind.

### Streaming, partial decode, projection pushdown

- **Streaming decoders** are values with `mut self` that accept bytes
  incrementally (`feed(chunk)`) and yield either a complete value or a
  `partial` schema with presence facts for what has arrived (decision D5).
- **Projection pushdown.** Whole-program compilation sees every field access
  on a decoded value. For each decode site the compiler computes the read set
  (conservatively: escapes to a function take that function's read set;
  escapes to a boundary or an encoder take everything) and specializes the
  walker to `skip()` unread subtrees. Skipped and unknown content is retained
  as raw bytes so the value can be re-emitted from its source. This is column
  pruning applied to deserialization; it is the capability only
  whole-program compilation can offer and the direct answer to Styx's
  compaction-versus-relay conflict.
- **Views** fall out: a value decoded with a maximal skip set is bytes plus a
  proof they parse, with lazy field access. No separate construct.

### Time: evolution over the plan

- **Identity.** Every field and declaration already has a stable UUID in the
  canonical `.pluto` format. Serialization identity rides on it. Renames,
  reorders, and moves are not events.
- **Layout hash.** Content-addressed over the data-model lowering of a type:
  field UUIDs, shapes, claims, variant UUIDs. Carried in every `PlutoBinary`
  frame and in every durable record written through `std.wal`/`std.blob`. No
  version numbers anywhere; Charon's `sp_version` and Styx's record `magic`
  retire.
- **`from` clauses** on class and schema fields, as in the February RFC:
  `total_cents: int from total: float => (total * 100.0).to_int()` and
  `from => 0` for a new required field. Pure. Inert until a plan supplies a
  predecessor.
- **Snapshot = predecessor `.pluto`.** The plan names the predecessor build(s)
  that may still be live or may have written durable data. The compiler diffs
  on UUIDs (added, removed, type-changed; rename is not a diff) and applies the
  evolution rules from rfc-evolution-rules.md: added-required-needs-`from`,
  type-change-needs-`from`, removal warns, variant removal errors.
- **Compiled upgraders.** The newer build contains decoders for each
  predecessor layout (hash → upgrader chain built from `from` clauses), so it
  reads old frames and old records. The older build reading the newer is
  permitted exactly for declared-compatible pairs (added nullable or defaulted
  fields; added variants where the reader has a wildcard arm); the interface
  hash gains a compatibility class so skew is rejected unless the plan says
  the pair was checked (decision D7).
- **Rolling deploy** = a plan with two builds on one boundary; the compiler
  verifies mutual readability before deploy. Two-phase expand/contract comes
  from the diff, as rfc-migration.md describes.
- **Dependency.** The deployment plan as a compile-time artifact is still the
  open question in distributed-model.md and the boundary RFC's mechanism 3.
  This RFC defines only what it consumes from the plan: (a) which boundaries
  are physical and which encoder each uses, (b) the predecessor builds. Phase 5
  waits for the plan RFC; phases 0–4 do not.

### Foreign boundaries and the interface hash

For `serve`/`at` the hash is unchanged in meaning: signatures + contracts +
layout, now computed over the data model. A served interface that a foreign
client speaks (pgwire, Kafka) is served *through* an encoder; the hash
includes the encoder's identity and the compatibility class. A published
contract for non-Pluto consumers is an emitted artifact (OpenAPI, `.proto`)
derived from schemas — a tool, not a declaration.

## What is removed or superseded

- `stdlib/wire/wire.pt`: `WireValue`, `WireValueEncoder/Decoder`,
  `JsonWireFormat` — replaced by `std.encode` + `std.json` encoders.
- The RPC text frame and `__pluto_wire_opt_wrap`/`__pluto_wire_escape` scalar
  helpers — replaced by the `PlutoBinary` envelope.
- `std.rpc` and the `__pluto_http_post` stub returning a constant; the legacy
  stage-to-stage `lower_rpc_call` path (decision D6).
- `src/marshal.rs` as a WireValue generator — rewritten as the walker
  generator over the data model (phase 1 is largely a rewrite of this file).
- rfc-wire-format.md's type-coded binary format (never implemented).
- rfc-schema.md's `when` clauses, purity rule, and "schemas are the only types
  that cross" rule.

## Phasing

Sizes are relative (S/M/L/XL) in this repo's agent-driven cadence, not
calendar estimates. Every phase ships with tests, an example, and book text,
and is independently mergeable. Each names the dogfood that proves it.

### Phase 0 — Table stakes (no design decisions) · M

- Move boundary-type checking from codegen into typeck: `wire_transferable`
  (renamed to data-model lowering) runs at `at`/`serve`/`remote` sites with a
  diagnostic naming both the site and the offending declaration. Retire
  `validate_serializable_types`' stage-only scope.
- Fix #499 (qualified symbol names in generated serve/encode functions);
  Hydra's `__wire_encode_hlc.Timestamp` ICE; multi-param generic marshaling;
  the `$$`/`__` mangling bug for generics nested in fields.
- `std.codec`: one shared module with contracts and a typed `Reader`; the four
  dogfood copies import it.
- Delete the `__pluto_http_post` stub and `std.rpc` (D6).
- **Acceptance:** Hydra's `at h { begin(rts) }` compiles and round-trips;
  Styx's `serve` of `MetadataService` compiles; `codec.pt` is deleted from all
  four dogfoods.

### Phase 1 — Data model, encoders, PlutoBinary, JSON · XL

- `src/serial/model.rs`; `std.encode` traits; walker generation replacing
  `marshal.rs`'s WireValue path; cycle guard retained; `FieldCtx` carrying
  name/UUID/index/claims/range.
- `PlutoBinary` in the stdlib; RPC envelope and error responses rebuilt on it;
  the two encodings collapse into one; every `distributed.rs` test passes over
  binary.
- `JsonEncoder`/`JsonDecoder`; `json.encode`/`json.decode<T>` for classes
  (strict) and enums; JSON enum representation as an encoder parameter (D4).
- Delete `std.wire`; port `wire.rs` tests (including the four ignored ones).
- **Acceptance:** Charon's `elemwire.pt` becomes `enum Elem` + one
  `TaggedBinary` encoder (format may change in this phase; byte-identity is
  phase 4's job). Kerberos's `record.pt` becomes one encoder. A JSON
  round-trip of a nested class/enum with no DOM code.

### Phase 2 — Schema · XL

- Lexer/parser/AST/binary (SCHEMA_VERSION bump); `PlutoType::Schema`; closure
  check; presence modes; defaults; `partial of`; presence facts
  and `is present` narrowing; tolerant decode lowering; unknown bag;
  decode-coverage extension; SDK/MCP; `pluto analyze` perception surface.
- `examples/schema/` (an HTTP API with `CreateOrder` → `Order` admission and a
  PATCH endpoint); book chapter "Schemas: the open world."
- **Acceptance:** the February pain cases rewritten (Meridian's snapshot codec,
  Nexus's manifest) with zero DOM calls; Styx's Kafka request/response types as
  schemas driven by a `KafkaWire` encoder, and `kafkawire.pt` deleted.

### Phase 3 — Streaming and projection pushdown · L

- Incremental `Decoder` feeding; partial-schema output; `skip()` on every
  decoder; read-set analysis per decode site; pruned walkers; raw retention
  and re-emit of skipped/unknown content.
- **Acceptance:** Styx compaction reads record keys through a pruned decode
  and relays batches byte-identically; a benchmark showing skip cost ≪ parse
  cost on a wide record.

### Phase 4 — Claims · L

- Field-level `satisfies` and the claim test enforced at declaration;
  `std.width` range claims wired into the facts engine and `FieldCtx.range`;
  `std.privacy.pii` as a kernel flow atom (sink reachability; plan clearance
  deferred to phase 5); the diagnostic for changing a foreign-spelled class.
- **Acceptance:** Charon's element frame byte-identical to its current format
  (`value` varint, `ts` fixed u64) via width claims; a program that logs a PII
  field fails to compile with both sites named.

### Phase 5 — Time · XL, gated on the plan RFC

- Layout hash in frames and durable records; `from` clauses; predecessor
  `.pluto` as snapshot; UUID diff; evolution rules; compiled upgraders;
  compatibility classes in the interface hash (D7); rolling-deploy
  verification.
- **Acceptance:** a two-build test in `distributed.rs` with a renamed field, an
  added nullable field, and a type change under `from`, in both directions;
  Charon savepoints drop `sp_version`; `std.wal` records carry layout hashes
  and a replay across a field rename succeeds.

### Follow-ons, out of scope here

- Storage as a construct (rfc-storage.md) and SQL migration generation.
- The Postgres library (boundary-doctrine acceptance) — it will be written as
  schemas plus a `PgWire` encoder, which is its own acceptance test for this
  RFC.
- Emitting OpenAPI / `.proto` from schemas.
- Plan clearances for PII and residency.

## Owner decisions

1. **D1 — Schema closure.** May a schema field be a class? *Recommend yes*: a
   class inside a schema is a strict sub-shape; the open world stops at the
   schema's own fields.
2. **D2 — Unknown-field bag.** Encoder-family-scoped raw preservation
   (*recommend*) vs a dynamic DOM value (`Json`) that any encoder could read.
3. **D3 — Same-type-different-spelling.** `std.width` range *claims* via
   `satisfies`, consumed by encoders as facts (*recommend*) vs width *types*
   (reopens rfc-number-types) vs per-field configuration inside the encoder
   (breaks "encoders know no types").
4. **D4 — Enum representation in JSON.** Encoder parameter (*recommend*) vs a
   claim on the enum.
5. **D5 — Decoder statefulness.** Values with `mut self` (*recommend*) vs
   objects. Objects would serialize access and allow sharing a streaming
   decoder across tasks; values keep the construct simple and let the copy
   rule apply.
6. **D6 — Legacy stage RPC.** Delete `lower_rpc_call`, `std.rpc`, and the HTTP
   stub in phase 0 (*recommend*); stages themselves stay as lifecycle shells.
7. **D7 — Skew policy.** Strict reject unless the plan has verified the pair
   (*recommend*) vs always-tolerant additive reads. The strict form keeps the
   "every surviving claim has a warrant" discipline.
8. **D8 — Spread composition.** Defer (*recommend*); revisit when a dogfood
   API has three schemas sharing five fields.
9. **D9 — Sequencing against the plan RFC.** Phases 0–4 proceed now; phase 5
   waits (*recommend*) vs designing the plan artifact inside this campaign.

## Risks

- **The class/schema split as a tax.** A trivial service whose request shape
  equals its domain type declares both plus an admission. Mitigation: strict
  class decode is one call; schema is reached for only when tolerance is
  actually needed. If dogfoods start loosening classes instead of writing
  schemas, the ergonomics have failed and must be revisited.
- **Encoder generality.** Some foreign formats need context an encoder cannot
  get from shapes plus claims (Kafka's per-API-version field sets, pgwire's
  message-type-dependent framing). Mitigation: encoders are ordinary Pluto
  with state; a protocol with per-message shapes is many schemas, one encoder.
  Phase 2's Styx acceptance is the test; if `kafkawire.pt` cannot be deleted,
  the factoring is wrong and comes back here.
- **Projection analysis precision.** Conservative read sets may prune nothing
  in escape-heavy code. Acceptable: pruning is an optimization; correctness
  does not depend on it.
- **Phase 5 blocked on the plan.** Explicit by design; `from` clauses are
  inert until then.
- **Scope.** This is larger than any in-flight RFC. It is phased to be
  mergeable per phase with dogfood acceptance at each step so that stopping
  after any phase leaves the language better than before it.

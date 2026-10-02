# RFC: Entity Lifecycle — the Release Protocol

**Status:** Accepted (2026-10-02); resolves rfc-objects.md open question 4 (the registry half)
**Author:** Design discussion
**Date:** 2026-10-01
**Related:** [rfc-objects.md](rfc-objects.md) (the registry this governs), [epistemics.md](epistemics.md) (warrants and leases — the lens), [rfc-verification.md](rfc-verification.md) (the lease-window doctrine, degradation), [rfc-distributed-safety.md](rfc-distributed-safety.md) (failure classification), [rfc-typestates.md](rfc-typestates.md) (must-release linearity), [distributed-model.md](distributed-model.md)

## The problem

Exporting an entity pins it as a GC root forever. Any long-running process that
hands out handles — which is what serving entities *is* — leaks every entity it
ever exported, plus everything each one transitively retains. rfc-objects.md
phase 2 shipped the registry with "a release protocol is open question 4"; this
RFC is that protocol.

The hard part is not freeing memory; it is deciding **when the home process may
honestly un-pin**. A handle can be forwarded by a third party, stored in a
field, re-exported in a response — the home cannot know who holds references.
Clients crash without saying goodbye. Partitions make every "are you still
there?" unanswerable for unbounded stretches. Whatever the protocol is, it must
say what a surviving handle *means* in each of those worlds.

Sections marked **(proposal)** are recommendations to accept, amend, or reject;
"Decisions for the owner" and "Open questions" are genuinely unsettled.

## Current-state audit (2026-10-01)

### What pins, exactly

The export path is one function, `__pluto_entity_export`
(`runtime/builtins.c`): first export of an entity appends its pointer to a
process-global `entity_registry` array, calls `__pluto_gc_add_pending_root(ptr)`,
and mints `id = index + 1`. Nothing ever removes either the registry slot or
the root. Export happens whenever codegen wire-encodes an object-typed value
(`src/codegen/lower/mod.rs`, `encode_wire_value`: object types route to
`__pluto_entity_encode`) — i.e. every time an entity crosses a boundary as an
argument or return value.

The pin mechanism is the GC's *pending-roots* list (`runtime/gc/marksweep.c`),
which already supports removal — `__pluto_gc_remove_pending_root` exists and is
used by the spawn path (a task handle is pinned between `spawn` and the new
thread registering its stack, then unpinned). Entity pins share this list but
are simply never removed. So the un-pinning primitive the protocol needs
already exists; what's missing is the decision procedure for calling it.

Client side, nothing pins: a foreign handle materializes as a 3-slot
`GC_TAG_HANDLE` stub (`[home_str][type_str][id]`) that is an ordinary GC object,
collected like any value. The leak is strictly home-side.

### The leak, quantified

A serve loop that returns a fresh entity per request (a session, a cursor, a
per-order workflow object) accumulates, **per request, forever**:

- one registry slot + one pending-root slot (16 bytes of bookkeeping),
- the entity allocation itself (header + fields + the hidden per-instance
  rwlock slot + the rwlock allocation), and
- the entity's entire transitively reachable graph — every value a pinned
  session holds stays live.

There is a second-order cost: export dedup is a linear scan of the registry, so
the Nth export does O(N) work — a server that has exported a million entities
pays a million pointer compares on every subsequent export, under the registry
mutex. The protocol's registry restructuring fixes this incidentally.

Note what does *not* leak: entities that never cross a boundary (normal GC),
and foreign stubs held by clients (normal GC). Only exported identity leaks.

### Related machinery already in place

- **Connection lifecycle:** `serve` runs one detached, GC-registered thread per
  connection (rfc-objects.md "Thread-per-connection serving"); each boundary
  call dials, sends, reads once, and closes — there is no connection state to
  hang a lifetime on, and no connection reuse. Blocking waits are GC-safe-region
  bracketed.
- **Pre-dispatch rejection is already a definite failure.** The entity dispatch
  arm answers `ERR\n__rejected\nunknown entity at its home domain` when an id
  does not resolve, and the client lowers that to `NetworkError` with
  `definite = true` (`emit_transport_response`). The failure-classification
  table in rfc-distributed-safety.md already lists "unknown entity" as
  Definite. **A stale handle failing definite-at-use is the behavior today**
  for an unknown id — a release protocol slots into existing machinery rather
  than inventing a new failure surface.
- **Forwarding is home-invisible.** `__pluto_entity_encode` on a handle stub
  re-emits the original triple without contacting the home. Third parties
  propagate references silently — by design (a forward is not an effect), and
  any protocol that requires forwards to be home-visible changes that.

### Audit surprises (pre-existing, protocol-relevant)

1. **Test mode doesn't pin at all.** Under `PLUTO_TEST_MODE`,
   `__pluto_gc_add_pending_root` is a no-op and the pending-root scan is
   compiled out. An exported entity whose local references die can be swept
   while the registry still points at it — `__pluto_entity_decode` would
   return a dangling pointer. (Largely latent because the deterministic suite
   doesn't exercise cross-process traffic, but it means the leak this RFC
   fixes *cannot even be reproduced* under `pluto test`, and the registry is
   unsound there today.)
2. **Fork drops entity pins.** `__pluto_gc_after_fork` zeroes the pending-root
   list in the child (correct for in-flight spawn pins, whose threads don't
   exist in the child; wrong for entity pins, which should survive). A forked
   child's registry can dangle after its first collection.
3. **Home restart silently aliases entities.** A serving process's home token
   is its dialable address (`127.0.0.1:<port>` or `PLUTO_SELF_ADDR`) — which is
   *stable across restarts*, while ids restart from 1. A handle minted by the
   previous incarnation resolves in the new one: in-range ids resolve to a
   **different entity** (wrong-object dispatch, no error), out-of-range ids to
   the unknown-entity rejection. Only the non-serving fallback token
   (`H<pid>-<time>`) is incarnation-unique. Any lifecycle story must add an
   incarnation component to the home token, independent of which release
   option wins.
4. **Reason strings conflate "never existed" with everything else.** The
   registry cannot currently distinguish "expired/released" from "never
   minted" — relevant to diagnostics once entries can disappear.

## What a surviving handle means

The epistemic framing (epistemics.md) decides this RFC. A handle is
**testimony** — "entity (home, type, id) existed when this was minted" — level
5 on the warrant gradient: stale the moment it arrives. The current registry
design makes the *home* treat that testimony as a permanent fact: "someone,
somewhere, may still hold a handle" pins the entity forever. That is the exact
collapse of belief into fact the epistemics document forbids — performed by the
language itself, on the home side. Permanence was always a lie: the client that
received the handle may have crashed years ago; the home just has no vocabulary
to stop believing.

The question "does anyone still hold a reference?" is, under partition,
formally unanswerable — it is a global-reachability fact across lossy channels,
and common knowledge of it is unattainable (epistemics.md, formal grounding).
The options below divide by how they cope with that unanswerability: pretend a
client will answer (a), pretend the home can count (b), replace the unknowable
global fact with a renewable local promise (c), or refuse the question (e).

## The options

Each option is analyzed against the four failure modes that matter: network
partition, client crash, forwarded/stored handles, and home crash/restart —
and for each, what a surviving handle means afterward.

### (a) Explicit release

A `release(handle)` operation (or `close` on the object): the client tells the
home it is done; the home un-pins when told.

- **Client crash:** no release is ever sent. The leak is back, unbounded — the
  protocol's core failure mode is the common case it exists for. Any explicit
  scheme needs a timeout backstop, at which point the backstop is the protocol.
- **Partition:** the release can't reach the home. The release message is
  idempotent so ambiguous-failure retry is legal — but the client must retry
  until heal, holding bookkeeping for an entity it wanted to forget.
- **Forwarded handles:** fatal. If A forwards to B and then releases, B's
  handle dies under it — use-after-free at a distance. Correctness requires
  counting releases against forwards, which is option (b).
- **Home restart:** releases targeting the old incarnation are meaningless;
  harmless given an incarnation token.
- **Linearity?** Making handles linear must-release values (riding
  rfc-typestates.md machinery) would guarantee release-on-every-path — but
  linearity forbids exactly what handles exist for: storage in fields,
  spawn capture, forwarding, duplication. must-release was designed for
  *evidence* values (grants, leases); a handle is a *reference*, duplicable by
  nature. The marking would reject the identity round-trip demo itself
  (store-and-forward). Rejected as a mandatory discipline; see (d) for
  release as a voluntary optimization.

**Verdict:** unsound alone; the crash case alone disqualifies it.

### (b) Distributed reference counting

Every handle duplication/forward sends `dup` to the home; every client-side
drop (GC of the stub) sends `drop`; home un-pins at zero.

This is the historically fragile design, and the fragility is structural, not
an implementation detail — each failure mode converts into either a permanent
leak or a premature free:

- **Lost `drop`** (client crash, partition): count never reaches zero —
  permanent leak. So crashed clients need a liveness timeout anyway — the
  scheme grows a lease inside it.
- **Lost or late `dup`** — the in-flight-forward race: A forwards to B and
  drops; A's `drop` arrives before B's `dup` (or before B even receives the
  handle); count hits zero; the home frees; B's handle dangles. Correctly
  ordering a three-party handoff over lossy channels requires the forwarder to
  pre-register the transfer with the home and the home to hold the count until
  acked — turning every forward into a home-visible, blocking effect. Today
  forwarding is a local string copy; refcounting makes it a distributed
  transaction.
- **Ambiguous `drop` retried:** decrement is not idempotent; a retry after an
  ambiguous failure double-decrements — premature free. Making decrements
  idempotent requires per-holder identity and per-holder state at the home —
  the home must track *who* holds, which is exactly what the problem statement
  says it cannot know.
- **Home restart:** counts are gone; same as every option (handles fail
  definite with an incarnation token).

Prior art agrees: Java RMI's Distributed GC — the canonical deployed
implementation of this design — is refcounting *plus leases*, because pure
counting cannot survive lost drops; the lease is the component doing the real
work. (DCOM pinging, Cap'n Proto's level-3 handoff complexity, and the E
language literature tell the same story.)

**Verdict:** rejected. Under partition it must choose between leaking and
corrupting, and its fix for each failure converges on leases.

### (c) Lease-based retention — (proposal)

An exported identity carries a **validity lease** at the home. The client
*runtime* renews leases for every foreign stub that is still GC-reachable in
its process; the home un-pins an entity whose lease has been unrenewed for
expiry + grace. No user code participates.

Mechanics (sketch; details in the implementation section):

- `__pluto_entity_export` stamps the registry entry with a deadline measured
  on the **home's own monotonic clock**. Any touch — re-export, inbound
  dispatch to that id, explicit renewal — re-arms it. Calls are implicit
  renewals: an entity that is actually being used never expires.
- Each client runtime tracks its live foreign stubs (per home) and
  periodically sends one batched renewal message per home listing the ids it
  still holds. Stub reachability is already GC-visible: stubs have their own
  tag, so the sweep can unregister dead ones — runtime-internal bookkeeping,
  not a user-facing finalizer.
- After expiry + grace with no renewal and no use, the home removes the
  registry entry and calls the existing `__pluto_gc_remove_pending_root`. The
  entity itself may still be live at home through ordinary references — only
  the *export pin* is released; identity-at-home is untouched.
- A call through a stale handle hits the existing unknown-entity path:
  `ERR __rejected` → `NetworkError` with `definite = true`. The home *knows*
  it released — the authority converts what would otherwise be ambiguity into
  a definite, typed failure, exactly the failure-classification doctrine.

Failure analysis:

- **Partition:** renewals stop; the entity is released after TTL + grace. The
  leak is **bounded by the window**, mechanically, with no cooperation from
  the dead or unreachable party. On heal, the client's calls fail definite and
  it re-acquires through whatever front door minted the handle originally.
- **Client crash:** indistinguishable from partition (correctly so — the home
  cannot tell and does not need to). Bounded leak, then reclamation.
- **Forwarded handles:** solved by *decentralizing renewal*: whoever currently
  holds a stub renews it. The home never needs to know who holds — it only
  needs *someone* to keep promising. A forward hands the renewal duty along
  with the triple, implicitly, because the recipient's runtime starts renewing
  on decode. No home-visible forward, no three-party race.
- **Stored-durable handles** (a handle written to a database, outliving every
  process): it expires. This is the honest answer — a durable identity
  reference crossing process *lifetimes* was never something the registry
  could promise; that is the deployment-plan/domain-identity problem
  (rfc-objects.md open question 5, distributed-model.md), not the release
  protocol's. The lease makes the limit explicit instead of silently
  pretending.
- **Home crash/restart:** all leases die with the registry, which is correct —
  the entities are gone. With the incarnation token (audit item 3), surviving
  handles fail definite instead of aliasing new entities.

### (d) Hybrid: lease default + explicit early release — (proposal, recommended)

Option (c) as the safety net, plus a voluntary `release(handle)` that drops
*this holder's* renewal registration immediately (and sends a best-effort,
idempotent hint to the home, which may shorten the wait if no one else renews
or uses the entity). Release is an optimization, never a correctness
obligation: forgetting it costs at most one lease window; calling it on a
forwarded handle harms no one (the other holders' renewals keep the entity
alive — the hint is "I stopped vouching," not "destroy").

Failure analysis is (c)'s: the explicit path only ever *shortens* retention,
and every failure of the explicit path degrades to the lease backstop.

### (e) Do-nothing-with-bounds: cap + LRU (strawman baseline)

Keep permanent pins but cap the registry (N entries, LRU eviction on
overflow).

- Evicts by registry pressure, not by holder liveness: under load it evicts
  entities whose clients are alive and active — definite failures for working
  callers, triggered by *other* traffic. A surviving handle means "valid until
  the home gets busy," which is not a meaning a program can act on.
- The cap is unknowable per deployment; LRU touch-tracking adds a write to
  every dispatch; idle-but-held entities (a dashboard's session) are exactly
  what LRU evicts first.
- Its one virtue — bounded memory with zero protocol — is real, and survives
  as an optional backstop *on top of* leases (a hard cap against renewal
  storms / hostile clients). As *the* protocol: rejected.

## Recommendation — (proposal)

**Adopt (d): lease-based retention with explicit release as a voluntary
accelerator.** The epistemics argument, written out:

1. **The pin was an unwarranted belief.** The home retains because "someone
   may hold a handle" — level-5 testimony about remote reachability, held
   forever. The warrant gradient is explicit about the only honest upgrade
   available for a remote fact: **a promise with a window** (level 4). The
   lease is the home's own doctrine applied to itself: "I will keep this
   identity resolvable until T, and I extend T while anyone keeps asking."
   Permanence was never available; the registry was pretending otherwise.
2. **Global reachability is unknowable; renewable promises are local.**
   "No one holds a reference" is a negative global fact over lossy channels —
   unattainable common knowledge. Refcounting pretends to compute it and
   corrupts or leaks when the pretense fails. The lease *decomposes* the
   unknowable question into unilateral warrants (each holder renews for
   itself) plus an authority-side check at the point of effect (registry
   resolution at dispatch) — exactly the decomposition rule the formal
   grounding section derives from the common-knowledge impossibility.
3. **Expiry fails definite, on the existing surface.** When the promise
   lapses, the next use is answered by the authority itself: unknown entity,
   `__rejected`, `NetworkError.definite = true`. No new failure vocabulary,
   no ambiguity laundering — the home has the warrant for definiteness (it
   released), and says so.
4. **The lease-window doctrine transfers, inverted and strengthened.**
   rfc-verification.md: the window is liveness, never safety; safety is judged
   at the point of effect by the authority. Here the "effect" is registry
   resolution, and the authority is the home: renewal cadence is pure
   liveness (a missed renewal can cause a premature *definite failure*, never
   a wrong dispatch), and expiry + grace is the safety margin. Crucially, the
   home measures elapsed-time-since-last-renewal on its **own monotonic
   clock** — no cross-host clock comparison exists anywhere in the protocol,
   so the classic bounded-clock-skew assumption is never needed. The only
   clock-adjacent assumption is the client-side one every lease system has: a
   client paused longer than TTL + grace (GC pause, VM freeze) loses its
   leases — and discovers it as a definite error, re-acquiring through the
   front door. Renewal is liveness; expiry + grace is the safety margin;
   declare the window, never default-trust it.

### Costs, honestly

- **Renewal traffic.** One dial per (client process, home) per renewal period
  — the current transport has no connection reuse, so each renewal batch is a
  fresh TCP connect (cheap at the default cadence; connection reuse is an
  independent transport improvement that would amortize it). Payload is
  O(held stubs) ids per home. A client holding 10k stubs to one home at a
  60s TTL / 20s cadence sends ~3 small frames a minute. Pathology to watch:
  N clients × M homes full mesh — still linear in actual reference edges.
- **A background thread per client process** (GC-registered, safe-region
  bracketed, like serve/spawn threads) — the first standing runtime thread
  not tied to user code. In `PLUTO_TEST_MODE` there are no pthreads, so the
  renewal loop must compile out (see implementation).
- **Typestate / must-release values referencing expired entities.** No new
  rule needed, and this is worth stating because it *looks* like a conflict:
  evidence values (grants, leases in the rfc-verification sense) reference
  their authority; if the authority's *export pin* expires, calls to it fail
  definite — which is just degradation, already in every effectful method's
  error set for degradable states. The must-release obligation is unaffected:
  "release" on evidence cleans up the *local belief* and must stay legal from
  degraded states — "an obligation the world cannot revoke"
  (rfc-verification.md). An expired handle inside a must-release value makes
  the release's remote half fail definite; the local discharge proceeds.
  Handles themselves are never linear (see option (a)).
- **Wire format.** v1 adds *nothing per-handle* to the wire: the triple stays
  `(home, type, id)`, the lease lives only in the home's table, and the
  renewal cadence is a client-runtime default. The home token grows an
  incarnation component (opaque to clients — format-compatible). The renewal
  message is one new transport-level token. If a later phase puts per-object
  TTLs on the wire (so clients renew at the right cadence per type), that is
  a handle-format extension — flagged now, deferred.
- **Interface hashing.** The renewal exchange is transport-level (like
  `__rejected`), keyed by a protocol tag, not by any object's interface hash —
  lease parameters are not part of a type's callable surface. If a per-object
  lease *declaration* is ever added, whether it hashes joins the existing
  phase-4 finding (no contract clause hashes yet; they enter the evolution
  surface together — rfc-properties.md "Evolution and honesty").
- **Deployment-plan future.** When the deployment plan replaces home tokens
  with routable domain identity, the lease table becomes per-domain state and
  the incarnation token folds into domain epoch. Colocated plans need no
  leases at all — a local entity reference is real GC reachability — so the
  machinery engages exactly when the physical plan crosses a process
  boundary, preserving plan symmetry (`at` means the same thing in both
  plans; only the retention mechanics differ).

## Decisions for the owner

Enumerated; each independent. Recommendations inline.

1. **The protocol itself:** hybrid (d) — leases with voluntary release — vs
   pure leases (c). *(Recommended: accept (d), with the explicit-release
   builtin allowed to land in a later phase; (a), (b), (e) rejected per the
   analysis above.)*
2. **Window ownership and defaults.** Who sets TTL / renewal cadence / grace?
   Options: a language-level constant; a runtime environment default
   (`PLUTO_ENTITY_LEASE_MS`, with cadence derived as TTL/3 and grace = 1
   TTL); a per-object declaration (`object Session lease 30s` — new syntax,
   hashing question, wire question). *(Recommended: runtime env default with
   generous values — e.g. TTL 10min — deferring per-object syntax until a
   concrete need; the declaration question then lands with the
   properties-evolution work.)*
3. **The stale-handle error surface.** Keep `NetworkError.definite = true`
   with a distinguishing reason string ("entity lease expired at its home" vs
   "unknown entity"), or introduce a dedicated typed error
   (`EntityReleased`)? A typed error is more honest but joins the inferred
   error set of every entity `at`-call, churning existing handling.
   *(Recommended: reason-string distinction in v1; revisit a typed error when
   degradation-error integration for entities is designed as a whole.)*
4. **Expiry observability: at use only, or also proactive?** Doctrine says
   degradation surfaces as typed errors **on use** (rfc-verification.md), and
   this RFC follows it: a holder learns its handle lapsed when a call fails
   definite. A proactive push — the home jolting holders when an entity is
   about to be (or was) released — was considered in design discussion and
   parked; it remains parked. Noting the connection only: lease expiry would
   be that mechanism's natural trigger if it is ever unparked, and nothing
   here forecloses it. *(Recommended: at-use only.)*
5. **Does the entity learn it was released?** A home-side hook ("last export
   pin dropped") is finalizer territory: non-deterministic timing, user code
   on a GC-adjacent path — the shape of runtime-`ensures`, which contracts.md
   rejected and the verification RFC kept rejected. The settled factoring
   (rfc-objects.md OQ4, 2026-09-28 partial resolution) already says cleanup
   obligations attach to *evidence values* via must-release linearity, not to
   entities. The tension is real — an object fronting external state (a vault
   handle, a file) "wants" to close on release — but the answer stays: the
   external resource's cleanup rides a must-release evidence value or an
   authority's explicit protocol, never an entity finalizer. *(Recommended:
   no hook; record the rejection in this RFC so it isn't re-litigated per
   feature.)*
6. **Migration default.** Existing programs assume permanent pins. Ship leases
   ON by default with a generous TTL (active entities never notice —
   calls renew), with `PLUTO_ENTITY_LEASE_MS=0` as the infinite-lease escape
   hatch (current behavior)? Or OFF by default (opt-in)? *(Recommended: on by
   default — the current behavior is a leak, not a contract anyone chose; the
   escape hatch preserves it for whoever disagrees.)*
7. **Implicit renewal by use.** Does any inbound dispatch to an id re-arm its
   lease (so actively-used entities never expire even if the renewal thread
   is partitioned away)? *(Recommended: yes — use is the strongest renewal;
   it also makes generous-TTL migration safe.)*
8. **The audit fixes.** Incarnation token in the home string, test-mode pin
   soundness, and fork behavior are pre-existing issues independent of the
   option chosen. *(Recommended: fold into phase 1 regardless.)*

## Implementation sketch — (proposal)

### Phase 1 — registry restructure + incarnation (no behavior change)

- Replace the append-only array with an id-keyed table (open-addressed map or
  slot array with a free list) plus a separate monotone `next_id` — ids are
  never reused within an incarnation, so a removed entry's id resolves to
  "released," never to a recycled entity. Keeps O(1) export dedup via a
  pointer→id side map (also fixes the O(N) export scan).
- Home token grows an incarnation component: `127.0.0.1:8080` becomes
  e.g. `127.0.0.1:8080@<boot-nonce>` (opaque to clients; `entity_decode`
  string-compares the whole token, so old-incarnation handles become foreign
  → routed → answered by the new process with a definite rejection rather
  than silently aliasing). `__pluto_entity_request` must strip the
  `@<nonce>` suffix when dialing.
- Distinguish rejection reasons: id < `next_id` and absent → "entity was
  released at its home"; otherwise "unknown entity."
- Test-mode soundness: under `PLUTO_TEST_MODE`, scan `entity_registry`
  entries as GC roots (the pending-root list stays a no-op there); fork: stop
  zeroing entity pins in `__pluto_gc_after_fork` (keep zeroing spawn pins —
  they are the ones whose threads died; distinguishable once entity pins move
  to their own list or the registry scan replaces pending-root pinning
  entirely, which is the cleaner end-state: *the registry itself is the
  root set*, and un-pinning is just table removal).

### Phase 2 — home-side expiry

- Registry entries gain `deadline` (CLOCK_MONOTONIC millis). Export,
  re-export, and entity dispatch re-arm it (decision 7). TTL from
  `PLUTO_ENTITY_LEASE_MS` (0 = infinite).
- Expiry sweep: piggyback on registry touches (export/dispatch/renewal walk a
  lazy cursor over a few entries) rather than a dedicated reaper thread —
  no new thread, no `PLUTO_TEST_MODE` hazard, bounded incremental cost.
  Removal = table delete + `__pluto_gc_remove_pending_root` (or nothing, if
  phase 1 made the registry the root set).
- With phase 2 alone (no renewal protocol yet), leases are only honest for
  *actively used* entities — hence the generous default TTL and the
  migration escape hatch must ship in this phase, not phase 3.

### Phase 3 — client auto-renewal

- `__pluto_entity_decode` registers each materialized stub in a per-process
  renewal table keyed by home token. The mark-sweep sweep already dispatches
  on type tags; sweeping a `GC_TAG_HANDLE` additionally removes it from the
  renewal table (runtime-internal, not a user finalizer).
- A renewal thread (production mode only; `#ifdef PLUTO_TEST_MODE` → absent —
  the fiber scheduler has no pthreads, and the deterministic suite has no
  cross-process boundaries to renew) wakes each cadence, batches per home,
  and sends a transport-level frame: token `@__renew#<proto-version>`,
  payload = newline-joined ids. GC-safe-region bracketed like all blocking
  runtime waits; honors `__pluto_gc_prepare_fork` via the existing patterns.
- Serve dispatch (`src/codegen/lower/mod.rs`, the entity-chain emitter)
  gains one arm before the method chains: `@__renew#…` → re-arm each listed
  id, reply `OK`. Transport-level like `__rejected`, outside any interface
  hash. New runtime decls in `src/codegen/runtime.rs`.
- Client runtime declarations: `__pluto_handle_track(stub)` /
  sweep-side untrack; `__pluto_renew_home(home_str, ids_payload)`.

### Phase 4 — explicit release (the (d) accelerator) + per-object windows

- `release(handle)` builtin (or method-position syntax — owner's call):
  unregisters local renewal immediately; sends best-effort idempotent
  `@__release` hint. Deferred until the lease core has soaked.
- Per-object lease declarations, if decision 2 ever wants them: syntax,
  hashing, and handle-format extension (TTL on the wire) land together.

### Test strategy

- **Unit-ish integration (prod mode):** the existing socket-based distributed
  tests (`tests/integration/distributed.rs`, `objects.rs`) extended with tiny
  `PLUTO_ENTITY_LEASE_MS` values and real sleeps: export, wait past
  TTL + grace, call → assert `NetworkError` with `definite == true` and the
  released-reason message; export, keep calling across the window → assert
  survival (implicit renewal); two-hop forward, originator exits, holder
  keeps renewing → assert survival (phase 3).
- **Determinism:** expiry is wall-clock at the home, so these are
  timing tests by nature; keep windows wide (e.g. TTL 200ms, assert at 2×)
  per the existing socket-test practices. A `PLUTO_ENTITY_LEASE_SWEEP=force`
  style env hook (sweep everything expired on next touch) makes the
  home-side decision deterministic even when the clock isn't.
- **`PLUTO_TEST_MODE` discipline:** every new runtime function must compile
  in both modes — pthread use behind `#ifndef PLUTO_TEST_MODE` no-op macros,
  verified with `cc -c runtime/builtins.c -DPLUTO_TEST_MODE` (a raw pthread
  reference breaks every `pluto test` binary). The deterministic suite gets
  the *registry* behavior (export, resolve, release bookkeeping) via direct
  single-process tests; the renewal thread is production-only by design.
- **Soundness regression for the audit fixes:** a test-mode program that
  exports an entity, drops local refs, forces GC, then resolves — must not
  dangle (phase 1).

## Open questions

1. **Durable handles.** A handle stored outside any process (database, config)
   always expires under this protocol. Is long-lived identity a future
   deployment-plan/domain-identity feature (entity pinned by *declaration*,
   not by holder liveness), or permanently out of scope for the registry?
2. **Renewal authentication.** Any process that can reach the home can renew
   (or, with phase 4, hint-release) any id. Today every dialer can already
   *call* any exported entity, so leases add no new exposure — but the
   capability story for handles (rfc-objects.md OQ5's fencing-token slot)
   should subsume renewal when it lands.
3. **Backstop cap.** Does a hard registry cap (option (e), demoted to a
   safety valve against renewal storms) ship, and what does hitting it do —
   refuse new exports (definite error on the exporting call) or evict?
4. **Connection reuse.** Renewal traffic makes the dial-per-message transport
   visibly wasteful; a kept-alive per-home connection would amortize renewal
   and calls alike. Independent transport work; noted as the first concrete
   pressure for it.
5. **Observability.** Should `pluto analyze` or a runtime stats surface report
   registry size / lease churn — the "how big is my export surface" question
   this RFC's leak made unaskable?

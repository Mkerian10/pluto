# RFC: std.fs as Evidence — a Verified Filesystem API

**Status:** Implemented (branch `fs-api-impl`, 2026-10-01) — owner accepted all
inline recommendations as the decisions (D1 Option A, D2 strong-sync + raise on
ENOTSUP with `sync_os()` deferred, D3 failed writes poison, D4 `SharedFile`
deferred, D5 `replace_all` in v1, D6 `Seek` enum, D7 noted only, D8 keep
`mkdir`/`rmdir` and add the `_all` forms). Deviations from the text as
proposed, all minor:

- **Two compiler fixes were needed after all** (the RFC predicted none; both
  were bugs in shipped machinery that typestates-across-a-module-boundary had
  simply never exercised): module flattening did not prefix state names inside
  `where S == Open` method clauses or `must_release` class clauses
  (`src/modules.rs`), and the diagnostic demangler broke module-qualified
  instance names at the dots (`fs.File$$fs.Write$fs.Open` rendered as
  `fs.File<fs>.Write$fs.Open`; `src/diagnostics/mod.rs`).
- **`FileError.path` is `""` for descriptor-based methods** (`read`, `seek`):
  the class carries only `fd`, as specified, so handle-level failures cannot
  name the path. One-shot and constructor failures always carry it.
- **Directory sync uses plain `fsync`**, not F_FULLFSYNC, on the dir fd
  (`sync_dir` and `replace_all` step 4) — the SQLite convention; D2's
  F_FULLFSYNC policy governs *file* syncs (`sync`/`sync_data` and
  `replace_all`'s temp-file sync).
- **One-shot close errors surface as `FileError`**, not `CloseError`: the
  packaged C helpers fold the close result into the single syscall-site errno
  they return. `CloseError` is the handle-level `close()` contract.
- **A `replace_all` temp-file sync failure reports as `FileError`** (old file
  intact, temp cleaned up) per the error contract's own definition — the
  SyncError arm is reserved for the post-rename directory-fsync failure.
- **Enum construction is dotted** (`fs.Seek.Start { offset: 0 }`), matching the
  language's actual variant syntax rather than the RFC's `Seek::Start` sketch.
- The #368 slots (`read_at`/`write_at`/`read_bytes`/`write_bytes`,
  `*_all_bytes`) are reserved as documented signatures in `fs.pt`, not
  compiled code — blocked on bytes extern support exactly as phase 4 states.
- Phase 3's metadata/directory fill (`stat`/`Metadata`, `create_dir_all`,
  `remove_dir_all`) landed with the phase 2 commit; `durable_config` and the
  README landed separately. Cosmetic.
**Author:** Design discussion
**Date:** 2026-10-01
**Related:** [rfc-typestates.md](rfc-typestates.md), [rfc-properties.md](rfc-properties.md), [contracts.md](contracts.md), [epistemics.md](epistemics.md), [rfc-objects.md](rfc-objects.md) ("Typestate and entities: resolved"), issues #367 (durability), #368 (bytes I/O + `read_at`/`write_at`)

## Motivation

`std.fs` predates the entire verification arc. It was written before typestates,
before must-release linearity, before degradation errors, before `requires`
static discharge, and before the epistemics doc named what any of those are
*for*. The result is an API that the language has outgrown:

- **`File` is an entity** (converted during the objects migration), but nothing
  shares one: no stdlib module imports `fs` at all, no example or test spawns
  with a `File`, and the #368 investigation showed per-instance serialization
  provides *zero* mitigation for the seek race anyway (non-mut methods take the
  read lock, which is shared). Entity semantics cost a per-call lock and buy
  nothing.
- **No durability primitive exists at any layer** (`grep fsync` is empty), and
  the #367 investigation found three live bugs besides: `write_all` /
  `append_all` / `copy` swallow `close()` errors (on NFS-class filesystems that
  is where deferred write errors surface — success reported for lost data),
  `File.write` does not loop on short writes, and no fs syscall is bracketed
  with GC safe regions (a slow `fsync` would stall every thread at the next
  collection).
- **Use-after-close, double-close, and fd leaks are all expressible** and only
  caught — if at all — by the OS at runtime.
- **One error type** (`FileError { message: string }`) carries everything from
  ENOENT to EIO; callers cannot branch on what happened without string
  matching.
- **Functionality gaps**: no stat/metadata, no `create_dir_all`, no positioned
  reads, no bytes I/O (#368), seek drives through magic-int whence constants.

This RFC redesigns the surface so the API *uses* the machinery: typestates with
`where` clauses, transition linearity, `must_release`, degradation errors,
strict invariants, `requires` with static discharge, and the
definite/ambiguous failure vocabulary. The owner-approved durability surface
from #367 (sync / sync_data / sync_dir / replace_all, F_FULLFSYNC-on-Darwin,
fsyncgate poisoning) is folded in — with poisoning upgraded from a runtime
flag to a typestate.

## The central question: entity or evidence?

The authorities-vs-evidence doctrine (rfc-objects.md, "Typestate and entities:
resolved") forces the question: what *is* an open file?

**The OS is the authority.** The filesystem owns the real state — contents,
offsets, directory entries, durability. A `File` value never was the file; it
is a *capability* the authority minted at `open()`: a linear, must-release
piece of evidence ("I opened this and have not yet closed it"). That claim is
level-1 on the warrant gradient — introspective knowledge of my own actions —
exactly the kind of claim typestate is sound for. The entity reading, by
contrast, asserts nothing the lock can actually deliver (the seek race is a
two-call protocol; serialization cannot make it atomic) and blocks the
verification machinery entirely (typestate never lives on an entity).

### Option A (recommended): `File` is a typestated class

```pluto
class File<M, S>           // M: mode (Read | Write) — fixed at open
                           // S: state (Open | Poisoned | Closed)
```

- `open_read` / `open_write` / `open_append` mint evidence: a
  `File<_, Open>` value in a **must-release** state.
- `close()` is a consuming transition to `Closed`. Dropping an open file,
  capturing it in a closure/spawn, or storing it in a field is a **compile
  error** — fd leaks become inexpressible.
- `read`/`write` exist only `where S == Open` (and only in the right mode) —
  use-after-close, double-close, and write-on-read-handle are not runtime
  errors; the methods *do not exist* on those types.
- A failed sync is a **degradation error** carrying `File<Write, Poisoned>` —
  the #367 fsyncgate latch becomes a state. Writing to or re-syncing a
  poisoned file is inexpressible; the only method on `Poisoned` is the
  discharge.

What this costs, honestly:

1. **No field storage.** Must-release slice-1 rules reject storing the
   obligation in any field — including entity fields. A service that wants to
   hold an open log file across calls cannot hold a `File<Write, Open>`.
   Livable patterns: (a) the one-shot helpers (`append_all`, `replace_all`) —
   which already cover ~90% of real usage surveyed (blog, http_server, every
   stdlib candidate); (b) an actor whose long-running method holds the file in
   a *local* for its whole life (the broker/WAL shape: the writer entity's run
   loop opens, writes/syncs per batch, closes on shutdown — locals carry
   obligations indefinitely and move cleanly through helper-function
   parameters); (c) a future `SharedFile` entity escape hatch (below).
2. **No consuming conversion out.** Method receivers do not move under the
   linearity rules, so a `fn into_shared(self) SharedFile` cannot discharge
   the obligation — a typestated `File` cannot be converted into a shareable
   entity today without a small language extension (owner decision D7). v1
   ships without a conversion.
3. **Direct construction breaks.** `fs.File { fd: -1 }` (one test uses this as
   a catch fallback) no longer typechecks as written; migration is trivial.

What it buys: compile-checked fd hygiene (no leak, no use-after-close, no
double-close), modes as types, poisoning as a state, and the whole contract
surface (`requires`, invariants) riding on a value the fact engine can
actually track. Sharing — the one thing entities are for — is precisely the
thing #368 proved was already broken for files.

### Option B: keep the entity, add contracts only

Keep `object File`, add `sync`/`sync_data` with a runtime `poisoned: bool`
latch (#367 §4 as originally proposed), keep `requires` on arguments, surface
close errors. No linearity: fd leaks, use-after-close, and double-close remain
runtime errors; poisoning remains a flag consulted at runtime; the seek race
remains open (mitigated only by `read_at`/`write_at`).

Option B is strictly less work (~1 day less) and strictly less language. It
spends none of the verification arc on the one stdlib module that is the
textbook case for it. If `std.fs` doesn't dogfood evidence semantics, nothing
in the stdlib does.

**Recommendation: Option A.** The usage survey removes the main risk (nobody
shares a `File`; nobody stores one in a field today — the only handle users
are one example block and three tests), and the escape hatches cover the
long-lived-handle case. Option B's poisoning flag is exactly the kind of
runtime bookkeeping the language exists to delete.

## Proposed surface (proposal)

State and mode markers are plain classes (phantom type arguments, per the
lease example):

```pluto
// Modes — fixed at open, never transition.
pub class Read { tag: int }
pub class Write { tag: int }

// States.
pub class Open { tag: int }
pub class Poisoned { tag: int }
pub class Closed { tag: int }
```

Errors (taxonomy rationale in the next section):

```pluto
pub error FileError { path: string, code: int, message: string }   // general definite OS failure
pub error NotFound { path: string }                                // ENOENT — the branch-worthy case
pub error CloseError { code: int, message: string }                // close() failed; fd is gone regardless
pub error SyncError { path: string, code: int, message: string }   // one-shot durability failure (sync_dir, replace_all)
pub error Degraded { file: File<Write, Poisoned>, code: int, message: string }
                                                                   // degradation: sync/write failed; warrant destroyed
```

The class:

```pluto
pub class File<M, S> {
    fd: int

    // The descriptor is fixed for the value's whole life within a state
    // (two-state frame invariant, template-proven once — the vocabulary is
    // param-independent). Transitions construct fresh values carrying the
    // same fd; no "-1 after close" sentinel exists anywhere.
    invariant self.fd == old(self.fd)

    // Evidence is not duplicable and may not be abandoned.
    must_release S == Open
    must_release S == Poisoned

    // ── Reading ──────────────────────────────────────────────
    fn read(self, max_bytes: int) string
        where M == Read, S == Open
        requires max_bytes > 0
    // "" means EOF; errors raise FileError (the current EOF/error conflation
    // in __pluto_fs_read is fixed — see implementation notes).

    fn read_at(self, offset: int, max_bytes: int) bytes      // #368 slot (pread)
        where M == Read, S == Open
        requires offset >= 0
        requires max_bytes > 0

    fn read_bytes(self, max_bytes: int) bytes                // #368 slot
        where M == Read, S == Open
        requires max_bytes > 0

    // ── Writing ──────────────────────────────────────────────
    fn write(self, data: string)
        where M == Write, S == Open
    // Loops to completion (short-write fix); raises Degraded on failure —
    // after a failed write the prefix state is unknown, so the handle
    // degrades just as it does on failed sync (owner decision D3).

    fn write_at(self, offset: int, data: bytes)              // #368 slot (pwrite)
        where M == Write, S == Open
        requires offset >= 0

    fn write_bytes(self, data: bytes)                        // #368 slot
        where M == Write, S == Open

    // ── Durability (#367) ────────────────────────────────────
    fn sync(self)        where M == Write, S == Open    // full durability: fsync / F_FULLFSYNC
    fn sync_data(self)   where M == Write, S == Open    // fdatasync / F_FULLFSYNC
    // On failure both raise Degraded { file: File<Write, Poisoned>, .. }.
    // The payload state differs from the receiver's at a state position, so
    // the compiler discriminates this as a degradation error: catching it
    // consumes the Open binding — the value now lives in the payload, in its
    // honest post-failure state.

    // ── Position ─────────────────────────────────────────────
    fn seek(self, to: Seek) int
        where S == Open
    // Both modes: seek is how a reader skips and how a writer (non-append)
    // repositions. Returns the new offset.

    // ── Transitions out ──────────────────────────────────────
    fn close(self) File<M, Closed> where S == Open {
        // Surfaces the close() result: raises CloseError on failure. Either
        // way the fd is released (POSIX deallocates the descriptor even when
        // close fails) — the error is a durability report, not a retry
        // invitation.
    }

    fn discard(self) File<M, Closed> where S == Poisoned {
        // The only exit from Poisoned. Closes the fd and NEVER raises: the
        // durability warrant for this fd is already destroyed (fsyncgate —
        // the kernel cannot re-issue it), so a close report would be
        // testimony about nothing. Recovery is reopen-and-rewrite from data
        // the program still owns, or crash and recover from a log.
    }
}
```

The `Closed` value is inert and droppable — `f.close()!` in statement
position is legal and the obligation is gone. A `Seek` enum replaces the
whence-int API:

```pluto
pub enum Seek {
    Start { offset: int }
    Current { delta: int }
    End { delta: int }
}
```

Constructors:

```pluto
pub fn open_read(path: string) File<Read, Open>      // raises NotFound | FileError
pub fn open_write(path: string) File<Write, Open>    // O_WRONLY|O_CREAT|O_TRUNC
pub fn open_append(path: string) File<Write, Open>   // O_WRONLY|O_CREAT|O_APPEND
// Append-ness lives in the fd (O_APPEND), not the type: a third mode marker
// would force method duplication (no disjunction in where clauses) for no
// checked gain. write_at on an O_APPEND fd appends regardless on Linux —
// documented, as everywhere else.
```

One-shot helpers (non-generic free functions — unchanged shape, fixed
implementations, new additions marked):

```pluto
pub fn read_all(path: string) string                     // raises NotFound | FileError
pub fn write_all(path: string, data: string)             // close errors surfaced (bug fix)
pub fn append_all(path: string, data: string)            // close errors surfaced (bug fix)
pub fn replace_all(path: string, data: string)           // NEW (#367): atomic durable replace
pub fn sync_dir(path: string)                            // NEW (#367): fsync a directory
pub fn read_all_bytes(path: string) bytes                // NEW (#368 slot)
pub fn write_all_bytes(path: string, data: bytes)        // NEW (#368 slot)
pub fn append_all_bytes(path: string, data: bytes)       // NEW (#368 slot)

pub fn stat(path: string) Metadata                       // NEW — raises NotFound | FileError
pub fn exists(path: string) bool
pub fn file_size(path: string) int                       // kept; = stat(path).size
pub fn is_dir(path: string) bool
pub fn is_file(path: string) bool
pub fn remove(path: string)
pub fn mkdir(path: string)
pub fn create_dir_all(path: string)                      // NEW — mkdir -p
pub fn rmdir(path: string)
pub fn remove_dir_all(path: string)                      // NEW — recursive; refuses "/" and ""
pub fn rename(from: string, to: string)
pub fn copy(from: string, to: string)                    // close errors surfaced (bug fix)
pub fn list_dir(path: string) [string]
pub fn temp_dir() string
```

```pluto
pub class Metadata {
    size: int
    modified: int        // unix seconds
    is_dir: bool
    is_file: bool
    mode: int            // permission bits

    invariant self.size >= 0
    // Constructed by the stdlib behind an `if size < 0 { raise }` guard —
    // the construction proof discharges from the flow fact. std.fs proves
    // its own invariant; callers assume size >= 0 for free downstream.
}
```

`SEEK_SET()`/`SEEK_CUR()`/`SEEK_END()` are deleted (subsumed by `Seek`).

### Usage shapes

The scoped case (unchanged ergonomics, now checked):

```pluto
let f = fs.open_read(path)!
let head = f.read(1024)
f.seek(Seek::Start { offset: 0 })!
let again = f.read(4096)
f.close()!
// Forgetting close(): compile error naming the Open obligation and
// suggesting the transitions out. Reading after close: "File<Read, Closed>
// has no method 'read' (method exists only where S == Open)".
```

The durability protocol with honest failure (the WAL/batch shape):

```pluto
fn flush_batch(w: File<Write, Open>, batch: string) File<Write, Open> {
    w.write(batch) catch e: Degraded {
        let p = e.file            // obligation moves to the payload binding
        p.discard()               // the only exit from Poisoned
        raise WalDead { }         // recovery = reopen + rewrite, upstream
    }
    w.sync_data() catch e: Degraded {
        let p = e.file
        p.discard()
        raise WalDead { }
    }
    return w                      // obligation returns to the caller's loop
}
```

A wildcard `catch` cannot swallow `Degraded` — its payload is must-release,
so only a typed handler (which takes on the obligation as `e.file`) is legal.
The fsyncgate rule — never retry a failed sync — is not documentation; the
method to retry does not exist on the type the failure hands you.

## Durability semantics (folding #367)

The approved investigation carries over wholesale; what changes is *where the
latch lives*.

- **`sync()` means full durability on every platform**: `fsync` on Linux,
  `fcntl(F_FULLFSYNC)` on Darwin. On F_FULLFSYNC ENOTSUP (SMB/NFS mounts):
  **raise**, don't silently degrade (owner decision D2). `sync_data()` =
  `fdatasync` on Linux, F_FULLFSYNC on Darwin (no honest cheaper Darwin form;
  F_BARRIERFSYNC is ordering-only — skipped for v1). The weaker form, if
  wanted, is a separate named `sync_os()` ("the OS has it; the drive may
  not") — deferred until asked for.
- **Poisoning is the type, not a flag.** #367 §4's `poisoned: bool` +
  runtime checks in `sync`/`write` are replaced by the `Poisoned` state: the
  epistemic content is identical (a failed sync destroys the warrant for all
  unsynced prior writes, and the kernel cannot re-issue it — errseq_t reports
  once and marks pages clean), but the stdlib no longer *checks* the latch;
  the methods that would need checking don't exist. The recovery path
  (close, reopen, rewrite from owned data) is the only well-typed program.
- **`replace_all` stays a single packaged operation**, not an exploded
  typestate protocol. The four steps (same-dir temp write → fsync temp →
  rename → fsync parent dir) are a protocol whose *only* safe traversal is
  the packaged one; exposing the intermediate states as transitions would add
  ceremony while enabling nothing the packaged form forbids — and
  intermediate failures must clean up the temp file, which a user-driven
  protocol can abandon. Error contract: `FileError` ⇒ the old file is
  intact (failure before rename, temp cleaned up); `SyncError` ⇒ the rename
  landed but its durability is unwarranted (directory fsync failed).
- **`sync_dir`** is required for crash-safe rename/create/remove compositions
  (the name→inode mapping is the directory's dirty page, not the file's) and
  has no handle form — it is a one-shot by nature.

## Error taxonomy and epistemics

Filesystem calls are local syscalls: the request never leaves the process in
the two-generals sense, so **every fs failure is Definite** — the effect is
known not-to-have-applied (or, for close/sync, known-to-have-failed). No
`definite` field, no ambiguity plumbing; this is stated so its absence reads
as a decision, not an oversight. The one epistemically interesting failure is
sync: a failed sync is *definite failure with retroactive scope* — it
destroys the warrant for previously-reported write successes. That is what
`Degraded`-carrying-`Poisoned` encodes.

The split is by *what callers do*, not by errno taxonomy-completeness:

- `NotFound` — the one branch-worthy case (the http_server 404 shape, config
  defaulting). Typed catch; everything else stays generic.
- `FileError { path, code, message }` — all other definite OS failures.
  `path` and raw `code` (captured at the syscall site, not via a later
  `strerror()` round-trip — the errno-clobber fix) replace message-string
  matching.
- `CloseError` / `SyncError` / `Degraded` — as above. Distinct types so a
  generic ENOENT-era handler cannot accidentally absorb a durability loss.

## What the contract sweep found (and deliberately rejected)

- **`requires` on every real precondition**: `max_bytes > 0`,
  `offset >= 0` (see surface above). Honest note on the static-discharge
  payoff: proven direct call sites elide the entry check via `$nochk` twins,
  but *generic callees never elide* (contracts.md phase 6 slice 1), and
  `File<M, S>` methods are generic-class methods — so today the elision only
  fires for the free functions. The clauses are still right: they feed the
  prover, and the elision lands for methods when phase 6 grows
  per-instantiation twins.
- **No `pos` field, no position `ensures`.** Mirroring the OS cursor in a
  field would be a belief about the authority's state presented as
  introspective knowledge — O_APPEND writes, `read` short-counts, and any
  future fd duplication falsify it. The two-state machinery finds no honest
  purchase here *because the class owns almost no state*; that is the design
  working, not a gap. The only invariant is the fd frame invariant (carried
  over from the entity dogfood). Programs that need positions they can reason
  about use `read_at`/`write_at`, which are offset-stateless.
- **No `guarded_by`/properties.** Guards on generic classes are rejected
  (rfc-properties.md), and there is no int-field protocol to fence. A future
  `std.verify.durable`-style property over fn arguments is conceivable but
  belongs to the properties roadmap, not here.

## Migration

Current users, exhaustively (survey: grep over stdlib/examples/tests):

| User | Uses | Impact |
|---|---|---|
| stdlib (all 19 modules) | none import fs | zero |
| examples/blog, examples/http_server | one-shot helpers only | zero (signatures unchanged); http_server's `catch ""` keeps working |
| examples/file_io | one `open_read`/`seek`/`close` block + one-shots | rewrite the seek call to `Seek::Start`; add nothing else — close was already called |
| tests/integration/fs.rs (31 tests) | 3 tests use File handles; 1 constructs `fs.File { fd: -1 }` as a catch fallback | rewrite 3 tests; the fallback-construction pattern is replaced by a terminating catch |
| `SEEK_SET()` et al. | file_io + 1 test | deleted; mechanical |

**Cost estimate: well under a day of migration**, dominated by writing *new*
tests (typestate rejections, poisoning, durability) rather than porting old
ones. No compatibility shim is proposed: the module has no external users yet
and the one-shot majority surface is unchanged — a deprecation period would
preserve exactly the bug classes this RFC exists to delete.

## Owner decisions

- **D1 — Entity vs evidence.** Recommend **Option A**: `File<M, S>` typestated
  class, `must_release` on `Open` and `Poisoned`; no entity `File`.
- **D2 — macOS sync semantics** (carried from #367 §3). Recommend: strong by
  default (F_FULLFSYNC), **raise** on ENOTSUP; `sync_os()` deferred until
  requested.
- **D3 — Degradation scope.** Does a failed `write` poison, or only a failed
  `sync`? Recommend **both** (a failed buffered write leaves the prefix state
  unknown — same destroyed warrant, and the unified rule is simpler than two
  failure models). The cost: plain `write` callers must handle `Degraded`'s
  must-release payload instead of a lighter error.
- **D4 — `SharedFile` escape hatch.** Ship a runtime-checked entity wrapper
  (today's `File` semantics + poison flag) alongside, for the
  stored-in-a-field case? Recommend **defer**: no surveyed program needs it,
  the actor-local-loop pattern covers the broker, and shipping it day one
  invites the unchecked API to stay the default. Revisit on the first real
  program that cannot restructure.
- **D5 — `replace_all` in v1.** Recommend **yes** — it is the packaged
  correctness story the one-shot users actually need (atomic durable config
  replace), and #367 already specced it.
- **D6 — `Seek` enum vs. `seek(pos)`/`seek_end(delta)` method split.**
  Recommend the **enum** (one method, no magic ints); the split's only
  advantage is a `requires pos >= 0` the OS checks anyway.
- **D7 — Consuming-method gap** (noted, not for this RFC): method receivers
  never move under must-release rules, so no method can consume `self` and
  return a *different* class — the reason D4's wrapper has no `File`
  conversion and `close` must return a `Closed` token rather than `void`.
  If the pattern recurs outside fs, a language-level answer (consuming
  methods, or free-function moves blessed in stdlib style) deserves its own
  RFC.
- **D8 — Naming sweep.** `mkdir`/`rmdir` kept as-is alongside new
  `create_dir_all`/`remove_dir_all`? Recommend **keep** (migration noise for
  zero semantic gain); revisit in a general stdlib naming pass.

## Implementation phasing

**Phase 1 — runtime honesty (independent of API shape; ~1–2 days, from #367):**
`__pluto_fs_sync`/`sync_data` (platform policy per D2), `__pluto_fs_sync_dir`,
`__pluto_fs_replace_all` (packaged, temp cleanup on failure); stop swallowing
`close()` in `write_all`/`append_all`/`copy`; short-write loop in
`__pluto_fs_write`; GC safe-region brackets on all blocking fs syscalls;
errno captured at the syscall site (negative-errno returns, retiring the
`__pluto_fs_strerror` read-after-return pattern); EOF/error disambiguation in
`__pluto_fs_read`; debug injection hook (`PLUTO_FS_SYNC_FAIL_AT`) for
deterministic poisoning tests. All plain syscalls — no pthreads/atomics, so
PLUTO_TEST_MODE needs no stubs; verify `cc -c runtime/builtins.c
-DPLUTO_TEST_MODE` per the standing rule.

**Phase 2 — the typestated surface (~2–3 days, pure stdlib + tests):**
rewrite `stdlib/fs/fs.pt` per this RFC (markers, errors, `File<M, S>`,
`Seek`, constructors, one-shot fixes); migrate examples/file_io and the three
fs.rs handle tests; new tests for every rejection class (use-after-close,
leak, double-close, mode violation, poisoned write, wildcard-catch-of-
`Degraded`) plus durability plumbing per #367 §6. Exit criterion: the
flush_batch example above compiles and its misuse variants all fail with the
documented diagnostics. **No compiler changes expected** — every construct
used (two state params, `where` conjunctions, general-form `must_release`,
degradation payload of a concrete instantiation, template-proven frame
invariant on a generic class) is shipped machinery; any gap discovered is a
bug in that machinery, not scope here.

**Phase 3 — functionality fill (~1–2 days):** `stat`/`Metadata`,
`create_dir_all`, `remove_dir_all`; `examples/durable_config/main.pt`
(replace_all + recovery) and examples/README.md per the merge checklist.

**Phase 4 — bytes slots (rides on #368, not duplicated here):** the
`*_bytes`, `read_at`, `write_at` signatures above land when #368 slice 1
lifts the extern-whitelist blocker (`PlutoType::Bytes` at register.rs). This
RFC reserves the names and their `where`/`requires` shapes so that work drops
in without touching the design.

**Deferred, named:** `SharedFile` (D4), `sync_os()` (D2), advisory locking
(an `fs.Lock` must-release evidence value — natural fit, wants the broker to
exist first), permissions API, temp *files* (`temp_dir` covers current
needs), streaming directory iteration.

## Test strategy note

Carried verbatim from #367 §6: syscall-faithful is testable in CI (plumbing,
error paths via the injection hook, the C-side counter for "fsync was actually
issued"); actual power-loss durability and drive-firmware honesty are not, and
the docs must say "syscall-faithful, not crash-tested" rather than imply
otherwise. The typestate layer *adds* a CI-testable tier the runtime design
lacked: every protocol violation is a compile error with a snapshot test.

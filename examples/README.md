# Pluto Examples

## strings

Comprehensive demonstration of string manipulation: basic operations (`len`), case conversion (`to_upper`, `to_lower`), trimming (`trim`, `trim_start`, `trim_end`), substring operations, character access (`char_at`, `byte_at`), string searching (`contains`, `starts_with`, `ends_with`, `index_of`, `last_index_of`, `count`), string replacement, splitting, repetition, concatenation, empty/whitespace checking, and number parsing with nullable types.

```bash
cargo run -- run examples/strings/main.pt --stdlib stdlib
```

## paths

Demonstrates the `std.path` module for path manipulation: `join` (path joining with separator handling), `basename` (extract filename), `dirname` (extract directory), `ext` (file extension), `is_absolute`, `has_trailing_slash`, `normalize` (resolve . and ..), and `split_ext` (filename/extension split).

```bash
cargo run -- run examples/paths/main.pt --stdlib stdlib
```

## file_io

Demonstrates `std.fs`: one-shot helpers (`write_all`, `read_all`, `append_all`, `copy`, `rename`), metadata (`exists`, `is_file`, `file_size`), directory operations (`mkdir`, `list_dir`), error handling with `catch`, the typestated file handle — `open_read` mints a `File<Read, Open>` that must be closed (forgetting `close()` is a compile error), with `seek` driven by the `Seek` enum — and binary file I/O (issue #368): an all-256-byte-values payload round-trips exactly through `write_all_bytes`/`read_all_bytes`, with positional `read_at` (pread) reading at an absolute offset without touching the descriptor's cursor. Also truncation (issue #397): `File.truncate` (ftruncate — the WAL-recovery shape: cut a torn tail in one syscall, then `sync()` to make the shrink durable) and the path-level one-shot `fs.truncate(path, len)`.

```bash
cargo run -- run examples/file_io/main.pt --stdlib stdlib
```

## durable_config

Durability with `std.fs`: `replace_all` (atomic durable config replace — same-dir temp write, full sync, rename, parent-directory fsync) with its two-sided error contract (`FileError` ⇒ old file intact, `SyncError` ⇒ rename landed without a durability warrant), and the handle-level WAL shape — `open_append`, `write` + `sync_data` per batch, where a failed sync raises `Degraded` carrying the handle as `File<Write, Poisoned>` and `discard()` is the only exit (retrying a failed sync is a compile error).

```bash
cargo run -- run examples/durable_config/main.pt --stdlib stdlib
```

## env_example

Demonstrates the `std.env` module for environment variable access: `get` (retrieve variable or empty string), `get_or` (with default fallback), `set` (set variable), `exists` (check if set), `remove` (delete variable), and `list_names` (enumerate all variables).

```bash
cargo run -- run examples/env_example/main.pt --stdlib stdlib
```

## logging

Demonstrates structured logging with `std.log`: setting log levels (`DEBUG`, `INFO`, `WARN`, `ERROR`), logging messages at different levels, and controlling which messages are displayed based on the current log level.

```bash
cargo run -- run examples/logging/main.pt --stdlib stdlib
```

## modules

Demonstrates the module system: `import` for importing modules, `pub` visibility for exported items, module organization with separate files, and accessing public functions and classes from imported modules.

```bash
cargo run -- run examples/modules/main.pt
```

## channels

Demonstrates channels for inter-task communication: `let (tx, rx) = chan<T>(capacity)`, blocking `send`/`recv`, non-blocking `try_send`/`try_recv`, `close()`, `for-in` iteration on receivers, and error handling with `catch`.

```bash
cargo run -- run examples/channels/main.pt
```

## select

Demonstrates `select` for channel multiplexing: waiting on multiple channels simultaneously, fan-in patterns with two producers, non-blocking select with `default`, and error handling when all channels close.

```bash
cargo run -- run examples/select/main.pt
```

## timeouts

Bounded waits: `rx.recv_timeout(ms)` raising a typed `TimedOut` (with `ChannelClosed` still winning on a closed channel), and the select `after` arm — a Raft-flavored follower waits for heartbeats with a randomized election timeout whose window re-randomizes on every loop iteration (the `after` expression is re-evaluated on each select entry).

```bash
cargo run -- run examples/timeouts/main.pt --stdlib stdlib
```

## concurrency

Demonstrates `spawn` for concurrent execution: spawning functions on separate threads, collecting results with `.get()`, error handling with `catch`, and void tasks.

```bash
cargo run -- run examples/concurrency/main.pt
```

## rust_ffi

Demonstrates calling plain Rust functions from Pluto via `extern rust`. A normal Rust crate with `pub fn` functions is imported with zero boilerplate — supported types (`i64`, `f64`, `bool`) are bridged automatically.

```bash
cargo run -- run examples/rust_ffi/main.pt
```

## testing

Demonstrates Pluto's built-in test framework with `test` blocks, `expect()` assertions, and multiple assertion methods (`to_equal`, `to_be_true`, `to_be_false`); raises assertions (`expect_raises(ErrorType) { ... }` asserts the block raises that error type — checked against the compiler's inferred error sets at compile time — and bare `expect_raises { ... }` asserts it raises anything); and test-local DI containers: seeding singleton classes inside test scope blocks to override what the graph wires (`scope(PriceFeed { quote: 50 }) |p: Portfolio| { ... }`).

```bash
cargo run -- test examples/testing/main.pt
```

## deterministic-testing

Demonstrates the deterministic scheduler harness: a `tests[scheduler: Random, seed: N, iterations: M]` block that fixes the schedule sampling in source, and a per-test pin (`test "name" [seed: N, iteration: M]`) that re-runs one exact interleaving as a permanent regression test. Failure output prints a repro block (strategy, seed, schedule token) that can be replayed with `pluto test --schedule ptsched:v1:...` or pinned with `[schedule: "..."]`.

```bash
cargo run -- test examples/deterministic-testing/main.pt
```

## wrapping

Demonstrates integer overflow semantics: signed 64-bit overflow on `+`/`-`/`*` is a defect that aborts the program (never a catchable error), and the `wrapping_add`/`wrapping_sub`/`wrapping_mul` builtins are the explicit escape hatch for deliberately-modular arithmetic (an FNV-style string mixer whose state multiply wraps mod 2^64).

```bash
cargo run -- run examples/wrapping/main.pt
```

## json

Demonstrates the `std.json` module: parsing JSON strings, accessing nested values, building JSON programmatically, and round-tripping through stringify/parse.

```bash
cargo run -- run examples/json/main.pt --stdlib stdlib
```

## blog

A static blog generator that reads markdown-ish `.txt` posts from `posts/`, converts them to HTML with inline formatting (`**bold**`, `*italic*`, `` `code` ``, headings, lists), and writes a full site to `output/`. Demonstrates `std.fs` (file I/O, directory listing), `std.strings` (split, trim, replace, index_of), error handling, and the `app` construct.

```bash
cd examples/blog
cargo run --manifest-path ../../Cargo.toml -- run main.pt --stdlib ../../stdlib
```

## bytes

Demonstrates the `byte` and `bytes` types: hex literals (`0xFF`), explicit casting (`as byte`/`as int`), truncation semantics, packed byte buffers (`bytes_new`, `push`, indexing), string conversion (`to_bytes`/`to_string`), iteration, unsigned ordering, bulk operations (`slice`, `extend`, `fill`, `copy_from`, `find`, `compare`, `bytes_filled`), and fixed-width integer codecs (`read_u8`/`write_u8` through `read_i64_le`/`write_i64_be`, both endiannesses).

```bash
cargo run -- run examples/bytes/main.pt
```

## packages

Demonstrates local path dependencies via `pluto.toml`. A project declares a `mathlib` dependency pointing to a local directory, then imports and uses functions and classes from it.

```bash
cargo run -- run examples/packages/main.pt
```

## pattern_matching

Demonstrates enum pattern matching: unit variants (no data), data-carrying variants with field destructuring, mixed variants (unit and data), exhaustiveness checking, wildcard arms (`_` matches everything not claimed by an earlier arm, in both statement and expression form), and nested pattern matching within match arms.

```bash
cargo run -- run examples/pattern_matching/main.pt
```

## if_expressions

Demonstrates if-as-expression: using `if` as an expression that returns a value, basic if-expressions in variable assignments, nested if-expressions for multi-way branches, if-expressions as function arguments, and if-expressions in conditions. All if-expressions require an else clause and all branches must return compatible types (with nullable coercion support).

```bash
cargo run -- run examples/if_expressions.pt
```

## chained_comparisons

Demonstrates chained comparisons (`a < b < c` ≡ `a < b && b < c`): range guards like `0 <= x <= 100`, chains of any length with mixed directions, single evaluation of middle operands, short-circuiting of later links, `>` chains coexisting with `>>`, bounds guards (`0 <= i < xs.len()`) feeding flow-fact narrowing, and parentheses opting out of chaining.

```bash
cargo run -- run examples/chained_comparisons/main.pt
```

## git-packages

Demonstrates git-based dependencies via `pluto.toml`. A project declares a `strutils` dependency pointing to a git repository, then imports and uses string utility functions from it.

```bash
cargo run -- run examples/git-packages/main.pt
```

## contracts

Demonstrates Pluto's contract system: `requires` (preconditions) and class `invariant` declarations as compile-time proof obligations, plus error-set shrinking — a caller guard that refutes a method's raise condition removes the handling obligation at that call site (no `!`, no `catch`).

```bash
cargo run -- run examples/contracts/main.pt
```

## errors

Demonstrates Pluto's typed error system: error declarations with multiple error types, `raise` to throw errors, `!` postfix for error propagation (at call sites only, never in signatures), `catch` with wildcard error handling, shorthand catch with default values, and compiler-inferred error-ability. Shows that error inference works identically for primitives and custom types — no error annotations are ever written in function signatures — and that caller facts shrink error sets: a guard that refutes every raise condition in the callee makes the call site provably infallible, needing no handling.

```bash
cargo run -- run examples/errors/main.pt
```

## binary-ast

Demonstrates the binary AST commands: `emit-ast` serializes a Pluto source file into a binary `.pluto` AST (with UUIDs and cross-references), and `generate-pt` reads a binary AST back into human-readable Pluto source.

```bash
# Serialize source to binary AST
cargo run -- emit-ast examples/binary-ast/main.pt -o /tmp/main.pt

# Read binary AST back to text
cargo run -- generate-pt /tmp/main.pt
```

## collections-lib

Demonstrates the `std.collections` functional collections library: `map`, `filter`, `fold`, `reduce`, `any`, `all`, `count`, `flat_map`, `for_each`, `reverse`, `take`, `drop`, `zip` (with `Pair`), `enumerate`, `flatten`, `sum`, and `sum_float`. Shows function composition by chaining filter, map, and fold — plus the fallible-callback variants (`try_map`, `try_filter`, `try_fold`, `try_for_each`), whose callbacks may `raise` (`fn(T) U!`); the first error propagates to the caller, which handles it with `catch`.

```bash
cargo run -- run examples/collections-lib/main.pt --stdlib stdlib
```

## parse-and-sort

Parsing numbers and sorting arrays. `strings.parse_int` / `strings.parse_float` raise a typed `strings.ParseError` (with `input` and `message`) for empty strings, stray characters, whitespace, and out-of-range values, instead of returning sentinels. `collections.sort_by` takes a strict less-than comparator and is stable; `sort`, `sort_floats`, and `sort_strings` cover the common cases. The example discovers log segment files by parsing their names and visits them in offset order.

```bash
cargo run -- run examples/parse-and-sort/main.pt --stdlib stdlib
```

## stdin

Demonstrates interactive I/O with `std.io`: reading input with `io.read_line()`, parsing strings to numbers with `.to_int()` and `.to_float()` (both return nullable types — use `?` to propagate none on invalid input), and string interpolation for output.

```bash
echo -e "Alice\n21\n72" | cargo run -- run examples/stdin/main.pt --stdlib stdlib
```

## time

Demonstrates the `std.time` module: wall-clock time (`now`, `now_ns`), monotonic clocks (`monotonic`, `monotonic_ns`), sleeping (`sleep`), and measuring elapsed time (`elapsed`).

```bash
cargo run -- run examples/time/main.pt --stdlib stdlib
```

## random

Demonstrates the `std.random` module: random integers (`next`, `between`), random floats (`decimal`, `decimal_between`), coin flips (`coin`), and seeded determinism (`seed`).

```bash
cargo run -- run examples/random/main.pt --stdlib stdlib
```

## nullable

Demonstrates first-class nullable types: `T?` syntax for nullable types, `none` literal for absent values, `?` postfix operator for null propagation (early-return none), implicit `T` to `T?` coercion, nullable classes, and `to_int()`/`to_float()` string parsing returning nullable types.

```bash
cargo run -- run examples/nullable/main.pt
```

## scope-blocks

Demonstrates scoped dependency injection with `scope()` blocks: creating per-request scoped class instances from seed values, auto-wiring dependency chains (`Handler` -> `UserService` -> `RequestCtx`), mixing scoped and singleton deps, binding multiple services from a single seed, and transient classes that get a fresh instance at every injection point (`transient class Tracer`).

```bash
cargo run -- run examples/scope-blocks/main.pt
```

## system

Demonstrates the `system` declaration for multi-app distributed systems. A system file composes multiple app modules (each with their own `app` declaration and DI graph) into named deployment members. The compiler produces one binary per member.

```bash
cargo run -- compile examples/system/main.pt -o /tmp/system_build
/tmp/system_build/api_server
/tmp/system_build/background
```

## generics

Demonstrates advanced generics: generic classes implementing traits (`class Box<T: Printable> impl Printable`), type bounds on generic parameters (`<T: Trait1 + Trait2>`), explicit type arguments on function calls (`make_pair<string, int>(...)`), generic methods with inferred or explicit type arguments (`fmt.wrap<string>(...)`), and dependency injection on generic classes (`class Repository<T>[db: Database]`).

```bash
cargo run -- run examples/generics/main.pt
```

## traits

Demonstrates traits: structural interfaces (`trait HasArea`), classes implementing multiple traits (`class Rect impl HasArea, Describable`), trait objects with dynamic dispatch (`fn print_area(shape: HasArea)`), and generic traits instantiated with concrete type arguments (`trait Producer<U>`, `class Doubler impl Producer<int>`, `Producer<int>` trait objects).

```bash
cargo run -- run examples/traits/main.pt
```

## stages

Demonstrates the `stage` language construct — a deployable unit for distributed systems. A stage is like `app` but designed as a future RPC boundary. Shows DI with bracket deps (`stage Api[users: UserService]`), `pub` methods (marking future RPC endpoints), private helper methods, and a `main` entry point.

```bash
cargo run -- run examples/stages/main.pt
```

## generators

Demonstrates generators with `stream T` return types and `yield`: lazy integer ranges, infinite Fibonacci sequence with early `break`, and composing multiple generators.

```bash
cargo run -- run examples/generators/main.pt
```

## http-api

A simple JSON API server using `std.http` and `std.json`. Demonstrates listening for HTTP requests, routing by path, parsing JSON request bodies, and returning JSON responses.

```bash
cargo run -- run examples/http-api/main.pt --stdlib stdlib
# Then in another terminal:
# curl http://localhost:8080/hello
# curl -X POST -d '{"name":"Alice"}' http://localhost:8080/echo
```

## uuid

Demonstrates the `std.uuid` module for generating RFC 4122 v4 UUIDs: generating random UUIDs (`generate()`), checking UUID structure and uniqueness, and using UUIDs as identifiers.

```bash
cargo run -- run examples/uuid/main.pt --stdlib stdlib
```

## base64

Demonstrates the `std.base64` module for encoding and decoding Base64: basic encoding (`encode`), decoding (`decode`), URL-safe variants (`encode_url_safe`, `decode_url_safe`), and roundtrip encoding/decoding.

```bash
cargo run -- run examples/base64/main.pt --stdlib stdlib
```

## hash

Demonstrates the `std.hash` module — data-integrity hashes implemented in pure Pluto: CRC-32C (Castagnoli, the Kafka/iSCSI/ext4 checksum) used for record framing and a crash-recovery scan that finds the valid prefix of a torn log; SHA-256 (FIPS 180-4) for content addressing and artifact verification, with incremental (`new_sha256`/`update`/`finish`) and one-shot (`sha256`, `sha256_hex`) APIs; and FNV-1a 64 for partition assignment. Also shows `to_hex` for digest display.

```bash
cargo run -- run examples/hash/main.pt --stdlib stdlib
```

## regex

Demonstrates the `std.regex` module for pattern matching: literal matching (`matches`), finding patterns (`find`, `find_all`), text replacement (`replace`, `replace_all`), splitting text by pattern (`split`), wildcards (`.`), quantifiers (`*`, `+`, `?`), anchors (`^`, `$`), and character shortcuts (`\d`, `\w`, `\s`).

```bash
cargo run -- run examples/regex/main.pt --stdlib stdlib
```

## reflection_demo

Demonstrates Phase 1 compile-time reflection with the `TypeInfo` trait: `TypeInfo::type_name<T>()` returns type names as strings, `TypeInfo::kind<T>()` provides detailed metadata (field names, types, offsets for classes; variant information for enums). Reflection intrinsics are generated at compile time with zero runtime overhead.

```bash
cargo run -- run examples/reflection_demo.pt
```

## rpc

Cross-service RPC where both ends are compiler-generated. The server exposes a service with the `serve` statement (which generates the accept loop, request parsing, method dispatch, and reply); the client calls it through a `remote` dependency, so `self.billing.charge(21)` is type-checked across the boundary and executed over a socket — raising `NetworkError` (handled with `catch`) on any transport failure. Scalars, classes, enums, nullables, arrays, maps, and sets all cross the boundary (`charge_all` sends and returns an `[int]`); complex types are marshaled through `std.wire`.

```bash
# Terminal 1 — start the server (prints its bound port)
cargo run -- run examples/rpc/server.pt --stdlib stdlib

# Terminal 2 — call it, pointing the env var at the server's port
PLUTO_REMOTE_BILLINGSERVICE=127.0.0.1:9000 \
  cargo run -- run examples/rpc/client/main.pt --stdlib stdlib
```

## typestates

State as a type parameter (docs/design/rfc-typestates.md): `Partition<Unowned>` and `Partition<Owned>` are different types, transitions are methods returning the new state, and `where S == Owned` methods exist only on the matching state — wrong-order protocol calls are type errors, not runtime failures.

```bash
cargo run -- run examples/typestates/main.pt
```

## lease

Degradable typestates and must-release obligations (rfc-typestates.md phase 3): `must_release Held` makes a `Lease<Held>` binding fully linear — it cannot be dropped, captured, or stored in a field; moves transfer the single obligation and a transition out of `Held` discharges it. A fallible transition raises a degradation error that *carries the lease in its post-failure state* (`Degraded { lease: Lease<Revoked> }`), so catching it consumes the stale binding and recovery is just extracting the payload. Comments tie each rule to the epistemics: evidence, definite failure, obligation.

```bash
cargo run -- run examples/lease/main.pt
```

## objects

The object construct (docs/design/rfc-objects.md): `object` declares an entity rather than a data structure — reference identity (`==` is identity), spawn shares the entity instead of deep-copying it, and sharing is safe because an object's methods are serialized. Objects never cross domain boundaries as values — they cross by reference, as identity handles (rfc-objects.md phase 2).

```bash
cargo run -- run examples/objects/main.pt
```

## generic-objects

Generic objects (rfc-objects.md phase 3): an `object` can take type parameters, and each monomorphized instantiation is a distinct entity type — `Topic<int>` and `Topic<string>` have separate identity spaces (they cannot even be compared), separate serialization locks, and separate boundary interface hashes. Within an instantiation the entity semantics are unchanged: `==` is identity, spawn shares the instance, methods are serialized.

```bash
cargo run -- run examples/generic-objects/main.pt
```

## placement

The distributed model's `at` expression (docs/design/distributed-model.md): `at self.pay { charge(21) }` places a computation in the `pay` logical execution domain, and the deployment binding — not the code — decides the physical plan. Unbound, the same binary calls the DI-wired colocated instance directly; with `PLUTO_DOMAIN_PAYMENTSERVICE=host:port` set, the identical binary crosses a socket to the served domain. The boundary contract (wire-shaped values, mandatory `catch` — a domain can be unreachable in *some* deployment even if not this one, typed errors crossing intact) is compile-time checked identically for both plans.

```bash
# Plan A — colocated: one process, direct call
cargo run -- run examples/placement/app/main.pt --stdlib stdlib

# Plan B — distributed: same app, domain bound to a separate server
# Terminal 1 (prints its port):
cargo run -- run examples/placement/server/main.pt --stdlib stdlib
# Terminal 2:
PLUTO_DOMAIN_PAYMENTSERVICE=127.0.0.1:<port> \
  cargo run -- run examples/placement/app/main.pt --stdlib stdlib
```

## distributed

A whole `system` of two services, checked end to end at compile time. `billing` serves a `BillingService` that takes a `ChargeRequest` and returns a `Receipt` (structs that marshal across the wire); `orders` calls it as a `remote` dependency and handles failures with multi-catch — a typed `PaymentError` (whose `reason` field arrives from the server) and a wildcard for transport errors.

The `system` declaration ties them together: before either binary is built, the compiler checks that `orders`' remote dependency is served by `billing`, that their interface signatures match, and that every error the server can raise is handled by the client.

```bash
# Compile the system → one binary per member (build/billing, build/orders)
cargo run -- compile examples/distributed/main.pt -o examples/distributed/build --stdlib stdlib

# Terminal 1 — start billing (serves on port 9000)
./examples/distributed/build/billing

# Terminal 2 — run orders, pointed at billing
PLUTO_REMOTE_BILLINGSERVICE=127.0.0.1:9000 ./examples/distributed/build/orders
# -> charged 30 to alice, balance now 70
```

Stop `billing` and re-run `orders` to see the wildcard path (`billing service unavailable`). Change the charge amount above the server's funds to see the typed error cross the wire (`payment declined: insufficient funds`).

## blob

The completed acceptance test of docs/design/rfc-verification.md ("Blob is a stdlib module"): the lockless single-writer blob store with fenced atomic writes now lives in the standard library as `std.blob` (`stdlib/blob/blob.pt`), and this example is a thin consumer that drives the protocol. A `BlobAuthority` entity owns the data and a fencing epoch; minting a `WriteGrant` advances the epoch (silently invalidating all older grants), and every write is judged against the current epoch at the point of effect — a stale grant raises a typed `StaleGrant` error before any byte lands. Each mechanism owns one guarantee: atomicity from entity method serialization, safety from the authority-side fence, liveness from the grant, discipline from typed errors plus `at`'s mandatory-handling contract. The safety theorem ships with the library and is checked where the library compiles: the authority declares `satisfies verify.monotonic(self.epoch), verify.fenced(self.data, self.epoch, WriteGrant)` (std.verify's named proof bundles — every write to the blob must be dominated by the fence, no code outside the authority may write `data` at all, and the epoch can never decrease), carries the exact mint relation `ensures self.epoch == old(self.epoch) + 1` and apply's frame ensures, and exports the property names as facts downstream code can assume without reading the implementation. The consumer could not weaken any of it: the proofs discharge against the stdlib module's own body. The same library serves across processes — a server owns the authority, clients call it through entity handles (tests/integration/distributed.rs).

```bash
cargo run -- run examples/blob/main.pt --stdlib stdlib
```

## properties

The `property` form (docs/design/rfc-properties.md slice 2): named, parameterized bundles of proof atoms. A library declares `property monotonic(f: field<int>) { invariant f >= old(f) }`; a type instantiates it with `satisfies verify.monotonic(self.score)`, and the compiler discharges the substituted atoms exactly as if they were hand-written — same invariant/dominance provers, no new checking machinery. The example declares a local `bounded_below(f: field<int>, lo: const int)` property alongside `std.verify`'s `monotonic`, stacks both on one class, and shows the proof discharging through an ordinary guard. Break either obligation and the compile error names BOTH sides: the property body ("required by property 'monotonic' (defined at verify, line N), instantiated with f = self.score") and the failing site in your code. Meta-parameter kinds: `field<T>` / `field`, `type`, `const int`.

```bash
cargo run -- run examples/properties/main.pt --stdlib stdlib
```

## retry

Ambiguous-failure retry licensed by a property (docs/design/rfc-properties.md phases 5 and 5.5 — the epistemics payoff). A `with_retry` combinator requires idempotency of its function-typed argument — `fn(string) int! provides verify.idempotent`, the same type surface that carries the fallibility contract `!` — and only provably-providing values flow in: pass a plain function or an ordinary closure and the boundary rejects it at compile time. Both legitimate warrants appear side by side. ASSUMED, at the trust boundary: `extern fn ... assume verify.idempotent(key = s)`, reported by `pluto analyze` on the assumption surface with a named owner. CHECKED, in-unit (phase 5.5): `PaymentLedger.apply` carries the dedup-guard shape — membership check, armed insert of the key into a monotone seen-set, then the effect — and the compiler PROVES the guard's placement (`std.verify.idempotent`'s body is the `dedup key` atom; break the shape and compilation fails with two-sided blame). The in-unit provider reaches the combinator through a strict-eta delegation closure (`(r: string) => ledger.apply(r)` carries the method's provides), and the combinator reads `NetworkError.definite`, retrying only from AMBIGUOUS failures — the retry the property licenses. `pluto analyze` prints both surfaces: assumed claims and checked claims.

```bash
cargo run -- run examples/retry/main.pt --stdlib stdlib
cargo run -- analyze examples/retry/main.pt --stdlib stdlib   # prints the assumption surface
```

## function-references

Named functions as first-class values: bind them to variables, pass them to
higher-order functions, store them in arrays, and return them. `compose`
builds a new function out of two named ones. (Generic functions can't be
referenced bare — wrap them in a closure with concrete types.)

Also shows fallibility in function types: `fn(int) int!` accepts functions
that may raise (calls through the value are handled with `!`/`catch`), while
plain `fn(int) int` is an infallible contract that rejects fallible values at
the boundary.

```bash
cargo run -- run examples/function-references/main.pt
```

## primitive-methods

Methods on primitive values: `x.to_string()`, `x.abs()`, `f.sqrt()`,
`f.floor()`/`ceil()`/`round()`, `f.to_int()`, `x.to_float()`, and
`true.to_string()`. They work on literals (`42.to_string()`,
`1.5.to_string()`) and chain like any other method
(`(-16.0).abs().sqrt()`). Math methods agree exactly with the free
builtins (`x.abs() == abs(x)`).

```bash
cargo run -- run examples/primitive-methods/main.pt
```

## bytes_io

Binary I/O with `bytes`: building a binary frame (all 256 byte values), moving
it through a TCP connection with the bytes-typed `read_bytes`/`write_bytes` on
`std.net` (no string laundering), a `requires frame.len() >= 4` contract on a
bytes API, and the same payload crossing the wire layer as a single base64 blob
(`WireValue.Bytes` via `std.wire`), plus binary-exact
`base64.encode_bytes`/`decode_bytes`.

```bash
cargo run -- run examples/bytes_io/main.pt --stdlib stdlib
```

## relay

Kernel-assisted file↔socket relay (`std.fs`, issue #373): `File.send_to`
moves file bytes into a TCP connection via `sendfile(2)` (offset-explicit —
the seek cursor never moves, the handle is not consumed) and
`File.receive_from` relays the socket straight into a file, with a file-side
write failure degrading the handle (`Degraded` carrying
`File<Write, Poisoned>`) exactly like `write`. Neither direction round-trips
bytes through the GC heap. A spawned task serves a 1 MiB binary payload over
loopback while the main task relays it into a byte-exact copy.

```bash
cargo run -- run examples/relay/main.pt --stdlib stdlib
```

<p align="center">
  <br />
  <img src="https://img.shields.io/badge/status-v0.1-blue?style=flat-square" alt="v0.1" />
  <img src="https://img.shields.io/badge/targets-macOS%20%7C%20Linux-brightgreen?style=flat-square" alt="macOS | Linux" />
  <img src="https://img.shields.io/badge/arch-ARM64%20%7C%20x86__64-orange?style=flat-square" alt="ARM64 | x86_64" />
  <br />
  <a href="https://github.com/Mkerian10/pluto/actions/workflows/nightly.yml">
    <img src="https://github.com/Mkerian10/pluto/actions/workflows/nightly.yml/badge.svg" alt="Nightly Build" />
  </a>
  <a href="https://mkerian10.github.io/pluto/coverage/">
    <img src="https://img.shields.io/endpoint?url=https://mkerian10.github.io/pluto/coverage-badge.json&style=flat-square" alt="Coverage" />
  </a>
  <a href="https://mkerian10.github.io/pluto/">
    <img src="https://img.shields.io/badge/nightly-dashboard-purple?style=flat-square" alt="Nightly Dashboard" />
  </a>
</p>

<h1 align="center">Pluto</h1>

<p align="center">
  <strong>The language for distributed backend systems.</strong>
</p>

<p align="center">
  Native compilation &bull; Language-level DI &bull; Compiler-inferred errors &bull; Contracts &bull; AI-native tooling
</p>

---

Every backend team rebuilds the same infrastructure: dependency injection frameworks, error handling conventions, service communication layers. These are platform problems solved with library duct tape. Pluto puts them in the compiler.

```
app OrderSystem[orders: OrderService, payments: PaymentProcessor] {
    ambient Logger

    fn main(self) {
        let order = self.orders.create(item) catch err {
            logger.warn(f"order failed: {err}")
            return
        }
        self.payments.charge(order)!
    }
}
```

**One declaration.** The compiler resolves the dependency graph, infers which calls can fail, wires singletons, and generates a native binary. No container. No annotations. No framework.

## Why Pluto

| | Go | Java/Spring | Pluto |
|---|---|---|---|
| **Dependency injection** | Manual wiring or `wire` | Runtime container + reflection | Compiler-resolved, zero overhead |
| **Error handling** | `if err != nil` (unchecked) | Checked exceptions (viral annotations) | Compiler-inferred, enforced, zero annotation |
| **Error propagation** | Manual return | `throws` chains | `!` (one character) |
| **Service structure** | `func main()` | `@SpringBootApplication` | `app` declaration with typed dep graph |
| **Contracts** | Comments / hope | Bean validation annotations | `requires` / `ensures` / `invariant` — statically proven |
| **Concurrency** | Goroutines (shared state) | Thread pools + `synchronized` | `spawn` + channels + select; values copy, entities serialize |

## Quick Start

```bash
git clone https://github.com/Mkerian10/pluto.git && cd pluto
cargo build --release

echo 'fn main() { print("hello, pluto") }' > hello.pluto
./target/release/pluto run hello.pluto
```

## A Real Program

```
import std.http
import std.json

class UserService[db: Database] {
    fn get(self, id: int) User {
        return self.db.query(f"SELECT * FROM users WHERE id = {id}")!
    }
}

class Database {
    fn query(self, sql: string) string {
        return "result"
    }
}

app API[users: UserService] {
    fn main(self) {
        let user = self.users.get(42) catch err {
            print("not found")
            return
        }
        print(user)
    }
}
```

The compiler sees `UserService` needs `Database`, allocates both as singletons in dependency order, and wires them. `get` calls `db.query` which can fail — the compiler infers this, requires handling at every call site, and rejects the program if you forget.

## Five Things That Justify a New Language

### 1. Dependency injection is a language construct

```
class Cache[store: RedisStore] {
    fn get(self, key: string) string? { ... }
}
```

Bracket deps are resolved at compile time. The compiler topologically sorts the graph, detects cycles, and generates zero-cost wiring. Classes with injected deps cannot be manually constructed — the DI system owns their lifecycle.

### 2. The compiler infers error handling

```
error NotFound { id: int }

fn find(id: int) User {
    if id <= 0 { raise NotFound { id: id } }
    return lookup(id)
}

fn process(id: int) string {
    let user = find(id)!           // propagate
    return user.name
}

fn main() {
    let name = process(42) catch "unknown"  // handle
}
```

No `throws`. No `Result<T, E>`. No `if err != nil`. The compiler analyzes the entire call graph, determines which functions are fallible, and enforces handling at every call site. If you forget `!` or `catch`, it does not compile.

### 3. `app` is a first-class construct

```
app PaymentSystem[orders: OrderService, billing: BillingService] {
    ambient Logger

    fn main(self) {
        self.orders.process_pending()!
    }
}
```

The `app` is the entry point, the dependency root, and the unit of deployment. It is not `func main()` with setup code — it is a structural declaration the compiler understands.

### 4. Contracts are executable specifications

```
class Account {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amount: int)
        requires amount > 0
        requires self.balance >= amount
        ensures self.balance == old(self.balance) - amount
    {
        self.balance = self.balance - amount
    }
}
```

Invariants are **compile-time proof obligations**: the compiler statically proves that every construction and every write preserves them, or the program does not compile — there are no runtime invariant checks (the one exception is validating decoded wire data). `requires` is checked at entry, `ensures` relates pre- and post-state through `old()` and is discharged statically, and `invariant self.segs.len() > 0` over a collection is provable too. The prover works over a decidable fragment (linear integer arithmetic over a type's own fields, flow facts); a property outside it is rejected at declaration rather than silently deferred to runtime.

### 5. Concurrency composes with everything

```
let t1 = spawn fetch_prices(catalog)
let t2 = spawn fetch_inventory(warehouse)

let prices = t1.get()!     // errors propagate from spawned tasks
let stock = t2.get()!

let (tx, rx) = chan<Order>(100)
spawn produce_orders(tx)

for order in rx {
    process(order)!
}
```

`spawn` returns `Task<T>`. Errors flow through `.get()` and are handled with the same `!` / `catch` as everything else. Channels provide typed, bounded communication between tasks.

Sharing is decided by kind, not by annotation. **Values** (classes, enums, arrays, maps, sets) are copied when they cross a `spawn` or a channel — a sent value can never become shared mutable state. **Objects** (entities — `object Name { ... }`) are shared by identity instead: they carry reference identity (`==` is identity, not structure), their methods serialize through a per-instance lock so distinct instances still run concurrently, and only their methods — never raw field pokes — mutate them. The split is what makes the concurrency safe without a borrow checker.

## The Language

| Feature | Syntax |
|---|---|
| Variables | `let x = 42` / `let mut y = 0` |
| Functions | `fn add(a: int, b: int) int { return a + b }` |
| Strings | `"hello {name}"` with interpolation |
| Arrays | `[1, 2, 3]` with `.len()`, `.push()`, indexing |
| Maps | `Map<string, int> { "a": 1 }` |
| Sets | `Set<int> { 1, 2, 3 }` |
| Classes | `class Point { x: int, y: int }` (value: copied, structural `==`) |
| Objects | `object Counter { n: int }` (entity: shared by identity, serialized methods) |
| Traits | `class Square impl HasArea { ... }` |
| Enums | `enum Color { Red, Blue }` + `match` |
| Closures | `(x: int) => x * 2` |
| Generics | `fn id<T>(x: T) T` (monomorphized) |
| Nullable | `T?` / `none` / `?` propagation / flow narrowing |
| For loops | `for x in items { ... }` / `for i in 0..10 { ... }` |
| Comparisons | `a < b`, chained `0 <= i < xs.len()` |
| Conversions | `x.to_float()` / `s.to_int()` (checked, returns `T?`) / `n.to_byte()` |
| Tests | `test "name" { expect(x).to_equal(y) }` / `expect_raises(E) { ... }` |
| Modules | `import math` / `pub fn` |
| Packages | `pluto.toml` with path and git deps |
| FFI | `extern rust "mycrate" { fn compute(x: int) int }` |

## Standard Library

25 modules. Highlights:

| Module | Highlights |
|---|---|
| `std.collections` | `map`, `filter`, `fold`, `reduce`, `zip`, `enumerate`, `flat_map`, stable `sort_by` |
| `std.strings` | `split`, `trim`, `replace`, `contains`, `starts_with`, `to_upper`, `parse_int`/`parse_float` |
| `std.json` | Parse, build, access nested values, stringify |
| `std.http` | HTTP server, request/response, routing |
| `std.fs` | Read, write, seek, `truncate`, bulk `bytes` ops + fixed-width codecs, kernel file→socket relay |
| `std.net` / `std.socket` | TCP listener, connections, read/write |
| `std.compress` | gzip / deflate one-shot compression (vendored miniz) |
| `std.wal` | Write-ahead log — crash-recovery as a typestate (`Wal<Unrecovered → Ready>`) |
| `std.blob` | Lockless single-writer blob store, fenced atomic writes (proven) |
| `std.math` | `abs`, `pow`, `sqrt`, `sin`, `cos`, `log`, `clamp` |
| `std.time` | Wall clock, monotonic, sleep, elapsed |
| `std.random` | Integers, floats, ranges, coin flips, seeded RNG |
| `std.io` | `read_line()` for interactive input |

Plus `std.base64`, `std.hash`, `std.env`, `std.path`, `std.log`, `std.regex`, `std.uuid`, `std.verify`, `std.wire`, `std.rpc`.

## Compiler

```bash
pluto compile main.pluto -o myapp    # Native binary
pluto run main.pluto                 # Compile + execute
pluto test tests.pluto               # Run test blocks
pluto run app.pluto --stdlib stdlib   # With standard library
```

**Pipeline:** Lex &rarr; Parse &rarr; Module Resolve &rarr; Flatten &rarr; Prelude/Stage/Ambient/Spawn transforms &rarr; Contract + Marshal validation &rarr; Type Check &rarr; Reflection + Monomorphize &rarr; Trait/Serializable checks &rarr; Closure Lift + Xref &rarr; Codegen (Cranelift) &rarr; Link

**Targets:** `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`

## AI-Native Development

Pluto exposes its compiler as a structured API. AI agents interact with declarations, types, and cross-references — not raw text.

- **MCP server** (read-only) for inspection: `load_module`, `list_declarations`, `get_declaration`, `usages_of`, `callers_of`, `error_set`, `check`, `compile`, `run`, `test`, `docs`
- **Binary AST** (`.pluto` PLTO format) with stable UUIDs per declaration
- **SDK** (`pluto-sdk`) for programmatic read/write at the semantic level — editing lives here, not in the MCP server

```bash
pluto emit-ast main.pluto -o main.pluto    # Source → binary AST
pluto generate-pt main.pluto               # Binary AST → readable source
```

## Project Status

**Working today:** Functions, classes, objects (entities), traits, enums, generics, closures, DI (`app` + bracket deps + ambient deps + scoped deps), typed error handling, statically-proven contracts (invariants + requires/ensures + two-state + collection-length), typestates with linear transitions and `must_release`, concurrency (spawn + channels + select, with value-copy / entity-serialize semantics), nullable types with flow narrowing, modules, packages (local + git), maps, sets, bytes with bulk ops + codecs, test framework (incl. `expect_raises`), deterministic concurrency testing (DPOR scheduler), Rust FFI, 25-module standard library, binary AST, read-only MCP server, SDK.

**Ahead:** Distribution (cross-pod RPC) and the boundary doctrine, a green-task concurrency model (spawn-per-pthread is the current ceiling), orchestration layer, LLVM backend, package registry, stages (programmable entry points).

## Book

**Read online:** [mkerian10.github.io/pluto](https://mkerian10.github.io/pluto/)

The [Pluto Book](book/) is a comprehensive guide written for experienced developers. It covers everything from the language's differentiating features to the full standard library reference.

```bash
cd book && mdbook serve    # Read locally at http://localhost:3000
```

## License

All rights reserved.

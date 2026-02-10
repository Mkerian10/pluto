<p align="center">
  <br />
  <img src="https://img.shields.io/badge/status-v0.1-blue?style=flat-square" alt="v0.1" />
  <img src="https://img.shields.io/badge/targets-macOS%20%7C%20Linux-brightgreen?style=flat-square" alt="macOS | Linux" />
  <img src="https://img.shields.io/badge/arch-ARM64%20%7C%20x86__64-orange?style=flat-square" alt="ARM64 | x86_64" />
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
            logger.warn("order failed: {err}")
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
| **Contracts** | Comments / hope | Bean validation annotations | `requires` / `ensures` / `invariant` — compiler-checked |
| **Concurrency** | Goroutines (shared state) | Thread pools + `synchronized` | `spawn` with task handles, channels, select |

## Quick Start

```bash
git clone https://github.com/Mkerian10/pluto.git && cd pluto
cargo build --release

echo 'fn main() { print("hello, pluto") }' > hello.pluto
./target/release/plutoc run hello.pluto
```

## A Real Program

```
import std.http
import std.json

class UserService[db: Database] {
    fn get(self, id: int) User {
        return self.db.query("SELECT * FROM users WHERE id = {id}")!
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

Invariants are checked after construction and every method call. Preconditions and postconditions are enforced at runtime. `old()` captures values at function entry. Violations abort — they are not catchable errors.

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

## The Language

| Feature | Syntax |
|---|---|
| Variables | `let x = 42` / `let mut y = 0` |
| Functions | `fn add(a: int, b: int) int { return a + b }` |
| Strings | `"hello {name}"` with interpolation |
| Arrays | `[1, 2, 3]` with `.len()`, `.push()`, indexing |
| Maps | `Map<string, int> { "a": 1 }` |
| Sets | `Set<int> { 1, 2, 3 }` |
| Classes | `class Point { x: int, y: int }` |
| Traits | `class Square impl HasArea { ... }` |
| Enums | `enum Color { Red, Blue }` + `match` |
| Closures | `(x: int) => x * 2` |
| Generics | `fn id<T>(x: T) T` (monomorphized) |
| Nullable | `T?` / `none` / `?` propagation |
| For loops | `for x in items { ... }` / `for i in 0..10 { ... }` |
| Casting | `x as float` |
| Tests | `test "name" { expect(x).to_equal(y) }` |
| Modules | `import math` / `pub fn` |
| Packages | `pluto.toml` with path and git deps |
| FFI | `extern rust "mycrate" { fn compute(x: int) int }` |

## Standard Library

| Module | Highlights |
|---|---|
| `std.collections` | `map`, `filter`, `fold`, `reduce`, `zip`, `enumerate`, `flat_map` |
| `std.json` | Parse, build, access nested values, stringify |
| `std.http` | HTTP server, request/response, routing |
| `std.fs` | Read, write, seek, directory listing, file metadata |
| `std.net` | TCP listener, connections, read/write |
| `std.strings` | `split`, `trim`, `replace`, `contains`, `starts_with`, `to_upper` |
| `std.math` | `abs`, `pow`, `sqrt`, `sin`, `cos`, `log`, `clamp` |
| `std.time` | Wall clock, monotonic, sleep, elapsed |
| `std.random` | Integers, floats, ranges, coin flips, seeded RNG |
| `std.io` | `read_line()` for interactive input |

## Compiler

```bash
plutoc compile main.pluto -o myapp    # Native binary
plutoc run main.pluto                 # Compile + execute
plutoc test tests.pluto               # Run test blocks
plutoc run app.pluto --stdlib stdlib   # With standard library
```

**Pipeline:** Lex &rarr; Parse &rarr; Module Resolve &rarr; Flatten &rarr; Closure Lift &rarr; Type Check &rarr; Monomorphize &rarr; Codegen (Cranelift) &rarr; Link

**Targets:** `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`

## AI-Native Development

Pluto exposes its compiler as a structured API. AI agents interact with declarations, types, and cross-references — not raw text.

- **MCP server** with 20+ tools: `load_module`, `inspect`, `xrefs`, `add_declaration`, `replace_declaration`, `check`, `compile`, `run`, `test`
- **Binary AST** (`.pluto` PLTO format) with stable UUIDs per declaration
- **SDK** for programmatic read/write at the semantic level

```bash
plutoc emit-ast main.pluto -o main.pluto    # Source → binary AST
plutoc generate-pt main.pluto               # Binary AST → readable source
```

## Project Status

**Working today:** Functions, classes, traits, enums, generics, closures, DI (`app` + bracket deps + ambient deps + scoped deps), typed error handling, contracts (invariants + requires/ensures + interface guarantees), concurrency (spawn + channels + select), nullable types, modules, packages (local + git), maps, sets, bytes, test framework, Rust FFI, standard library, LSP, binary AST, MCP server, SDK.

**Ahead:** Distribution (cross-pod RPC), orchestration layer, LLVM backend, package registry, stages (programmable entry points), inferred synchronization.

## Book

The [Pluto Book](book/) is a comprehensive guide written for experienced developers. It covers everything from the language's differentiating features to the full standard library reference.

```bash
cd book && mdbook serve    # Read locally at http://localhost:3000
```

## License

All rights reserved.

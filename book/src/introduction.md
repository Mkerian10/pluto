# The Pluto Programming Language

Most languages give up at the function boundary. They see your code as isolated units — functions, classes, modules — analyzed in isolation, then stitched together at runtime through reflection, containers, or shared conventions. The compiler type-checks each piece, generates code for each piece, and trusts that you wired everything correctly.

**Pluto sees your entire program at compile time.** It analyzes the complete call graph, traces error propagation through every path, resolves your dependency graph, and generates a self-contained native binary with all the knowledge baked in. No runtime container. No reflection. No framework discovering your code at startup.

This is **whole-program compilation**, and it changes everything.

## What Whole-Program Compilation Gives You

When the compiler sees your entire program, it can do things that are impossible with separate compilation:

**Zero-cost dependency injection.** The compiler builds the complete dependency graph at compile time, performs a topological sort, and generates direct allocation and wiring code. There is no container looking up service registrations at runtime. There is no reflection scanning for `@Inject` annotations. The cost is literally zero — it compiles down to a sequence of `calloc` calls and pointer assignments in your `main()` function.

**Compiler-inferred error handling.** The compiler traces every function call in your program, determines which paths can raise errors, and computes the exact error set for each function. You never write `throws` or `Result<T, E>`. The compiler infers fallibility from the complete call graph and enforces handling at every call site. If you forget a `!` or `catch`, the program does not compile.

**Dead code elimination.** The compiler sees every call site in your program. If you import a module with 50 functions but only call 3 of them, the other 47 don't make it into your binary. If you wire up a service that's declared but never used, it doesn't get allocated. This isn't LTO doing cleanup after the fact — the compiler knows what you use because it sees all of it.

**Monomorphization of generics.** The compiler sees every instantiation of every generic type and function across your entire program. It generates exactly the concrete versions you use — `Box<int>`, `Pair<string, float>`, `Option<User>` — and nothing else. No vtables, no boxing, no type erasure. The type parameters are erased at runtime because they were resolved at compile time.

**Compile-time proofs.** Class invariants are not runtime checks — the compiler proves them at every construction and write site, or compilation fails. The proofs feed back into error handling: a guard that makes a callee's `raise` impossible removes the handling obligation at that call site. Whole-program analysis is what makes "every write site" a knowable set.

**Placement over logical domains.** `at domain { expr }` runs a computation where it logically belongs; whether that boundary is crossed in-process or over a socket is decided by the deployment binding at startup, not by the code. The compiler checks the boundary contract — wire-shaped values, typed errors, mandatory handling, interface-hash version guards — identically in every plan. A `system` declaration compiles multiple services as one checked program, one binary per member.

This is the core thesis of Pluto: **If the compiler sees your whole program, it can solve problems that otherwise require frameworks, containers, and runtime magic.**

## What Makes Pluto Different

**Language-level dependency injection.** Classes declare their dependencies in brackets. The compiler sees every class in your program, builds the full dependency graph, and generates explicit wiring code. At runtime, there is no container, no service locator, no `getInstance()` calls — just a sequence of allocations in dependency order. This only works because the compiler sees the whole program.

```
class OrderService[db: Database, cache: Cache] {
    fn get_order(self, id: string) Order {
        return self.db.query("SELECT * FROM orders WHERE id = {id}")
    }
}
```

**Compiler-inferred error handling.** You never annotate functions as fallible. The compiler walks the complete call graph for your program, discovers every `raise` statement, traces error propagation through every function call, and computes the exact error set each function can produce. It enforces handling at every call site. If you forget `!` or `catch`, compilation fails. This is only possible with whole-program analysis.

```
fn process(id: string) string {
    let order = find_order(id)!      // compiler knows find_order can fail
    let receipt = charge(order)!      // compiler knows charge can fail
    return receipt.confirmation       // compiler knows this function can fail
}
```

**The app as a first-class construct.** The `app` declaration is the entry point, the dependency root, and the unit of deployment. It is not a function with a special name -- it is a structural declaration that the compiler understands and can reason about.

```
app PaymentSystem[orders: OrderService, payments: PaymentProcessor] {
    fn main(self) {
        self.orders.process("ORD-42") catch err {
            print("payment failed")
            return
        }
    }
}
```

**Contracts.** Classes declare invariants that the compiler **proves at compile time** — every construction and write site is statically discharged, or compilation fails with a diagnostic naming the missing fact. No runtime invariant checks exist inside the program; the only runtime validation is at wire-decode boundaries, where external data is testimony rather than proof. Functions declare `requires` preconditions, runtime-checked at entry and assumed by the prover.

```
class BankAccount {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amount: int)
        requires amount > 0
        requires self.balance >= amount
    {
        self.balance = self.balance - amount    // proven: invariant preserved
    }
}
```

**Concurrency with spawn and tasks.** `spawn` runs a function on a new thread and returns a `Task<T>`. Error handling composes naturally -- errors from spawned functions flow through `.get()` and are caught the same way as any other error.

```
let t1 = spawn compute_prices(catalog)
let t2 = spawn fetch_inventory(warehouse)
let prices = t1.get()!
let stock = t2.get()!
```

**Values and entities.** `class` declares a value: structural `==`, deep-copied across `spawn`. `object` declares an entity: reference identity, shared across `spawn` with per-instance serialized methods, crossing placement boundaries as an identity handle rather than a copy. The distinction most languages leave as convention is a declaration the compiler enforces.

```
object Counter {
    value: int
    fn increment(mut self) { self.value = self.value + 1 }
}
```

**Typestates.** A protocol is a type: `Lease<Held>` and `Lease<Idle>` are different types, a method can exist only in some states, and transitions consume the old binding so stale aliases are compile errors. States can be marked `must_release` — dropping the obligation without discharging it does not compile.

**AI-native development.** Pluto's compiler exposes a structured API (MCP tools and a programmatic SDK) so AI agents can read, write, and refactor code at the semantic level -- declarations, types, cross-references -- rather than manipulating raw text.

## Why Separate Compilation Fails for Backend Systems

Go, Java, and Rust all use separate compilation. They compile each package or crate independently, then link them together. This is great for build times and incremental compilation, but it means the compiler never sees your complete program.

The result? All the cross-cutting backend concerns get pushed to runtime:

- **Dependency injection** becomes a runtime container (Spring) or a code generation tool run as a separate build step (`wire`). The container uses reflection to discover services at startup. Errors happen at runtime, not compile time.

- **Error handling** becomes conventions. Go returns `(T, error)` tuples and you write `if err != nil` at every call site — but the compiler doesn't enforce it. Java has checked exceptions, but you annotate every function signature manually with `throws`. Rust has `Result<T, E>`, but you choose between `.unwrap()` (crash) and `.expect()` (crash with message) or explicit `match`.

- **Service communication** becomes frameworks. You add `@RestController` annotations and a framework scans them at startup, builds routing tables via reflection, and handles serialization with more reflection. Or you write explicit HTTP client code, manual JSON marshaling, and duplicate error handling logic.

The common thread: **the compiler doesn't know what you're building**, so it can't help you build it correctly.

Pluto's whole-program compilation makes the compiler your infrastructure. It knows your dependency graph. It knows your error propagation. It will know your service boundaries. And it generates code accordingly — no runtime, no reflection, no surprises.

## Implemented vs. Designed

Pluto is transparent about its maturity. The following features are implemented and working today:

- Dependency injection (bracket deps and ambient deps), compile-time wired
- Error handling with compiler-inferred fallibility, enforced handling, and proof-based error-set shrinking at call sites
- The `app` construct with synthetic main generation
- Contracts: invariants proven at compile time (strict), `requires` runtime-checked at entry, `assert` as prover input
- Objects (entities): identity `==`, spawn-sharing with per-instance serialized methods, generic objects, identity handles across boundaries
- Typestates: `where S == State` method gating, transition linearity, `must_release` states, degradation errors carrying post-failure state
- Concurrency via `spawn`, `Task<T>` (must-use, detach, cancel), channels, copy-on-spawn, and inferred synchronization for DI singletons
- Distribution: `at` placement over domain dependencies, `serve`, `stage` declarations, `system` compilation (one binary per member), schema-level wire format, interface hashing
- Nullable types (`T?`, `none`, `?` propagation) and flow narrowing
- Modules, packages, and visibility (`pub`)
- Generics (monomorphized)
- Test framework (`test "name" { ... }` with expect assertions)
- Standard library: base64, collections, env, fs, http, io, json, log, math, net, path, random, regex, rpc, socket, strings, time, uuid, wire

Still direction, not implementation: the deployment-plan artifact that binds domains to physical placement, deadline/cancellation propagation across boundaries, the distributed proof shapes (monotonic fields, guarded effects), and library-defined properties like `idempotent` — see the Vision chapters.

The language is real, compiles to real binaries, and runs real programs. The unimplemented features represent the roadmap, not the reality.

## How to read this book

This book is written for experienced developers. It does not explain what a variable is or how a for loop works. It explains what Pluto does differently and why.

Part 1 (Getting Started) gets you running code. Part 2 (What Sets Pluto Apart) covers the features that justify a new language. Part 3 (The Language) is the reference for syntax, types, and standard library. Part 4 (The Vision) covers where the language is headed: AI-native development and verified distribution.

See Chapter 2 for a complete working example.

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

**Future: Cross-service RPC generation.** Because the compiler sees the full call graph, it will know which function calls cross service boundaries. You write `payment_service.charge(order)` and the compiler generates serialization, HTTP transport, and error propagation automatically. The function call syntax is the same. The compiler generates different code based on where the callee lives.

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

**Contracts.** Classes can declare invariants that are checked at runtime after construction and every method call. Functions can declare preconditions and postconditions. These are not comments or documentation -- they are executable specifications.

```
class BankAccount {
    balance: int
    invariant self.balance >= 0

    fn withdraw(mut self, amount: int)
        requires amount > 0
        requires self.balance >= amount
    {
        self.balance = self.balance - amount
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
- Error handling with compiler-inferred fallibility and enforced handling
- The `app` construct with synthetic main generation
- Contracts (invariants, requires, ensures) with runtime checking
- Concurrency via `spawn`, `Task<T>`, and channels
- Nullable types (`T?`, `none`, `?` propagation)
- Modules, packages, and visibility (`pub`)
- Generics (monomorphized)
- Test framework (`test "name" { ... }` with expect assertions)
- Rust FFI (`extern rust`)
- Standard library: collections, json, math, strings, fs, http, net, time, random

The following features are designed but not yet implemented:

- App stages and lifecycle hooks
- `mut self` tracking for inferred synchronization
- Copy-on-spawn semantics
- Distributed replication and the system layer
- AI-native binary format (`.pluto` as canonical binary AST with stable UUIDs)

The language is real, compiles to real binaries, and runs real programs. The unimplemented features represent the roadmap, not the reality.

## How to read this book

This book is written for experienced developers. It does not explain what a variable is or how a for loop works. It explains what Pluto does differently and why.

Part 1 (Getting Started) gets you running code. Part 2 (What Sets Pluto Apart) covers the five features that justify a new language. Part 3 (The Language) is the reference for syntax, types, and standard library. Part 4 (The Vision) covers the AI-native development direction.

See Chapter 2 for a complete working example.

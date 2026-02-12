# The Pluto Programming Language

Every backend team rebuilds the same infrastructure. Dependency injection frameworks. Error handling conventions. Service communication layers. Deployment pipelines. Configuration management. These are not application problems -- they are platform problems, and solving them with libraries and conventions produces systems that are fragile, inconsistent, and expensive to maintain. Pluto is a language designed from the ground up for distributed backend systems, where the solutions to these problems are built into the compiler, not bolted on after the fact.

Pluto compiles to native code. It has no runtime VM, no garbage collection pauses at scale, and no framework lock-in. What it does have is a set of language-level constructs that eliminate entire categories of boilerplate and bugs that backend engineers deal with daily.

## What makes Pluto different

**Language-level dependency injection.** Classes declare their dependencies in brackets. The compiler resolves the dependency graph at compile time, performs a topological sort, and wires everything as singletons. No containers, no reflection, no service locators.

```
class OrderService[db: Database, cache: Cache] {
    fn get_order(self, id: string) Order {
        return self.db.query("SELECT * FROM orders WHERE id = {id}")
    }
}
```

**Compiler-inferred error handling.** You never annotate functions as fallible. The compiler analyzes the entire call graph, determines which functions can raise errors, and enforces handling at every call site. `!` propagates, `catch` handles. If you forget to handle an error, the program does not compile.

```
fn process(id: string) string {
    let order = find_order(id)!
    let receipt = charge(order)!
    return receipt.confirmation
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

## Implemented vs. designed

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

# Whole-Program Compilation

The foundational difference between Pluto and other backend languages is this: **Pluto sees your entire program at compile time.**

Go compiles each package separately. Java compiles each class separately. Rust compiles each crate separately. They link the pieces together later, but the compiler never sees the complete picture. It sees isolated functions, isolated modules, isolated libraries.

Pluto's compiler sees all of it: every function, every class, every call site, every error that could be raised, every dependency relationship. It builds a complete call graph, traces error propagation across the entire program, resolves all type parameters, and generates a self-contained native binary with all the wiring baked in.

This is called **whole-program compilation**, and it's the reason Pluto can do things that would be impossible — or require runtime magic — in other languages.

## What the Compiler Sees

When you compile a Pluto program, the compiler builds:

**The complete call graph.** Every function call in your program, resolved to a specific function. Not a virtual dispatch table, not a runtime lookup — the compiler knows exactly which function is called at every call site.

**The dependency graph.** Every class that depends on another class, traced through bracket dependencies. The compiler performs a topological sort and knows the exact allocation order for every singleton in your program.

**The error propagation graph.** Every function that can raise an error, every call site that propagates errors with `!`, every `catch` block that handles errors. The compiler computes the exact error set each function can produce and enforces handling at every call site.

**The complete type instantiation set.** Every generic type or function instantiated with concrete type arguments. If you use `Box<int>` and `Box<string>`, the compiler generates exactly those two concrete versions. If you never use `Box<float>`, it doesn't exist in your binary.

**The module structure.** Every import, every `pub` declaration, every cross-module reference. The compiler knows which code is reachable from your `app.main()` and which code is dead.

This isn't runtime analysis. This isn't profiling. This is compile-time, whole-program understanding.

## What It Gives You

### Zero-Cost Dependency Injection

Because the compiler sees the complete dependency graph, it can generate explicit allocation and wiring code. At runtime, there is no container, no reflection, no service discovery — just a sequence of `calloc` calls in dependency order.

```
app API[users: UserService, db: Database] {
    fn main(self) { ... }
}
```

The compiler sees that `API` depends on `UserService` and `Database`. It sees that `UserService` might also depend on `Database`. It builds the full graph, topologically sorts it, and generates:

```c
Database *db = calloc(1, sizeof(Database));
UserService *users = calloc(1, sizeof(UserService));
users->db = db;  // wire the dependency
API *app = calloc(1, sizeof(API));
app->users = users;
app->db = db;
API_main(app);
```

This is literally what the compiler generates (the actual output is Cranelift IR, but the concept is the same). No container. No reflection. No runtime cost. The dependency graph is resolved at compile time.

### Compiler-Inferred Error Handling

Because the compiler sees the complete call graph, it can trace error propagation across every function call and compute the exact error set for each function.

```
fn find_user(id: int) User {
    if id <= 0 { raise NotFound { id: id } }
    return query_db(id)!  // might raise DatabaseError
}
```

The compiler walks the call graph:
- `find_user` contains `raise NotFound` → add `NotFound` to its error set
- `find_user` calls `query_db!` → propagate `DatabaseError` to its error set
- Final error set for `find_user`: `{NotFound, DatabaseError}`

Every caller of `find_user` must use `!` or `catch`. If you forget, compilation fails:

```
fn process(id: int) {
    let user = find_user(id)  // ERROR: unhandled fallible call
}
```

This analysis only works because the compiler sees the whole program. With separate compilation, you'd have to annotate every function signature manually (`throws NotFound, DatabaseError`). Pluto infers it.

### Dead Code Elimination

Because the compiler sees every call site, it knows what's reachable from `main()` and what's not.

```
fn used() { print("called") }
fn unused() { print("never called") }

fn main() { used() }
```

The compiler sees that `unused` is never called from `main()` or any function reachable from `main()`. It doesn't generate code for it. The final binary contains `used` and `main`, but not `unused`.

This extends to classes, imports, and generics. If you import a module but don't use any of its `pub` functions, they don't exist in your binary. If you declare a generic class but never instantiate it, it doesn't exist in your binary.

### Monomorphization of Generics

Because the compiler sees every instantiation of every generic type, it knows exactly which concrete versions to generate.

```
class Box<T> {
    value: T
}

fn main() {
    let b1 = Box<int> { value: 42 }
    let b2 = Box<string> { value: "hello" }
}
```

The compiler sees two instantiations: `Box<int>` and `Box<string>`. It generates two concrete classes:

```c
struct Box__int { int64_t value; };
struct Box__string { void *value; };
```

If you never instantiate `Box<float>`, it doesn't exist. No vtables, no type erasure, no runtime type checking. The type parameters are resolved at compile time.

### Future: Cross-Service RPC Generation

This is the big one. Because the compiler will see the complete call graph, it will know which function calls cross service boundaries.

```
// service A
fn process_order(order: Order) Receipt {
    let payment = payment_service.charge(order)!  // crosses service boundary
    return generate_receipt(payment)
}
```

The compiler will see that `payment_service.charge` is declared in service B. It will generate serialization code for `Order`, HTTP client code for the call, deserialization code for `Receipt`, and automatic error propagation for network failures.

You write function calls. The compiler generates RPC. This is only possible with whole-program analysis.

## The Trade-Off

Whole-program compilation has a cost: build times. The compiler must see your entire program every time you compile. For small programs, this is fine. For large programs, this can be slow.

Pluto mitigates this with incremental compilation: the compiler caches type-checked modules and only recompiles what changed. But the final link step — resolving the dependency graph, computing error sets, generating the synthetic main — requires seeing the whole program.

This is the trade-off: longer build times in exchange for zero-cost abstractions, compiler-enforced correctness, and the ability to solve cross-cutting concerns at compile time rather than runtime.

For backend services, where deployment frequency is measured in hours or days, and where runtime performance and correctness matter more than local iteration speed, this is the right trade-off.

## Why Other Languages Don't Do This

The short answer: backwards compatibility and build times.

C and C++ have separate compilation because build times for large programs would be unbearable otherwise. They use link-time optimization (LTO) to recover some whole-program visibility, but LTO is slow and optional.

Go has separate compilation because it prioritizes fast incremental builds. The Go toolchain is famously fast, but it means the compiler can't do whole-program analysis. Dependency injection becomes manual wiring or `wire` (a separate code generation tool). Error handling becomes conventions (`if err != nil`).

Java has separate compilation because the JVM needs to support loading classes at runtime. It can't see the whole program because the program isn't fully known until runtime. Dependency injection becomes Spring (a runtime container with reflection). Error handling becomes checked exceptions (manual annotations).

Rust has separate compilation because incremental builds are a priority for developer ergonomics. It uses monomorphization like Pluto, but only within a crate. Cross-crate generics use trait objects (dynamic dispatch) unless you enable LTO.

Pluto starts from a different constraint: **backend services, where correctness and runtime performance matter more than local build speed.** The trade-off makes sense for this domain.

## Summary

Whole-program compilation is not free. It costs build time. But it gives you:

- Zero-cost dependency injection (no container, no reflection)
- Compiler-inferred error handling (no manual annotations)
- Dead code elimination (only what you use)
- Monomorphized generics (no vtables, no boxing)
- Future: RPC generation from function calls

All of Pluto's distinguishing features — DI, error inference, the app model, contracts — are built on this foundation. The compiler sees your entire program, understands its structure, and generates exactly the code you need.

This is what justifies a new language.

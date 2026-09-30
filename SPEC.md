# Pluto Language Specification

**Version:** 0.1.0-draft
**Date:** 2026-02-08

---

## Overview

Pluto is a domain-specific programming language for distributed backend systems. It compiles to native code and treats distribution, geographic awareness, and inter-service communication as first-class language concerns.

## Design Principles

- **Logical placement, physical execution.** Programs state *where* computation logically belongs (`at domain { ... }`); the compiler and deployment plan decide how the boundary is physically crossed (network, IPC, in-process). Distribution is explicit in the source — crossing a domain is visible, its failure modes are typed and must be handled — but transport is never the programming model. See docs/design/distributed-model.md.
- **Whole-program compilation.** All source code must be available at compile time. The compiler sees the entire system and uses this to verify correctness, infer error-ability, and optimize topology.
- **Dependencies are declared, not configured.** Code declares what it needs via dependency injection. How those dependencies are provided is an upstream concern.
- **Errors are unavoidable.** Every error must be handled. The compiler infers which functions can error and enforces handling at every call site.
- **Explicit mutation.** Mutability is opt-in. The compiler leverages this for concurrency safety, replication, and cross-pod optimization.

## Language Identity

| Property          | Value                                                              |
| ----------------- | ------------------------------------------------------------------ |
| Domain            | Distributed backend systems with geographic awareness              |
| Implementation    | Rust (compiler + runtime)                                          |
| Code generation   | Native via Cranelift or LLVM                                       |
| Compilation model | Whole-program. Incremental compilation with final link-time analysis |
| Memory management | Garbage collected                                                  |
| Syntax style      | Rust-like                                                          |
| Paradigm          | Multi-paradigm: imperative, OOP (classes + traits), CSP            |
| 0th class object  | The `app`                                                          |

## Design Documents

Detailed design for each area of the language:

| Document | Area |
| --- | --- |
| [Program Structure](docs/design/program-structure.md) | Apps, modules, entry points |
| [Type System](docs/design/type-system.md) | Classes, traits, generics, nominal + structural typing |
| [Error Handling](docs/design/error-handling.md) | Typed errors, inference, `!` and `catch` |
| [Dependency Injection](docs/design/dependency-injection.md) | Bracket deps, ambient DI, auto-wiring, environment opacity |
| [Concurrency](book/src/whats-different/concurrency.md) | Tasks, channels, structured concurrency |
| [Contracts](docs/design/contracts.md) | Invariants, pre/post conditions, failure semantics, protocol contracts |
| [Objects](docs/design/rfc-objects.md) | Entities: reference identity, serialized methods, boundary handles, placement |
| [Typestates](docs/design/rfc-typestates.md) | State-parameterized classes, `where` constraints, transition linearity |
| [Verification](docs/design/rfc-verification.md) | Flow-fact engine, compile-time invariant proofs |
| [Communication](docs/design/communication.md) | Synchronous calls, channels, serialization |
| [Mutability](docs/design/mutability.md) | Explicit mutation, compiler optimizations |
| [Runtime](docs/design/runtime.md) | The Pluto "VM", GC, process lifecycle, crash recovery |
| [Compilation](docs/design/compilation.md) | Whole-program model, incremental builds, link-time analysis |
| [Compiler Runtime ABI](docs/design/compiler-runtime-abi.md) | C runtime surface, data layouts, calling conventions |
| [Orchestration](docs/design/orchestration.md) | The separate layer built on top of Pluto |
| [Open Questions](docs/design/open-questions.md) | Unresolved design areas |

## Syntax Preview

```
error NotFoundError { id: string }
error ValidationError { field: string, message: string }

trait Validator {
    fn validate(self) bool
}

class Order impl Validator {
    id: string
    user_id: string
    items: [Item]
    total: float

    fn validate(self) bool {
        return self.items.len() > 0 && self.total > 0.0
    }
}

// Bracket deps for explicit DI
class OrderService[db: APIDatabase, accounts: AccountsService] uses Logger {
    fn create(mut self, order: Order) Order {
        if !order.validate() {
            raise ValidationError { field: "order", message: "invalid order" }
        }

        let user = self.accounts.get_user(order.user_id)!
        self.db.insert(order)!
        logger.info(f"created order {order.id} for {user.name}")
        return order
    }
}

app OrderApp[order_service: OrderService] {
    ambient Logger

    fn main(self) {
        self.order_service.create(some_order)!
    }
}

// Scoped DI — per-request instances
scoped class RequestCtx {
    user_id: string
    trace_id: string
}

scoped class UserService[db: Database, ctx: RequestCtx] {
    fn current_user(self) string {
        return self.ctx.user_id
    }
}

// scope blocks create fresh instances per call
scope(RequestCtx { user_id: "42", trace_id: "abc" }) |svc: UserService| {
    print(svc.current_user())
}

// Generics
fn first<T>(items: [T]) T {
    return items[0]
}

class Box<T> {
    value: T
}

// Nullable types
fn find_user(id: int) string? {
    if id <= 0 {
        return none
    }
    return f"User {id}"
}

fn greet(id: int) string? {
    let name = find_user(id)?    // unwrap or propagate none
    return f"Hello, {name}!"
}

// Maps and Sets
let m = Map<string, int> { "a": 1, "b": 2 }
let s = Set<int> { 1, 2, 3 }

// Objects — entities with reference identity (docs/design/rfc-objects.md)
object Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

let mut c = Counter { value: 0 }
let alias = c               // same entity: alias == c (identity, not structure)
let t = spawn c.increment() // spawn SHARES the entity; methods serialize per instance

// Entity placement: run the call where the entity lives (direct call when local)
let n = at c { get() } catch -1

// Compile-time invariants: proof obligations, not runtime checks
// (docs/design/rfc-verification.md) — every construction and write site
// must be statically proven to preserve the invariant, or compilation fails
class Account {
    invariant self.balance >= 0

    balance: int

    fn withdraw(mut self, amount: int) {
        if amount >= 0 && amount <= self.balance {
            self.balance = self.balance - amount   // proven from the guard
        }
    }
}

// Typestates: state-restricted methods + linear transitions
// (docs/design/rfc-typestates.md)
class Partition<S> {
    id: int

    fn acquire(self) Partition<Owned> where S == Unowned {
        return Partition<Owned> { id: self.id }
    }

    fn consume(self) int where S == Owned {
        return self.id
    }
}

let u = Partition<Unowned> { id: 7 }
let o = u.acquire()   // transition: 'u' is consumed; using it again is an error
o.consume()           // only exists where S == Owned
```

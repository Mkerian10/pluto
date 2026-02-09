# Pluto Language Specification

**Version:** 0.1.0-draft
**Date:** 2026-02-08

---

## Overview

Pluto is a domain-specific programming language for distributed backend systems. It compiles to native code and treats distribution, geographic awareness, and inter-service communication as first-class language concerns.

## Design Principles

- **Distribution is invisible until it matters.** Cross-pod function calls look like local calls. The compiler handles serialization, networking, and failure modes. But errors from remote calls are explicit and must be handled.
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
        logger.info("created order {order.id} for {user.name}")
        return order
    }
}

app OrderApp[order_service: OrderService] {
    ambient Logger

    fn main(self) {
        self.order_service.create(some_order)!
    }
}

// Generics
fn first<T>(items: [T]) T {
    return items[0]
}

class Box<T> {
    value: T
}

// Maps and Sets
let m = Map<string, int> { "a": 1, "b": 2 }
let s = Set<int> { 1, 2, 3 }
```

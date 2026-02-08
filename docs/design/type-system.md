# Type System

## Overview

Pluto uses a **nominal** type system by default, with **structural** typing for trait satisfaction. Types are compatible only if explicitly declared as the same type, except where structural compatibility is used for interfaces.

## Primitive Types

| Type     | Description              |
| -------- | ------------------------ |
| `int`    | 64-bit signed integer    |
| `float`  | 64-bit floating point    |
| `bool`   | `true` / `false`         |
| `string` | UTF-8 string             |
| `byte`   | Single byte              |
| `bytes`  | Byte array               |

## Option Type

Absent values are represented with `Option<T>`. There is no `null` keyword.

```
fn get_user(self, id: string) Option<User> {
    // returns Some(user) or None
}
```

Ergonomic sugar:
```
let user = self.get_user(id) ?? default_user   // coalesce
let name = self.get_user(id)?.name             // null-safe chain
```

## Collections

Built-in collection types with literal syntax:

```
let items = [1, 2, 3]                    // List<int>
let scores = {"alice": 95, "bob": 87}    // Map<string, int>
let ids = {1, 2, 3}                      // Set<int>
```

## Enums

Enums model data with variants. Variants can carry data.

```
enum Status {
    Active,
    Suspended { reason: string },
    Deleted { at: Timestamp },
}
```

Note: `error` types are a separate language concept (see [Error Handling](error-handling.md)), not sugar over enums.

## Classes

Classes hold data and define methods. There is **no inheritance**. Code reuse is achieved through traits and composition.

```
class OrderProcessor {
    inject db: APIDatabase
    inject logger: Logger

    orders_processed: int

    fn new() OrderProcessor {
        return OrderProcessor { orders_processed: 0 }
    }

    fn process(mut self, order: Order) {
        self.db.insert(order)!
        self.orders_processed += 1
        self.logger.info("processed order {order.id}")
    }

    fn count(self) int {
        return self.orders_processed
    }
}
```

Key properties:
- Classes can declare `inject` dependencies (auto-wired by the runtime)
- Methods that modify fields must declare `mut self`
- Methods that only read fields use `self` (immutable)
- No `extends` or `inherits` keyword exists
- Constructors are explicit `fn new()` methods

## Traits

Traits define shared behavior contracts with optional default implementations.

```
trait Serializable {
    fn serialize(self) bytes
    fn deserialize(data: bytes) Self
}

trait HealthCheck {
    fn health(self) Status {
        return Status.Ok
    }
}

class OrderProcessor impl Serializable, HealthCheck {
    fn serialize(self) bytes { ... }
    fn deserialize(data: bytes) OrderProcessor { ... }
    // health() uses the default implementation
}
```

Key properties:
- Traits can have default method implementations
- A class can implement multiple traits
- The compiler auto-requires `Serializable` for any type sent over a cross-pod channel

## Generics

Angle bracket syntax with trait constraints:

```
// Unconstrained
fn first<T>(items: List<T>) T { ... }

// Constrained by trait
fn sort<T: Comparable>(items: List<T>) List<T> { ... }

// Multiple constraints
fn send_all<T: Serializable + Comparable>(items: List<T>) { ... }
```

## Nominal vs Structural

- **Nominal (default):** Two types with identical fields are NOT interchangeable unless they are the same named type. `APIDatabase` and `AccountsDatabase` are distinct types even if they have the same fields.
- **Structural (traits):** If a class has all the methods a trait requires, it may satisfy the trait. Exact rules TBD — may require explicit `impl` declaration.

## Open Questions

- [ ] Pattern matching — syntax and exhaustiveness checking
- [ ] Closures / lambdas — syntax and capture semantics
- [ ] Exactly where structural vs nominal boundary applies

# Type System

## Overview

Pluto uses a **nominal** type system by default, with **structural** typing for trait satisfaction. Types are compatible only if explicitly declared as the same type, except where structural compatibility is used for interfaces.

## Primitive Types

| Type     | Size | Description              | Status |
| -------- | ---- | ------------------------ | ------ |
| `int`    | I64  | 64-bit signed integer    | Implemented |
| `float`  | F64  | 64-bit floating point    | Implemented |
| `bool`   | I8   | `true` / `false`         | Implemented |
| `string` | ptr  | Heap-allocated byte string (no UTF-8 enforcement) | Implemented |
| `void`   | —    | No value                 | Implemented |
| `byte`   | I8   | Unsigned 8-bit value (0-255) | Implemented |
| `bytes`  | ptr  | Packed byte array (`[len][cap][data_ptr]`) | Implemented |

## Arrays

Arrays are the built-in collection type. They are heap-allocated, homogeneous, and dynamically sized.

```
let items = [1, 2, 3]           // array of int
let names = ["alice", "bob"]    // array of string
let empty: [int] = []           // empty typed array

items.push(4)                   // append
let n = items.len()             // length
let first = items[0]            // index access
items[0] = 10                   // index assignment

for item in items {             // iteration
    print(item)
}
```

Type syntax: `[int]`, `[string]`, `[Point]`, etc.

## Enums

Enums model data with variants. Variants can be unit (no data) or carry named fields. Pattern matching via `match` with exhaustiveness checking.

```
enum Color {
    Red,
    Green,
    Blue,
}

enum Shape {
    Circle { radius: float },
    Rectangle { width: float, height: float },
}

match shape {
    Shape.Circle { radius } {
        print(radius)
    }
    Shape.Rectangle { width, height } {
        print(width * height)
    }
}
```

Note: `error` types are a separate language concept (see [Error Handling](error-handling.md)), not sugar over enums.

## Classes

Classes hold data and define methods. There is **no inheritance**. Code reuse is achieved through traits and composition.

```
class Point {
    x: int
    y: int

    fn distance(self, other: Point) float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return sqrt(dx * dx + dy * dy)
    }
}

let p = Point { x: 1, y: 2 }
print(p.x)
```

Classes can declare injected dependencies using bracket syntax:

```
class OrderService[db: Database, logger: Logger] {
    fn process(self, order: Order) {
        self.db.insert(order)!
        self.logger.info("done")
    }
}
```

Key properties:
- Bracket deps `[dep: Type]` are auto-wired by the DI system at compile time
- Methods that modify fields must declare `mut self`
- Methods that only read fields use `self` (immutable)
- No `extends` or `inherits` keyword exists
- Struct literals construct instances: `Point { x: 1, y: 2 }`

## Traits

Traits define shared behavior contracts with optional default implementations. Conformance is structural — if a class has the required methods with matching signatures, it satisfies the trait (explicit `impl` declaration is required).

```
trait Printable {
    fn to_string(self) string
}

trait Describable {
    fn describe(self) string {
        return "an object"
    }
}

class Point impl Printable, Describable {
    x: int
    y: int

    fn to_string(self) string {
        return "{self.x}, {self.y}"
    }
    // describe() uses the default implementation
}
```

Key properties:
- Traits can have default method implementations
- A class can implement multiple traits
- Trait-typed parameters use vtable dispatch at runtime

## Closures

Arrow function syntax with capture by value:

```
let add = (x: int, y: int) => x + y
let greet = (name: string) => {
    let msg = "hello {name}"
    return msg
}

fn apply(f: fn(int) int, x: int) int {
    return f(x)
}
```

Function type syntax: `fn(int, float) string`, `fn() void`

Closures capture variables from their enclosing scope by value (snapshot at creation time). Heap types (strings, arrays, classes) share the underlying data.

## Generics

Generics use monomorphization — the compiler generates concrete copies for each set of type arguments used.

```
fn identity<T>(x: T) T {
    return x
}

class Box<T> {
    value: T

    fn get(self) T {
        return self.value
    }
}

enum Option<T> {
    Some { value: T }
    None
}
```

Usage:

```
let b = Box<int> { value: 42 }
let s = Box<string> { value: "hello" }
let n = Option<int>.Some { value: 10 }

// Type arguments inferred on function calls
let x = identity(42)        // inferred as identity<int>
```

Key properties:
- Function type arguments are always inferred (no explicit type args on calls)
- Monomorphized names use `__` mangling: `Box__int`, `identity__string`
- Generic classes, functions, and enums are supported
- Current restrictions: no generic trait impls, no DI on generic classes, no type bounds

## Maps and Sets

Maps and sets are built-in collection types backed by GC-managed hash tables.

```
// Maps
let m = Map<string, int> { "a": 1, "b": 2 }
let empty = Map<string, int> {}
m["c"] = 3
print(m["a"])
m.insert("d", 4)
m.remove("a")
print(m.contains("b"))
print(m.len())
for k in m.keys() { print(k) }
for v in m.values() { print(v) }

// Sets
let s = Set<int> { 1, 2, 3 }
s.insert(4)
s.remove(1)
print(s.contains(2))
print(s.len())
let arr = s.to_array()
```

Key types for map keys: `int`, `float`, `bool`, `string`, enums (hashable primitives only).

## Nominal vs Structural

- **Nominal (default):** Two types with identical fields are NOT interchangeable unless they are the same named type. `APIDatabase` and `AccountsDatabase` are distinct types even if they have the same fields.
- **Structural (traits):** A class satisfies a trait if it declares `impl Trait` and provides all required methods with matching signatures. The compiler generates vtables for trait dispatch.

## Not Yet Implemented

- **Option type** — `Option<T>` for absent values, `??` coalesce, `?.` null-safe chain

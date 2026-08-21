# Types

This chapter defines the type system of Pluto.

Pluto is a statically typed language. Every expression has a type known at compile time. The type system uses nominal typing — two types with identical structure are distinct unless they are the same named type.

## Primitive Types

| Type | Description | Size | Zero Value |
|------|-------------|------|------------|
| `int` | Signed 64-bit integer | 8 bytes | `0` |
| `float` | 64-bit IEEE 754 double precision | 8 bytes | `0.0` |
| `bool` | Boolean | 1 byte | `false` |
| `byte` | Unsigned 8-bit integer | 1 byte | `0` |
| `string` | UTF-8 encoded text | heap-allocated | `""` |

### int

The `int` type represents a signed 64-bit integer with a range of −9,223,372,036,854,775,808 to 9,223,372,036,854,775,807.

Integer arithmetic wraps on overflow using two's complement semantics. A future version of the language may provide compile-time overflow detection through the contracts system.

### float

The `float` type represents a 64-bit IEEE 754 double-precision floating-point number.

Floating-point arithmetic follows IEEE 754 rules, including special values (`Infinity`, `-Infinity`, `NaN`). Comparison with `NaN` always returns `false`, including `NaN == NaN`.

### bool

The `bool` type has exactly two values: `true` and `false`.

### byte

The `byte` type represents an unsigned 8-bit integer with a range of 0 to 255. It is primarily used for raw binary data.

### string

The `string` type represents an immutable sequence of UTF-8 encoded bytes. Strings are heap-allocated and garbage collected.

## Void

Functions that do not return a value have an implicit return type of `void`. The `void` type cannot appear in type annotations — it is inferred by the compiler when a function has no return type.

```
fn greet(name: string) {
    print(f"Hello, {name}!")
}
// greet has return type void (implicit)
```

## Composite Types

### Arrays

An array type is written `[T]` where `T` is the element type. Arrays are homogeneous — all elements must be the same type.

```
let numbers: [int] = [1, 2, 3]
let names: [string] = ["Alice", "Bob"]
let empty: [int] = []
```

Arrays are heap-allocated, dynamically sized, and garbage collected.

The `bytes` type is an alias for `[byte]`.

### Maps

A map type is written `Map<K, V>` where `K` is the key type and `V` is the value type.

Key types are restricted to hashable types: `int`, `float`, `bool`, `string`, and enum types.

```
let ages = Map<string, int> { "Alice": 30, "Bob": 25 }
let empty = Map<string, int> {}
```

### Sets

A set type is written `Set<T>` where `T` is the element type.

Element types are restricted to hashable types: `int`, `float`, `bool`, `string`, and enum types.

```
let ids = Set<int> { 1, 2, 3 }
let empty = Set<string> {}
```

## Nullable Types

Any type `T` can be made nullable by appending `?`, written `T?`. A nullable type can hold either a value of type `T` or `none`.

```
let name: string? = "Alice"
let missing: string? = none
```

Nested nullable types are not permitted: `T??` is a compile error.

`void?` is not permitted.

### The `none` Literal

The literal `none` represents the absence of a value. Its type is `T?` where `T` is inferred from context. Using `none` without sufficient type context is a compile error.

### Null Propagation

The postfix `?` operator unwraps a nullable value, returning the inner value if present or propagating `none` to the enclosing function's return value.

```
fn get_name(id: int) string? {
    let user = find_user(id)?  // if none, return none
    return user.name
}
```

The `?` operator may only appear in functions whose return type is nullable.

## Function Types

A function type is written `fn(P1, P2, ...) R` where `P1, P2, ...` are the parameter types and `R` is the return type. If the function returns void, the return type is omitted.

```
let transform: fn(int) string = (x: int) => x.to_string()
let predicate: fn(int) bool = (x: int) => x > 0
let callback: fn(string) = (s: string) => print(s)
```

Function types use structural compatibility — two function types are compatible if they have the same parameter types and return type.

## User-Defined Types

### Classes

Classes define nominal types with named fields and methods. See the [Classes](classes.md) chapter for full details.

```
class Point {
    x: float
    y: float
}
```

Two classes with identical fields are distinct types. A `Point` is not interchangeable with a `Coordinate` even if both have `x: float` and `y: float`.

### Traits

Traits define interfaces that classes can implement. Conformance is nominal — a class must explicitly declare `impl Trait` to satisfy a trait.

```
trait Printable {
    fn to_string(self) string
}

class User impl Printable {
    name: string

    fn to_string(self) string {
        return self.name
    }
}
```

Having the right methods is not sufficient — the `impl` declaration is required.

### Enums

Enums define types with a fixed set of variants. Variants may be unit variants (no data) or data-carrying variants (with named fields).

```
enum Color {
    Red
    Green
    Blue
}

enum Shape {
    Circle { radius: float }
    Rectangle { width: float, height: float }
}
```

### Errors

Error types define structured error values for the error handling system. See the [Error Handling](error-handling.md) chapter.

```
error NotFoundError {
    id: string
}
```

## Concurrency Types

### Task

`Task<T>` represents a handle to a concurrently executing computation that will produce a value of type `T`.

```
let t: Task<int> = spawn compute(42)
let result: int = t.get()
```

### Channels

`Sender<T>` and `Receiver<T>` represent the two ends of a typed channel for inter-task communication.

```
let (tx, rx) = chan<int>()
// tx: Sender<int>, rx: Receiver<int>
```

### Stream

`Stream<T>` represents a lazily-produced sequence of values of type `T`, generated by a function using `yield`.

## Range

The `Range` type represents a range of integers, created with the `..` (exclusive) or `..=` (inclusive) operators.

```
let r = 0..10    // 0, 1, 2, ..., 9
let r = 0..=10   // 0, 1, 2, ..., 10
```

Ranges are primarily used in `for` loops.

## Generics

Types and functions can be parameterized with type parameters. Pluto uses monomorphization — generic code is compiled into separate copies for each concrete type used.

```
class Box<T> {
    value: T
}

fn identity<T>(x: T) T {
    return x
}
```

See the [Generics](generics.md) chapter for full details on type parameters, bounds, and instantiation.

## Type Conversions

### Implicit Conversions

Pluto performs the following implicit type conversions (widening):

| From | To | Notes |
|------|----|-------|
| `byte` | `int` | Always safe (u8 to i64) |
| `int` | `float` | Always safe for values < 2^53 |
| `T` | `T?` | Any value implicitly wraps to nullable |
| `none` | `T?` | The `none` literal is assignable to any nullable type |

No other implicit conversions exist. In particular, `float` to `int` requires an explicit cast.

### Explicit Conversions

The `as` operator performs explicit type conversion between compatible types:

| Cast | Behavior |
|------|----------|
| `int as float` | Convert integer to floating-point |
| `float as int` | Truncate toward zero |
| `int as bool` | `0` → `false`, non-zero → `true` |
| `bool as int` | `false` → `0`, `true` → `1` |
| `int as byte` | Truncate to low 8 bits |
| `byte as int` | Zero-extend to 64 bits |

All other casts are compile errors.

## Type Compatibility

Two types are compatible (one can be used where the other is expected) according to these rules:

1. **Identity**: A type is always compatible with itself.
2. **Implicit conversion**: If an implicit conversion exists from type `A` to type `B`, then `A` is compatible with `B`.
3. **Trait satisfaction**: A class type is compatible with a trait type if the class explicitly declares `impl` for that trait.
4. **Function compatibility**: Two function types are compatible if they have the same number of parameters, each parameter type is compatible, and the return types are compatible.

### Variance

All generic types in Pluto are invariant. `Box<Dog>` is not compatible with `Box<Animal>`, even if `Dog` implements `Animal`. Similarly, `[Dog]` is not compatible with `[Animal]`.

## Arithmetic Semantics

### Integer Arithmetic

Integer division truncates toward zero. Division by zero is a contract violation that aborts the program — it is not a catchable error.

The remainder operator `%` follows the same sign convention as the dividend: `-7 % 3 == -1`.

Integer overflow wraps using two's complement semantics.

### Floating-Point Arithmetic

Floating-point arithmetic follows IEEE 754 rules. Division by zero produces `Infinity` or `-Infinity` (not an error). `0.0 / 0.0` produces `NaN`.

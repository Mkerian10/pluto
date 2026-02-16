# Closures and Generics

## Closures

Closures are anonymous functions that capture variables from their enclosing scope.

### Syntax

Arrow syntax: `(params) => expression` or `(params) => { block }`.

```
let double = (x: int) => x * 2
let greet = () => "hello"
let add = (a: int, b: int) => a + b
```

For multi-statement bodies, use a block with explicit `return`:

```
let classify = (x: int) => {
    if x > 0 {
        return "positive"
    }
    return "non-positive"
}
```

### Function Types

The type of a closure is `fn(param_types) return_type`:

| Type | Meaning |
|------|---------|
| `fn(int) int` | Takes an int, returns an int |
| `fn(int, float) string` | Takes int and float, returns string |
| `fn() int` | Takes nothing, returns an int |

### Capture by Value

Closures capture a snapshot at creation time:

```
fn main() {
    let a = 10
    let f = (x: int) => x + a
    print(f(5))    // 15
    a = 999
    print(f(5))    // still 15
}
```

Primitives (int, float, bool) are copied. Heap types (strings, classes, arrays) share the underlying data -- the pointer is copied, not the object.

### Higher-Order Functions

Functions accept closures as parameters and can return them:

```
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn make_adder(n: int) fn(int) int {
    return (x: int) => x + n
}

fn main() {
    print(apply((x: int) => x * 3, 7))    // 21

    let add5 = make_adder(5)
    print(add5(3))    // 8
}
```

### Closures in Arrays

```
fn main() {
    let ops: [fn(int) int] = [
        (x: int) => x + 1,
        (x: int) => x * 2,
        (x: int) => x * x
    ]
    for op in ops {
        print(op(5))
    }
}
```

## Generics

Generics let you write functions, classes, and enums that work with any type. Pluto uses monomorphization: the compiler generates a specialized copy for each concrete type, like Rust. Zero runtime overhead.

### Generic Functions

Type parameters go in angle brackets after the function name:

```
fn identity<T>(x: T) T {
    return x
}

fn main() {
    print(identity(42))        // 42
    print(identity("hello"))   // hello
}
```

Type arguments are usually inferred from usage. You can also specify them explicitly: `identity<int>(42)`.

### Multiple Type Parameters

```
fn first<A, B>(a: A, b: B) A {
    return a
}

print(first(1, "hello"))    // 1
```

### Generic Classes

Classes can have type parameters. Specify the concrete type when constructing:

```
class Box<T> {
    value: T

    fn get(self) T {
        return self.value
    }
}

fn main() {
    let b = Box<int> { value: 42 }
    print(b.get())    // 42

    let s = Box<string> { value: "hello" }
    print(s.get())    // hello
}
```

Multiple type parameters:

```
class Pair<A, B> {
    first: A
    second: B
}

let p = Pair<string, int> { first: "age", second: 25 }
```

### Generic Enums

```
enum Result<T> {
    Ok { value: T }
    Err { message: string }
}

fn divide(a: int, b: int) Result<int> {
    if b == 0 {
        return Result<int>.Err { message: "division by zero" }
    }
    return Result<int>.Ok { value: a / b }
}

fn main() {
    let r = divide(10, 3)
    match r {
        Result.Ok { value } { print(value) }
        Result.Err { message } { print(message) }
    }
}
```

### How Monomorphization Works

`Box<int>` and `Box<string>` become completely separate types at compile time. No type erasure, no boxing, no vtable overhead. Generic code is as fast as hand-written specialized code, at the cost of binary size when many instantiations exist.

### Type Bounds

You can constrain type parameters to types that implement one or more traits:

```
fn print_area<T: HasArea>(shape: T) {
    print(shape.area())
}

fn process<T: Readable + Writable>(item: T) {
    let data = item.read()
    item.write(data)
}
```

The compiler validates bounds at every instantiation site.

### Explicit Type Arguments

Type arguments are usually inferred, but you can specify them explicitly:

```
let x = identity<int>(42)
```

### Generic Classes with Traits

Generic classes can implement traits. The compiler validates method signatures after monomorphization:

```
class Box<T> impl Printable {
    value: T
    fn to_string(self) string { return "Box" }
}
```

### Generic DI

Classes with bracket deps can have type parameters:

```
class Repository<T>[db: Database] {
    fn find(self, id: int) T {
        return self.db.query("SELECT * WHERE id = {id}")!
    }
}
```

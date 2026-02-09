# Generics

Generics let you write functions, classes, and enums that work with any type.

## Generic Functions

Add type parameters in angle brackets after the function name:

```
fn identity<T>(x: T) T {
    return x
}

fn main() {
    print(identity(42))         // 42
    print(identity("hello"))    // "hello"
}
```

Type arguments are inferred from usage -- you don't need to specify them at the call site.

## Multiple Type Parameters

```
fn first<A, B>(a: A, b: B) A {
    return a
}

fn main() {
    print(first(1, "hello"))    // 1
    print(first("hi", 42))     // "hi"
}
```

## Generic Classes

Classes can have type parameters:

```
class Box<T> {
    value: T

    fn get(self) T {
        return self.value
    }
}

fn main() {
    let b = Box<int> { value: 42 }
    print(b.get())              // 42

    let s = Box<string> { value: "hello" }
    print(s.get())              // "hello"
}
```

When creating an instance, you specify the concrete type: `Box<int>`.

## Generic Classes with Multiple Parameters

```
class Pair<A, B> {
    first: A
    second: B
}

fn main() {
    let p = Pair<string, int> { first: "age", second: 25 }
    print(p.first)      // "age"
    print(p.second)     // 25
}
```

## Generic Enums

Enums can also be generic:

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
        Result.Ok { value } {
            print(value)
        }
        Result.Err { message } {
            print(message)
        }
    }
}
```

## Option&lt;T&gt;

Pluto includes a built-in `Option<T>` type in the prelude for representing optional values:

```
enum Option<T> {
    Some { value: T }
    None
}
```

You don't need to define it -- it's always available:

```
fn find(id: int) Option<int> {
    if id > 0 {
        return Option<int>.Some { value: id }
    }
    return Option<int>.None
}

fn main() {
    let result = find(42)
    match result {
        Option.Some { value } {
            print("found: {value}")
        }
        Option.None {
            print("not found")
        }
    }
}
```

## How It Works

Pluto uses **monomorphization** -- the compiler generates a specialized copy of each generic function/class/enum for every concrete type it's used with. This means generics have zero runtime overhead. `Box<int>` and `Box<string>` become completely separate types at compile time.

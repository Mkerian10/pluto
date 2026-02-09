# Closures

Closures are anonymous functions that can capture variables from their surrounding scope.

## Basic Syntax

Closures use arrow syntax: `(params) => expression`:

```
fn main() {
    let double = (x: int) => x * 2
    print(double(5))        // 10
}
```

## No Parameters

```
let greet = () => "hello"
print(greet())              // "hello"
```

## Multiple Parameters

```
let add = (a: int, b: int) => a + b
print(add(3, 7))           // 10
```

## Block Bodies

For multi-statement closures, use a block with `return`:

```
let classify = (x: int) => {
    if x > 0 {
        return "positive"
    }
    return "non-positive"
}

print(classify(5))          // "positive"
print(classify(-3))         // "non-positive"
```

## Capture by Value

Closures capture variables from the enclosing scope by value. This means they take a snapshot at creation time:

```
fn main() {
    let a = 10
    let f = (x: int) => x + a
    print(f(5))             // 15

    a = 999
    print(f(5))             // still 15 -- captured the original value
}
```

## Multiple Captures

Closures can capture any number of variables:

```
fn main() {
    let a = 10
    let b = 20
    let f = (x: int) => x + a + b
    print(f(5))             // 35
}
```

## Higher-Order Functions

Functions can take closures as parameters. The type syntax for a closure is `fn(param_types) return_type`:

```
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn main() {
    let triple = (x: int) => x * 3
    print(apply(triple, 7))     // 21
}
```

## Returning Closures

Functions can return closures:

```
fn make_adder(n: int) fn(int) int {
    return (x: int) => x + n
}

fn main() {
    let add5 = make_adder(5)
    let add10 = make_adder(10)
    print(add5(3))      // 8
    print(add10(3))     // 13
}
```

## Closures in Arrays

Closures can be stored in arrays and called dynamically:

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
    // prints: 6, 10, 25
}
```

## Function Type Syntax

The type `fn(A, B) R` describes a function that takes parameters of types `A` and `B` and returns `R`:

| Type | Description |
|------|-------------|
| `fn(int) int` | Takes an int, returns an int |
| `fn(int, int) string` | Takes two ints, returns a string |
| `fn() int` | Takes nothing, returns an int |

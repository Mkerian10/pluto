# Functions

## Defining Functions

Functions are declared with `fn`, followed by the name, parameters, an optional return type, and the body:

```
fn add(a: int, b: int) int {
    return a + b
}

fn greet(name: string) {
    print("hello, {name}")
}
```

If a function doesn't return a value, you omit the return type (it implicitly returns `void`).

## Calling Functions

```
fn add(a: int, b: int) int {
    return a + b
}

fn main() {
    let result = add(3, 4)
    print(result)       // 7
}
```

## Return

Use `return` to exit a function with a value:

```
fn abs(x: int) int {
    if x < 0 {
        return -x
    }
    return x
}
```

You can also use `return` without a value in void functions to exit early:

```
fn maybe_print(x: int) {
    if x < 0 {
        return
    }
    print(x)
}
```

## The main Function

Every Pluto program needs a `fn main()` as its entry point (unless you're using an `app` declaration, covered later):

```
fn main() {
    print("hello, world")
}
```

## Functions Calling Functions

Functions can call other functions freely, and order of definition doesn't matter:

```
fn double(x: int) int {
    return x * 2
}

fn quadruple(x: int) int {
    return double(double(x))
}

fn main() {
    print(quadruple(3))    // 12
}
```

## Recursion

Functions can call themselves:

```
fn factorial(n: int) int {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

fn main() {
    print(factorial(5))    // 120
}
```

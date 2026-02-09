# Error Handling

Pluto has a built-in error handling system. Errors are a first-class concept -- the compiler infers which functions can fail and enforces handling at every call site.

## Defining Errors

Errors are declared with the `error` keyword:

```
error NotFound {
    message: string
}

error InvalidInput {
    code: int
}

error Empty {}
```

Errors can have fields (like classes) or be empty.

## Raising Errors

Use `raise` to signal an error:

```
error NotFound {
    message: string
}

fn find_user(id: int) string {
    if id < 0 {
        raise NotFound { message: "invalid id" }
    }
    return "user_{id}"
}
```

## Catching Errors

### Shorthand catch

The simplest form provides a default value:

```
fn main() {
    let name = find_user(-1) catch "unknown"
    print(name)     // "unknown"

    let name2 = find_user(5) catch "unknown"
    print(name2)    // "user_5"
}
```

The catch value must match the function's return type.

### Wildcard catch

To access the error, use `catch err { expr }`:

```
fn main() {
    let name = find_user(-1) catch err { "error: got an error" }
    print(name)     // "error: got an error"
}
```

## Error Propagation with !

Use `!` to propagate an error to the caller:

```
error BadInput {
    code: int
}

fn validate(x: int) int {
    if x < 0 {
        raise BadInput { code: x }
    }
    return x
}

fn process(x: int) int {
    let v = validate(x)!    // If validate raises, process raises too
    return v * 10
}

fn main() {
    let a = process(-5) catch 0
    print(a)        // 0

    let b = process(3) catch 0
    print(b)        // 30
}
```

The `!` operator is Pluto's way of saying "if this fails, pass the error up to my caller."

## Transitive Error Propagation

Errors propagate through any number of function calls:

```
error Fail {}

fn step1() int {
    raise Fail {}
    return 0
}

fn step2() int {
    let x = step1()!
    return x + 1
}

fn step3() int {
    let x = step2()!
    return x + 1
}

fn main() {
    let result = step3() catch -1
    print(result)       // -1
}
```

## Compiler Enforcement

The compiler automatically infers which functions can raise errors by analyzing the entire call graph. It then **enforces** that every call to a fallible function is handled:

- If you call a function that can raise and don't use `!` or `catch`, you get a compile error
- If you use `!` or `catch` on a function that can't raise, you also get a compile error
- The `catch` value type must match the function's return type

You never need to annotate which errors a function can raise -- the compiler figures it out.

## Method Calls

Error handling works the same way with method calls:

```
error Fail {}

class Service {
    fn do_thing(self) int {
        raise Fail {}
        return 0
    }
}

fn main() {
    let s = Service {}
    let result = s.do_thing() catch -1
    print(result)       // -1
}
```

## Errors Are Not Exceptions

Pluto's errors are not exceptions. There is no stack unwinding. When a function raises an error, it returns immediately with an error value. The `!` operator checks for this and returns early. The `catch` operator checks for it and provides a fallback. This makes error flow explicit and predictable.

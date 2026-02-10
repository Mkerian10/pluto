# Nullable Types

Nullable types represent values that may be absent. They are a first-class language concept in Pluto, with dedicated syntax and compiler support.

## The `T?` Type

Append `?` to any type to make it nullable:

```
let x: int? = 42       // holds a value
let y: int? = none      // absent
```

This works with all types:

```
let s: string? = "hello"
let f: float? = none
let b: bool? = true
```

## The `none` Literal

`none` represents an absent value. It can be assigned to any nullable variable or returned from a nullable function:

```
let x: int? = none
```

Bare `none` requires type context -- the compiler must know what `T?` it represents:

```
let x: int? = none           // OK: context from type annotation
let y = find_positive(-1)    // OK: context from function return type
```

## Returning Nullable Values

Functions that may return nothing use `T?` as their return type:

```
fn find_positive(x: int) int? {
    if x > 0 {
        return x        // T is implicitly wrapped to T?
    }
    return none
}
```

A plain `T` value is automatically coerced to `T?` -- no wrapping needed.

## The `?` Operator

The `?` postfix operator unwraps a nullable value. If the value is `none`, it immediately returns `none` from the enclosing function:

```
fn double_positive(x: int) int? {
    let val = find_positive(x)?     // unwrap or return none
    return val * 2
}
```

This is null propagation -- identical in spirit to how `!` propagates errors. The `?` operator can only be used in functions that themselves return a nullable type.

## Chaining `?`

Multiple `?` operations can be chained:

```
fn step1() int? {
    return 10
}

fn step2(x: int) int? {
    return x + 5
}

fn pipeline() int? {
    let a = step1()?
    let b = step2(a)?
    return b
}
```

If any step returns `none`, the entire chain short-circuits.

## Nullable Parameters

Functions can accept nullable parameters:

```
fn describe(val: int?) int? {
    let v = val?
    print("got {v}")
    return none
}
```

## String Parsing

The string methods `to_int()` and `to_float()` return nullable types:

```
fn main() int? {
    let n = "42".to_int()?          // int? -> unwrap to int
    print(n)                         // 42

    let f = "3.14".to_float()?      // float? -> unwrap to float
    print(f)                         // 3.140000

    let bad = "abc".to_int()        // returns none
    return none
}
```

## Nullable Classes

Class instances can be nullable too:

```
class Point {
    x: int
    y: int
}

fn find_origin() Point? {
    return Point { x: 0, y: 0 }
}

fn find_nothing() Point? {
    return none
}

fn use_point() int? {
    let p = find_origin()?
    return p.x + p.y
}
```

## Restrictions

- **No nested nullables**: `int??` is a compile error
- **No `void?`**: void cannot be nullable
- **`?` requires nullable context**: using `?` on a non-nullable expression is a compile error
- **`?` requires nullable return**: the enclosing function must return `T?` to use `?`

# Nullable Types

Every mainstream language has gotten null wrong at least once.

Go has `nil`. Any pointer, interface, slice, map, or channel can be nil. The compiler does not help -- a nil dereference is a runtime panic.

Java has `null`. Any reference type can be null at any time. `NullPointerException` is the most common crash in Java's history. Kotlin fixed this with `String?` vs `String`, but it required a new language.

Rust has `Option<T>`. Correct, but ceremonious. Every nullable value needs `Option<T>`, every access needs `match` or `if let Some(x)` or `.map()`. Safety at the cost of verbosity.

Pluto treats nullability as a first-class type modifier: compiler-enforced, minimal syntax.

## `T?`, `none`, and Nullable Returns

Any type becomes nullable by appending `?`. The literal `none` represents absence:

```
let x: int? = none
let name: string? = "Alice"
let p: Point? = Point { x: 1, y: 2 }
```

Functions declare nullable return types with `T?`. Returning a concrete `T` coerces implicitly to `T?`:

```
fn find_positive(x: int) int? {
    if x > 0 {
        return x
    }
    return none
}
```

## The `?` Operator

The `?` postfix operator unwraps a nullable. If `none`, it early-returns `none` from the enclosing function:

```
fn double_it() int? {
    let x = get_value()?    // returns none if get_value() returned none
    return x * 2
}
```

This mirrors how `!` works for error propagation. Multiple `?` calls chain with short-circuit semantics:

```
fn pipeline() int? {
    let a = step1()?       // returns none if step1 returned none
    let b = step2(a)?      // returns none if step2 returned none
    return b
}
```

When `step1()` returns `none`, `step2` is never called.

## `?` in Void Functions

In void functions, `?` acts as an early return instead of returning `none`:

```
fn process(line: string?) {
    let value = line?      // if none, return from process()
    print(value)
}

fn main() {
    process("hello")       // prints "hello"
    process(none)           // silently returns
    print("done")          // prints "done"
}
```

## Coercion Rules

`T` is assignable to `T?` (implicit wrap). `T?` is NOT assignable to `T` -- you must unwrap with `?`:

```
fn takes_int(x: int) { }

fn main() int? {
    let x: int? = 42
    takes_int(x)            // COMPILE ERROR: int? is not int
    takes_int(x?)           // OK: unwraps or early-returns none
    return none
}
```

This applies uniformly to parameters, classes, and all other types:

```
fn unwrap_it(x: int?) int? {
    let val = x?
    return val + 1
}

fn maybe_point() Point? {
    return Point { x: 1, y: 2 }
}

fn use_it() int? {
    let p = maybe_point()?
    return p.x + p.y
}
```

## String Parsing

The built-in `to_int()` and `to_float()` return nullable types rather than raising errors:

```
let n = "42".to_int()       // int?
let f = "3.14".to_float()   // float?
let bad = "abc".to_int()    // int? (none)
```

## Restrictions

- **No nested nullables.** `int??` is a compile error.
- **No `void?`.** Void cannot be nullable.
- **`?` requires a nullable operand.** Using `?` on a non-nullable is a compile error.
- **`?` requires a compatible return type.** The enclosing function must return `T?` or `void`.

## Comparison

| | Go | Java | Kotlin | Rust | Pluto |
|---|---|---|---|---|---|
| Nullable syntax | implicit | implicit | `String?` | `Option<String>` | `string?` |
| Null literal | `nil` | `null` | `null` | `None` | `none` |
| Safe access | none | none | `?.` | `.map()` / `if let` | `?` |
| Compiler enforced | No | No | Yes | Yes | Yes |
| Verbosity | Low | Low | Low | High | Low |

Pluto's nullable types give you Kotlin's ergonomics with Rust's safety guarantees, without the ceremony of `Option<T>` and `match`.

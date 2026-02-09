# Variables and Types

## Declaring Variables

Variables are declared with `let`:

```
let x = 42
let name = "pluto"
let pi = 3.14
let active = true
```

Pluto infers types from the value on the right-hand side. You can also annotate types explicitly:

```
let x: int = 42
let name: string = "pluto"
```

## Mutability

Variables are mutable by default. You can reassign them freely:

```
fn main() {
    let x = 1
    print(x)        // 1
    x = 2
    print(x)        // 2
    x = x + 10
    print(x)        // 12
}
```

## Types

Pluto has four primitive types:

| Type | Description | Examples |
|------|-------------|---------|
| `int` | 64-bit signed integer | `42`, `-1`, `0` |
| `float` | 64-bit floating point | `3.14`, `-0.5` |
| `bool` | Boolean | `true`, `false` |
| `string` | Heap-allocated string | `"hello"`, `""` |

There's also `void`, which is the return type of functions that don't return a value.

## Operators

**Arithmetic:**

```
let sum = 10 + 3       // 13
let diff = 10 - 3      // 7
let prod = 10 * 3      // 30
let quot = 10 / 3      // 3 (integer division)
let rem = 10 % 3       // 1
```

**Comparison:**

```
print(5 > 3)           // true
print(5 >= 5)          // true
print(3 < 5)           // true
print(3 <= 3)          // true
print(5 == 5)          // true
print(5 != 3)          // true
```

**Logical:**

```
print(true && false)   // false
print(true || false)   // true
print(!true)           // false
```

**Negation:**

```
let x = 5
let y = -x             // -5
```

## Type Conversions

Pluto does not have implicit type coercion. You cannot mix `int` and `float` in arithmetic, and you cannot use a non-boolean as a condition.

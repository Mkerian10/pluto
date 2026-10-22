# Syntax at a Glance

Compressed reference for variables, functions, operators, control flow, and builtins. No semicolons -- Pluto uses newline-based statement termination. Comments with `//` only (no block comments).

## Variables

```pluto
let x = 42              // inferred as int
let x: int = 42         // explicit type annotation
let pi = 3.14           // float (f64)
let name = "alice"      // string
let flag = true         // bool
let b = (65).to_byte()  // byte (0-255, unsigned)
```

Variables are **immutable by default** -- reassigning a plain `let` is a compile error. Declare with `let mut` to allow reassignment:

```pluto
let mut x = 1
x = 2
x = x + 10
```

## Primitive Types

| Type     | Size | Description                     |
|----------|------|---------------------------------|
| `int`    | i64  | 64-bit signed integer           |
| `float`  | f64  | 64-bit floating point           |
| `bool`   | i8   | `true` or `false`               |
| `string` | heap | Heap-allocated, immutable chars |
| `byte`   | u8   | Unsigned 0-255                  |

Numeric literals support underscores: `1_000_000`, `1_000.50`. Hex literals: `0xFF`, `0x0A`.

## Type Conversions

Explicit only, as named methods. No implicit coercion and no `as` cast. Widening conversions are total; a conversion that can lose information returns a nullable, which the compiler narrows to a plain value where it can prove the input is in range (a literal, a guard, a mask).

```pluto
(42).to_float()          // int -> float
(3.14).to_int()          // float -> int?  (none for NaN/inf/out of range; truncates toward zero)
true.to_int()            // bool -> int (true = 1, false = 0)
n != 0                   // int -> bool is just a comparison
(255).to_byte()          // int -> byte?  (none outside 0..255; a literal narrows to byte)
(300).low_byte()         // int -> byte, keeping the low 8 bits by name
b.to_int()               // byte -> int (zero-extends)
"42".to_int()            // string -> int?  (none if it does not parse)
```

## Operators

**Arithmetic** (int and float):

| Op  | Meaning             | Compound |
|-----|---------------------|----------|
| `+` | add / string concat | `+=`     |
| `-` | subtract            | `-=`     |
| `*` | multiply            | `*=`     |
| `/` | divide              | `/=`     |
| `%` | modulo              | `%=`     |

**Increment/decrement** (int only): `x++`, `x--`

**Comparison** (returns `bool`): `==`, `!=`, `<`, `>`, `<=`, `>=`

**Logical** (bool only): `&&`, `||`, `!`

**Bitwise** (int only): `&` (AND), `|` (OR), `^` (XOR), `~` (NOT), `<<` (shl), `>>` (shr). Precedence: `&` > `^` > `|`.

## Functions

```pluto
fn add(a: int, b: int) int {
    return a + b
}

fn greet(name: string) {
    print(f"hello {name}")
}
```

Void functions omit the return type. All non-void paths require an explicit `return` -- no implicit return of the last expression. Trailing commas allowed in argument lists.

## String Interpolation

Arbitrary expressions inside `{}` within an `f`-prefixed string. A plain `"..."` string never interpolates -- `f"{x}"` is the literal text `{x}`.

```pluto
print(f"hello {name}")          // hello alice
print(f"{a} + {b} = {a + b}")   // 1 + 2 = 3
print(f"use {{braces}}")        // use {braces}  (escape with doubling)
```

Supports int, float, bool, string, byte. Class instances cannot be interpolated.

**Escape sequences:** `\n`, `\r`, `\t`, `\\`, `\"`

## Control Flow

**if / else if / else:**

```pluto
if x > 10 {
    print("big")
} else if x > 0 {
    print("small")
} else {
    print("non-positive")
}
```

**while:**

```pluto
let i = 0
while i < 10 {
    print(i)
    i++
}
```

**for-in** -- arrays, ranges, and strings:

```pluto
for x in [1, 2, 3] {           // array iteration
    print(x)
}

for i in 0..10 {               // exclusive range: 0..9
    print(i)
}

for i in 0..=10 {              // inclusive range: 0..10
    print(i)
}

for c in "abc" {               // string: yields single-char strings
    print(c)
}
```

Range endpoints can be expressions: `for i in 0..(n * 2) { ... }`

**break / continue:** work in both `while` and `for`. `break` exits the innermost loop. Cannot be used inside closures.

## Builtins

| Function | Signature | Notes |
|----------|-----------|-------|
| `print(val)` | any printable | Prints with newline |
| `abs(x)` | int/float | Absolute value |
| `min(a, b)` | int/float | Minimum |
| `max(a, b)` | int/float | Maximum |
| `pow(base, exp)` | int/float | `pow(int, negative)` raises `MathError` |
| `sqrt(x)` | float | Square root |
| `floor(x)` | float | Floor |
| `ceil(x)` | float | Ceiling |
| `round(x)` | float | Round to nearest |
| `sin(x)`, `cos(x)`, `tan(x)` | float | Trig (radians) |
| `log(x)` | float | Natural logarithm |
| `time_ns()` | -> int | Monotonic nanosecond timestamp |

Builtin names are reserved -- user functions cannot shadow them.

## Quick Comparison

| Concept | Go | Rust | Pluto |
|---------|-----|------|-------|
| Variable | `x := 42` | `let mut x = 42;` | `let mut x = 42` |
| Type annotation | `var x int = 42` | `let x: i64 = 42;` | `let x: int = 42` |
| String interp | `fmt.Sprintf("hi %s", n)` | `format!("hi {n}")` | `f"hi {n}"` |
| Range loop | `for i := 0; i < 10; i++` | `for i in 0..10` | `for i in 0..10` |
| No return value | implicit | implicit | omit return type |
| Terminator | newline/`;` | `;` | newline |
| Mutability | mutable default | immutable default | immutable default |

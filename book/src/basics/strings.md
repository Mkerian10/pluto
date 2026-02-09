# Strings

## String Literals

Strings are enclosed in double quotes:

```
let greeting = "hello, world"
```

## Concatenation

Use `+` to concatenate strings:

```
let first = "hello"
let second = " world"
let combined = first + second
print(combined)     // "hello world"
```

## Length

Every string has a `.len()` method:

```
print("hello".len())    // 5
print("".len())          // 0
```

## String Interpolation

Pluto supports embedding expressions inside strings with `{expr}`:

```
let name = "alice"
print("hello, {name}")          // "hello, alice"

let x = 42
print("the answer is {x}")     // "the answer is 42"
```

Interpolation works with any expression, not just variables:

```
let a = 3
let b = 4
print("{a} + {b} = {a + b}")   // "3 + 4 = 7"
```

You can call methods inside interpolation:

```
let s = "hello"
print("length is {s.len()}")   // "length is 5"
```

## Escape Sequences

Pluto supports standard escape sequences in strings:

| Sequence | Character |
|----------|-----------|
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\\` | Backslash |
| `\"` | Double quote |

```
print("line one\nline two")
print("column\tone\ttwo")
print("she said \"hello\"")
```

## Comparison

Strings can be compared with `==` and `!=`:

```
let a = "hello"
let b = "hello"
print(a == b)       // true
print(a != "world") // true
```

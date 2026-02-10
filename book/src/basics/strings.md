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

## Built-in Methods

Strings have built-in methods for common operations:

### Searching

```
let s = "hello, world"
print(s.contains("world"))     // true
print(s.starts_with("hello"))  // true
print(s.ends_with("world"))    // true
print(s.index_of("world"))     // 7
print(s.index_of("xyz"))       // -1
```

### Transforming

```
let s = "  Hello, World  "
print(s.trim())                // "Hello, World"
print(s.trim().to_upper())     // "HELLO, WORLD"
print(s.trim().to_lower())     // "hello, world"
print("aabbcc".replace("bb", "XX"))  // "aaXXcc"
```

### Extracting

```
let s = "hello"
print(s.substring(1, 3))      // "ell"
print(s.char_at(0))           // "h"
```

`substring(start, len)` takes a start index and a length. Out-of-range values are clamped (never aborts).

`char_at(index)` returns a single-character string. Out-of-bounds aborts (same as indexing).

### Splitting

```
let csv = "a,b,c"
let parts = csv.split(",")    // ["a", "b", "c"]
for part in parts {
    print(part)
}
```

### Method Chaining

Methods can be chained naturally:

```
let input = "  Hello World  "
let result = input.trim().to_lower().replace(" ", "-")
print(result)   // "hello-world"
```

## Indexing

Access individual characters by index. Returns a single-character string:

```
let s = "hello"
print(s[0])     // "h"
print(s[4])     // "o"
```

Out-of-bounds indexing aborts at runtime (same as arrays). Strings are immutable -- `s[0] = "x"` is a compile error.

## Iterating

Use `for` to iterate over each character:

```
let s = "hello"
for c in s {
    print(c)    // "h", "e", "l", "l", "o"
}
```

Each character `c` is a single-character `string`.

## Comparison

Strings can be compared with `==` and `!=`:

```
let a = "hello"
let b = "hello"
print(a == b)       // true
print(a != "world") // true
```

## Byte Semantics

All string operations work on raw bytes, not Unicode code points. For ASCII text this makes no difference. For multi-byte UTF-8 characters, `len()`, indexing, and iteration operate on individual bytes.

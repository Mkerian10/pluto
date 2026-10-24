# std.strings

String utility functions. Pluto strings are immutable; all operations return new strings.

```
import std.strings
```

## Functions

### substring

```
strings.substring(s: string, start: int, length: int) string
```

Returns `length` characters starting at byte offset `start`.

```
strings.substring("hello, world", 0, 5)     // "hello"
strings.substring("hello, world", 7, 5)     // "world"
```

### contains

```
strings.contains(s: string, needle: string) bool
```

```
strings.contains("hello world", "world")    // true
strings.contains("hello world", "xyz")       // false
```

### starts_with / ends_with

```
strings.starts_with(s: string, prefix: string) bool
strings.ends_with(s: string, suffix: string) bool
```

```
strings.starts_with("hello", "hel")    // true
strings.ends_with("hello", "llo")      // true
```

### index_of

```
strings.index_of(s: string, needle: string) int
```

Returns the byte offset of the first occurrence, or `-1` if not found.

```
strings.index_of("hello", "ll")     // 2
strings.index_of("hello", "xyz")    // -1
```

### trim

```
strings.trim(s: string) string
```

Strips leading and trailing whitespace.

```
strings.trim("  hello  ")    // "hello"
```

### trim_start / trim_end

```
strings.trim_start(s: string) string
strings.trim_end(s: string) string
```

Strip whitespace from one end only.

```
strings.trim_start("  hi  ")    // "hi  "
strings.trim_end("  hi  ")      // "  hi"
```

### to_upper / to_lower

```
strings.to_upper(s: string) string
strings.to_lower(s: string) string
```

```
strings.to_upper("hello")    // "HELLO"
strings.to_lower("HELLO")    // "hello"
```

Only ASCII letters change case; every other character is copied unchanged.

### replace

```
strings.replace(s: string, old: string, new_str: string) string
```

Replaces all occurrences of `old` with `new_str`.

```
strings.replace("hello world", "world", "pluto")    // "hello pluto"
```

### split

```
strings.split(s: string, delim: string) [string]
```

Splits into an array by delimiter.

```
let parts = strings.split("a,b,c", ",")
// ["a", "b", "c"]
```

### join

```
strings.join(arr: [string], sep: string) string
```

Joins an array of strings with a separator.

```
strings.join(["a", "b", "c"], ", ")    // "a, b, c"
```

### char_at

```
strings.char_at(s: string, index: int) string
```

Returns the character at byte offset `index` as a single-character string.

```
strings.char_at("hello", 0)    // "h"
strings.char_at("hello", 4)    // "o"
```

### byte_at

```
strings.byte_at(s: string, index: int) int
```

Returns the raw byte value at the given offset.

```
strings.byte_at("A", 0)    // 65
```

### len

```
strings.len(s: string) int
```

Returns the byte length of the string. Equivalent to `s.len()`.

```
strings.len("hello")    // 5
```

### format_float

```
strings.format_float(v: float) string
```

Formats a float as a string with decimal representation.

### last_index_of

```
strings.last_index_of(s: string, needle: string) int
```

Returns the index of the last occurrence of `needle`, or `-1` if absent.

```
strings.last_index_of("a/b/c", "/")    // 3
```

### count

```
strings.count(s: string, needle: string) int
```

Counts non-overlapping occurrences of `needle`.

```
strings.count("banana", "an")    // 2
```

### is_empty / is_whitespace

```
strings.is_empty(s: string) bool
strings.is_whitespace(s: string) bool
```

`is_empty` is true for `""`. `is_whitespace` is true when the string is empty or contains only whitespace.

### repeat

```
strings.repeat(s: string, count: int) string
```

Concatenates `count` copies of `s`; `0` or less gives `""`.

```
strings.repeat("ab", 3)    // "ababab"
```

### compare

```
strings.compare(a: string, b: string) int
```

Lexicographic byte-wise comparison: `-1` if `a` sorts first, `0` if equal, `1` if `a` sorts after `b`. A proper prefix sorts first, and bytes compare unsigned, so for UTF-8 text this is code-point order.

```
strings.compare("ab", "abc")    // -1
strings.compare("b", "a")       // 1
```

### parse_int / parse_float

```
strings.parse_int(s: string) int       // raises strings.ParseError
strings.parse_float(s: string) float   // raises strings.ParseError
```

Strict parsers that raise `ParseError { input, message }` instead of returning `none` (compare `s.to_int()` / `s.to_float()`). `parse_int` accepts an optional sign and ASCII digits over the full int range, and rejects whitespace anywhere (trim first for padded input) and out-of-range values. `parse_float` accepts `[+-]? DIGITS ('.' DIGITS)? ([eE] [+-]? DIGITS)?`; `.5`, `5.`, `inf`, `nan` and values too large for a finite float are rejected.

```
let n = strings.parse_int("-42")!          // -42
let x = strings.parse_float("2.5e-3")!     // 0.0025
let bad = strings.parse_int(" 7") catch err: strings.ParseError {
    print(err.message)                     // invalid character at index 0
    0
}
```

## Example: CSV Parsing

```
import std.strings

fn main() {
    let line = "Alice,30,Engineering"
    let fields = strings.split(line, ",")
    let name = fields[0]
    let dept = strings.to_upper(fields[2])
    print(f"{name} works in {dept}")
    // Alice works in ENGINEERING
}
```

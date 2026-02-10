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

### to_upper / to_lower

```
strings.to_upper(s: string) string
strings.to_lower(s: string) string
```

```
strings.to_upper("hello")    // "HELLO"
strings.to_lower("HELLO")    // "hello"
```

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

## Example: CSV Parsing

```
import std.strings

fn main() {
    let line = "Alice,30,Engineering"
    let fields = strings.split(line, ",")
    let name = fields[0]
    let dept = strings.to_upper(fields[2])
    print("{name} works in {dept}")
    // Alice works in ENGINEERING
}
```

# std.strings

The `std.strings` module provides utility functions for working with strings.

```
import std.strings
```

## Functions

### substring

```
strings.substring(s: string, start: int, length: int) string
```

Returns a substring starting at `start` with the given `length`:

```
let s = "hello, world"
print(strings.substring(s, 0, 5))      // "hello"
print(strings.substring(s, 7, 5))      // "world"
```

### contains

```
strings.contains(s: string, needle: string) bool
```

Returns `true` if `s` contains `needle`:

```
print(strings.contains("hello world", "world"))   // true
print(strings.contains("hello world", "xyz"))      // false
```

### starts_with / ends_with

```
strings.starts_with(s: string, prefix: string) bool
strings.ends_with(s: string, suffix: string) bool
```

```
print(strings.starts_with("hello", "hel"))    // true
print(strings.ends_with("hello", "llo"))       // true
```

### index_of

```
strings.index_of(s: string, needle: string) int
```

Returns the index of the first occurrence, or `-1` if not found:

```
print(strings.index_of("hello", "ll"))    // 2
print(strings.index_of("hello", "xyz"))   // -1
```

### trim

```
strings.trim(s: string) string
```

Removes leading and trailing whitespace:

```
print(strings.trim("  hello  "))    // "hello"
```

### to_upper / to_lower

```
strings.to_upper(s: string) string
strings.to_lower(s: string) string
```

```
print(strings.to_upper("hello"))    // "HELLO"
print(strings.to_lower("HELLO"))    // "hello"
```

### replace

```
strings.replace(s: string, old: string, new_str: string) string
```

Replaces all occurrences of `old` with `new_str`:

```
print(strings.replace("hello world", "world", "pluto"))
// "hello pluto"
```

### split

```
strings.split(s: string, delim: string) [string]
```

Splits a string into an array by delimiter:

```
let parts = strings.split("a,b,c", ",")
for p in parts {
    print(p)
}
// prints: a, b, c
```

### char_at

```
strings.char_at(s: string, index: int) string
```

Returns the character at the given index as a single-character string:

```
print(strings.char_at("hello", 0))    // "h"
print(strings.char_at("hello", 4))    // "o"
```

### len

```
strings.len(s: string) int
```

Returns the length of the string:

```
print(strings.len("hello"))    // 5
```

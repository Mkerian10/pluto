# Strings and Collections

Pluto's built-in collection types: strings, byte buffers, arrays, maps, and sets. Plus `std.collections` for functional operations on arrays.

## Strings

String literals use double quotes. Concatenation with `+`. Interpolation with `{}`.

```pluto
let s = "hello " + "world"
let name = "alice"
print("hello {name}")           // hello alice
print("{1 + 2}")                // 3
print("use {{braces}}")         // use {braces}  (escape with doubling)
```

**Escape sequences:** `\n`, `\r`, `\t`, `\\`, `\"`

**Indexing** returns a single-character string (read-only; `s[0] = "x"` is a compile error):

```pluto
let s = "hello"
print(s[0])                     // h
```

**Iteration** yields single-character strings:

```pluto
for c in "abc" {
    print(c)                    // a, b, c
}
```

### String Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `.len()` | `() int` | Length in bytes |
| `.contains(sub)` | `(string) bool` | Substring check |
| `.starts_with(pre)` | `(string) bool` | Prefix check |
| `.ends_with(suf)` | `(string) bool` | Suffix check |
| `.index_of(sub)` | `(string) int` | First occurrence, or -1 |
| `.trim()` | `() string` | Strip leading/trailing whitespace |
| `.to_upper()` | `() string` | Uppercase copy |
| `.to_lower()` | `() string` | Lowercase copy |
| `.replace(old, new)` | `(string, string) string` | Replace all occurrences |
| `.split(delim)` | `(string) [string]` | Split into array; `""` splits by char |
| `.substring(offset, len)` | `(int, int) string` | Substring by offset and length |
| `.char_at(idx)` | `(int) string` | Single character at index |
| `.to_int()` | `() int?` | Parse as integer, `none` on failure |
| `.to_float()` | `() float?` | Parse as float, `none` on failure |
| `.to_bytes()` | `() bytes` | Convert to byte buffer |

`to_int()` and `to_float()` return nullable types. Use `?` to propagate:

```pluto
fn parse(s: string) int? {
    let v = s.to_int()?
    return v * 2
}
```

Method chaining works naturally: `"  Hello  ".trim().to_lower().contains("hello")` returns `true`.

## Bytes

The `byte` type is an unsigned 8-bit integer (0-255). No byte literal -- use `as byte` to cast from int.

```pluto
let b = 0xFF as byte
print(b as int)                 // 255
```

Byte ordering is **unsigned**: `0xFF as byte > 0x7F as byte` is `true`.

### Byte Buffers

Create with `bytes_new()`. Type is `bytes`. Supports `.push()`, `.len()`, indexing (read/write), and iteration.

```pluto
let buf = bytes_new()
buf.push(72 as byte)
buf.push(105 as byte)
print(buf.len())                // 2
print(buf[0] as int)            // 72
buf[0] = 90 as byte             // index write

for b in buf {                  // iteration yields byte values
    print(b as int)
}
```

**String conversion** -- roundtrips cleanly:

```pluto
let buf = "Hello".to_bytes()
let s = buf.to_string()         // "Hello"
```

## Arrays

Literal syntax with type `[T]`. All elements must have the same type.

```pluto
let nums = [1, 2, 3]           // [int]
let names = ["a", "b"]         // [string]
```

**Indexing** (read and write) and **iteration**:

```pluto
let a = [10, 20, 30]
print(a[0])                     // 10
a[1] = 99                       // index write

for x in a {
    print(x)
}
```

### Array Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `.len()` | `() int` | Element count |
| `.push(val)` | `(T)` | Append to end |
| `.pop()` | `() T` | Remove and return last |
| `.first()` | `() T` | First element |
| `.last()` | `() T` | Last element |
| `.is_empty()` | `() bool` | Length == 0 check |
| `.clear()` | `()` | Remove all elements |
| `.contains(val)` | `(T) bool` | Element presence |
| `.index_of(val)` | `(T) int` | First index, or -1 |
| `.remove_at(idx)` | `(int) T` | Remove at index, shift left |
| `.insert_at(idx, val)` | `(int, T)` | Insert at index, shift right |
| `.slice(start, end)` | `(int, int) [T]` | Sub-array [start, end) |
| `.reverse()` | `()` | Reverse in place |

Arrays work as function parameters (`fn f(a: [int])`) and return values (`fn f() [int]`).

## Maps

Hash maps with typed keys and values. Keys must be hashable: `int`, `float`, `bool`, `string`, `byte`, or enum.

```pluto
let m = Map<string, int> { "a": 1, "b": 2 }
let empty = Map<string, int> {}

print(m["a"])                   // 1
m["c"] = 3                      // insert via index
m["a"] = 99                     // overwrite
```

### Map Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `.len()` | `() int` | Number of entries |
| `.insert(key, val)` | `(K, V)` | Insert or overwrite |
| `.contains(key)` | `(K) bool` | Key presence |
| `.remove(key)` | `(K)` | Remove entry |
| `.keys()` | `() [K]` | Array of all keys |
| `.values()` | `() [V]` | Array of all values |

**Iteration** via `.keys()` or `.values()`:

```pluto
for k in m.keys() {
    print("{k}: {m[k]}")
}
```

## Sets

Unordered collections with unique elements. Same hashable constraint as map keys.

```pluto
let s = Set<int> { 1, 2, 3 }
let empty = Set<string> {}
```

### Set Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `.len()` | `() int` | Number of elements |
| `.insert(val)` | `(T)` | Add element (no-op if present) |
| `.contains(val)` | `(T) bool` | Membership check |
| `.remove(val)` | `(T)` | Remove element |
| `.to_array()` | `() [T]` | Convert to array (for iteration) |

Duplicate inserts are silently ignored. Iterate via `.to_array()`:

```pluto
for x in s.to_array() {
    print(x)
}
```

## std.collections

Functional operations on arrays. Import with `import std.collections`.

```pluto
import std.collections

fn main() {
    let nums = [1, 2, 3, 4, 5]
    let doubled = collections.map(nums, (x: int) => x * 2)
    let evens = collections.filter(nums, (x: int) => x % 2 == 0)
    let total = collections.fold(nums, 0, (acc: int, x: int) => acc + x)
}
```

### Function Reference

| Function | Signature | Description |
|----------|-----------|-------------|
| `map(arr, f)` | `([T], fn(T) U) [U]` | Transform each element |
| `filter(arr, f)` | `([T], fn(T) bool) [T]` | Keep matching elements |
| `fold(arr, init, f)` | `([T], U, fn(U, T) U) U` | Left fold with accumulator |
| `reduce(arr, f)` | `([T], fn(T, T) T) T` | Fold using first element as seed |
| `any(arr, f)` | `([T], fn(T) bool) bool` | Any element matches |
| `all(arr, f)` | `([T], fn(T) bool) bool` | All elements match |
| `count(arr, f)` | `([T], fn(T) bool) int` | Count matching elements |
| `flat_map(arr, f)` | `([T], fn(T) [U]) [U]` | Map then flatten |
| `for_each(arr, f)` | `([T], fn(T))` | Side-effecting iteration |
| `reverse(arr)` | `([T]) [T]` | New reversed array |
| `take(arr, n)` | `([T], int) [T]` | First n elements |
| `drop(arr, n)` | `([T], int) [T]` | Skip first n elements |
| `zip(a, b)` | `([A], [B]) [Pair<A, B>]` | Pair elements by index |
| `enumerate(arr)` | `([T]) [Pair<int, T>]` | Pair each element with its index |
| `flatten(arr)` | `([[T]]) [T]` | Flatten one level of nesting |
| `sum(arr)` | `([int]) int` | Sum of integers |
| `sum_float(arr)` | `([float]) float` | Sum of floats |

`zip` and `enumerate` return `Pair<A, B>` (from `std.collections`) with fields `first` and `second`:

```pluto
import std.collections

fn main() {
    let pairs = collections.enumerate(["alice", "bob"])
    for p in pairs {
        print("{p.first}: {p.second}")  // 0: alice, 1: bob
    }
}
```

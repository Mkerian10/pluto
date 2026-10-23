/// Pluto language reference documentation and stdlib docs.

pub fn get_docs(topic: Option<&str>) -> String {
    match topic {
        None => full_reference(),
        Some(t) => match t.to_lowercase().as_str() {
            "types" => types_doc(),
            "operators" => operators_doc(),
            "statements" => statements_doc(),
            "declarations" => declarations_doc(),
            "strings" => strings_doc(),
            "errors" => errors_doc(),
            "closures" => closures_doc(),
            "generics" => generics_doc(),
            "modules" => modules_doc(),
            "contracts" => contracts_doc(),
            "concurrency" => concurrency_doc(),
            "gotchas" => gotchas_doc(),
            _ => format!(
                "Unknown topic: `{t}`. Available topics: types, operators, statements, \
                 declarations, strings, errors, closures, generics, modules, contracts, \
                 concurrency, gotchas"
            ),
        },
    }
}

pub fn get_stdlib_docs(module: Option<&str>) -> Result<String, String> {
    match module {
        None => Ok(stdlib_overview()),
        Some(m) => match m.to_lowercase().as_str() {
            "strings" => Ok(stdlib_strings()),
            "math" => Ok(stdlib_math()),
            "fs" => Ok(stdlib_fs()),
            "json" => Ok(stdlib_json()),
            "http" => Ok(stdlib_http()),
            "net" => Ok(stdlib_net()),
            "socket" => Ok(stdlib_socket()),
            "collections" => Ok(stdlib_collections()),
            "io" => Ok(stdlib_io()),
            "random" => Ok(stdlib_random()),
            "time" => Ok(stdlib_time()),
            "compress" => Ok(stdlib_compress()),
            _ => Err(format!(
                "Unknown stdlib module: `{m}`. Available: strings, math, fs, json, http, \
                 net, socket, collections, io, random, time, compress"
            )),
        },
    }
}

// ---------------------------------------------------------------------------
// Full reference (all topics concatenated)
// ---------------------------------------------------------------------------

fn full_reference() -> String {
    [
        "# Pluto Language Reference\n",
        &types_doc(),
        "\n---\n",
        &operators_doc(),
        "\n---\n",
        &statements_doc(),
        "\n---\n",
        &declarations_doc(),
        "\n---\n",
        &strings_doc(),
        "\n---\n",
        &errors_doc(),
        "\n---\n",
        &closures_doc(),
        "\n---\n",
        &generics_doc(),
        "\n---\n",
        &modules_doc(),
        "\n---\n",
        &contracts_doc(),
        "\n---\n",
        &concurrency_doc(),
        "\n---\n",
        &gotchas_doc(),
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// Individual topic docs
// ---------------------------------------------------------------------------

fn types_doc() -> String {
    r#"## Types

### Primitive types
- `int` — 64-bit signed integer
- `float` — 64-bit IEEE 754 floating point
- `bool` — boolean (`true` / `false`)
- `string` — heap-allocated UTF-8 string
- `void` — no value (used as return type)

### Composite types
- `[T]` — array of `T` (e.g., `[int]`, `[string]`)
- `Map<K, V>` — hash map. Keys must be hashable: int, float, bool, string, enum
- `Set<T>` — hash set. Elements must be hashable
- `T?` — nullable type. Can hold a value of type `T` or `none`
- `Task<T>` — handle to a spawned concurrent task returning `T`
- `fn(P1, P2) R` — function/closure type

### User-defined types
- `class` — nominal class with fields, methods, and optional DI bracket deps
- `enum` — tagged union with unit and data-carrying variants
- `trait` — structural interface (duck typing)
- `error` — typed error declaration

### Literals
- `42`, `-7` — int
- `3.14`, `-0.5` — float
- `true`, `false` — bool
- `"hello"` — string; `f"hello {name}"` interpolates (plain `"..."` never does)
- `[1, 2, 3]` — array
- `Map<string, int> { "a": 1, "b": 2 }` — map literal
- `Set<int> { 1, 2, 3 }` — set literal
- `none` — null value (for nullable types)
- `1..10` — range (exclusive end), `1..=10` — range (inclusive end)"#
        .to_string()
}

fn operators_doc() -> String {
    r#"## Operators

### Arithmetic
`+`, `-`, `*`, `/`, `%` (modulo)

Signed 64-bit overflow on `+`, `-`, `*`, unary `-`, and `int.min / -1` is a
**defect**: the program prints `pluto: defect: integer overflow in ...` and
aborts. Division and modulo by zero are defects too. Defects are not typed
errors and cannot be caught. Deliberate modular (mod 2^64) arithmetic must
be visible: use the builtins `wrapping_add(a, b)`, `wrapping_sub(a, b)`,
`wrapping_mul(a, b)` (int, int) -> int, which wrap two's-complement and
never trap.

### Comparison
`==`, `!=`, `<`, `>`, `<=`, `>=`

### Logical
- `&&` — logical AND
- `||` — logical OR
- `!expr` — logical NOT (prefix only)

### Bitwise (int only)
`&`, `|`, `^`, `~` (bitwise NOT), `<<`, `>>`

### Assignment
`=`, `+=`, `-=`, `*=`, `/=`

### Type cast
`expr as type` — postfix cast. Allowed: int↔float, int↔bool

### Error propagation (postfix)
`expr!` — propagate error to caller (like Rust's `?`). Only in fallible functions.

### Null propagation (postfix)
`expr?` — unwrap nullable, early-return `none` if null. Only in functions returning `T?`.

### Indexing
- `arr[i]` — array/string index (0-based)
- `map[key]` — map lookup
- `map[key] = val` — map insert/update

### Method call
`expr.method(args)` — call method on object

### Precedence (high to low)
1. Postfix: `.method()`, `[index]`, `!`, `?`, `as`
2. Prefix: `!`, `-`, `~`
3. Multiplicative: `*`, `/`, `%`
4. Additive: `+`, `-`
5. Shift: `<<`, `>>`
6. Bitwise AND: `&`
7. Bitwise XOR: `^`
8. Bitwise OR: `|`
9. Comparison: `<`, `>`, `<=`, `>=`
10. Equality: `==`, `!=`
11. Logical AND: `&&`
12. Logical OR: `||`

**Important:** `!expr.method()` parses as `(!expr).method()`, not `!(expr.method())`. Use parentheses: `!(expr.method())`."#
        .to_string()
}

fn statements_doc() -> String {
    r#"## Statements

No semicolons — statements are newline-terminated.

### Variable binding
```
let x = 42
let name: string = "hello"
let mut counter = 0
```

### If / else if / else
```
if x > 0 {
    print("positive")
} else if x == 0 {
    print("zero")
} else {
    print("negative")
}
```
Note: `else if` is supported (not `elif` or `elsif`).

### While loop
```
while condition {
    // body
}
```

### For-in loop
```
for item in array {
    print(item)
}
for i in 0..10 {
    print(i)
}
for i in 0..=10 {
    // inclusive range
}
```

### Match (pattern matching)
```
match value {
    Enum.Variant1 => expr
    Enum.Variant2 { field } => expr
}
```
Match requires exhaustive coverage of all variants.

### Return
```
return value
return
```

### Break / Continue
```
break
continue
```

### Raise (throw error)
```
raise MyError { message: "something went wrong" }
```

### Assignment
```
x = new_value
arr[i] = val
map[key] = val
obj.field = val
```"#
    .to_string()
}

fn declarations_doc() -> String {
    r#"## Declarations

### Functions
```
fn add(a: int, b: int) int {
    return a + b
}

pub fn greet(name: string) string {
    return f"hello {name}"
}
```
- `pub` makes the function visible to other modules
- Return type goes after the parameter list
- Omit return type for `void` functions

### Generators
```
fn evens(limit: int) stream int {
    for i in 0..limit {
        if i % 2 == 0 {
            yield i
        }
    }
}

for n in evens(10) { print(n) }   // 0 2 4 6 8
```
- A `stream T` return type makes a function a generator; calling it runs nothing until iterated
- Each `for` iteration runs the body to the next `yield`; the stream ends at the end of the body or a bare `return` (no `return` needed)
- `yield` works inside `if`, `while`, `for` and `match` arms, but not inside closures
- Streams are lazy (infinite generators are fine; `break` stops pulling) and single-pass (call again for a fresh stream)
- Generators can be generic (`fn each<T>(xs: [T]) stream T`) and take streams as parameters
- Generators must be top-level functions, not methods

### Classes
```
class Point {
    x: float
    y: float

    fn distance(self, other: Point) float {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return sqrt(dx * dx + dy * dy)
    }

    fn move_by(mut self, dx: float, dy: float) {
        self.x = self.x + dx
        self.y = self.y + dy
    }
}
```
- Fields declared at top of class body
- Methods use `self` (immutable) or `mut self` (mutable) as first param
- Construct with `Point { x: 1.0, y: 2.0 }`
- No inheritance — use traits for polymorphism

### Classes with DI (bracket deps)
```
class UserService[db: Database, cache: CacheService] {
    fn get_user(self, id: int) User {
        // db and cache are injected fields
    }
}
```

### Enums
```
enum Color {
    Red
    Green
    Blue
}

enum Shape {
    Circle { radius: float }
    Rectangle { width: float, height: float }
}
```
- Unit variants: `Color.Red`
- Data variants: `Shape.Circle { radius: 5.0 }`

### Traits
```
trait Printable {
    fn to_string(self) string
}

impl Printable for Point {
    fn to_string(self) string {
        return "({self.x}, {self.y})"
    }
}
```

### Errors
```
error ValidationError {
    message: string
}

error NotFoundError {
    id: int
}
```
- Errors are special types, not enums or exceptions
- Raised with `raise`, propagated with `!`, caught with `catch`

### App (entry point + DI container)
```
app MyApp {
    fn main(self) {
        // entry point
    }
}
```
- Exactly one `app` per program
- The compiler resolves all DI dependencies and wires them at compile time

### Tests
```
test "addition works" {
    expect(1 + 1).to_equal(2)
}

test "string contains" {
    expect("hello world".contains("world")).to_be_true()
}
```
- Run with `pluto test <file>`
- Assertions: `expect(val).to_equal(expected)`, `.to_be_true()`, `.to_be_false()`
- Raises assertions: `expect_raises(ErrorType) { ... }` asserts the block raises
  that error type (`!` inside propagates to the construct; the raise is consumed);
  bare `expect_raises { ... }` asserts the block raises anything. Checked against
  the inferred error sets at compile time: a block that cannot raise, or an error
  type outside the block's error set, is a compile error."#
        .to_string()
}

fn strings_doc() -> String {
    r#"## Strings

### String interpolation
Only an `f`-prefixed string interpolates; `{expr}` inside it is evaluated and spliced in. A plain `"..."` string never interpolates — `"{x}"` is the literal text `{x}`.
```
let name = "world"
let greeting = f"hello {name}"
let math = f"1 + 2 = {1 + 2}"
let literal = "{name}"            // the five characters {name}
let braces = f"use {{braces}}"    // use {braces}  (double a brace to escape it)
```
Interpolated values may be int, float, bool, string or byte; class instances cannot be interpolated directly.

### String methods (built-in)
- `.len()` — returns length in bytes (strings are UTF-8)
- `.contains(s)` — returns bool
- `.starts_with(s)` — returns bool
- `.ends_with(s)` — returns bool
- `.to_int()` — returns `int?` (nullable, none if parse fails or overflows; surrounding whitespace allowed). For a typed error instead, use `strings.parse_int`
- `.to_float()` — returns `float?` (nullable, none if parse fails). For a typed error instead, use `strings.parse_float`
- `.substring(offset, len)` — returns `len` bytes starting at `offset` (clamped to the string)
- `.char_at(i)` / `.byte_at(i)` — one-byte string / byte value at byte index `i` (indexing and `for c in s` are byte-wise too, so they split non-ASCII characters)
- `.index_of(s)` / `.last_index_of(s)` — returns int (-1 if not found)
- `.count(s)` — non-overlapping occurrences
- `.replace(old, new)` — returns new string
- `.split(delimiter)` — returns `[string]`; `""` splits into characters
- `.trim()` / `.trim_start()` / `.trim_end()` — whitespace-stripped copy
- `.is_empty()` / `.is_whitespace()` — returns bool
- `.repeat(n)` — `n` concatenated copies
- `.to_upper()` / `.to_lower()` — case-changed copy (ASCII letters only)
- `.to_bytes()` — returns `bytes`

### String concatenation
Use `+` to concatenate strings:
```
let full = first + " " + last
```

### Multiline strings
Strings can span multiple lines — newlines are preserved."#
        .to_string()
}

fn errors_doc() -> String {
    r#"## Error Handling

### Error declarations
```
error MyError {
    message: string
    code: int
}
```

### Raising errors
```
fn validate(x: int) {
    if x < 0 {
        raise MyError { message: "negative", code: 1 }
    }
}
```

### Error propagation with `!`
```
fn caller() {
    let result = validate(input)!
}
```
The `!` postfix propagates the error to the caller. The compiler infers which functions are fallible.

### Catching errors
```
// Catch all errors from a call
let result = risky_call() catch {
    // handle error, must return compatible type or return/raise
    return default_value
}

// Catch specific error
let result = risky_call() catch err {
    print("Error: {err.message}")
    return fallback
}
```

### Compiler-inferred error-ability
- You do NOT annotate functions as fallible — the compiler infers it from the call graph
- If a function calls a fallible function without `catch`, it becomes fallible itself
- All fallible call sites must use `!` (propagate) or `catch` (handle)
- The compiler enforces this at every call site"#
        .to_string()
}

fn closures_doc() -> String {
    r#"## Closures

### Syntax
Arrow function syntax with explicit parameter types:
```
let add_one = (x: int) => x + 1
let multiply = (a: int, b: int) => a * b

// Multi-line body with braces
let process = (x: int) => {
    let doubled = x * 2
    return doubled + 1
}
```

### Closure types
```
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

let result = apply((x: int) => x * 2, 5)
```

### Capture semantics
- Closures capture variables by value (snapshot at creation time)
- Heap-allocated types (strings, arrays, classes) share the underlying data
- Captured values cannot be mutated from inside the closure

### Passing closures
```
let numbers = [1, 2, 3, 4, 5]
// Using stdlib collections
import std.collections
let doubled = collections.map(numbers, (x: int) => x * 2)
let evens = collections.filter(numbers, (x: int) => x % 2 == 0)
```"#
    .to_string()
}

fn generics_doc() -> String {
    r#"## Generics

### Generic functions
```
fn identity<T>(x: T) T {
    return x
}

fn first<T>(arr: [T]) T {
    return arr[0]
}
```
- Type parameters inferred from arguments (no explicit type args at call site)
- Monomorphized at compile time (concrete copies generated)

### Generic classes
```
class Box<T> {
    value: T

    fn get(self) T {
        return self.value
    }
}

let b = Box<int> { value: 42 }
```

### Generic enums
```
enum Option<T> {
    Some { value: T }
    None
}
```

### Multiple type parameters
```
class Pair<A, B> {
    first: A
    second: B
}

let p = Pair<string, int> { first: "hello", second: 42 }
```

### Restrictions
- No type bounds (yet)
- No explicit type args on function calls (always inferred)
- No generic trait impls
- No DI on generic classes"#
        .to_string()
}

fn modules_doc() -> String {
    r#"## Modules

### Importing
```
import math
import std.fs
import std.collections
```

### Accessing imported items
```
let result = math.add(1, 2)
let point = math.Point { x: 1.0, y: 2.0 }
```

### Module sources
- **Directory module:** `<name>/` directory containing `.pluto` files (auto-merged)
- **File module:** `<name>.pluto` sibling file
- **Stdlib:** `std.<name>` (e.g., `std.fs`, `std.math`)

### Visibility
- `pub fn`, `pub class`, `pub trait`, `pub enum` — visible to importers
- Without `pub`, declarations are module-private

### Stdlib import
```
import std.fs
import std.strings

let content = fs.read_all("file.txt")!
let upper = strings.to_upper(content)
```
Note: Use `--stdlib <path>` flag to specify stdlib location when compiling."#
        .to_string()
}

fn contracts_doc() -> String {
    r#"## Contracts

### Class invariants
```
class Account {
    balance: float

    invariant self.balance >= 0.0

    fn withdraw(mut self, amount: float) {
        self.balance = self.balance - amount
    }
}
```
- Checked after construction and after every method call
- Violation aborts the program (not a catchable error)

### Function preconditions
```
fn divide(a: float, b: float) float
    requires b != 0.0
{
    return a / b
}
```
- `requires` clauses go between the signature and body `{`
- Postconditions are handled by class invariants, not `ensures`

### Trait contracts
```
trait Positive {
    fn value(self) int
        requires self.is_valid()
}
```
- Implementing classes inherit the trait's contracts
- Classes cannot add `requires` to trait methods (Liskov principle)

### Decidable fragment
Contracts are restricted to a decidable expression subset:
- Comparisons, arithmetic, logical ops
- `.len()` on arrays/strings
- Field access, literals
- No function calls, indexing, closures, or casts"#
        .to_string()
}

fn concurrency_doc() -> String {
    r#"## Concurrency

### Spawning tasks
```
fn compute(x: int) int {
    return x * x
}

let task = spawn compute(42)
let result = task.get()
```

### Task type
`spawn func(args)` returns `Task<T>` where `T` is the function's return type.

### Getting results
`.get()` blocks until the task completes and returns the result.

### Error handling with tasks
```
// If the spawned function is fallible:
let result = task.get()!        // propagate error
let result = task.get() catch { // catch error
    return default
}
```

### Restrictions
- `spawn` only works with direct function calls (not method calls or closures)
- No `.cancel()` or `.detach()` yet
- No structured concurrency yet
- Shared mutable heap is the programmer's responsibility"#
        .to_string()
}

fn gotchas_doc() -> String {
    r#"## Known Gotchas

### Plain strings do not interpolate
`"hello {name}"` is the literal text `hello {name}`. Interpolation needs the `f` prefix: `f"hello {name}"`. Inside an f-string, write `{{` and `}}` for literal braces.

### Variables are immutable by default
`let x = 1` cannot be reassigned; use `let mut x = 1` when the binding will change.

### No array concatenation with `+`
Arrays cannot be concatenated with `+`. Build arrays by iterating or use stdlib functions.

### No semicolons
Pluto uses newlines for statement termination. Don't add semicolons.

### `>>` is two tokens
The right-shift operator `>>` is parsed as two `>` tokens (to avoid conflict with generic syntax). This works transparently in expressions but is worth knowing.

### Nullable vs errors
- Use `T?` and `none` / `?` for values that may be absent
- Use `error` and `raise` / `!` / `catch` for failure conditions
- They are separate systems — don't use nullable for error handling or vice versa

### No `as` casts
`x as T` is a syntax error. Conversions are named methods: `n.to_float()`, `f.to_int()` (returns `int?`, `none` for NaN/infinity/out of range), `n.to_byte()` (returns `byte?`, narrowed to `byte` when the value is provably in 0..255), `n.low_byte()`, `b.to_int()`, `flag.to_int()`, and `n != 0` for int → bool. Strings convert with `s.to_int()` / `s.to_float()` (nullable) or `strings.parse_int` / `strings.parse_float` (typed errors).

### Method resolution
String/array built-in methods (`.len()`, `.contains()`, etc.) are compiler intrinsics, not regular methods. They cannot be overridden or called on other types."#
        .to_string()
}

// ---------------------------------------------------------------------------
// Stdlib documentation
// ---------------------------------------------------------------------------

fn stdlib_overview() -> String {
    r#"# Pluto Standard Library

Import with `import std.<module>`, access with `<module>.function()`.
Compile with `--stdlib <path-to-stdlib-dir>`.

| Module | Description |
|--------|-------------|
| `std.strings` | String manipulation: substring, split, join, trim, case conversion, compare, parse_int/parse_float |
| `std.math` | Math utilities: abs, clamp, gcd, lcm, factorial, trig, constants |
| `std.fs` | File system: read/write files, directories, File class for streaming |
| `std.json` | JSON parsing and construction: Json class with typed accessors |
| `std.http` | HTTP server: request handling, response builders, `listen()` |
| `std.net` | TCP networking: TcpListener and TcpConnection classes |
| `std.socket` | Low-level BSD sockets: create, bind, listen, accept, read, write |
| `std.collections` | Functional array operations: map, filter, fold, zip, enumerate, stable sort |
| `std.io` | Console I/O: println, print, read_line |
| `std.random` | Random number generation: seed, next, between, decimal |
| `std.time` | Time utilities: now, sleep, elapsed, monotonic clock |
| `std.compress` | Compression: gzip/gunzip and raw deflate/inflate over bytes (one-shot) |

Use `stdlib_docs` with a module name for detailed function signatures.

The following modules also ship in `stdlib/` but have no `stdlib_docs` entry yet; see the book's stdlib chapters or the module source for their full API.

| Module | Description |
|--------|-------------|
| `std.base64` | Standard and URL-safe base64: encode/decode for strings and for bytes |
| `std.blob` | Lockless single-writer blob store with fenced atomic writes |
| `std.env` | Environment variables: get, get_or, set, exists, remove, list_names |
| `std.hash` | Data-integrity hashes: CRC-32C, SHA-256, FNV-1a 64, hex encoding (one-shot and incremental) |
| `std.log` | Leveled logging: debug, info, warn, log_error, with set_level/get_level |
| `std.path` | Path manipulation: join, basename, dirname, ext, split_ext, normalize, is_absolute |
| `std.regex` | Simple pattern matching: literals, `.`, `^`/`$`, `*`/`+`/`?`, `\d`/`\w`/`\s`, backslash escapes |
| `std.rpc` | HTTP RPC client for cross-stage method calls |
| `std.uuid` | UUID generation: `generate()` |
| `std.verify` | Named proof bundles over the verification atoms |
| `std.wal` | Write-ahead log with recovery as a typestate (`Wal<Unrecovered → Ready>`) |
| `std.wire` | Schema-level wire values and encoders used by `serve`/`remote` marshaling |"#
        .to_string()
}

fn stdlib_strings() -> String {
    r#"# std.strings

Import: `import std.strings`

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `substring` | `(s: string, start: int, end: int) string` | Extract substring by byte indices |
| `contains` | `(haystack: string, needle: string) bool` | Check if string contains substring |
| `starts_with` | `(s: string, prefix: string) bool` | Check prefix |
| `ends_with` | `(s: string, suffix: string) bool` | Check suffix |
| `index_of` | `(haystack: string, needle: string) int` | Find first occurrence (-1 if not found) |
| `last_index_of` | `(haystack: string, needle: string) int` | Find last occurrence (-1 if not found) |
| `count` | `(haystack: string, needle: string) int` | Non-overlapping occurrences |
| `trim` | `(s: string) string` | Remove leading/trailing whitespace |
| `trim_start` | `(s: string) string` | Remove leading whitespace |
| `trim_end` | `(s: string) string` | Remove trailing whitespace |
| `is_empty` | `(s: string) bool` | Length == 0 |
| `is_whitespace` | `(s: string) bool` | Empty or all whitespace |
| `repeat` | `(s: string, count: int) string` | Concatenate count copies (non-positive count gives "") |
| `to_upper` | `(s: string) string` | Convert ASCII letters to uppercase |
| `to_lower` | `(s: string) string` | Convert to lowercase |
| `replace` | `(s: string, old: string, new_str: string) string` | Replace all occurrences |
| `split` | `(s: string, delimiter: string) [string]` | Split string into array |
| `char_at` | `(s: string, index: int) string` | One-byte string at byte index |
| `len` | `(s: string) int` | String length in bytes |
| `byte_at` | `(s: string, index: int) int` | Get byte value at index |
| `format_float` | `(value: float, decimals: int) string` | Format float with N decimal places |
| `join` | `(arr: [string], separator: string) string` | Join string array with separator |
| `compare` | `(a: string, b: string) int` | Lexicographic byte order: -1, 0, or 1 |
| `parse_int` | `(s: string) int` | Parse base-10 int; raises `ParseError` |
| `parse_float` | `(s: string) float` | Parse base-10 float; raises `ParseError` |

## Errors

```
pub error ParseError {
    input: string    // the string that failed to parse
    message: string  // what was wrong ("empty string", "out of range for int", ...)
}
```

`parse_int` accepts an optional leading `-`/`+` followed by one or more ASCII
digits; leading zeros are fine. It raises `ParseError` for the empty string, a
bare sign, any other character (whitespace included; `trim` first to accept
padded input), and values outside the int range. Overflow is detected, never
wrapped or trapped.

`parse_float` accepts `[+-]? DIGITS ('.' DIGITS)? ([eE] [+-]? DIGITS)?`, and
raises `ParseError` otherwise (`.5`, `5.`, `inf`, `nan`, whitespace) or when
the value overflows to infinity (`1e999`).

```
let port = strings.parse_int(raw) catch err: strings.ParseError {
    log.warn(f"bad port {err.input}: {err.message}")
    8080
}
```

Note: Many of these overlap with built-in string methods (`.len()`, `.contains()`, etc.). The stdlib versions take the string as a first argument."#
        .to_string()
}

fn stdlib_math() -> String {
    r#"# std.math

Import: `import std.math`

## Constants
- `PI` — `fn() float` → 3.14159265358979...
- `E` — `fn() float` → 2.71828182845904...
- `TAU` — `fn() float` → 6.28318530717958... (2π)

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `(x: int) int` | Absolute value (int) |
| `min` | `(a: int, b: int) int` | Minimum of two ints |
| `max` | `(a: int, b: int) int` | Maximum of two ints |
| `pow` | `(base: int, exp: int) int` | Integer exponentiation |
| `clamp` | `(value: int, lo: int, hi: int) int` | Clamp int to range |
| `clamp_float` | `(value: float, lo: float, hi: float) float` | Clamp float to range |
| `sign` | `(x: int) int` | Sign: -1, 0, or 1 |
| `gcd` | `(a: int, b: int) int` | Greatest common divisor |
| `lcm` | `(a: int, b: int) int` | Least common multiple |
| `factorial` | `(n: int) int` | Factorial (n!) |
| `is_even` | `(x: int) bool` | Check if even |
| `is_odd` | `(x: int) bool` | Check if odd |
| `to_radians` | `(degrees: float) float` | Degrees to radians |
| `to_degrees` | `(radians: float) float` | Radians to degrees |

Note: `sqrt`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `log` are compiler builtins (no import needed), as are `wrapping_add`, `wrapping_sub`, `wrapping_mul` — two's-complement wrapping int arithmetic for deliberately-modular code (hashes, PRNGs); plain `+`/`-`/`*` trap on overflow (defect)."#
        .to_string()
}

fn stdlib_fs() -> String {
    r#"# std.fs

Import: `import std.fs`

## Errors
- `FileError { path: string, code: int, message: string }` — general definite OS failure (raw errno captured at the syscall site)
- `NotFound { path: string }` — ENOENT, the branch-worthy case
- `CloseError { code: int, message: string }` — close() failed; the fd is released regardless
- `SyncError { path: string, code: int, message: string }` — one-shot durability failure (sync_dir, replace_all)
- `Degraded { file: File<Write, Poisoned>, code: int, message: string }` — a write/sync on an open handle failed; the payload is the handle in its post-failure state

## The Typestated Handle
`File<M, S>` — M is the mode (`Read` | `Write`, fixed at open), S the state (`Open` | `Poisoned` | `Closed`). `Open` and `Poisoned` are must-release states: an open handle cannot be dropped, captured by a closure/spawn, or stored in a field — it must be closed (or discarded), moved onward, or returned. Methods exist only in the right mode and state; use-after-close, double-close, and wrong-mode I/O are compile errors.

```
class File<M, S> {
    fn read(self, max_bytes: int) string   // where M == Read, S == Open; "" = EOF; raises FileError
    fn read_bytes(self, max_bytes: int) bytes          // where M == Read, S == Open; empty = EOF; raises FileError
    fn read_at(self, offset: int, max_bytes: int) bytes // where M == Read, S == Open; pread — never moves the cursor
    fn write(self, data: string)           // where M == Write, S == Open; loops to completion; raises Degraded
    fn write_bytes(self, data: bytes)      // where M == Write, S == Open; loops to completion; raises Degraded
    fn write_at(self, offset: int, data: bytes) // where M == Write, S == Open; pwrite — never moves the cursor; raises Degraded
    fn truncate(self, len: int)            // where M == Write, S == Open; ftruncate — shrink discards the tail, extend zero-fills, cursor unmoved; negative len raises FileError (handle still sound), syscall failure raises Degraded
    fn sync(self)                          // where M == Write, S == Open; full durability (fsync / F_FULLFSYNC); raises Degraded
    fn sync_data(self)                     // where M == Write, S == Open; fdatasync (F_FULLFSYNC on Darwin); raises Degraded
    fn seek(self, to: Seek) int            // where S == Open; raises FileError
    fn close(self) File<M, Closed>         // where S == Open; consuming transition; raises CloseError
    fn discard(self) File<M, Closed>       // where S == Poisoned; the only exit from Poisoned; never raises
}
```

A failed write or sync destroys the durability warrant for the fd (the kernel reports a writeback error once — never retry a failed sync). The `Degraded` payload must be caught with a typed handler and discharged via `discard()`; recovery is reopen-and-rewrite from data the program still owns.

## Seek
```
enum Seek {
    Start { offset: int }      // from beginning
    Current { delta: int }     // from current position
    End { delta: int }         // from end
}
```

## Metadata
`Metadata { size: int, modified: int, is_dir: bool, is_file: bool, mode: int }` with invariant `size >= 0` (`modified` is unix seconds, `mode` is permission bits).

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `open_read` | `(path: string) File<Read, Open>` | Open for reading (raises NotFound \| FileError) |
| `open_write` | `(path: string) File<Write, Open>` | Open/create, truncating (fallible) |
| `open_append` | `(path: string) File<Write, Open>` | Open/create for appending (fallible) |
| `read_all` | `(path: string) string` | Read entire file (raises NotFound \| FileError) |
| `write_all` | `(path: string, content: string)` | Write, overwriting; close errors surfaced (fallible) |
| `append_all` | `(path: string, content: string)` | Append; close errors surfaced (fallible) |
| `read_all_bytes` | `(path: string) bytes` | Read entire file as binary (raises NotFound \| FileError) |
| `write_all_bytes` | `(path: string, data: bytes)` | Write binary, overwriting; close errors surfaced (fallible) |
| `append_all_bytes` | `(path: string, data: bytes)` | Append binary; close errors surfaced (fallible) |
| `replace_all` | `(path: string, content: string)` | Atomic durable replace: temp + sync + rename + dir fsync. FileError ⇒ old file intact; SyncError ⇒ rename landed, durability unwarranted |
| `replace_all_bytes` | `(path: string, data: bytes)` | Same atomic durable replace for binary content, same error contract |
| `sync_dir` | `(path: string)` | fsync a directory (crash-safe rename/create/remove); raises SyncError |
| `stat` | `(path: string) Metadata` | File metadata (raises NotFound \| FileError) |
| `exists` | `(path: string) bool` | Check if path exists |
| `file_size` | `(path: string) int` | File size in bytes (raises NotFound \| FileError) |
| `is_dir` | `(path: string) bool` | Check if path is a directory |
| `is_file` | `(path: string) bool` | Check if path is a file |
| `remove` | `(path: string)` | Delete a file (fallible) |
| `mkdir` | `(path: string)` | Create directory (fallible) |
| `create_dir_all` | `(path: string)` | mkdir -p: create all missing components (fallible) |
| `rmdir` | `(path: string)` | Remove empty directory (fallible) |
| `remove_dir_all` | `(path: string)` | Recursive delete; refuses "/" and "" (fallible) |
| `rename` | `(old: string, new_path: string)` | Rename/move file (fallible) |
| `copy` | `(src: string, dst: string)` | Copy file; close errors surfaced (fallible) |
| `truncate` | `(path: string, len: int)` | Set file length (truncate(2)): shrink discards the tail, extend zero-fills; negative len raises FileError (raises NotFound \| FileError). Not durable by itself — use the handle form + sync() when the shrink must survive a crash |
| `list_dir` | `(path: string) [string]` | List directory contents (fallible) |
| `temp_dir` | `() string` | Create a fresh temp directory |

Durability note: `sync`/`sync_data`/`replace_all` are syscall-faithful, not crash-tested — `sync()` means full durability on every platform (F_FULLFSYNC on Darwin; ENOTSUP raises rather than silently degrading)."#
        .to_string()
}

fn stdlib_json() -> String {
    r#"# std.json

Import: `import std.json`

## Error
`JsonError { message: string }` — raised by `parse` (malformed input) and by `get_int` (value is not an integer).

## Json Class
Represents a JSON value. Build one with the factory functions, or get one from `parse()`.
Kinds: null, bool, int, float, string, array, object. Numbers parse as `int` when they are
integer literals that fit 64 bits and as `float` otherwise.

### Factory Functions
| Function | Signature | Description |
|----------|-----------|-------------|
| `null` | `() Json` | JSON null |
| `bool` | `(v: bool) Json` | JSON boolean |
| `int` | `(v: int) Json` | JSON number from an int |
| `float` | `(v: float) Json` | JSON number from a float |
| `string` | `(v: string) Json` | JSON string |
| `array` | `() Json` | Empty JSON array; fill with `.push` |
| `object` | `() Json` | Empty JSON object; fill with `.set` |
| `parse` | `(text: string) Json` | Parse JSON text (fallible: `JsonError`) |

### Kind Predicates
`.is_null()`, `.is_bool()`, `.is_int()`, `.is_float()`, `.is_string()`, `.is_array()`, `.is_object()` — each `() bool`.

### Json Methods
| Method | Returns | Description |
|--------|---------|-------------|
| `.get_bool()` | `bool` | Value of a bool node |
| `.get_int()` | `int` | Value of an int node, or a float node that holds an integer (fallible: `JsonError`) |
| `.get_float()` | `float` | Value of a number node (ints widen) |
| `.get_string()` | `string` | Value of a string node |
| `.get(key: string)` | `Json` | Object field by key; JSON null when absent or not an object |
| `.at(index: int)` | `Json` | Array element by index (out of range is a runtime defect) |
| `.len()` | `int` | Element count of an array, key count of an object, 0 otherwise |
| `.set(key: string, value: Json)` | | Insert or overwrite an object field (`mut self`) |
| `.push(value: Json)` | | Append to an array (`mut self`) |
| `.to_string()` | `string` | Serialize to compact JSON text |

### Example
```pluto
import std.json

fn main() {
    let doc = json.parse("{\"name\": \"pluto\", \"tags\": [1, 2]}")!
    print(doc.get("name").get_string())      // pluto
    print(doc.get("tags").at(1).get_int()!)  // 2
    print(doc.get("missing").is_null())      // true

    let mut out = json.object()
    out.set("ok", json.bool(true))
    let mut arr = json.array()
    arr.push(json.int(7))
    out.set("xs", arr)
    print(out.to_string())                   // {"ok":true,"xs":[7]}
}
```"#
        .to_string()
}

fn stdlib_http() -> String {
    r#"# std.http

Import: `import std.http`

## Error
`HttpError { message: string }`

## Classes

### Request
```
class Request {
    fn method(self) string      // "GET", "POST", etc.
    fn path(self) string        // Request path
    fn body(self) string        // Request body
    fn header(self, name: string) string  // Get header value
}
```

### Response
Built via factory functions (not constructed directly).

### HttpConnection
```
class HttpConnection {
    fn respond(self, response: Response)  // Send response (fallible)
}
```

### HttpServer
```
class HttpServer {
    fn accept(self) HttpConnection  // Accept next connection (fallible)
    fn connection_request(self, conn: HttpConnection) Request  // Get request from connection (fallible)
}
```

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `listen` | `(port: int) HttpServer` | Start HTTP server on port (fallible) |
| `ok` | `(body: string) Response` | 200 OK with text body |
| `ok_json` | `(body: string) Response` | 200 OK with JSON content type |
| `not_found` | `(body: string) Response` | 404 Not Found |
| `bad_request` | `(body: string) Response` | 400 Bad Request |
| `response` | `(status: int, body: string) Response` | Custom status code |"#
        .to_string()
}

fn stdlib_net() -> String {
    r#"# std.net

Import: `import std.net`

Low-level TCP networking.

## Classes

### TcpListener
```
class TcpListener {
    fn accept(self) TcpConnection  // Accept connection (fallible)
}
```

### TcpConnection
```
class TcpConnection {
    fn read(self, size: int) string   // Read up to size bytes (fallible)
    fn write(self, data: string)      // Write data (fallible)
    fn close(self)                    // Close connection (fallible)
}
```

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `listen` | `(port: int) TcpListener` | Bind and listen on TCP port (fallible) |
| `connect` | `(host: string, port: int) TcpConnection` | Connect to TCP host:port (fallible) |"#
        .to_string()
}

fn stdlib_socket() -> String {
    r#"# std.socket

Import: `import std.socket`

BSD-level socket operations. Lower level than `std.net`.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `create` | `(domain: int, sock_type: int, protocol: int) int` | Create a socket (`2, 1, 0` = IPv4 TCP), returns fd or -1 |
| `bind` | `(fd: int, host: string, port: int) int` | Bind to host:port; host may be an IPv4 literal or a hostname; 0 on success, -1 on failure |
| `listen` | `(fd: int, backlog: int)` | Start listening (fallible) |
| `accept` | `(fd: int) int` | Accept connection, returns client fd (fallible) |
| `connect` | `(fd: int, host: string, port: int) int` | Connect to host:port; host may be an IPv4 literal or a hostname; 0 on success, -1 on failure |
| `read` | `(fd: int, max_bytes: int) string` | Read up to max_bytes as a string |
| `read_bytes` | `(fd: int, max_bytes: int) bytes` | Read up to max_bytes as binary |
| `write` | `(fd: int, data: string) int` | Write a string, returns bytes written or -1 |
| `write_bytes` | `(fd: int, data: bytes) int` | Write binary, returns bytes written or -1 |
| `close` | `(fd: int)` | Close socket |
| `set_reuseaddr` | `(fd: int)` | Set SO_REUSEADDR option |
| `get_port` | `(fd: int) int` | Get bound port number (useful with port 0) |
| `set_read_timeout` | `(fd: int, ms: int) int` | Per-fd read deadline in ms (0 clears); 0 on success |
| `read_timed_out` | `() int` | 1 if the last read on this thread stopped at its deadline rather than on data or an error |"#
        .to_string()
}

fn stdlib_collections() -> String {
    r#"# std.collections

Import: `import std.collections`

Functional operations on arrays. All functions are generic.

## Pair Class
```
class Pair<A, B> {
    first: A
    second: B
}
```
Used by `zip` and `enumerate`.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `map` | `<T, U>(arr: [T], f: fn(T) U) [U]` | Transform each element |
| `filter` | `<T>(arr: [T], f: fn(T) bool) [T]` | Keep elements matching predicate |
| `fold` | `<T, U>(arr: [T], init: U, f: fn(U, T) U) U` | Left fold with accumulator |
| `reduce` | `<T>(arr: [T], f: fn(T, T) T) T` | Reduce without initial value |
| `any` | `<T>(arr: [T], f: fn(T) bool) bool` | True if any element matches |
| `all` | `<T>(arr: [T], f: fn(T) bool) bool` | True if all elements match |
| `for_each` | `<T>(arr: [T], f: fn(T) void)` | Execute side effect for each |
| `flat_map` | `<T, U>(arr: [T], f: fn(T) [U]) [U]` | Map then flatten |
| `count` | `<T>(arr: [T], f: fn(T) bool) int` | Count matching elements |
| `reverse` | `<T>(arr: [T]) [T]` | Reverse array |
| `take` | `<T>(arr: [T], n: int) [T]` | Take first N elements |
| `drop` | `<T>(arr: [T], n: int) [T]` | Drop first N elements |
| `zip` | `<A, B>(a: [A], b: [B]) [Pair<A, B>]` | Zip two arrays into pairs |
| `enumerate` | `<T>(arr: [T]) [Pair<int, T>]` | Pair each element with its index |
| `flatten` | `<T>(arr: [[T]]) [T]` | Flatten nested array |
| `sum` | `(arr: [int]) int` | Sum int array |
| `sum_float` | `(arr: [float]) float` | Sum float array |
| `sort_by` | `<T>(arr: [T], less: fn(T, T) bool) [T]` | Stable sort by a strict less-than comparator |
| `try_map` | `<T, U>(arr: [T], f: fn(T) U!) [U]` | `map` with a fallible function; the first raise propagates |
| `try_filter` | `<T>(arr: [T], f: fn(T) bool!) [T]` | `filter` with a fallible predicate |
| `try_fold` | `<T, U>(arr: [T], init: U, f: fn(U, T) U!) U` | `fold` with a fallible step |
| `try_for_each` | `<T>(arr: [T], f: fn(T)!)` | `for_each` with a fallible action |
| `sort` | `(arr: [int]) [int]` | Stable ascending sort of ints |
| `sort_floats` | `(arr: [float]) [float]` | Stable ascending sort of floats |
| `sort_strings` | `(arr: [string]) [string]` | Stable lexicographic sort (`strings.compare` order) |

## Sorting

All sorts return a new array and leave the input untouched. They are
**stable**: elements that compare equal keep their original relative order, so
sorting by a secondary key and then a primary key orders by primary key with
ties broken by the secondary key. Implementation is a bottom-up merge sort
(O(n log n) comparisons, O(n) extra space).

`less(a, b)` must return true only when `a` belongs strictly before `b`
(`(a, b) => a < b` ascending, `(a, b) => a > b` descending). Returning true for
equal elements breaks the stability guarantee.

```
let ordered = collections.sort_by(segments, (a: Segment, b: Segment) => a.offset < b.offset)
```"#
        .to_string()
}

fn stdlib_io() -> String {
    r#"# std.io

Import: `import std.io`

Console input/output.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `println` | `(s: string)` | Print string with newline |
| `print` | `(s: string)` | Print string without newline |
| `read_line` | `() string` | Read line from stdin |

Note: The global `print()` function is a compiler builtin and always available without import. `std.io.print` is the same function exposed as a module."#
        .to_string()
}

fn stdlib_random() -> String {
    r#"# std.random

Import: `import std.random`

Pseudo-random number generation.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `seed` | `(s: int)` | Seed the RNG |
| `next` | `() int` | Next random int |
| `between` | `(lo: int, hi: int) int` | Random int in [lo, hi) |
| `decimal` | `() float` | Random float in [0.0, 1.0) |
| `decimal_between` | `(lo: float, hi: float) float` | Random float in [lo, hi) |
| `coin` | `() bool` | Random boolean (50/50) |"#
        .to_string()
}

fn stdlib_compress() -> String {
    r#"# std.compress

Import: `import std.compress`

One-shot compression over `bytes`, backed by the vendored miniz DEFLATE core.
Two codecs: `gzip`/`gunzip` (a complete gzip member, RFC 1952 — the
`Content-Encoding: gzip` wire format, interoperable with the `gzip` tool and
HTTP) and `deflate`/`inflate` (raw DEFLATE, RFC 1951 — no header or checksum,
for protocols that frame and check the data themselves). Streaming is a
planned follow-up; these are whole-buffer-in, whole-buffer-out.

Decompression of corrupt, truncated, or oversized input raises `CompressError`
rather than trapping; it also refuses to expand past `default_max_size()`
(64 MiB), rejecting decompression bombs. Compression raises `CompressError`
only on an out-of-range level.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `gzip` | `(data: bytes, level: int) bytes` | Compress to a gzip member; `level` 0..9 (6 is a good default) |
| `gunzip` | `(data: bytes) bytes` | Decompress a gzip member; raises `CompressError` on bad input |
| `deflate` | `(data: bytes, level: int) bytes` | Compress to raw DEFLATE; `level` 0..9 |
| `inflate` | `(data: bytes) bytes` | Decompress raw DEFLATE; raises `CompressError` on bad input |
| `default_max_size` | `() int` | The decompression output cap in bytes (64 MiB) |

## Errors

`pub error CompressError { message: string }` — raised by `gunzip`/`inflate`
on invalid/truncated/oversized input, and by `gzip`/`deflate` on an
out-of-range level."#
        .to_string()
}

fn stdlib_time() -> String {
    r#"# std.time

Import: `import std.time`

Time utilities.

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `now` | `() int` | Current wall-clock time in milliseconds (Unix epoch) |
| `now_ns` | `() int` | Current time in nanoseconds |
| `monotonic` | `() int` | Monotonic clock in milliseconds (for measuring durations) |
| `monotonic_ns` | `() int` | Monotonic clock in nanoseconds |
| `sleep` | `(ms: int)` | Sleep for N milliseconds |
| `elapsed` | `(start: int) int` | Milliseconds elapsed since `start` (monotonic) |"#
        .to_string()
}

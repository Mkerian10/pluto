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
            _ => Err(format!(
                "Unknown stdlib module: `{m}`. Available: strings, math, fs, json, http, \
                 net, socket, collections, io, random, time"
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
- `"hello"` — string (supports interpolation with `{expr}`)
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
    return "hello {name}"
}
```
- `pub` makes the function visible to other modules
- Return type goes after the parameter list
- Omit return type for `void` functions

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
- Assertions: `expect(val).to_equal(expected)`, `.to_be_true()`, `.to_be_false()`"#
        .to_string()
}

fn strings_doc() -> String {
    r#"## Strings

### String interpolation
Pluto uses `{expr}` inside double-quoted strings for interpolation:
```
let name = "world"
let greeting = "hello {name}"
let math = "1 + 2 = {1 + 2}"
```

**Important:** Curly braces `{` and `}` inside strings always trigger interpolation. There is no escape sequence for literal braces. If you need literal braces, store them in a variable or use a function.

### String methods (built-in)
- `.len()` — returns int length
- `.contains(s)` — returns bool
- `.starts_with(s)` — returns bool
- `.ends_with(s)` — returns bool
- `.to_int()` — returns `int?` (nullable, none if parse fails)
- `.to_float()` — returns `float?` (nullable, none if parse fails)
- `.substring(start, end)` — returns substring
- `.index_of(s)` — returns int (-1 if not found)
- `.replace(old, new)` — returns new string
- `.split(delimiter)` — returns `[string]`
- `.trim()` — returns trimmed string
- `.to_upper()` — returns uppercase string
- `.to_lower()` — returns lowercase string

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

### Operator precedence with `!`
`!expr.method()` parses as `(!expr).method()`, NOT `!(expr.method())`.
Always use parentheses: `!(expr.method())`.

### No literal braces in strings
`{` and `}` in strings always trigger interpolation. There is no escape sequence.
Workaround: use a variable: `let brace = "{" // won't work either` — this is a known limitation.

### Empty struct literals don't parse
`Foo {}` with zero fields doesn't work. Always add at least one field to classes.

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

### `as` cast limitations
Only `int↔float` and `int↔bool` casts are supported. No string conversions via `as`.

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
| `std.strings` | String manipulation: substring, split, join, trim, case conversion |
| `std.math` | Math utilities: abs, clamp, gcd, lcm, factorial, trig, constants |
| `std.fs` | File system: read/write files, directories, File class for streaming |
| `std.json` | JSON parsing and construction: Json class with typed accessors |
| `std.http` | HTTP server: request handling, response builders, `listen()` |
| `std.net` | TCP networking: TcpListener and TcpConnection classes |
| `std.socket` | Low-level BSD sockets: create, bind, listen, accept, read, write |
| `std.collections` | Functional array operations: map, filter, fold, zip, enumerate |
| `std.io` | Console I/O: println, print, read_line |
| `std.random` | Random number generation: seed, next, between, decimal |
| `std.time` | Time utilities: now, sleep, elapsed, monotonic clock |

Use `stdlib_docs` with a module name for detailed function signatures."#
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
| `trim` | `(s: string) string` | Remove leading/trailing whitespace |
| `to_upper` | `(s: string) string` | Convert to uppercase |
| `to_lower` | `(s: string) string` | Convert to lowercase |
| `replace` | `(s: string, old: string, new_str: string) string` | Replace all occurrences |
| `split` | `(s: string, delimiter: string) [string]` | Split string into array |
| `char_at` | `(s: string, index: int) string` | Get character at index |
| `len` | `(s: string) int` | String length in bytes |
| `byte_at` | `(s: string, index: int) int` | Get byte value at index |
| `format_float` | `(value: float, decimals: int) string` | Format float with N decimal places |
| `join` | `(arr: [string], separator: string) string` | Join string array with separator |

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

Note: `sqrt`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `log` are compiler builtins (no import needed)."#
        .to_string()
}

fn stdlib_fs() -> String {
    r#"# std.fs

Import: `import std.fs`

## Error
`FileError { message: string }` — raised by all fallible fs operations.

## File Class
```
class File {
    fn read(self, size: int) string     // Read up to `size` bytes
    fn write(mut self, data: string)    // Write string data
    fn seek(mut self, offset: int, whence: int)  // Seek to position
    fn close(mut self)                  // Close the file handle
    fn flush(mut self)                  // Flush write buffer
}
```

## Seek Constants
- `SEEK_SET()` → 0 (from beginning)
- `SEEK_CUR()` → 1 (from current position)
- `SEEK_END()` → 2 (from end)

## Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `open_read` | `(path: string) File` | Open file for reading (fallible) |
| `open_write` | `(path: string) File` | Open/create file for writing (fallible) |
| `open_append` | `(path: string) File` | Open/create file for appending (fallible) |
| `read_all` | `(path: string) string` | Read entire file to string (fallible) |
| `write_all` | `(path: string, content: string)` | Write string to file, overwriting (fallible) |
| `append_all` | `(path: string, content: string)` | Append string to file (fallible) |
| `exists` | `(path: string) bool` | Check if path exists |
| `file_size` | `(path: string) int` | Get file size in bytes (fallible) |
| `is_dir` | `(path: string) bool` | Check if path is a directory |
| `is_file` | `(path: string) bool` | Check if path is a file |
| `remove` | `(path: string)` | Delete a file (fallible) |
| `mkdir` | `(path: string)` | Create directory (fallible) |
| `rmdir` | `(path: string)` | Remove empty directory (fallible) |
| `rename` | `(old: string, new_path: string)` | Rename/move file (fallible) |
| `copy` | `(src: string, dst: string)` | Copy file (fallible) |
| `list_dir` | `(path: string) [string]` | List directory contents (fallible) |
| `temp_dir` | `() string` | Get system temp directory path |"#
        .to_string()
}

fn stdlib_json() -> String {
    r#"# std.json

Import: `import std.json`

## Error
`JsonError { message: string }` — raised by parse and typed accessors.

## Json Class
Represents a JSON value. Constructed via factory functions or `parse()`.

### Factory Functions
| Function | Signature | Description |
|----------|-----------|-------------|
| `null` | `() Json` | Create JSON null |
| `bool` | `(value: bool) Json` | Create JSON boolean |
| `int` | `(value: int) Json` | Create JSON number (from int) |
| `float` | `(value: float) Json` | Create JSON number (from float) |
| `string` | `(value: string) Json` | Create JSON string |
| `array` | `(elements: [Json]) Json` | Create JSON array |
| `object` | `(keys: [string], values: [Json]) Json` | Create JSON object |
| `parse` | `(text: string) Json` | Parse JSON string (fallible) |

### Json Methods
| Method | Returns | Description |
|--------|---------|-------------|
| `.to_string(self)` | `string` | Serialize to JSON string |
| `.get(self, key: string)` | `Json` | Get object field (fallible) |
| `.get_index(self, index: int)` | `Json` | Get array element (fallible) |
| `.as_int(self)` | `int` | Extract as int (fallible) |
| `.as_float(self)` | `float` | Extract as float (fallible) |
| `.as_bool(self)` | `bool` | Extract as bool (fallible) |
| `.as_string(self)` | `string` | Extract as string (fallible) |
| `.as_array(self)` | `[Json]` | Extract as array (fallible) |
| `.len(self)` | `int` | Array/object length (fallible) |
| `.keys(self)` | `[string]` | Object keys (fallible) |
| `.is_null(self)` | `bool` | Check if null |
| `.kind(self)` | `string` | Type name: "null", "bool", "number", "string", "array", "object" |"#
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
| `create` | `() int` | Create a TCP socket, returns fd |
| `bind` | `(fd: int, port: int)` | Bind socket to port (fallible) |
| `listen` | `(fd: int, backlog: int)` | Start listening (fallible) |
| `accept` | `(fd: int) int` | Accept connection, returns client fd (fallible) |
| `connect` | `(fd: int, host: string, port: int)` | Connect to host:port (fallible) |
| `read` | `(fd: int, size: int) string` | Read from socket (fallible) |
| `write` | `(fd: int, data: string) int` | Write to socket, returns bytes written (fallible) |
| `close` | `(fd: int)` | Close socket |
| `set_reuseaddr` | `(fd: int)` | Set SO_REUSEADDR option |
| `get_port` | `(fd: int) int` | Get bound port number (useful with port 0) |"#
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
| `sum_float` | `(arr: [float]) float` | Sum float array |"#
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

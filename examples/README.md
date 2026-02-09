# Pluto Examples

Example programs demonstrating Pluto's features.

## Running Examples

Build the compiler, then run any example:

```bash
cargo build --release
cargo run --release -- run examples/<name>/main.pluto
```

For examples that use the standard library, pass the `--stdlib` flag:

```bash
cargo run --release -- run --stdlib stdlib examples/<name>/main.pluto
```

Or compile to a binary and run it separately:

```bash
cargo run --release -- compile examples/add/main.pluto -o my_program
./my_program
```

## Examples

| Example | Features | Description |
|---------|----------|-------------|
| [add](add/) | functions, int arithmetic | Basic function definition and calling |
| [point](point/) | classes, fields, methods | Class with fields and a method |
| [strings](strings/) | string ops, `.len()`, equality | String concatenation, length, comparison |
| [string_interp](string_interp/) | string interpolation | Embed expressions in strings with `{expr}` |
| [arrays](arrays/) | arrays, `.len()`, `.push()`, indexing | Array creation, mutation, and iteration |
| [control_flow](control_flow/) | if/else, while, for, string interp | FizzBuzz with loops and for-each |
| [enums](enums/) | enums, match, data variants | Sum types with pattern matching |
| [closures](closures/) | closures, `fn` types, capture | Lambdas, higher-order functions, closures |
| [traits](traits/) | traits, default methods, polymorphism | Interfaces with dynamic dispatch |
| [app_demo](app_demo/) | `app`, dependency injection | Auto-wired services with bracket deps |
| [stdlib](stdlib/) | `import std.strings`, `import std.math` | String utilities and math functions from the standard library |
| [collections](collections/) | `Map<K,V>`, `Set<T>`, index, methods | Maps and sets with insert, remove, contains, iteration |
| [networking](networking/) | `import std.net`, `import std.socket`, TCP | TCP echo server with TcpListener and TcpConnection |
| [file_io](file_io/) | `import std.fs`, file ops, error handling | File read/write, copy, rename, directories, seek |
| [ambient-di](ambient-di/) | `uses`, `ambient`, dependency injection | Ambient DI with bare variable access to injected deps |
| [http_server](http_server/) | `import std.net`, `import std.fs`, `import std.strings` | Static file HTTP server with content-type detection and 404s |
| [numeric](numeric/) | bitwise ops, `as` casting, math builtins | Bitwise operators, type casting, math functions, fallible `pow` |

## Language Quick Reference

```
// Functions
fn add(a: int, b: int) int {
    return a + b
}

// Variables
let x = 42
let name = "pluto"

// Classes
class Point {
    x: int
    y: int

    fn sum(self) int {
        return self.x + self.y
    }
}
let p = Point { x: 1, y: 2 }

// Traits
trait HasArea {
    fn area(self) int
}
class Square impl HasArea {
    side: int
    fn area(self) int { return self.side * self.side }
}

// Enums + match
enum Color { Red, Green, Blue }
let c = Color.Red
match c {
    Color.Red { print("red") }
    Color.Green { print("green") }
    Color.Blue { print("blue") }
}

// Arrays
let nums = [1, 2, 3]
nums.push(4)
print(nums[0])
print(nums.len())

// Maps
let m = Map<string, int> { "a": 1, "b": 2 }
m["c"] = 3
print(m["a"])
for k in m.keys() { print(k) }

// Sets
let s = Set<int> { 1, 2, 3 }
s.insert(4)
print(s.contains(2))

// Control flow
if x > 0 { print("positive") }
while x > 0 { x = x - 1 }
for item in nums { print(item) }

// String interpolation
let msg = "x is {x} and name is {name}"

// Closures
let double = (x: int) => x * 2
fn apply(f: fn(int) int, x: int) int { return f(x) }

// Dependency injection (explicit)
class Database { fn query(self) string { return "data" } }
class Service[db: Database] {
    fn run(self) { print(self.db.query()) }
}
app MyApp[svc: Service] {
    fn main(self) { self.svc.run() }
}

// Ambient dependency injection
class Logger { fn info(self, msg: string) { print(msg) } }
class OrderService uses Logger {
    fn process(self) { logger.info("processing") }
}
app MyApp2[svc: OrderService] {
    ambient Logger
    fn main(self) { self.svc.process() }
}
```

## Types

| Type | Description | Example |
|------|-------------|---------|
| `int` | 64-bit signed integer | `42`, `-1` |
| `float` | 64-bit floating point | `3.14`, `-0.5` |
| `bool` | Boolean | `true`, `false` |
| `string` | Heap-allocated string | `"hello"` |
| `[T]` | Array of T | `[1, 2, 3]` |
| `Map<K, V>` | Hash map | `Map<string, int> { "a": 1 }` |
| `Set<T>` | Hash set | `Set<int> { 1, 2, 3 }` |
| `fn(T) R` | Function/closure type | `fn(int) string` |
| Class names | Nominal class types | `Point`, `Database` |
| Trait names | Structural trait types | `HasArea` |
| Enum names | Sum types | `Color`, `Result` |

<p align="center">
  <h1 align="center">Pluto</h1>
  <p align="center">
    <strong>A programming language for distributed backend systems</strong>
  </p>
  <p align="center">
    Native compilation &bull; Dependency injection &bull; Typed errors &bull; Whole-program analysis
  </p>
</p>

---

Pluto is a compiled language designed for building backend services where distribution, dependency injection, and error handling are first-class concerns. It compiles to native code via Cranelift, uses garbage collection, and treats the `app` as its fundamental building block.

```
class OrderService[db: Database, accounts: AccountsService] uses Logger {
    fn create(mut self, order: Order) Order {
        if !order.validate() {
            raise ValidationError { field: "order", message: "invalid" }
        }

        let user = accounts.get_user(order.user_id)!
        db.insert(order)!
        logger.info("created order {order.id} for {user.name}")
        return order
    }
}

app OrderApp[orders: OrderService] {
    ambient Logger

    fn main(self) {
        self.orders.create(some_order)!
    }
}
```

## Features

**Compile-time dependency injection** -- Classes declare what they need with bracket deps `[db: Database]`. The compiler resolves the dependency graph, verifies it at compile time, and generates wiring code. No runtime container, no reflection.

**Compiler-inferred error handling** -- Functions don't declare what errors they throw. The compiler analyzes the entire call graph and infers error-ability automatically. You just handle them: `!` to propagate, `catch` to recover.

**Whole-program compilation** -- The compiler sees all your code at once. It verifies every error is handled, every dependency is satisfiable, and every type is correct across the full program.

**Native performance** -- Compiles to machine code via Cranelift. Benchmarked against C, Go, and Python across compute-heavy workloads.

**Rust-like syntax, no semicolons** -- Familiar syntax for anyone coming from Rust, Go, or TypeScript. Newline-terminated statements keep things clean.

## Quick Start

```bash
# Build the compiler
git clone https://github.com/Mkerian10/pluto.git
cd pluto
cargo build --release

# Hello world
echo 'fn main() { print("hello, world") }' > hello.pluto
./target/release/plutoc run hello.pluto

# Check the version
./target/release/plutoc --version
```

## Language Tour

### Functions and Variables

```
fn fibonacci(n: int) int {
    if n <= 1 {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    let result = fibonacci(30)
    print("fib(30) = {result}")
}
```

### Classes and Traits

```
trait HasArea {
    fn area(self) int
}

class Square impl HasArea {
    side: int

    fn area(self) int {
        return self.side * self.side
    }
}

class Rect impl HasArea {
    w: int
    h: int

    fn area(self) int {
        return self.w * self.h
    }
}

fn print_area(shape: HasArea) {
    print(shape.area())
}
```

### Closures and Higher-Order Functions

```
fn apply(f: fn(int) int, x: int) int {
    return f(x)
}

fn make_adder(n: int) fn(int) int {
    return (x: int) => x + n
}

fn main() {
    let double = (x: int) => x * 2
    print(apply(double, 10))    // 20

    let add5 = make_adder(5)
    print(add5(100))            // 105
}
```

### Enums and Pattern Matching

```
enum Shape {
    Circle { radius: float }
    Rectangle { w: float, h: float }
    Point
}

fn describe(s: Shape) string {
    let result = ""
    match s {
        Shape.Circle { radius: r } {
            result = "circle with radius {r}"
        }
        Shape.Rectangle { w: w, h: h } {
            result = "{w} x {h} rectangle"
        }
        Shape.Point {
            result = "a point"
        }
    }
    return result
}
```

### Generics

```
fn identity<T>(x: T) T {
    return x
}

class Box<T> {
    value: T
}

fn main() {
    let b = Box<int> { value: 42 }
    let name = identity("pluto")
}
```

### Nullable Types

```
fn find_user(id: int) string? {
    if id <= 0 {
        return none
    }
    return "User {id}"
}

fn main() int? {
    let user = find_user(42)?      // unwrap or propagate none
    print(user)

    let n = "123".to_int()?        // string parsing returns int?
    print(n * 2)
    return none
}
```

### Error Handling

Errors are a first-class concept. The compiler infers which functions can fail -- no annotations needed.

```
error NotFoundError { id: int }
error ValidationError { message: string }

fn find_user(id: int) string {
    if id <= 0 {
        raise ValidationError { message: "invalid id" }
    }
    if id > 1000 {
        raise NotFoundError { id: id }
    }
    return "User {id}"
}

fn main() {
    // Propagate with !
    let user = find_user(42)!

    // Handle with catch
    let result = find_user(-1) catch "unknown"
}
```

### Dependency Injection and Apps

The `app` is Pluto's entry point. Dependencies are declared, resolved at compile time, and wired automatically.

```
class Logger {
    fn info(self, msg: string) {
        print("[INFO] {msg}")
    }
}

class Database {
    fn query(self, sql: string) string {
        return "result: {sql}"
    }
}

class UserService[db: Database] uses Logger {
    fn get_user(self, id: int) string {
        logger.info("fetching user {id}")
        return self.db.query("SELECT * FROM users WHERE id = {id}")
    }
}

app MyApp[users: UserService] {
    ambient Logger

    fn main(self) {
        let result = self.users.get_user(42)
        print(result)
    }
}
```

### Built-in Test Framework

```
fn add(a: int, b: int) int {
    return a + b
}

test "addition works" {
    expect(add(1, 2)).to_equal(3)
    expect(add(-1, 1)).to_equal(0)
}

test "boolean checks" {
    expect(true).to_be_true()
    expect(1 > 2).to_be_false()
}
```

```bash
plutoc test my_tests.pluto
```

### Collections

```
fn main() {
    // Arrays
    let nums = [1, 2, 3, 4, 5]
    print(nums[0])
    print(nums.len())

    // Maps
    let scores = Map<string, int> { "alice": 95, "bob": 87 }
    print(scores["alice"])
    scores["charlie"] = 91

    // Sets
    let tags = Set<string> { "fast", "compiled", "native" }
    print(tags.contains("fast"))
}
```

### HTTP Server (with stdlib)

```
import std.http
import std.json

fn handle(req: http.Request) http.Response {
    if req.path == "/hello" {
        let body = json.object()
        body.set("message", json.string("Hello, World!"))
        return http.ok_json(body.to_string())
    }
    return http.not_found()
}

fn main() {
    let server = http.listen("0.0.0.0", 8080)!
    print("listening on :8080")

    while true {
        let conn = server.accept()!
        let req = conn.read_request()!
        conn.send_response(handle(req))
        conn.close()
    }
}
```

## Standard Library

| Module | Description |
|--------|-------------|
| `std.math` | `abs`, `min`, `max`, `pow`, `clamp` |
| `std.strings` | `substring`, `contains`, `starts_with`, `split`, `trim`, `replace`, `to_upper`, `to_lower` |
| `std.json` | JSON parsing, building, and serialization |
| `std.http` | HTTP server with request/response handling |
| `std.fs` | File I/O: read, write, seek, directory operations |
| `std.net` | TCP listener and connection wrappers |
| `std.socket` | Low-level socket operations |
| `std.collections` | `map`, `filter`, `fold`, `reduce`, `zip`, `enumerate`, and more |
| `std.time` | Wall-clock time, monotonic clocks, sleep, elapsed |
| `std.random` | Random integers, floats, coin flips, seeded RNG |
| `std.io` | `println` and `print` |

## Compiler

```
plutoc compile main.pluto -o myapp    # Compile to native binary
plutoc run main.pluto                 # Compile and run
plutoc test tests.pluto               # Run test blocks
plutoc --version                      # Print version
```

The compiler pipeline: **Lex** &#8594; **Parse** &#8594; **Module Resolve** &#8594; **Flatten** &#8594; **Closure Lift** &#8594; **Type Check** &#8594; **Monomorphize** &#8594; **Codegen** (Cranelift) &#8594; **Link**

Supported targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`

## Benchmarks

Pluto ships with a benchmark suite covering fibonacci, sorting, N-body simulation, FFT, spectral norm, and more. Run them with:

```bash
benchmarks/run_benchmarks.sh
```

Cross-language comparison against C (-O2), Go, and Python:

```bash
benchmarks/compare.sh
```

## Project Status

Pluto is in early development (v0.1). The compiler supports a substantial set of features but the language is not yet stable. See [SPEC.md](SPEC.md) for the full language specification and [docs/design/](docs/design/) for detailed design documents.

### What works today
- Functions, classes, traits, enums, generics, closures
- Compile-time dependency injection with `app`
- Typed error handling with compiler inference
- First-class nullable types (`T?`, `none`, `?` operator)
- Concurrency (`spawn`, `Task<T>`, channels, `select`)
- Design-by-contract (invariants, requires/ensures, interface guarantees)
- Modules, imports, and package dependencies (local + git)
- Maps, sets, arrays, string interpolation
- Built-in test framework
- Standard library (JSON, HTTP, filesystem, networking, collections, time, random)
- LSP with diagnostics, go-to-definition, hover, and document symbols
- Native compilation on macOS and Linux (ARM64, x86_64)

### What's ahead
- Distribution (cross-pod RPC, geographic awareness)
- Orchestration layer
- LLVM backend
- Package manager (registry)

## License

All rights reserved.

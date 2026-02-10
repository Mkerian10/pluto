# Rust FFI

Pluto can call functions from Rust crates directly using `extern rust`. This lets you leverage the Rust ecosystem from Pluto with zero boilerplate.

## Basic Usage

Given a Rust crate with `pub fn` functions:

```rust
// my_math/src/lib.rs
pub fn fibonacci(n: i64) -> i64 {
    if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn is_even(n: i64) -> bool {
    n % 2 == 0
}
```

Import it in Pluto with `extern rust`:

```
extern rust "./my_math" as math

fn main() {
    print(math.fibonacci(10))   // 55
    print(math.add(1.5, 2.5))   // 4.0
    print(math.is_even(42))     // true
}
```

The path is relative to your `.pluto` file and points to a Rust crate directory (containing `Cargo.toml`).

## How It Works

The compiler:

1. Reads the Rust crate's `src/lib.rs` to find `pub fn` signatures
2. Generates C-ABI glue code that bridges Pluto's calling convention to Rust's
3. Builds the Rust crate as a static library (`cargo build --release`)
4. Links everything together into a single binary

All of this happens automatically at compile time.

## Supported Types

The FFI bridges these types between Pluto and Rust:

| Pluto Type | Rust Type |
|------------|-----------|
| `int` | `i64` |
| `float` | `f64` |
| `bool` | `bool` |

Functions with unsupported parameter or return types (structs, strings, references, etc.) are silently skipped -- they won't be available from Pluto.

## Compiling

Compile and run as usual:

```bash
plutoc run my_program.pluto
```

The compiler detects `extern rust` declarations and handles the Rust build automatically. The Rust crate must be a valid Cargo project with a `Cargo.toml` and `src/lib.rs`.

## Example: Complete Project

Directory structure:

```
my_project/
  main.pluto
  my_math/
    Cargo.toml
    src/
      lib.rs
```

`my_math/Cargo.toml`:

```toml
[package]
name = "my_math"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["staticlib"]
```

`my_math/src/lib.rs`:

```rust
pub fn factorial(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * factorial(n - 1) }
}
```

`main.pluto`:

```
extern rust "./my_math" as math

fn main() {
    for i in 1..8 {
        print("{i}! = {math.factorial(i)}")
    }
}
```

## Limitations

- Only `pub fn` at the crate root are bridged (not methods, trait impls, or nested modules)
- Only `i64`, `f64`, and `bool` types are supported
- The Rust crate must use `crate-type = ["staticlib"]` in its `Cargo.toml`
- Rust functions that take `&self`, `&str`, `String`, or other complex types are skipped

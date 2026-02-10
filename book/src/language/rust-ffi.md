# Rust FFI

Pluto can call functions from Rust crates directly. You declare the crate as an `extern rust` dependency, and the compiler handles building, linking, and type bridging.

## Declaring an Extern Crate

```
extern rust "./my_math" as math

fn main() {
    print("fib(10) = {math.fibonacci(10)}")
    print("is_even(4) = {math.is_even(4)}")
}
```

The path is relative to the Pluto source file. The `as` keyword assigns a namespace -- all functions from the crate are accessed through it (e.g., `math.fibonacci`).

The compiler discovers all `pub fn` declarations in the crate's `src/lib.rs`, matches their signatures to Pluto types, and makes them available under the given namespace.

## Supported Types

Only primitive types cross the FFI boundary:

| Rust type | Pluto type |
|-----------|------------|
| `i64`     | `int`      |
| `f64`     | `float`    |
| `bool`    | `bool`     |
| `()`      | `void`     |

Functions with unsupported parameter or return types (e.g., `&str`, `String`, `Vec<T>`) are silently skipped. They will not appear in the namespace.

## Fallible Functions

Rust functions returning `Result<T, E>` where `T` is a supported type and `E: ToString` are bridged as fallible Pluto functions. The compiler handles the error conversion automatically:

```
extern rust "./my_math" as math

fn main() {
    let result = math.safe_divide(10.0, 3.0)!
    print(result)
    let fallback = math.safe_divide(10.0, 0.0) catch -1.0
    print(fallback)
}
```

The standard Pluto error handling rules apply: you must use `!` to propagate or `catch` to handle. Calling a fallible FFI function without handling the error is a compile error.

## The Rust Side

The Rust crate is a normal `staticlib` crate. No special attributes, no `#[no_mangle]`, no `extern "C"`. The compiler generates the glue code.

**`my_math/Cargo.toml`:**

```toml
[package]
name = "my_math"
version = "0.1.0"
edition = "2021"

[lib]
```

**`my_math/src/lib.rs`:**

```rust
pub fn fibonacci(n: i64) -> i64 {
    if n <= 1 { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
}

pub fn is_even(n: i64) -> bool {
    n % 2 == 0
}

pub fn safe_divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 { Err("division by zero".to_string()) } else { Ok(a / b) }
}
```

## Compiling

```
$ plutoc compile main.pluto -o main
$ ./main
```

The compiler detects `extern rust` declarations, runs `cargo build --release` on each crate, and links the resulting static library into the final binary. No separate build step needed.

## Project Structure

A typical project with Rust FFI:

```
project/
  main.pluto
  my_math/
    Cargo.toml
    src/
      lib.rs
```

## Limitations

- Only root-level `pub fn` declarations are bridged. Methods, trait impls, and nested functions are ignored.
- Only `i64`, `f64`, `bool`, and `()` cross the boundary (plus `Result<T, E>` wrappers for fallible functions).
- `extern rust` declarations must be in the root program file, not in imported modules.
- Each extern crate must have a unique alias. Duplicate aliases are a compile error.
- An `extern rust` alias cannot conflict with an `import` alias.
- Panics in Rust FFI functions abort the process with a diagnostic message.

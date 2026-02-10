# Pluto Examples

## strings

Demonstrates built-in string methods (`contains`, `starts_with`, `trim`, `to_upper`, `replace`, `split`, etc.), string indexing (`s[0]`), character iteration (`for c in s`), and method chaining.

```bash
cargo run -- run examples/strings/main.pluto
```

## channels

Demonstrates channels for inter-task communication: `let (tx, rx) = chan<T>(capacity)`, blocking `send`/`recv`, non-blocking `try_send`/`try_recv`, `close()`, `for-in` iteration on receivers, and error handling with `catch`.

```bash
cargo run -- run examples/channels/main.pluto
```

## select

Demonstrates `select` for channel multiplexing: waiting on multiple channels simultaneously, fan-in patterns with two producers, non-blocking select with `default`, and error handling when all channels close.

```bash
cargo run -- run examples/select/main.pluto
```

## concurrency

Demonstrates `spawn` for concurrent execution: spawning functions on separate threads, collecting results with `.get()`, error handling with `catch`, and void tasks.

```bash
cargo run -- run examples/concurrency/main.pluto
```

## rust_ffi

Demonstrates calling plain Rust functions from Pluto via `extern rust`. A normal Rust crate with `pub fn` functions is imported with zero boilerplate — supported types (`i64`, `f64`, `bool`) are bridged automatically.

```bash
cargo run -- run examples/rust_ffi/main.pluto
```

## testing

Demonstrates Pluto's built-in test framework with `test` blocks, `expect()` assertions, and multiple assertion methods (`to_equal`, `to_be_true`, `to_be_false`).

```bash
cargo run -- test examples/testing/main.pluto
```

## json

Demonstrates the `std.json` module: parsing JSON strings, accessing nested values, building JSON programmatically, and round-tripping through stringify/parse.

```bash
cargo run -- run examples/json/main.pluto --stdlib stdlib
```

## bytes

Demonstrates the `byte` and `bytes` types: hex literals (`0xFF`), explicit casting (`as byte`/`as int`), truncation semantics, packed byte buffers (`bytes_new`, `push`, indexing), string conversion (`to_bytes`/`to_string`), iteration, and unsigned ordering.

```bash
cargo run -- run examples/bytes/main.pluto
```

## packages

Demonstrates local path dependencies via `pluto.toml`. A project declares a `mathlib` dependency pointing to a local directory, then imports and uses functions and classes from it.

```bash
cargo run -- run examples/packages/main.pluto
```

## git-packages

Demonstrates git-based dependencies via `pluto.toml`. A project declares a `strutils` dependency pointing to a git repository, then imports and uses string utility functions from it.

```bash
cargo run -- run examples/git-packages/main.pluto
```

## http-api

A simple JSON API server using `std.http` and `std.json`. Demonstrates listening for HTTP requests, routing by path, parsing JSON request bodies, and returning JSON responses.

```bash
cargo run -- run examples/http-api/main.pluto --stdlib stdlib
# Then in another terminal:
# curl http://localhost:8080/hello
# curl -X POST -d '{"name":"Alice"}' http://localhost:8080/echo
```

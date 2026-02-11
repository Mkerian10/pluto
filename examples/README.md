# Pluto Examples

## strings

Comprehensive demonstration of string manipulation: basic operations (`len`), case conversion (`to_upper`, `to_lower`), trimming (`trim`, `trim_start`, `trim_end`), substring operations, character access (`char_at`, `byte_at`), string searching (`contains`, `starts_with`, `ends_with`, `index_of`, `last_index_of`, `count`), string replacement, splitting, repetition, concatenation, empty/whitespace checking, and number parsing with nullable types.

```bash
cargo run -- run examples/strings/main.pluto --stdlib stdlib
```

## paths

Demonstrates the `std.path` module for path manipulation: `join` (path joining with separator handling), `basename` (extract filename), `dirname` (extract directory), `ext` (file extension), `is_absolute`, `has_trailing_slash`, `normalize` (resolve . and ..), and `split_ext` (filename/extension split).

```bash
cargo run -- run examples/paths/main.pluto --stdlib stdlib
```

## env_example

Demonstrates the `std.env` module for environment variable access: `get` (retrieve variable or empty string), `get_or` (with default fallback), `set` (set variable), `exists` (check if set), `remove` (delete variable), and `list_names` (enumerate all variables).

```bash
cargo run -- run examples/env_example/main.pluto --stdlib stdlib
```

## logging

Demonstrates structured logging with `std.log`: setting log levels (`DEBUG`, `INFO`, `WARN`, `ERROR`), logging messages at different levels, and controlling which messages are displayed based on the current log level.

```bash
cargo run -- run examples/logging/main.pluto --stdlib stdlib
```

## modules

Demonstrates the module system: `import` for importing modules, `pub` visibility for exported items, module organization with separate files, and accessing public functions and classes from imported modules.

```bash
cargo run -- run examples/modules/main.pluto
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

## blog

A static blog generator that reads markdown-ish `.txt` posts from `posts/`, converts them to HTML with inline formatting (`**bold**`, `*italic*`, `` `code` ``, headings, lists), and writes a full site to `output/`. Demonstrates `std.fs` (file I/O, directory listing), `std.strings` (split, trim, replace, index_of), error handling, and the `app` construct.

```bash
cd examples/blog
cargo run --manifest-path ../../Cargo.toml -- run main.pluto --stdlib ../../stdlib
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

## pattern_matching

Demonstrates enum pattern matching: unit variants (no data), data-carrying variants with field destructuring, mixed variants (unit and data), exhaustiveness checking, and nested pattern matching within match arms.

```bash
cargo run -- run examples/pattern_matching/main.pluto
```

## git-packages

Demonstrates git-based dependencies via `pluto.toml`. A project declares a `strutils` dependency pointing to a git repository, then imports and uses string utility functions from it.

```bash
cargo run -- run examples/git-packages/main.pluto
```

## contracts

Demonstrates Pluto's contract system: `requires` (preconditions), `ensures` (postconditions), `old()` for capturing values at function entry, the `result` keyword in postconditions, and class `invariant` declarations.

```bash
cargo run -- run examples/contracts/main.pluto
```

## errors

Demonstrates Pluto's typed error system: error declarations with multiple error types, `raise` to throw errors, `!` postfix for error propagation, `catch` with wildcard error handling, shorthand catch with default values, and compiler-inferred error-ability (no annotations needed on intermediate functions).

```bash
cargo run -- run examples/errors/main.pluto
```

## binary-ast

Demonstrates the binary AST commands: `emit-ast` serializes a Pluto source file into a binary `.pluto` AST (with UUIDs and cross-references), and `generate-pt` reads a binary AST back into human-readable Pluto source.

```bash
# Serialize source to binary AST
cargo run -- emit-ast examples/binary-ast/main.pluto -o /tmp/main.pluto

# Read binary AST back to text
cargo run -- generate-pt /tmp/main.pluto
```

## collections-lib

Demonstrates the `std.collections` functional collections library: `map`, `filter`, `fold`, `reduce`, `any`, `all`, `count`, `flat_map`, `for_each`, `reverse`, `take`, `drop`, `zip` (with `Pair`), `enumerate`, `flatten`, `sum`, and `sum_float`. Shows function composition by chaining filter, map, and fold.

```bash
cargo run -- run examples/collections-lib/main.pluto --stdlib stdlib
```

## stdin

Demonstrates interactive I/O with `std.io`: reading input with `io.read_line()`, parsing strings to numbers with `.to_int()` and `.to_float()` (both return nullable types — use `?` to propagate none on invalid input), and string interpolation for output.

```bash
echo -e "Alice\n21\n72" | cargo run -- run examples/stdin/main.pluto --stdlib stdlib
```

## time

Demonstrates the `std.time` module: wall-clock time (`now`, `now_ns`), monotonic clocks (`monotonic`, `monotonic_ns`), sleeping (`sleep`), and measuring elapsed time (`elapsed`).

```bash
cargo run -- run examples/time/main.pluto --stdlib stdlib
```

## random

Demonstrates the `std.random` module: random integers (`next`, `between`), random floats (`decimal`, `decimal_between`), coin flips (`coin`), and seeded determinism (`seed`).

```bash
cargo run -- run examples/random/main.pluto --stdlib stdlib
```

## nullable

Demonstrates first-class nullable types: `T?` syntax for nullable types, `none` literal for absent values, `?` postfix operator for null propagation (early-return none), implicit `T` to `T?` coercion, nullable classes, and `to_int()`/`to_float()` string parsing returning nullable types.

```bash
cargo run -- run examples/nullable/main.pluto
```

## scope-blocks

Demonstrates scoped dependency injection with `scope()` blocks: creating per-request scoped class instances from seed values, auto-wiring dependency chains (`Handler` -> `UserService` -> `RequestCtx`), mixing scoped and singleton deps, and binding multiple services from a single seed.

```bash
cargo run -- run examples/scope-blocks/main.pluto
```

## system

Demonstrates the `system` declaration for multi-app distributed systems. A system file composes multiple app modules (each with their own `app` declaration and DI graph) into named deployment members. The compiler produces one binary per member.

```bash
cargo run -- compile examples/system/main.pluto -o /tmp/system_build
/tmp/system_build/api_server
/tmp/system_build/background
```

## generics

Demonstrates advanced generics: generic classes implementing traits (`class Box<T: Printable> impl Printable`), type bounds on generic parameters (`<T: Trait1 + Trait2>`), explicit type arguments on function calls (`make_pair<string, int>(...)`), and dependency injection on generic classes (`class Repository<T>[db: Database]`).

```bash
cargo run -- run examples/generics/main.pluto
```

## stages

Demonstrates the `stage` language construct — a deployable unit for distributed systems. A stage is like `app` but designed as a future RPC boundary. Shows DI with bracket deps (`stage Api[users: UserService]`), `pub` methods (marking future RPC endpoints), private helper methods, and a `main` entry point.

```bash
cargo run -- run examples/stages/main.pluto
```

## generators

Demonstrates generators with `stream T` return types and `yield`: lazy integer ranges, infinite Fibonacci sequence with early `break`, and composing multiple generators.

```bash
cargo run -- run examples/generators/main.pluto
```

## http-api

A simple JSON API server using `std.http` and `std.json`. Demonstrates listening for HTTP requests, routing by path, parsing JSON request bodies, and returning JSON responses.

```bash
cargo run -- run examples/http-api/main.pluto --stdlib stdlib
# Then in another terminal:
# curl http://localhost:8080/hello
# curl -X POST -d '{"name":"Alice"}' http://localhost:8080/echo
```

## uuid

Demonstrates the `std.uuid` module for generating RFC 4122 v4 UUIDs: generating random UUIDs (`generate()`), checking UUID structure and uniqueness, and using UUIDs as identifiers.

```bash
cargo run -- run examples/uuid/main.pluto --stdlib stdlib
```

## base64

Demonstrates the `std.base64` module for encoding and decoding Base64: basic encoding (`encode`), decoding (`decode`), URL-safe variants (`encode_url_safe`, `decode_url_safe`), and roundtrip encoding/decoding.

```bash
cargo run -- run examples/base64/main.pluto --stdlib stdlib
```

## regex

Demonstrates the `std.regex` module for pattern matching: literal matching (`matches`), finding patterns (`find`, `find_all`), text replacement (`replace`, `replace_all`), splitting text by pattern (`split`), wildcards (`.`), quantifiers (`*`, `+`, `?`), anchors (`^`, `$`), and character shortcuts (`\d`, `\w`, `\s`).

```bash
cargo run -- run examples/regex/main.pluto --stdlib stdlib
```

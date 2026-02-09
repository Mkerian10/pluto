# Open Design Questions

Areas that need further design work before implementation.

## Language Features

- [ ] **Null / optional** — how are absent values represented? Option type? nullable types?
- [ ] **Range syntax** — `0..n`, `0..=n` for loops and slices
- [ ] **`loop` keyword** — infinite loop construct (currently use `while true`)
- [ ] **`break` / `continue`** — loop control flow (currently use early `return`)

## Communication

- [ ] **Process spawning** — `spawn` syntax and semantics, process identity
- [ ] **Geographic annotations** — syntax for region/locality constraints
- [ ] **Service discovery** — how do apps find each other?
- [ ] **Channels** — `chan<T>()`, directional types, backpressure strategies
- [ ] **Cross-pod calls** — compiler-generated RPC code, serialization format

## Runtime

- [ ] **Configuration format** — how do you configure DI bindings, region constraints, restart policies?
- [ ] **Supervision strategies** — one-for-one, one-for-all, rest-for-one?
- [ ] **Observability** — built-in metrics, tracing, logging hooks?
- [ ] **Runtime ↔ orchestration interface** — how do they communicate?

## Dependency Injection

- [ ] **Provider registration** — how are DI bindings configured per environment?
- [ ] **Lifecycle** — singleton vs per-request vs per-process
- [ ] **Module ↔ app relationship** — can a module contain an app? how do apps compose?

## Concurrency

- [ ] **Concurrency primitives** — mutexes, atomics, or purely message-based?
- [ ] **Shared state** — is shared mutable state ever allowed, or is everything message-passing?
- [ ] **Async/await** — is there an async model, or is everything synchronous + spawned processes?

## Tooling

- [ ] **Testing** — built-in test framework, distributed testing support
- [ ] **Standard library** — scope and core modules
- [ ] **Package manager** — dependency resolution for libraries
- [ ] **Formatter / linter** — built-in code formatting (like `go fmt`)
- [ ] **LSP** — language server for IDE support

---

## Resolved

Previously open questions that have been designed and implemented.

- [x] **Pattern matching** — `match` with exhaustiveness checking on enums (unit + data variants)
- [x] **Enums / union types** — enum declarations with unit and data-carrying variants
- [x] **Closures / lambdas** — arrow syntax `(x: int) => x + 1`, capture by value
- [x] **String interpolation** — `"hello {name}"` with `{expr}` syntax
- [x] **Variable declarations** — `let` and `let mut` with type inference
- [x] **Control flow** — `if/else`, `match`, `for`, `while` with newline-based termination
- [x] **Primitive types** — `int` (I64), `float` (F64), `bool` (I8), `string` (heap), `void`
- [x] **Error handling** — `error` declarations, `raise`, `!` propagation, `catch` shorthand/wildcard
- [x] **Imports** — `import module` syntax with qualified access (`module.item`)
- [x] **Visibility** — `pub` keyword; private by default
- [x] **Namespacing** — dot-separated qualified names (`math.add`)
- [x] **DI scope** — bracket deps `class Foo[dep: Type]` in classes, app bracket deps
- [x] **Ambient DI** — `uses` on classes, `ambient` in app, bare variable access desugared to `self.field`
- [x] **DI depth** — transitive DI with topological sort, cycle detection at compile time
- [x] **Early return** — `return` in functions and methods
- [x] **Arrays** — literal syntax, indexing, `push`, `len`, `for-in` iteration
- [x] **Extern functions** — `extern fn` declarations for FFI with C runtime
- [x] **Generics** — monomorphization strategy, `fn first<T>(items: [T]) T`, `class Box<T>`, `enum Option<T>`
- [x] **Collections** — built-in `Map<K, V>` and `Set<T>` with hash-table implementation
- [x] **Garbage collection** — mark-and-sweep GC in the C runtime, tag-based tracing
- [x] **Standard library (core)** — `std.strings`, `std.math`, `std.net`, `std.socket`, `std.fs`
- [x] **String escape sequences** — `\n`, `\r`, `\t`, `\\`, `\"`

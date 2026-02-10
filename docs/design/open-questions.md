# Open Design Questions

Areas that need further design work before implementation.

## Language Features

- [x] **Null / optional** — first-class nullable types (`T?`, `none`, `?` operator). `T?` for any type, `none` for absent, `?` for null propagation. Compiler infers nullability transitively.

## Communication

- [ ] **Geographic annotations** — syntax for region/locality constraints
- [ ] **Service discovery** — how do apps find each other?
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

- [ ] **Move semantics on spawn** — how does move-on-spawn interact with closures? Explicit `move` annotation?
- [ ] **Task groups / scopes** — structured concurrency construct for managing multiple tasks with automatic cancellation?
- [x] **Select / race** — waiting on the first of multiple tasks or channels to complete (implemented as `select` statement)

## Contracts

- [ ] **Contract inheritance on generics** — how do invariants interact with generics? Does `Box<T>` inherit T's invariants?
- [ ] **Quantifiers** — should a future version support bounded quantifiers (`forall item in self.items: item.price > 0`)?
- [ ] **Contract testing mode** — `@test` mode that inserts runtime assertions for all contracts (for debugging)?
- [ ] **`old()` deep copy semantics** — what values can `old()` capture? Deep clone for heap types?
- [ ] **Protocol composition** — can protocols be composed or extended?
- [ ] **`@assume` scope** — should `@assume` apply to a single call, a block, or an entire function?
- [ ] **Gradual adoption** — should contracts be opt-in per module, or always enforced?

## AI-Native Representation

- [ ] **Binary format** — protobuf, FlatBuffers, Cap'n Proto, or custom? Needs benchmarking
- [ ] **Derived data staleness** — how does the compiler detect stale derived data? Content hash? Version counter?
- [ ] **Incremental analysis** — can `plutoc analyze` update only affected derived data, or full recompute?
- [ ] **Cross-project UUIDs** — UUID namespace management across library boundaries
- [ ] **SDK language bindings** — Rust crate is primary, but AI agents may need Python/TS bindings (FFI? gRPC?)
- [ ] **Diff tooling** — custom `git diff` driver for binary `.pluto` files, or rely on `.pt` diffs?
- [ ] **IDE integration** — editors work with `.pt` and sync on save? Or SDK-powered LSP on `.pluto` directly?
- [ ] **Concurrent SDK access** — multiple AI agents editing same `.pluto` file (locking? CRDT?)

## Tooling

- [ ] **Testing** — built-in test framework, distributed testing support
- [ ] **Standard library** — scope and core modules
- [ ] **Package manager** — dependency resolution for libraries
- [ ] **Formatter / linter** — built-in code formatting (like `go fmt`)

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
- [x] **Generics** — monomorphization strategy, `fn first<T>(items: [T]) T`, `class Box<T>`, generic enums
- [x] **Collections** — built-in `Map<K, V>` and `Set<T>` with hash-table implementation
- [x] **Garbage collection** — mark-and-sweep GC in the C runtime, tag-based tracing
- [x] **Standard library (core)** — `std.strings`, `std.math`, `std.net`, `std.socket`, `std.fs`
- [x] **String escape sequences** — `\n`, `\r`, `\t`, `\\`, `\"`
- [x] **Concurrency model** — tasks (green threads) + OS threads, no shared mutable state, channels for communication
- [x] **Concurrency primitives** — message-passing only, no mutexes/atomics exposed to user code
- [x] **Spawn semantics** — `spawn` returns `Task<T>`, `.get()` is fallible (preserves error types + TaskCancelled)
- [x] **Structured concurrency** — tasks must be consumed (`.get()` or `.detach()`), structured by default
- [x] **Contract system** — 5-type contract stack: invariants, pre/post conditions, protocol contracts, failure semantics, interface guarantees. Compile-time first with runtime checks at boundaries.
- [x] **`break` / `continue`** — loop control flow with `break` to exit and `continue` to skip to next iteration. Validated at compile time (must be inside loop, cannot escape closures)
- [x] **Range syntax** — `0..n` (exclusive) and `0..=n` (inclusive) for integer iteration in `for` loops
- [x] **`loop` keyword** — rejected; use `while true` instead. No dedicated infinite loop construct needed
- [x] **Channels** — `chan<T>()` with `Sender<T>`/`Receiver<T>`, blocking/non-blocking send/recv, for-in iteration, error integration
- [x] **LSP** — language server (`plutoc lsp`) with diagnostics, go-to-definition, hover, and document symbols. Zed extension with tree-sitter grammar for syntax highlighting.

# Open Design Questions

Areas that need further design work before implementation.

## Language Features

- [ ] **Generics / type parameters** — syntax, constraints, monomorphization vs type erasure
- [ ] **Pattern matching** — syntax, exhaustiveness checking, integration with error handling
- [ ] **Enums / union types** — beyond the `error` keyword, general-purpose algebraic data types
- [ ] **Closures / lambdas** — syntax, capture semantics (by value? by reference? move?)
- [ ] **String interpolation** — syntax for format strings (the `{order.id}` syntax used in examples)
- [ ] **Variable declarations** — `let`, `let mut`, type inference rules
- [ ] **Control flow** — `if/else`, `match`, `for`, `while`, `loop` — exact syntax
- [ ] **Primitive types** — concrete types, sizes, numeric tower (i32, i64, int, float, etc.)
- [ ] **Collections** — built-in List, Map, Set? or stdlib?
- [ ] **Null / optional** — how are absent values represented? Option type? nullable types?

## Module System

- [ ] **Imports** — syntax for importing from other modules
- [ ] **Visibility** — public/private modifiers, what's the default?
- [ ] **Namespacing** — how are names organized?
- [ ] **Module ↔ app relationship** — can a module contain multiple apps?

## Dependency Injection Details

- [ ] **Scope** — where can `inject` appear? Any class? Only app-level?
- [ ] **Depth** — can deeply nested classes declare `inject`?
- [ ] **Provider registration** — how are DI bindings configured?
- [ ] **Lifecycle** — singleton vs per-request vs per-process

## Communication Details

- [ ] **Process spawning** — `spawn` syntax and semantics, process identity
- [ ] **Geographic annotations** — syntax for region/locality constraints
- [ ] **Service discovery** — how do apps find each other?
- [ ] **Backpressure strategies** — beyond channel buffering

## Runtime Details

- [ ] **Configuration format** — how do you configure DI bindings, region constraints, restart policies?
- [ ] **Supervision strategies** — one-for-one, one-for-all, rest-for-one?
- [ ] **Observability** — built-in metrics, tracing, logging hooks?
- [ ] **Runtime ↔ orchestration interface** — how do they communicate?

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
- [ ] **REPL** — interactive evaluation (may not make sense for whole-program compilation)

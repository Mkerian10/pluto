# Program Structure

Pluto programs are organized around a three-layer model. Each layer addresses a different concern:

| Layer | Concern | Construct | Has Fields? | Has DI? |
|-------|---------|-----------|-------------|---------|
| **Data & Behavior** | What does this thing do? | `class` | Yes | Yes |
| **Lifecycle & Wiring** | How does this thing run? | Stages | No | Yes |
| **Topology** | What things exist and how do they relate? | System (future) | — | — |

The data layer provides the building blocks (classes with state, behavior, contracts). The lifecycle layer orchestrates them (stages wire dependencies and define runtime behavior). The topology layer composes multiple stages into a distributed system.

## The Three Layers

### Layer 1: Classes — Data + Behavior + State

Classes are the workhorses. They hold state, define behavior, declare dependencies, and enforce contracts:

```
class OrderProcessor[db: Database, notifier: Notifier] {
    processed_count: int

    invariant self.processed_count >= 0

    fn handle(mut self, order: Order) {
        self.db.insert(order)!
        self.notifier.send("Order {order.id} created")!
        self.processed_count = self.processed_count + 1
    }
}
```

A class can have:
- **Fields** — mutable state it owns
- **Bracket deps** — injected dependencies (`[db: Database]`)
- **Methods** — behavior operating on state and deps
- **Contracts** — invariants, requires, ensures
- **Trait implementations** — structural conformance

Classes do NOT know how they're orchestrated. `OrderProcessor` doesn't know if it's called from an HTTP handler, a queue consumer, or a test. That's the entry point's job.

### Layer 2: Stages — Lifecycle + DI, No State

Stages are a distinct construct from classes. They define **how a program runs** — startup, shutdown, the main execution loop — and wire together the classes that do the actual work.

The critical constraint: **stages have DI but no fields.** They cannot hold state. All state lives in the injected classes.

```
daemon OrderWorker[queue: MessageQueue, processor: OrderProcessor] {
    fn run(self) {
        for msg in self.queue.subscribe("orders") {
            self.processor.handle(msg.body)!
        }
    }

    fn shutdown(self) {
        self.queue.close()
    }
}
```

This separation is deliberate:
- Stages are **thin lifecycle wrappers**. They say "start here, stop here, wire these pieces."
- Classes are **where work happens**. They hold state, enforce invariants, encapsulate logic.
- Because stages can't hold state, they can't become god objects. The architecture stays clean.

Stages are **programmable** — library authors and users can define new kinds of stages using inheritance. The `stage` keyword defines a lifecycle template, and the template's name becomes a keyword for instantiation (e.g., `stage Daemon { ... }` lets users write `daemon MyWorker { ... }`). See [RFC: Stages](rfc-entry-points.md) for the full design.

### Layer 3: System / Graph — Topology (Future)

The system layer sits above individual stages and defines the topology of a distributed application: which stages exist, how they communicate, where they deploy.

```
// Future — syntax TBD
system OrderPlatform {
    api: OrderApi              // an http_server stage
    worker: OrderWorker        // a daemon stage
    reporter: DailyReport      // a scheduled_job stage
}
```

This is where Pluto's distributed systems story comes together — the compiler sees the full graph of stages and can generate RPC code, validate contracts across service boundaries, and inform the orchestration layer.

The system layer is not yet designed. See [Open Questions](#open-questions) below.

## Project Kinds

Pluto has four distinct kinds of project, each serving a different purpose:

| Kind | Contains | Entry Point | DI? | Use Case |
|------|----------|-------------|-----|----------|
| **Library** | Declarations only | None | No | Reusable code, imported by others, tool surfaces for AI agents |
| **Script** | Declarations + top-level statements | Auto-generated | No | Quick experiments, prototyping, one-off tasks |
| **App** | Declarations + a stage instance | Defined by the stage | Yes | Services, workers, CLI tools, scheduled jobs |
| **System** | Stage composition | Orchestration entry | Yes | Distributed applications, multi-process topology |

### Detection Rules

The compiler auto-detects the project kind. The rules are unambiguous — every file falls into exactly one category:

1. **App file:** Contains a stage instance declaration (`app`, `daemon`, `http_server`, or any user-defined stage). Exactly one per file.
2. **System file:** Contains a `system` declaration. Composes multiple stage instances into a distributed application.
3. **Script file:** Contains one or more top-level statements (any executable code outside a declaration). No stage instance, no system.
4. **Library file:** Contains only declarations (functions, classes, traits, enums, errors, stage definitions). No top-level statements, no stage instance, no system.

Conflict rules:
- A file with **both** a stage instance and top-level statements is a **compile error**. Stages define their own lifecycle; ambient statements contradict that.
- A file with **multiple** stage instances is a **compile error**. One stage instance per compilation unit.
- A file with **both** a stage instance and a system declaration is a **compile error**. A system composes stages; it doesn't contain them inline.

Note: **stage definitions** (`stage Daemon { ... }`) are library declarations — they define a template, not an entry point. **Stage instances** (`daemon MyWorker { ... }`) are entry points — they use a template to create a runnable program.

### Scripts

Scripts are the lowest-ceremony way to write Pluto. Top-level statements execute in order. Declarations are hoisted (available before the line they appear on):

```
// This is a script — it has top-level statements
import std.math

fn double(n: int) int {
    return n * 2
}

let x = 42
print("double({x}) = {double(x)}")
print("sqrt(16) = {std.math.sqrt(16.0)}")
```

The compiler wraps the top-level statements in a synthetic `main()`. Declarations (functions, classes, etc.) are extracted and registered before any statements execute.

Scripts are useful for:
- Quick experiments and prototyping
- One-off data processing
- Learning Pluto
- Simple automation tasks

Scripts do NOT support DI (there's no bracket dep syntax for a script). If you need DI, use an entry point.

### Stage Instance Files

Stage instance files contain a stage instance — a concrete use of a stage template that defines how the program runs. `App` is the most basic stage (defined in stdlib); more specialized stages like `Daemon`, `HttpServer`, etc. provide richer lifecycle patterns.

```
// This is a stage instance file — it uses the App stage
import std.http

class Router[db: Database] {
    fn handle(self, req: http.Request) http.Response {
        // ...
    }
}

app MyApi[router: Router] {
    fn main(self) {
        http.serve(8080, self.router)
    }
}
```

Or with a more specific stage:

```
// This uses the HttpServer stage (inherits from Daemon)
import std.http.HttpServer

http_server MyApi[db: Database] {
    fn routes(self) Router { ... }
    fn port(self) int { return 8080 }
    fn shutdown(self) { self.db.close() }
}
```

Stage instance files can contain any number of supporting declarations (classes, functions, traits, etc.) alongside the single stage instance.

### Libraries

Library files contain only declarations — no entry point, no top-level statements. They exist to be imported by other files:

```
// This is a library — only declarations, all pub
pub class Vector {
    x: float
    y: float
}

pub fn dot(a: Vector, b: Vector) float {
    return a.x * b.x + a.y * b.y
}

pub fn magnitude(v: Vector) float {
    return sqrt(v.x * v.x + v.y * v.y)
}
```

Libraries are the default — if a file has no entry point and no top-level statements, it's a library. Every module imported via `import` is a library.

Libraries are especially powerful in an AI-native context. Because they expose typed functions and classes without requiring an entry point, they naturally become **tool surfaces for AI agents**. An MCP server can introspect a Pluto library's public declarations and expose them as callable tools — no HTTP server, no CLI wrapper, no glue code. The library's type signatures, error sets, and contracts give agents everything they need to call functions correctly.

### Systems

System files define the **topology** of a distributed application — which stages exist, how they relate, and how they communicate. A system composes multiple stage instances into a single deployable unit:

```
// Future — syntax TBD
system OrderPlatform {
    api: OrderApi              // an http_server stage
    worker: OrderWorker        // a daemon stage
    reporter: DailyReport      // a scheduled_job stage
}
```

Systems are Pluto code — not YAML manifests, not separate config files. This means the full power of the type system, error checking, and contracts applies to your deployment topology. The compiler sees the complete graph and can:

- Generate RPC code between stages
- Validate contracts across service boundaries
- Infer communication patterns (channels, queues, direct calls)
- Inform the orchestration layer about scaling and isolation requirements

The system layer is the orchestration layer written **in** Pluto. It takes in stages and manages their lifecycle — starting them, stopping them, wiring their communication. Because systems are code, they can compose stages from different libraries, override DI bindings for different environments, and even manage non-Pluto workloads.

The system kind is not yet designed in detail. See [Open Questions](#open-questions) and [RFC: Stages](rfc-entry-points.md).

## The Programming Model

A Pluto program is a graph of classes wired together by the compiler. The programmer declares what each class needs (dependencies) and what it does (methods). The compiler handles everything else: wiring, lifecycle, concurrency safety, error propagation, and (eventually) distribution.

### Classes as Roles

Each class in a Pluto program plays a **role** in the system. A class encapsulates:

1. **State** — fields it owns and manages
2. **Behavior** — methods that operate on that state
3. **Dependencies** — other classes it needs (injected by the compiler)
4. **Contracts** — invariants that must hold, pre/post conditions on methods

The combination is powerful. A class isn't just a data structure with methods — it's a self-contained unit that can own state, sync with external systems, enforce its own correctness, and participate safely in concurrent execution:

```
class FeatureFlagService[store: FlagStore] {
    flags: Map<string, bool>

    fn is_enabled(self, flag: string) bool {
        return self.flags[flag] catch false
    }

    fn refresh(mut self) {
        self.flags = self.store.load_all()!
    }
}
```

This class:
- Owns a cache of feature flags (state)
- Serves lookups to the rest of the app (behavior)
- Gets its backing store injected — could be Redis, a database, a config file (dependency)
- Can refresh itself in a background task (lifecycle)

Nothing about this class knows it runs in a web server, or across multiple pods, or that 500 request handler threads hit it concurrently. It's just a class with a map and two methods. The compiler and runtime handle the rest.

### Stages Wire Everything Together

The stage instance is where the lifecycle is defined:

```
app MyApp[
    handler: RequestHandler,
    flags: FeatureFlagService,
    registry: ServiceRegistry
] {
    fn main(self) {
        let flag_sync = spawn self.flags.start_sync()
        flag_sync.detach()

        let registry_sync = spawn self.registry.start_sync()
        registry_sync.detach()

        for conn in listen(8080) {
            spawn self.handler.handle(conn)
        }
    }
}
```

The stage instance declares what it needs. The compiler:
1. **Builds the dependency graph** — resolves all transitive dependencies
2. **Infers synchronization** — which singletons are accessed concurrently and need locking
3. **Infers error sets** — which methods can fail, what errors they can raise
4. **Generates a synthetic `main()`** — allocates singletons in dependency order, wires them, invokes the stage lifecycle

The programmer writes business logic. The compiler builds the infrastructure.

### What the Programmer Doesn't Write

In a typical backend framework (Spring, Express, Rails), the programmer must explicitly handle:

| Concern | Framework approach | Pluto approach |
|---|---|---|
| Dependency wiring | Container config, annotations, factory methods | Declared with `[dep: Type]`, auto-wired by compiler |
| Thread safety | `synchronized`, `@ThreadSafe`, mutexes | Compiler-inferred from `mut self` + concurrency analysis |
| Error propagation | `throws`, `try/catch`, manual checking | Compiler-inferred from call graph, enforced with `!` and `catch` |
| Lifecycle management | `@Scope`, `@PostConstruct`, `@PreDestroy` | Compiler-inferred from dependency graph |
| Correctness | Unit tests, code review, hope | Contracts (invariants, requires/ensures) verified at compile time and runtime |

The goal: the programmer writes a class with fields, methods, and declared dependencies. Everything else is the compiler's job.

## Why This Layering?

Different languages have different "0th class objects":
- C: the executable
- Java: the JAR / JVM application
- Go: the binary

Pluto has **two** 0th class objects at different levels:
- The **stage** is the unit of execution — a single deployable process
- The **system** (future) is the unit of deployment — a graph of stages that form a distributed application

This layering exists because backend systems are inherently multi-process. A single "app" is rarely sufficient — you need HTTP servers, background workers, scheduled jobs, admin tools. Each is a stage instance. The system layer is where they compose.

The "no fields on stages" constraint enforces this separation. If stages could hold state, they'd become monolithic god objects. By forcing state into classes, every piece of logic stays encapsulated, testable, and reusable across different stages.

## Modules

Modules organize code into separate files and namespaces:

```
import math
import utils as u

fn main() {
    let v = math.add(1, 2)
    u.log("result: {v}")
}
```

Key properties:
- `import <name>` loads a sibling file (`<name>.pluto`) or directory (`<name>/`)
- Items must be marked `pub` to be visible across modules (private by default)
- Imported items are accessed via qualified names: `math.add()`, `math.Point { x: 1, y: 2 }`
- Files in the same directory are auto-merged (no import needed)
- Hierarchical imports supported: `import net.http`
- Import aliases: `import utils as u`
- Modules are always libraries — they cannot contain stage instances (only the entry file can). They *can* contain stage definitions (`stage Daemon { ... }`)

## Testing

Pluto's test framework works across all file kinds:

```
test "order processing" {
    let processor = OrderProcessor { processed_count: 0 }
    processor.handle(test_order)!
    expect(processor.processed_count).to_equal(1)
}
```

- `test "name" { body }` blocks can appear in any file
- Tests are stripped from normal compilation; executed with `pluto test <file>`
- Assertions: `expect(x).to_equal(y)`, `expect(b).to_be_true()`, `expect(b).to_be_false()`

Testing with DI (integration tests) is an open design question — see [RFC: Stages](rfc-entry-points.md) for discussion of test-scoped DI overrides.

## Open Questions

### System Layer
- [ ] **System syntax and semantics** — What does a `system` declaration look like? Is it a language construct with its own keyword, or a special kind of stage?
- [ ] **Cross-stage communication** — How do stages within a system communicate? Compiler-generated RPC? Shared queues? Channels? Something more efficient for co-located stages?
- [ ] **Cross-stage type sharing** — When two stages share a type (e.g., `Order`), how is serialization/deserialization handled?
- [ ] **System DI** — Does a system have its own DI graph, or does each stage maintain an independent graph?
- [ ] **Non-Pluto workloads** — Can systems manage non-Pluto services (Go, Python, external executors)?

### Scripts
- [ ] **Script DI** — Should scripts support lightweight runtime DI wiring? e.g., manually instantiating stages with inline deps. May add too much complexity.
- [ ] **Script limitations** — Should scripts support more features over time (e.g., imports, stage instantiation)?

### Libraries & MCP
- [ ] **Library introspection** — How does an MCP server discover and expose a library's public declarations as tools? Is this built into `pluto` or a separate tool?
- [ ] **Library versioning** — How are library API changes tracked and communicated to consumers (both human and AI)?

### Projects & Workspaces
- [ ] **Multi-stage projects** — Can a project directory contain multiple stage instance files that compile to separate binaries?
- [ ] **Workspaces** — How do monorepos with multiple services, shared libraries, and shared types work?
- [ ] **Test-scoped DI** — How do integration tests override DI bindings with mocks?

### Stage-specific
- [ ] See [RFC: Stages](rfc-entry-points.md) for detailed open questions about inheritance, DI merging, generics, stages as values, etc.

# Pluto v1.0 Vision

## A Rocket Engine for Distributed Systems

Building distributed systems today is like building a rocket by hand — thousands of components that must work in perfect concert, where a single miscommunication between subsystems is catastrophic. Engineers wire together services with YAML and prayer, write defensive code against failures they can only imagine, and deploy systems they can never fully reason about.

Pluto is a rocket engine. Complex, precisely engineered, immense power — but when you ignite it, everything works together as one system. The compiler sees your entire distributed application, proves properties about it that no runtime check ever could, and produces binaries that start in milliseconds. You write services. Pluto makes them correct.

Where Rust brought compile-time safety to memory management, Pluto brings it to distributed systems. Not just thread safety — **distributed safety**. Ownership across service boundaries. Contracts that are mathematical proofs, not runtime assertions. A type system that understands your entire system topology, not just one binary.

This is not a language for the faint of heart. It is a language for engineers who build systems that cannot afford to be wrong.

## The Language

### Whole-Program Compilation

This is the foundation everything else is built on. Pluto compiles your entire distributed system — every service, every boundary, every data flow — as a single program. This is not an optimization. It is the architecture.

Because the compiler sees everything:

- Type safety extends across service boundaries, not just within a single binary
- Error propagation is **inferred** across the entire call graph — you never annotate which functions can fail, the compiler traces every call path and figures it out
- Dead services, unreachable states, impossible error paths — all detected before a single byte is deployed
- Schema changes are validated against every consumer at compile time

The whole-program model is what makes Pluto's safety guarantees possible. You cannot prove distributed properties if you can only see one node at a time.

### Static Verification

The verification engine is the single mechanism for all of Pluto's safety guarantees — from data structure invariants to distributed ownership to resource lifecycle.

Contracts in Pluto are not runtime assertions. They are **compile-time proofs**. The verification engine statically proves properties about your program before it ever runs. Invariants, preconditions, postconditions — these form a proof chain across your entire call graph. If Service A guarantees an ordering invariant, Service B can depend on that guarantee without a single defensive check, because the compiler has already proven it holds.

The goal is not to catch bugs. It is to make entire categories of bugs **structurally impossible** — inexpressible in the language, the way use-after-free is inexpressible in Rust.

**Ownership as a contract pattern.** Ownership in Pluto is not a separate language system like Rust's borrow checker. It is a contract pattern expressed through the verification engine. The stdlib provides common predicates — `owns`, `holds_lease`, `is_leader` — and users define their own. The compiler verifies the proof chain: if a method requires ownership, something upstream must have established it. Ownership is dynamic at runtime — leaders change, leases expire, partitions rebalance — but the compiler proves that your code handles every case correctly.

This is more ergonomic than Rust's ownership because it's scoped to where it matters. Local data doesn't need ownership tracking. Ownership contracts only apply where distributed safety is at stake.

**Typestates via generics.** Objects can carry state as a type parameter. A `Partition<Unowned>` and a `Partition<Owned>` are different types — you cannot call `consume()` on an unowned partition because the method doesn't exist on that type. State transitions are method calls that return the object in its new state. The compiler enforces valid sequencing through the type system, not runtime checks. Users define their own states — `Connected`/`Disconnected`, `Leader`/`Follower`, `Active`/`Drained` — using the same generics system that already exists.

**Auto-executing ensures.** Postconditions (`ensures`) can be auto-executing: the compiler both proves the condition will be satisfied and generates the code to satisfy it. A lease with `ensures released(self)` is automatically released at scope exit — every code path, every error branch, every early return. The compiler inserts the cleanup and proves it happens. One requirement: auto-executing contracts must be infallible. If cleanup can fail — because it's a distributed operation, a network call, a coordination step — the compiler forces you to handle it explicitly and verifies you did. Infallible cleanup is auto-executed. Fallible cleanup is verified.

**Structural impossibility.** The verification engine, combined with typestates and auto-executing ensures, makes certain distributed bugs inexpressible:

- A lease is revoked mid-operation — but every write carries a fencing token embedded by the type system, so revoked writes cannot land. Not "you remembered to check." The type system doesn't allow a write without a valid lease.
- A resource is acquired but never released — but auto-executing ensures guarantees cleanup on every path. Not "you remembered to defer." The compiler inserts it and proves it.
- A stateful protocol is called out of order — but typestates make invalid transitions a type error. Not "you tested the happy path." The wrong sequence doesn't compile.
- Two services both write to the same state — but the DI graph and ownership contracts prove single-writer access. Not "you documented the rule." The compiler enforces it.

### Error Handling

Errors in Pluto are not exceptions. They are not sum types. They are a distinct language concept with dedicated syntax and compiler support.

You declare error types. You `raise` them. You propagate them with `!` or handle them with `catch`. And the compiler **infers error-ability** — it analyzes your entire call graph and determines which functions can fail, what errors they can produce, and whether you've handled every case. No annotations, no `throws` clauses, no guessing.

This is possible because of whole-program compilation. The compiler sees every call path, every error source, every handler. If an error can reach a call site unhandled, it's a compile error.

### Dependency Injection

DI in Pluto is not a framework — it's a compiler feature. Services declare their dependencies, and the compiler resolves, validates, and wires everything at compile time. No runtime container. No reflection. No service locator pattern.

The compiler resolves the full dependency graph, validates every binding, detects cycles and captive dependencies, and verifies that the deployment environment satisfies all configuration requirements — secrets exist, endpoints are bound, nothing is missing. The same code runs in dev, staging, and production; only the DI configuration changes.

DI also drives **topology**. How objects are wired determines what's local and what's remote. Start with everything on one machine — the wiring is simple, no distributed overhead. When you need to split a service out, the wiring changes, and the compiler responds: it generates serialization, adds network failure to affected error sets, and enforces ownership rules at the new boundary. The code doesn't change. The wiring does. The compiler adapts.

Lifecycle management — singleton and scoped — is a first-class concept. Singletons are long-lived, shared instances. Scoped instances are bound to a unit of work (a request, a task) and the compiler ensures they don't escape their scope.

### Objects

Pluto distinguishes between **classes** and **objects**. This is not a cosmetic difference — it is a fundamental modeling distinction that reclaims what object-oriented programming was originally meant to be.

A class is a data structure. It is bytes in memory with a known layout. When you call `getValue()` on a class holding a secret, you are reading a field. If someone told you that call hit AWS, you would be shocked. A class doesn't *mean* anything beyond its bytes.

An object is an entity. It semantically represents **the actual thing** — not a snapshot, not a local copy, but the thing itself. When you call `getValue()` on a `Secret` object, of course that might go to a vault. That's what the secret *is*. When you call `rotate()`, the real secret rotates. The compiler knows this is an entity, and enforces rules accordingly — method calls may fail, state may change between calls, ownership and access patterns matter.

This is what Alan Kay originally meant by objects and what the industry lost along the way. Languages turned objects into "structs with methods." Pluto's `object` reclaims the original meaning: an entity that represents something real, where method calls are messages to that entity, and the implications are honest and visible in the type system.

Objects are the building blocks of a Pluto system. They compose and extend naturally — objects support inheritance, where classes do not. This is deliberate. Class inheritance is about inheriting data layout, which breeds fragile base classes and deep hierarchies that resist reasoning. Object inheritance is about specializing behavior on entities, constrained by contracts. If `HTTPServer` has contracts, any extension must satisfy them — the compiler enforces this. Inheritance on objects is safe because the verification engine ensures every subtype honors the guarantees of its parent.

**Classes get traits. Objects get inheritance.** Two different things, two different composition models, each suited to what they represent.

The distinction between class and object also maps naturally to distributed systems. A class is local — it lives on your machine, you own it, do what you want. An object is an entity that may be local today and distributed tomorrow. When all your objects are on one machine, there is no distributed overhead. When the DI topology splits them across boundaries, the compiler enforces the necessary safety guarantees. The code doesn't change. The nature of the object was always clear.

*This construct is under active design.*

## Program Structure

A Pluto program decomposes naturally from a single file to a planet-scale system. There is no ceremony threshold — no point where you have to "upgrade" your project structure or adopt a framework.

**A bare file is a valid program.** `print("Hello world")` compiles and runs. This is how you learn Pluto, try an idea, or write a quick tool.

**Objects are the building blocks.** As your program grows, you define objects, declare their dependencies, and the compiler wires everything together. There is no special `main` function or `app` construct — objects and their dependency graph *are* the program. The compiler discovers what exists and does the right thing.

**Services declare needs, not implementations.** An object says "I need a MetricsCollector and a Database." It doesn't say where they come from, how they're deployed, or whether they're in-process or on another continent. When you're developing locally, the metrics collector is in-memory and the database is SQLite. In production, the collector is Prometheus on its own cluster and the database is Postgres on RDS. **The service code is identical.** The topology determines how needs are fulfilled.

**Migration requires no service changes.** A capability starts in-process. It grows. Eventually it needs its own machine, its own scaling, its own lifecycle. You move it out by changing the topology. The compiler detects the new boundary, generates serialization, adds network error paths, enforces ownership contracts. The service that depended on it doesn't change a single line. It declared a need. The need is still met.

## The Distributed Model

Pluto does not hide the network. When you cross a service boundary, you know it. The latency is real, the failure modes are real, the partial availability is real. But Pluto makes distributed concerns **so ergonomic to handle** that designing for them becomes natural.

**Infrastructure enters the type system.** Pluto can read existing infrastructure artifacts — k8s YAML, SQL schemas, protobuf definitions, Terraform files, environment configs — statically at compile time, parse them into richly typed Pluto representations, and validate them against service code. Your k8s deployment manifest declares 3 replicas but your service's contracts require single-writer consistency — compile error. Your SQL schema dropped a column that a service still references — compile error. Infrastructure is validated against service code at compile time, and against the live environment at deploy time.

This means Pluto doesn't require you to throw away existing tools. Your YAML still works with kubectl. Your Terraform still works with `tf apply`. But Pluto has also ingested those files, typed them, and proven they're compatible with your code. If you want to write infrastructure natively in Pluto — with full contracts, static verification, and compiler support — you can. But you don't have to on day one.

The exact mechanism — compiler plugins, built-in parsers, DSL encoding, or some combination — is an open design question. The direction is clear: **infrastructure artifacts are compile-time inputs to the type system, not opaque files the compiler ignores.**

## Performance

Pluto compiles to native code via Cranelift. There is no VM, no interpreter, no JIT warmup.

A specific goal: **near-instant startup times**. Pluto services should start in single-digit milliseconds — fast enough for serverless, fast enough for rapid scaling, fast enough that cold starts are a non-issue. Native compilation with minimal runtime initialization makes this the default.

Whole-program compilation also opens the door to aggressive cross-boundary optimizations — dead service elimination, cross-service inlining, global data flow analysis. These are longer-term opportunities, but the architecture makes them possible.

## Standard Library

Pluto ships **batteries-included**. The standard library is comprehensive enough that most distributed backend services can be built without third-party dependencies:

- HTTP client and server, sockets, networking
- Serialization — JSON, wire format, and more
- Database drivers and connection management
- Queuing and messaging primitives
- Cryptography and encoding
- File system, paths, environment
- Logging and observability — metrics, tracing
- Collections with functional operations
- Concurrency primitives — tasks, channels, select
- Time, UUID, regex, random

Beyond the basics, Pluto's stdlib includes rich, type-safe interfaces to **cloud infrastructure** — Kubernetes, AWS services, and core distributed primitives. Where existing cloud SDKs are loosely typed and fail silently at runtime, Pluto's versions carry contracts, static guarantees, and compiler-checked configurations. Declare a queue with retention and partition constraints, and the compiler proves your consumers are compatible before you deploy. This is something no cloud SDK in any language offers today.

The stdlib is not an afterthought. It is a core part of the language's value proposition — well-documented, well-tested, and designed to work naturally with Pluto's type system, error handling, and ownership model.

## Observability

Observability in Pluto is not a library you add or an SDK you sprinkle in. It is a natural consequence of how the language works.

**Objects are inherently traceable.** An object is an entity with identity. Every method call on an object is a meaningful operation — not just "function was called" but "this entity did this thing." Operations on objects carry semantic meaning that maps directly to trace spans, without annotations or instrumentation code.

**The dependency graph is inherently visible.** The compiler knows the full topology — every object, every dependency, every boundary. Service maps, dependency diagrams, and call graphs don't need to be built manually or discovered at runtime. The compiler already has them.

**Boundaries are inherently instrumented.** When a call crosses a service boundary, the compiler knows. It can generate the telemetry — latency, error rates, payload sizes — at the same point where it generates serialization and error handling.

The developer doesn't add observability. They write objects with contracts, and observability falls out of that.

## Migrations

Schema evolution is a language-level capability in Pluto, not an external tool or manual process.

The compiler knows every type in the system. When a type changes — a field is added, removed, renamed, or restructured — the compiler diffs the old and new schemas, classifies each change (safe, dangerous, or incompatible), and acts accordingly:

- **Safe changes** (adding an optional field, adding an enum variant) — the compiler generates migration code automatically
- **Dangerous changes** (removing a field, changing a type) — the compiler rejects the change unless the developer provides an explicit migration path
- **Incompatible changes** — compile error, with a clear explanation of what broke and who is affected

Because of whole-program compilation, migrations are validated **across the entire system**. If Service A changes the `Payment` class and Service B consumes it, the compiler verifies compatibility or requires both to be updated together.

This migration engine is exposed as a language-level tool, not a compiler internal. The same diffing and validation that works on Pluto classes also works on database schemas described as Pluto objects. A Postgres table schema, a Kafka topic schema, an API contract — anything with a typed shape can be diffed, validated, and migrated through the same system. Contracts apply to migrations: declare `invariant backward_compatible` on an API schema, and the compiler rejects any change that would break existing consumers.

## Modules & Packages

Modules and packages are the same thing. A module is the unit of code organization, distribution, and dependency. Whether a module is a local directory in your project or a published package from a registry, the import is identical — `import payments`. The compiler resolves where it comes from.

A module is self-contained. It has types (classes), entities (objects), error declarations, contracts, and infrastructure — everything needed to describe a bounded piece of your system. Modules control their public surface explicitly — only `pub` declarations are visible to importers. Internals stay internal.

Modules compose naturally. A module can re-export selected items from sub-modules, curating an API surface without exposing implementation details. A payments module might publish `PaymentService`, `Payment`, and `PaymentError` while keeping `PaymentDB` and internal helpers private.

Import is uniform across all sources:

- A sibling directory in your project
- A git dependency
- A published package from the registry
- A stdlib module

The syntax is the same. The resolution is the compiler's concern. This means splitting a monolithic project into separate packages requires no code changes — just move the directory and update the manifest. The imports don't change.

## Developer Experience

The developer experience at 1.0 must be pristine. You only get one first impression.

**Editor support** — an LSP server with diagnostics, go-to-definition, hover, completions, and rename. First-class support in VS Code and/or Zed.

**Tooling** — `pluto fmt` (opinionated, zero-config), `pluto test` (built-in runner), `pluto check` (fast type checking), `pluto run` (compile and execute), `pluto coverage` (coverage reporting). Every tool is fast, clear, and unsurprising.

**Package registry** — a central registry for publishing and consuming modules, a lock file for reproducible builds, semantic versioning with compatibility checking.

**Documentation** — a language guide with tutorials, a "build your first distributed app" walkthrough, searchable stdlib API docs with examples, and migration guides from Go, Java, and TypeScript.

## AI-Native Development

Pluto's binary AST format and SDK enable AI agents to be first-class authors of Pluto code. Agents can read, write, and modify programs through a structured API rather than manipulating text. Every declaration carries a stable UUID, enabling precise cross-reference and incremental modification.

This is a bet on the future of software development — that AI agents will be co-authors, not just assistants, and that languages should be designed for both human and machine readability.

## Roadmap

### Near-Term
- Static verification engine — contracts as compile-time proofs, typestates via generics, auto-executing ensures
- Object construct — design, prototyping, and inheritance model
- Program structure — bare file to distributed system, no ceremony threshold
- DI-driven topology — wiring determines locality, compiler adapts at boundaries

### Mid-Term
- LSP server and editor integration
- Formatter, package registry, lock file
- Infrastructure as Pluto code — topology declarations, deploy-time validation
- Stdlib expansion — database drivers, messaging, crypto, observability
- Cross-boundary type checking and serialization codegen
- Cloud infrastructure APIs — type-safe Kubernetes and AWS interfaces
- Distributed contract predicates — stdlib patterns for ownership, leader election, lease management
- Migration engine — schema diffing, validation, and generation as a language-level tool

### Long-Term
- Incremental compilation for large codebases
- Advanced static verification — protocol contracts, bounded quantifiers
- Geographic annotations and placement constraints
- Observability integration — tracing, metrics, and topology visualization derived from language constructs
- Comprehensive documentation and migration guides
- Real-world hardening — building non-trivial distributed systems in Pluto and learning from it

### 1.0

All pillars working end-to-end. A pristine developer experience. A comprehensive standard library with cloud-native APIs. Documentation that welcomes newcomers and respects experts. And at least one real, non-trivial distributed system built entirely in Pluto — proof that the rocket engine works.

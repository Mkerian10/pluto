# RFC: Stages — Programmable Entry Points

**Status:** Draft
**Author:** Design discussion
**Related:** [Program Structure](program-structure.md), [Dependency Injection](dependency-injection.md)

## Summary

Stages are a new construct — distinct from classes — that define **how a program runs**. They have dependency injection but no fields. They support **inheritance**, allowing library authors and users to define reusable lifecycle templates (Daemon, HttpServer, Lambda, etc.) that others can build on.

The keyword is **`stage`**, inspired by rocket stages — each stage has a defined lifecycle (ignite, burn, separate), stages build on each other (multi-stage rockets), and the payload (your classes) rides inside.

## Motivation

Historically, `app` was the only way to define an entry point in Pluto. It works well for simple cases, but real backend systems have many kinds of programs:

- Long-running HTTP servers with routing and middleware
- Background workers consuming from message queues
- Scheduled jobs that run periodically
- One-shot tasks (migrations, data pipelines)
- CLI tools with subcommands
- Serverless handlers where the platform owns the event loop

Each has a different lifecycle pattern. With only `app`, the lifecycle logic (signal handling, graceful shutdown, HTTP serving, queue subscription) is reimplemented every time.

**The insight:** these lifecycle patterns form a natural **refinement hierarchy**. A daemon is a more specific kind of entry point (it adds shutdown). An HTTP server is a more specific kind of daemon (it adds routing). A company's internal API server is a more specific kind of HTTP server (it adds auth middleware).

Inheritance — specifically the Template Method pattern — is the right model for this hierarchy. This is one of the few domains where inheritance genuinely works well, because:

- The hierarchy is **naturally shallow** (2-4 levels)
- Each level **refines** the lifecycle (adds constraints, provides defaults)
- Program structure is **linear** (one lifecycle, no diamond problem)
- The "base class" is a **skeleton algorithm**, not a data container

## Design

### The Construct

Stages are a top-level construct with these properties:

| Property | Classes | Stages |
|----------|---------|--------|
| Fields | Yes | **No** |
| Bracket deps (DI) | Yes | Yes |
| Methods | Yes | Yes |
| Inheritance | **No** | Yes |
| `requires` methods | No | Yes (abstract) |
| `override` methods | No | Yes (explicit) |
| Contracts | Yes | TBD |
| Traits | Yes | TBD |
| Manual construction | Yes (struct literal) | **No** (compiler-generated) |

The "no fields" constraint is the key differentiator. Stages cannot hold state — they are pure lifecycle orchestrators. All state lives in the injected classes.

### Defining a Stage

Library authors define stages with `stage` and `requires` for abstract methods:

```
stage Daemon {
    requires fn run(self)
    requires fn shutdown(self)

    fn main(self) {
        signal.on_sigterm(() => self.shutdown())
        self.run()
    }
}
```

`requires fn` declares a method that concrete implementations must provide. The stage provides default methods (`main` here) that call the required ones — this is the Template Method pattern.

### Inheritance

Stages can inherit from other stages, refining the lifecycle:

```
stage HttpServer : Daemon {
    requires fn routes(self) Router

    override fn run(self) {
        let router = self.routes()
        http.serve(8080, router)
    }
    // shutdown is still required — inherited from Daemon
}
```

`HttpServer` inherits from `Daemon`:
- It inherits `Daemon.main()` (signal handling + calls `run()`)
- It overrides `run()` with HTTP-specific logic — using **explicit `override fn`**
- It adds a new requirement: `routes()`
- It inherits the `shutdown()` requirement from `Daemon`

Further refinement:

```
stage CompanyApi : HttpServer {
    requires fn middleware(self) [Middleware]

    override fn run(self) {
        let router = self.routes()
        for mw in self.middleware() {
            router = router.use(mw)
        }
        http.serve(8080, router)
    }
}
```

### Override Semantics

Method overrides must use the **`override`** keyword. This is explicit — unlike classes (which don't support inheritance), stages require clarity about what's being overridden vs. what's new:

```
stage HttpServer : Daemon {
    // CORRECT — explicitly overriding Daemon.run()
    override fn run(self) { ... }

    // COMPILE ERROR — run() exists in parent, must use override
    fn run(self) { ... }

    // CORRECT — new method, not in parent
    fn helper(self) { ... }

    // COMPILE ERROR — nothing to override
    override fn nonexistent(self) { ... }
}
```

### Using a Stage

Users instantiate a stage by using its name (lowercased) as a keyword:

```
company_api OrderService[db: Database, auth: AuthService] {
    fn routes(self) Router {
        Router.new()
            .get("/orders", self.list_orders)
            .post("/orders", self.create_order)
    }

    fn middleware(self) [Middleware] {
        return [self.auth.middleware()]
    }

    fn shutdown(self) {
        self.db.close()
    }

    // Additional methods — not required by the stage, but available
    fn list_orders(self, req: Request) Response { ... }
    fn create_order(self, req: Request) Response { ... }
}
```

The compiler:
1. Validates all `requires` methods are implemented (transitively up the chain)
2. Resolves the full lifecycle: `Daemon.main()` → signal handling → `CompanyApi.run()` (overrides `HttpServer.run()`) → calls `routes()` and `middleware()`
3. Builds the DI graph from bracket deps
4. Generates a synthetic `main()` that allocates singletons, wires deps, and invokes the lifecycle

### DI Across the Chain

Each level in the inheritance chain can declare its own bracket deps:

```
stage Daemon {
    // No deps at this level
    requires fn run(self)
    requires fn shutdown(self)
    ...
}

stage HttpServer : Daemon {
    // Could declare deps if needed, e.g.:
    // [logger: Logger]
    requires fn routes(self) Router
    ...
}

company_api OrderService[db: Database, auth: AuthService] {
    // Concrete deps for this specific service
    ...
}
```

The compiler merges bracket deps from all levels in the chain. If `HttpServer` declares `[logger: Logger]` and `OrderService` declares `[db: Database]`, the full DI graph includes both.

### The Base Stage: `App`

The base stage is defined in the standard library (or prelude):

```
// In stdlib
stage App {
    requires fn main(self)
}
```

`app` is **not** a language keyword — it's simply the name of the most basic stage. When a user writes:

```
app MyService[db: Database] {
    fn main(self) {
        // ...
    }
}
```

This is using the `App` stage, just like `daemon MyWorker` uses the `Daemon` stage. No special-casing in the compiler.

### Where Stages Live

Stages can be defined in:

1. **The standard library** — Common patterns like `App`, `Daemon`, `HttpServer`, `ScheduledJob`
2. **Third-party libraries** — Frameworks define their own: `GraphqlServer`, `GrpcService`, `LambdaHandler`
3. **User code** — Organizations define internal stages: `CompanyApi`, `CompanyWorker`

Stages are imported like any other declaration:

```
import std.http.HttpServer

http_server MyApi[db: Database] {
    fn routes(self) Router { ... }
    fn shutdown(self) { ... }
}
```

## Crash Recovery Philosophy

Stages do **not** need Erlang-style supervision trees or process isolation for crash recovery. Pluto's type system eliminates the need:

| Failure | Pluto's approach | Can crash the process? |
|---------|-----------------|----------------------|
| Unhandled error | Compile error — can't happen | No |
| Null dereference | Impossible — nullable types + `?` | No |
| Type error | Impossible — static typing | No |
| Array out of bounds | Runtime error (catchable) | No |
| Division by zero | Runtime error (catchable) | No |
| Contract violation | Hard abort (Phase 4 may change) | Yes (by design) |
| OOM | Process dies | Yes (unrecoverable) |
| Stack overflow | Process dies | Yes (unrecoverable) |

The only things that kill a process are genuinely unrecoverable (resource exhaustion) or deliberate (contract violations = "your code is wrong, fix it"). There's no need for a supervisor to restart crashed processes because **Pluto programs don't crash from user-level errors.**

The system layer deploys stages as separate processes for **scaling and operational isolation** (you want your HTTP server and queue worker to scale independently), not for crash recovery.

## Examples

### Daemon (Background Worker)

```
stage Daemon {
    requires fn run(self)
    requires fn shutdown(self)

    fn main(self) {
        signal.on_sigterm(() => self.shutdown())
        self.run()
    }
}

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

### HTTP Server

```
stage HttpServer : Daemon {
    requires fn routes(self) Router
    requires fn port(self) int

    override fn run(self) {
        http.serve(self.port(), self.routes())
    }
}

http_server UserApi[db: Database] {
    fn routes(self) Router {
        Router.new()
            .get("/users", self.list)
            .post("/users", self.create)
    }

    fn port(self) int { return 8080 }
    fn shutdown(self) { self.db.close() }

    fn list(self, req: Request) Response { ... }
    fn create(self, req: Request) Response { ... }
}
```

### Serverless Handler

```
stage LambdaHandler {
    requires fn handle(self, event: LambdaEvent) LambdaResponse

    fn main(self) {
        lambda.serve((event: LambdaEvent) => self.handle(event))
    }
}

lambda_handler ImageResizer[storage: S3Storage] {
    fn handle(self, event: LambdaEvent) LambdaResponse {
        let image = self.storage.get(event.key)!
        let resized = resize(image, 800, 600)
        self.storage.put(event.key + "_thumb", resized)!
        return LambdaResponse { status: 200 }
    }
}
```

### Scheduled Job

```
stage ScheduledJob {
    requires fn execute(self)

    fn main(self) {
        self.execute()
        // Exit cleanly — external scheduler (cron, k8s CronJob) handles timing
    }
}

scheduled_job DailyReport[db: Database, email: EmailService] {
    fn execute(self) {
        let data = self.db.aggregate_daily()!
        self.email.send_report(data)!
    }
}
```

### CLI Tool

```
stage CliTool {
    requires fn commands(self) Map<string, fn() void>

    fn main(self) {
        let args = cli.parse_args()
        let cmds = self.commands()
        if cmds.contains(args.command) {
            cmds[args.command]()
        } else {
            print("Unknown command: {args.command}")
            print("Available: {cmds.keys()}")
        }
    }
}

cli_tool AdminCli[db: Database, users: UserService] {
    fn commands(self) Map<string, fn() void> {
        return Map<string, fn() void> {
            "list-users": () => self.list_users(),
            "reset-password": () => self.reset_password()
        }
    }

    fn list_users(self) { ... }
    fn reset_password(self) { ... }
}
```

## Interaction with Existing Features

### Error Handling

Stage methods participate in Pluto's error inference like any other function. If `run()` calls fallible functions, the compiler infers error-ability and enforces handling.

The lifecycle stage can choose how to handle errors from required methods — e.g., `Daemon.main()` might catch errors from `run()` and log them before shutting down.

### Contracts

Since stages have no fields, `invariant` is meaningless. But `requires`/`ensures` on lifecycle methods could be valuable — e.g., "port must return a value between 1 and 65535":

```
stage HttpServer : Daemon {
    requires fn port(self) int
        ensures return > 0 && return <= 65535
    ...
}
```

This is a future consideration.

### Generics

Stages could potentially be generic:

```
stage QueueWorker<T> : Daemon {
    requires fn process(self, item: T)
    requires fn queue_name(self) string
    ...
}
```

This is a future consideration — generics on stages add complexity and may not be needed if the DI pattern handles type variation.

### Modules

Stages follow the same module rules as other declarations:
- Defined in library files with `pub` visibility
- Imported with `import`
- Accessed via qualified names

A file that **uses** a stage (e.g., `http_server MyApi`) is an entry point file and cannot be imported as a module.

## Decided

- [x] **Keyword:** `stage` — defines a new stage template
- [x] **`app` is not a keyword** — `App` is a stage defined in stdlib. `app MyService` is sugar for using the `App` stage.
- [x] **Override semantics:** Explicit — `override fn run(self)` required when overriding a parent method. Compiler errors if `override` is used on a non-existent parent method, or if a parent method is redefined without `override`.
- [x] **No crash recovery / supervision** — Pluto's error system makes user-level crashes impossible. Process isolation is for scaling, not safety.

## Open Questions

### Fundamental Design

- [ ] **Inheritance depth limits?** — Should the compiler enforce a max depth? Probably not needed — natural usage will be 2-4 levels.
- [ ] **Can stages have non-required methods?** — Can a stage define helper methods that aren't abstract? (The examples above assume yes.)
- [ ] **`requires` on concrete instances** — Can a concrete instance (e.g., `daemon MyWorker`) itself be used as a stage by others? Or is `stage` the only way to define something inheritable?

### DI and Wiring

- [ ] **DI merging across chain** — How are bracket deps from multiple levels in the chain merged? Are duplicates an error?
- [ ] **Singleton scope** — Are deps injected at the stage template level shared with deps at the concrete level?
- [ ] **Construction order** — When the compiler generates `main()`, does it construct classes bottom-up (concrete deps first) or top-down (template deps first)?

### Stages as Values

- [ ] **Can stages be instantiated in scripts/systems?** — If stages are values you can create and `.start()`, they become composable at runtime: `let worker = daemon OrderWorker[queue: q]` then `worker.start()`. This is especially powerful for the system layer — a system is just Pluto code that creates and manages stage instances.
- [ ] **Stages as runtime management interfaces** — Stages may define not just the internal lifecycle (`requires fn run`, `requires fn shutdown`) but also the external control surface (`fn start`, `fn stop`, `fn status`). The stage author decides what "start" means — green thread, external executor, new process — and users just call `.start()`. This makes stages both a lifecycle template AND a runtime management abstraction.
- [ ] **Runtime DI for scripts** — When instantiating a stage in a script (no compile-time DI context), can you wire deps manually inline? e.g., `daemon OrderWorker[queue: mock_queue]`. Or is this too complex?

### Type System

- [ ] **Are stages types?** — If stages can be values (see above), they need types. `OrderWorker` would be a concrete type that is-a `Daemon`. Could you have `let workers: [Daemon] = [order_worker, email_worker]`?
- [ ] **Trait implementation** — Can stages implement traits? Could be useful for cross-cutting concerns (e.g., `impl Monitorable`).
- [ ] **Generics** — Can stages be generic? (e.g., `QueueWorker<T>`)

### Runtime and Compilation

- [ ] **How does the compiler discover the lifecycle chain?** — The compiler needs to see the full inheritance chain to generate `main()`. This requires the stage definitions to be available at compile time (always true for Pluto's whole-program model).
- [ ] **Multiple entry points per project** — Can a project directory contain multiple entry point files? If so, does `pluto build` produce multiple binaries?
- [ ] **Template method dispatch** — Is the lifecycle chain resolved statically (monomorphized) or dynamically (vtable)? Static is simpler and aligns with Pluto's whole-program model.

### Ecosystem

- [ ] **Stdlib stages** — Which stages should the standard library provide? Candidates: `App`, `Daemon`, `HttpServer`, `ScheduledJob`, `CliTool`, `LambdaHandler`.
- [ ] **Third-party stages** — How do frameworks distribute stages? Same as any library module?
- [ ] **Versioning** — When a stage in a library changes (e.g., adds a new `requires` method), how is backward compatibility handled?

### Interaction with System Layer

- [ ] **Does the system layer reference stage types or concrete instances?** — e.g., `system MyPlatform { api: HttpServer, worker: Daemon }` vs `system MyPlatform { api: MyApi, worker: MyWorker }`
- [ ] **Cross-entry-point communication** — How do stages within a system communicate? RPC generated by the compiler? Shared queues? Channels?
- [ ] **Same DI graph or separate?** — Do stages in a system share a DI graph (same singletons) or have independent graphs (separate processes)?

## Alternatives Considered

### Just Use Traits

Instead of a new construct, lifecycle patterns could be traits that an entry point implements:

```
trait Daemon {
    fn run(self)
    fn shutdown(self)
}

app MyWorker[queue: MessageQueue] impl Daemon {
    fn main(self) {
        signal.on_sigterm(() => self.shutdown())
        self.run()
    }
    fn run(self) { ... }
    fn shutdown(self) { ... }
}
```

**Rejected because:** The `main()` boilerplate is still manually written every time. The whole point of the Template Method pattern is that the base defines the skeleton. With traits, every entry point reimplements the lifecycle from scratch.

### Macros / Code Generation

A macro system could generate the lifecycle boilerplate:

```
@daemon
app MyWorker[queue: MessageQueue] {
    fn run(self) { ... }
    fn shutdown(self) { ... }
}
```

**Rejected because:** Macros hide the lifecycle logic. With stages, you can read the stage source and understand exactly what happens. The inheritance chain is explicit and inspectable.

### Fixed Set of Keywords

The compiler hardcodes `app`, `daemon`, `handler`, `job`, etc. as built-in keywords:

**Rejected because:** It's inflexible. New lifecycle patterns require compiler changes. Users can't define their own patterns for their domain. Stages let the ecosystem evolve without language changes.

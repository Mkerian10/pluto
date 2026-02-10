# The App Model

Most languages treat "application" as an afterthought. Your program is a `main()` function. Maybe you wire up a dependency injection container in the first 50 lines, maybe you don't. The framework handles lifecycle, or you handle it yourself with ad hoc initialization code. The language has no opinion.

Pluto treats the app as the 0th-class object. The `app` declaration is a top-level language construct that defines your program's entry point, its dependency graph, and its lifecycle -- all in one place, all verified at compile time.

## The `app` Declaration

```
app OrderSystem[svc: OrderService, db: Database] {
    ambient Logger

    fn main(self) {
        self.svc.process("ORD-1", 100)!
    }
}
```

This is a complete Pluto program entry point. The compiler reads it and knows:

- **What the program needs.** `OrderService` and `Database` are bracket deps -- singleton services created at startup and wired automatically.
- **What is globally available.** `Logger` is an ambient type. Any class that declares `uses Logger` gets a `Logger` instance injected without listing it as a bracket dep.
- **How the program starts.** `fn main(self)` is the entry point. The compiler generates a synthetic `main()` that allocates all singletons in dependency order, wires them together, and calls `self.main()`.

Compare this to the equivalent in Spring Boot: a `@SpringBootApplication` class, component scanning, `@Autowired` annotations scattered across files, runtime reflection to resolve the graph. In Pluto, the entire DI topology is visible in one declaration and verified at compile time.

## The App as DI Root

The `app` is the root of the dependency injection graph. Its bracket deps are the top-level singletons, and their transitive dependencies form the full graph:

```
class Database {
    fn query(self, sql: string) string {
        return sql
    }
}

class UserService[db: Database] {
    fn get_user(self, id: string) string {
        return self.db.query("SELECT * FROM users WHERE id = {id}")
    }
}

class OrderService[db: Database, users: UserService] {
    fn process(self, order_id: string, amount: int) {
        let user = self.users.get_user("42")
        print("Processing order {order_id} for {user}: {amount}")
    }
}

app OrderSystem[svc: OrderService] {
    fn main(self) {
        self.svc.process("ORD-1", 100)
    }
}
```

The compiler resolves the full graph: `OrderSystem` needs `OrderService`, which needs `Database` and `UserService`, which needs `Database`. It topologically sorts the dependencies, detects cycles, and generates startup code that allocates one instance of each class in the correct order. `Database` is created once and shared between `UserService` and `OrderService` -- it is a singleton.

## App Rules

The constraints are strict by design:

- **Exactly one per program.** A compilation unit has at most one `app` declaration. Multiple entry points are a compile error.
- **Must have `fn main(self)`.** The app must define a `main` method. This is the program's entry point after DI wiring completes.
- **All deps are singletons.** Bracket deps on the app and their transitive dependencies are created once and live for the entire process. (See the section on lifecycle overrides below for how to change this.)
- **Cycle detection at compile time.** If `A` depends on `B` and `B` depends on `A`, the compiler rejects it immediately. No runtime `BeanCurrentlyInCreationException`.
- **No manual construction of injected classes.** If a class has bracket deps (is DI-managed), you cannot construct it with a struct literal. The compiler owns construction.

## Ambient Types

Bracket deps are explicit -- you access them through `self.dep`. Ambient types provide a lighter-weight alternative for cross-cutting concerns:

```
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class OrderService uses Logger [db: Database] {
    fn process(self) {
        logger.info("processing order")
        print(self.db.query("SELECT 1"))
    }
}

app MyApp[svc: OrderService] {
    ambient Logger

    fn main(self) {
        self.svc.process()
    }
}
```

`uses Logger` on a class means "inject a `Logger` and let me access it as `logger`" (type name, first letter lowercased). The `ambient Logger` declaration in the app registers it as available for `uses` clauses. Every class that `uses Logger` shares the same singleton instance.

This is syntactic sugar -- the compiler desugars `uses` into hidden injected fields before type checking. But it eliminates significant boilerplate for types like loggers, config, and metrics that every class needs.

## Lifecycle Overrides

By default, every DI-managed class is a singleton. But real systems need per-request state -- a request context, a database transaction, a metrics collector scoped to a single operation.

Pluto supports `scoped` and `transient` lifecycle annotations on classes:

```
scoped class RequestCtx {
    request_id: string
}

class Handler[ctx: RequestCtx] {
    fn handle(self) string {
        return "handling {self.ctx.request_id}"
    }
}
```

The compiler infers that `Handler` is also scoped because it depends on a scoped class. A singleton depending on a scoped class is a compile error -- the captive dependency problem that Spring silently ignores and .NET catches at runtime.

The app can also override a class's default lifecycle:

```
app MyApp {
    scoped ConnectionPool

    fn main(self) {
        // ConnectionPool is now per-scope instead of singleton
    }
}
```

Overrides can shorten a lifecycle (singleton to scoped) but not lengthen it (scoped to singleton). The compiler enforces this.

## Project Kinds

Not every Pluto program needs an `app`. The compiler auto-detects four project kinds:

| Kind | Contains | Entry Point | DI? |
|------|----------|-------------|-----|
| **Library** | Declarations only | None | No |
| **Script** | Declarations + top-level statements | Auto-generated `main()` | No |
| **App** | Declarations + a stage instance | Defined by the stage | Yes |
| **System** | Stage composition | Orchestration entry | Yes |

A **script** is the simplest form -- top-level statements execute in order, the compiler wraps them in a synthetic `main()`. No DI, no ceremony:

```
let x = 42
print("the answer is {x}")
```

A **library** is declarations only -- functions, classes, traits, enums. Libraries are imported by other programs and cannot have entry points.

A **system** (future) composes multiple stages into a distributed application. See the stages section below.

## The Three-Layer Model

Pluto programs are organized around three layers, each addressing a different concern:

| Layer | Concern | Construct | Has Fields? | Has DI? |
|-------|---------|-----------|-------------|---------|
| Data + Behavior | What does this thing do? | `class` | Yes | Yes |
| Lifecycle + Wiring | How does this thing run? | Stages | No | Yes |
| Topology | What things exist and how do they relate? | System (future) | -- | -- |

**Layer 1: Classes** are the workhorses. They hold state, define behavior, declare dependencies, and enforce contracts. A class does not know how it is orchestrated -- `OrderProcessor` works the same whether it is called from an HTTP handler, a queue consumer, or a test.

**Layer 2: Stages** define how a program runs. They wire together the classes that do the actual work. The critical constraint: stages have DI but no fields. They cannot hold state. All state lives in the injected classes. This prevents stages from becoming god objects.

**Layer 3: System** (future) defines the topology of a distributed application -- which stages exist, how they communicate, where they deploy.

## Stages: Programmable Entry Points

> **Status: Designed.** Stages are fully designed but not yet implemented. Today, `app` is the only entry point kind. The design below represents the planned direction.

The `app` you have seen so far is actually the simplest stage. In the planned design, `App` is defined in the standard library:

```
stage App {
    requires fn main(self)
}
```

When you write `app MyService[db: Database] { fn main(self) { ... } }`, you are using the `App` stage. The keyword `app` is not special syntax -- it is the lowercased name of the `App` stage template.

Stages support **inheritance**, allowing library authors to define reusable lifecycle templates:

```
stage Daemon {
    requires fn run(self)
    requires fn shutdown(self)

    fn main(self) {
        signal.on_sigterm(() => self.shutdown())
        self.run()
    }
}

stage HttpServer : Daemon {
    requires fn routes(self) Router
    requires fn port(self) int

    override fn run(self) {
        http.serve(self.port(), self.routes())
    }
}
```

`Daemon` defines a lifecycle: set up signal handling, call `run()`, shut down gracefully on SIGTERM. `HttpServer` inherits that lifecycle and overrides `run()` to serve HTTP. Users instantiate a stage by using its name as a keyword:

```
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

The compiler validates that all `requires` methods are implemented, resolves the full lifecycle chain, merges DI bracket deps from all levels, and generates a synthetic `main()`.

### More Stage Examples

**Scheduled job** -- runs once and exits. External scheduler handles timing:

```
stage ScheduledJob {
    requires fn execute(self)

    fn main(self) {
        self.execute()
    }
}

scheduled_job DailyReport[db: Database, email: EmailService] {
    fn execute(self) {
        let data = self.db.aggregate_daily()!
        self.email.send_report(data)!
    }
}
```

**Serverless handler** -- the platform owns the event loop:

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

Stages can be defined in the standard library, in third-party packages, or in your own codebase. An organization can define a `CompanyApi` stage that bakes in auth middleware, observability, and internal routing conventions -- every new service inherits the pattern without reimplementing it.

### Why Inheritance Works Here

Pluto classes do not support inheritance. Stages do. This is not a contradiction. Inheritance works well for stages because the hierarchy is naturally shallow (2-4 levels), each level refines a linear lifecycle (no diamond problem), and the base is a skeleton algorithm -- the Template Method pattern -- not a data container. Stages cannot hold fields, so the failure modes of classical inheritance (fragile base class, unclear ownership of mutable state) do not apply.

## What the Compiler Does for You

To appreciate what the `app` model provides, consider what you do not write:

| Concern | Framework approach | Pluto approach |
|---|---|---|
| Dependency wiring | Container config, annotations, factory methods | Declared with `[dep: Type]`, auto-wired by compiler |
| Lifecycle management | `@PostConstruct`, `@PreDestroy`, init blocks | Stage lifecycle template, compiler-generated `main()` |
| Scope violations | Runtime crash or silent bug | Compile-time error |
| Missing dependency | Runtime `NoSuchBeanException` | Compile-time error |
| Circular dependency | Runtime crash or proxy magic | Compile-time error |

The app declaration is not just an entry point. It is a contract between the programmer and the compiler: "Here is what my program needs. Build it for me, and prove it is correct."

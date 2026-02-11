# Dependency Injection

## Overview

Pluto has language-level dependency injection. Dependencies are declared on classes and auto-wired by the compiler based on type. There are two consumption modes:

- **Bracket deps (explicit):** `class Foo[db: Database]` — accessed via `self.db`
- **Ambient deps (`uses`):** `class Foo uses Logger` — accessed as bare variables (`logger.info(...)`)

## Declaring Dependencies

### Bracket Deps (Explicit)

Bracket deps declare dependencies that are accessed explicitly through `self`:

```
class OrderService[db: APIDatabase, accounts: AccountsService] {
    fn process(self) {
        self.db.query("SELECT 1")!
        self.accounts.get_user("42")!
    }
}
```

### Ambient Deps (`uses`)

Ambient deps declare dependencies accessed as bare variables — no `self.` prefix:

```
class OrderService uses Logger, Config [db: Database] {
    fn process(self, id: int) {
        logger.info("Processing order {id}")      // ambient — bare access
        let url = config.db_url()                  // ambient — bare access
        let result = self.db.query("SELECT 1")!    // explicit — self.field
        logger.info(result)
    }
}
```

The variable name is the type name with the first letter lowercased: `Logger` becomes `logger`, `Config` becomes `config`.

### App Registration

The app declares bracket deps and registers ambient types:

```
app MyApp[svc: OrderService] {
    ambient Logger
    ambient Config

    fn main(self) {
        self.svc.process(42)
    }
}
```

Every type used in a `uses` clause must be declared `ambient` in the app. The compiler validates this at compile time.

## How Ambient DI Works

Ambient deps are syntactic sugar. Before type checking, the compiler runs a desugaring pass that:

1. Adds hidden injected fields to classes (e.g., `logger: Logger` with `is_injected=true`)
2. Rewrites bare variable references to `self.field` access (e.g., `logger.info(...)` becomes `self.logger.info(...)`)
3. Respects variable shadowing — `let logger = 42` correctly shadows the ambient `logger` in subsequent code

After desugaring, typeck and codegen see regular injected fields and `self.field` accesses. No special handling needed in those passes.

## Specific Types, Not Generic Types

Dependencies are injected by **specific** type, not generic type. If you have two databases, you define distinct types:

```
class APIDatabase impl Database { ... }
class AccountsDatabase impl Database { ... }

class OrderService[db: APIDatabase, accounts: AccountsDatabase] {
    // unambiguous — each field has a unique type
}
```

This ensures DI resolution is always unambiguous — no `@Qualifier` annotations or named bindings needed.

## Environment Opacity

The code does not know what concrete implementation backs its dependency. The same `[db: APIDatabase]` works whether `APIDatabase` is:
- A local SQLite in development
- A shared Postgres in staging
- A replicated Aurora cluster in production

The environment configuration (which lives outside the language, in the runtime/orchestration layer) determines what gets injected. Code never changes between environments.

## Compile-Time Verification

The whole-program compiler verifies:
- Every bracket dep and ambient dep has a provider in the dependency graph
- No cycles in the dependency graph
- Every `uses` type is declared `ambient` in the app
- Generic classes cannot use ambient deps
- Classes with injected fields cannot be constructed manually via struct literals

Missing dependencies are compile-time errors, not runtime surprises.

## Lifecycles and Scope Blocks

Pluto supports three DI lifecycles, ordered by duration:

- **`singleton`** (default) — one instance for the entire process, created at startup
- **`scoped`** — one instance per scope block, created fresh each time the scope runs
- **`transient`** — fresh instance at every injection point (deferred, not yet implemented)

### Declaring Scoped Classes

Use the `scoped` keyword before `class`:

```
scoped class RequestCtx {
    user_id: string
    trace_id: string
}
```

### Lifecycle Inference

Classes inherit the shortest-lived dependency's lifecycle automatically. A class that depends on a scoped class is itself scoped — no annotation needed:

```
// UserService is inferred scoped because it depends on scoped RequestCtx
class UserService[ctx: RequestCtx] {
    fn current_user(self) string {
        return self.ctx.user_id
    }
}
```

The compiler prevents captive dependencies: a singleton cannot depend on a scoped class, because the singleton would hold a stale reference after the scope ends.

### Scope Blocks

Scope blocks create fresh scoped instances. Seeds are user-provided values; bindings are auto-wired by the compiler:

```
scope(RequestCtx { user_id: "42", trace_id: "abc" }) |svc: UserService| {
    print(svc.current_user())
}
```

- **Seeds:** Scoped classes with regular (non-injected) fields must be provided as seeds
- **Auto-constructible:** Scoped classes with only injected fields are created automatically
- **Singleton access:** Singleton dependencies are available automatically inside scope blocks
- **Isolation:** Each scope block creates independent instances — two scope calls don't share state

### Safety

- **Spawn restrictions:** Spawning tasks that capture scope bindings is rejected at compile time (scoped instances would outlive the scope)
- **Closure escape analysis:** Closures that capture scope bindings are tracked; they cannot escape the scope block via return or assignment to outer variables
- **App-level overrides:** The app can shorten a class's lifecycle (e.g., `scoped ConnectionPool`) but cannot lengthen it

For the full design, see the [DI Lifecycle RFC](rfc-di-lifecycle.md).

## Implementation Details

DI is implemented with compile-time wiring:

- **Bracket deps:** `class Foo[dep: Type]` — stored before regular fields in memory layout
- **Ambient deps:** `class Foo uses Type` — desugared to hidden injected fields before typeck
- **App as root:** The `app` declaration is the DI root with bracket deps and ambient registrations
- **Topological sort:** The compiler orders singletons by dependency, detects cycles at compile time
- **Synthetic main:** Codegen generates a `main()` that allocates all singletons (including ambient types), wires dependencies, then calls the app's `main(self)`
- **Singleton sharing:** Ambient types are singletons shared across all classes that `uses` them

```
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class Database {
    fn query(self, q: string) string {
        return q
    }
}

class OrderService uses Logger [db: Database] {
    fn process(self) {
        logger.info("processing")
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

## DI as Architecture

DI in Pluto isn't just a testing convenience or a way to swap implementations. It's the architectural backbone that makes complex distributed systems feel simple. The combination of DI + `mut self` + compiler inference means classes can encapsulate rich, self-contained behavior while remaining fully extensible and testable.

### Self-Contained Services

A DI singleton isn't just a bag of methods — it can own state, manage its own lifecycle, and coordinate with external systems, all through its injected dependencies:

```
class ServiceRegistry[discovery: DiscoveryClient] {
    services: Map<string, ServiceEndpoint>

    fn lookup(self, name: string) ServiceEndpoint? {
        return self.services[name]
    }

    fn refresh(mut self) {
        let latest = self.discovery.fetch_all()!
        self.services = latest
    }

    fn start_sync(mut self) {
        while true {
            self.refresh() catch err {
                print("sync failed: {err}")
            }
            sleep(30000)
        }
    }
}
```

This class:
- **Owns its state** (`services` map) and keeps it current
- **Manages its own sync lifecycle** (`start_sync` runs forever, refreshing periodically)
- **Gets its external dependency injected** (`DiscoveryClient` — could be Consul, DNS, a mock)
- **Is fully testable** — inject a mock `DiscoveryClient`, call `refresh()`, assert the map contents
- **Is automatically thread-safe** — the compiler sees concurrent access and adds synchronization (see [Concurrency v2 RFC](rfc-concurrency-v2.md))

The consuming code doesn't know or care about any of this:

```
class RequestHandler[registry: ServiceRegistry] {
    fn route(self, req: Request) Response {
        let endpoint = self.registry.lookup(req.service)?
        return forward(endpoint, req)!
    }
}
```

`RequestHandler` just calls `lookup()`. It doesn't know that `ServiceRegistry` syncs with Consul every 30 seconds, doesn't know about the background thread, doesn't know about the locking. It just gets the right answer.

### The Compiler Does the Hard Work

The power comes from what the compiler infers automatically based on how classes are used:

| What you write | What the compiler infers |
|---|---|
| `fn method(self)` vs `fn method(mut self)` | Which operations are read-only vs mutating |
| Class used from spawned tasks | Needs synchronization (reader/writer locks) |
| Class depends on a `scoped` class | Class is itself scoped (lifecycle inference) |
| Method calls a fallible function | Method is fallible (error inference) |
| Class invariants + `mut self` methods | Invariant checks after mutation only |

The programmer declares dependencies and marks mutation honestly. The compiler handles concurrency, lifecycle, error propagation, and correctness verification.

### DI + Concurrency: The Full Picture

In a real backend, DI singletons naturally fall into two categories:

**Stateless services** — route calls, apply business logic, no mutable fields. These are the majority of your DI graph. They need zero synchronization:

```
class OrderService[db: Database, accounts: AccountService] {
    fn create(self, order: Order) Order {
        let user = self.accounts.get_user(order.user_id)!
        self.db.insert(order)!
        return order
    }
}
```

**Stateful singletons** — caches, registries, pools, rate limiters. These own mutable state and may have background sync behavior. The compiler detects concurrent access and adds synchronization automatically:

```
class RateLimiter {
    counts: Map<string, int>
    max_per_minute: int

    fn check(mut self, client_id: string) {
        let count = self.counts[client_id] catch 0
        if count >= self.max_per_minute {
            raise RateLimitExceeded { client_id: client_id }
        }
        self.counts[client_id] = count + 1
    }

    fn reset(mut self) {
        self.counts = Map<string, int> {}
    }
}
```

Both are just classes. Both use DI. The compiler treats them differently because it can see the difference: one has `mut self` methods reachable from concurrent contexts, the other doesn't.

### Extensibility Without Frameworks

Because every dependency is injected by type, replacing behavior is as simple as providing a different class with the same interface:

- **Development:** `DiscoveryClient` reads from a local file
- **Staging:** `DiscoveryClient` talks to a test Consul cluster
- **Production:** `DiscoveryClient` talks to production Consul with mTLS

The code never changes. The orchestration layer (or test harness) provides the right implementations. No Spring profiles, no environment variables scattered through code, no `if (env == "prod")` conditionals.

## Configuration, Secrets, and External Values

### Config Is Just a Class

Configuration doesn't need a special language construct. A config object is a class with fields, methods, and contracts:

```
class DatabaseConfig {
    host: string
    port: int
    max_connections: int
    password: Secret<string>

    invariant self.port > 0
    invariant self.port < 65536
    invariant self.max_connections > 0
    invariant self.host.len() > 0

    fn connection_string(self) string {
        return "{self.host}:{self.port}"
    }

    fn is_production(self) bool {
        return self.host != "localhost"
    }
}
```

This is better than every config system in every framework because:

- **Validation is built-in.** Invariants fire at construction. If someone provides `port: -1` or `host: ""`, it fails immediately — not when the first request hits the database 20 minutes later.
- **Logic lives with the data.** `connection_string()` isn't a utility function somewhere else — it's a method on the config. `is_production()` encodes knowledge about what the config means.
- **It's a real type.** You get autocomplete, compiler checking, refactoring support. Not stringly-typed YAML keys.
- **It participates in DI.** Inject it like anything else:

```
class Database[config: DatabaseConfig] {
    fn connect(self) Connection {
        // self.config.host, self.config.port
        // self.config.connection_string()
    }
}
```

No `@Value("${database.host}")`. No `config.get("database.host")`. No `process.env.DATABASE_HOST`. Just a typed field on a class, injected by the compiler.

### Why Not a `config` Keyword?

Config doesn't need special language treatment because classes already do everything config needs:

| Config need | Pluto feature |
|---|---|
| Typed fields | Class fields |
| Validation | Contracts (invariants) |
| Derived values | Methods |
| Injection | DI (bracket deps) |
| Testability | DI (inject mock config) |
| Environment opacity | Same as all DI — runtime provides implementations |

Adding a `config` keyword would just be a restricted class. Keep it simple: config is a class.

### `Secret<T>` — Protecting Sensitive Values

`Secret<T>` is a built-in generic class that wraps a value and prevents accidental leakage. The compiler enforces the protection:

```
class DatabaseConfig {
    host: string
    port: int
    password: Secret<string>
    api_key: Secret<string>
}
```

```
let pw = self.config.password       // Secret<string>
print(pw)                           // COMPILE ERROR: cannot print Secret<string>
"{pw}"                              // COMPILE ERROR: cannot interpolate Secret<string>
channel.send(pw)                    // COMPILE ERROR: cannot serialize Secret<string>

pw.expose()                         // string — explicit unwrap
pw == other_secret                  // OK: comparison allowed
authenticate(pw)                    // OK: pass to functions accepting Secret<string>
```

Key properties:
- **Generic** — `Secret<string>`, `Secret<bytes>`, `Secret<ApiKey>` — wraps any type
- **Compiler-enforced** — not a convention, a guarantee. Accidentally logging a password is a compile-time error, not a "we leaked to CloudWatch" incident
- **Explicit unwrap** — `.expose()` is the only way to access the inner value. Every access point is auditable — grep for `.expose()` and you have a complete list of where secrets are used
- **Composable** — works with DI, contracts, methods. A config class with `Secret<string>` fields is just a class

### Environment Access: `Env` as a DI Singleton

There is no `System.getenv()` in Pluto. No global function for reading environment variables. If you want env vars, you inject `Env`:

```
class DatabaseConfig[env: Env] {
    fn host(self) string {
        return self.env.get("DATABASE_HOST") catch "localhost"
    }

    fn port(self) int {
        return self.env.get("DATABASE_PORT").to_int() catch 5432
    }

    fn password(self) Secret<string> {
        return self.env.secret("DATABASE_PASSWORD")!
    }

    fn connection_string(self) string {
        return "{self.host()}:{self.port()}"
    }
}

class Database[config: DatabaseConfig] {
    fn connect(self) Connection {
        // self.config.host(), self.config.port(), self.config.password()
        // Database doesn't know about env vars — DatabaseConfig encapsulates that
    }
}
```

`Env` is a singleton provided by the runtime. At the simplest level, it wraps the process environment. In a production deployment, the orchestration layer can replace it with a version that sources from Vault, a config service, or a refresh worker — same interface, different backing.

Why DI instead of a global function:

- **Testable.** Inject a mock `Env` with whatever values you want. No `setenv()` hacks in test setup, no global state pollution between tests.
- **Visible.** The compiler sees the dependency. It knows exactly which classes use env vars because `Env` is in the DI graph. You can trace every env var dependency by looking at who injects `Env`.
- **Controllable.** The orchestration layer provides the `Env` implementation. Dev gets a `.env` file loader. Prod gets a Vault-backed version. The consuming code doesn't change.
- **No scatter.** You can't have 47 files each calling `System.getenv("DATABASE_URL")` independently with no visibility. Env access is concentrated in config classes that inject `Env` and expose typed methods.

### The Configuration Stack

Config in Pluto is a layered pattern, not a framework:

| Layer | What it does | Example |
|---|---|---|
| **`Env`** | Raw key-value access to environment | `env.get("DATABASE_HOST")`, `env.secret("DB_PASSWORD")` |
| **Config class** | Typed wrapper with logic + validation | `DatabaseConfig` with `host()`, `port()`, `connection_string()`, invariants |
| **Consuming class** | Uses config through DI | `Database[config: DatabaseConfig]` — doesn't know about env vars |

Each layer adds structure. The simplest app just injects `Env` directly. A well-structured app wraps it in typed config classes. Both work, both are testable, both are DI.

### Static vs Dynamic Config

Most config is **static** — set at startup, never changes. `Env` handles this with zero ceremony.

Some config is **dynamic** — feature flags, rate limits, rotating secrets. For dynamic config, the pattern is a class with a DI'd data source and a `mut self` refresh method. The orchestration layer can wrap it in a worker task to keep it updated. The consuming code doesn't change — it still calls the same methods. Whether the value was set once at startup or refreshed 5 seconds ago is invisible.

The language provides DI. The runtime provides `Env`. The orchestration layer optionally adds refresh behavior. Each layer adds capability without the lower layers changing.

## Open Questions

- [ ] `Env` API surface — what methods should `Env` have? `.get(key) string?`, `.secret(key) Secret<string>?`, `.require(key) string` (raises if missing)?
- [x] ~~Lifecycle management~~ — resolved: singleton/scoped/transient with scope blocks and lifecycle inference. See [DI Lifecycle RFC](rfc-di-lifecycle.md).
- [ ] Scoped overrides — `with` blocks for providing alternative implementations in tests
- [ ] Should `Env` be a trait so the runtime can provide different implementations? Or a concrete class that the runtime populates?
- [ ] Convention for config class naming — should there be a pattern (e.g., `XxxConfig` suffix) or is it freeform?

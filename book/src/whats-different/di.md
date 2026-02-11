# Dependency Injection

In Spring Boot, you annotate a class with `@Service`, annotate its constructor parameters with `@Autowired`, register a `@Configuration` class that returns a `@Bean`, add `@Qualifier` when types collide, configure `@Scope("request")` for per-request state, and pray that the runtime container wires everything correctly on the first try. When it does not, you get a stack trace 200 lines deep pointing at a `BeanCreationException` that tells you almost nothing.

In Go, you write a `main()` function that manually constructs every dependency and passes it to the next constructor. It is explicit and honest. It is also 300 lines of boilerplate that nobody reads, and every new service means editing `main()`.

In Node with NestJS, you decorate classes with `@Injectable()`, register them in modules, and debug circular dependency errors at runtime with console output that says "Nest cannot resolve dependencies of the CatsService."

Pluto has language-level dependency injection. It is resolved at compile time. There is no container. There is no reflection. There are no annotations. The compiler reads the dependency graph, topologically sorts it, detects errors, and generates the wiring code. If something is wrong, you get a compile error, not a runtime crash.

## Bracket Deps: Explicit Dependencies

A class declares its dependencies inside square brackets:

```
class OrderService[db: Database, auth: AuthService] {
    fn process(self, order_id: int) string {
        let user = self.auth.current_user()!
        return self.db.query("SELECT * FROM orders WHERE id = {order_id}")!
    }
}
```

`db` and `auth` are injected fields. You access them through `self`, just like regular fields. The compiler allocates them before regular fields in memory and wires them automatically at startup.

You cannot construct a class with injected deps manually. This is a compile error:

```
let svc = OrderService { db: some_db, auth: some_auth }
// COMPILE ERROR: classes with injected dependencies cannot be manually constructed
```

This is intentional. If you could bypass DI and construct services manually, the compiler could not guarantee the dependency graph is correct.

## Ambient Deps: Background Services

Some dependencies are cross-cutting. A `Logger` is used by almost every class. A `Config` object is read everywhere. Threading these through bracket deps gets noisy:

```
// Without ambient deps -- every class needs [logger: Logger]
class OrderService[db: Database, auth: AuthService, logger: Logger] { ... }
class PaymentService[gateway: PaymentGateway, logger: Logger] { ... }
class NotificationService[mailer: Mailer, logger: Logger] { ... }
```

Ambient deps solve this. A class declares that it `uses` a type, and accesses it as a bare variable:

```
class OrderService uses Logger [db: Database, auth: AuthService] {
    fn process(self, order_id: int) string {
        logger.info("Processing order {order_id}")
        let user = self.auth.current_user()!
        let result = self.db.query("SELECT 1")!
        logger.info("Order processed")
        return result
    }
}
```

The variable name is the type name with the first letter lowercased: `Logger` becomes `logger`, `Config` becomes `config`, `RequestCtx` becomes `requestCtx`.

A class can use multiple ambient deps: `class OrderService uses Logger, Config [db: Database]`. Under the hood, ambient deps are desugared to hidden injected fields before type checking. `logger.info(...)` becomes `self.logger.info(...)`. The rest of the compiler sees regular field access.

## The App Declaration

Every Pluto application has an `app` declaration. It is the root of the dependency graph:

```
app MyApp[svc: OrderService] {
    ambient Logger
    ambient Config

    fn main(self) {
        self.svc.process()!
    }
}
```

The app declares:

- **Bracket deps** -- the top-level services it needs direct access to
- **Ambient registrations** -- which types are available as ambient deps throughout the program
- **Methods** -- including the required `fn main(self)` entry point

Every type used in a `uses` clause anywhere in the program must be declared `ambient` in the app. If you forget, the compiler tells you:

```
// COMPILE ERROR: 'Logger' is not declared ambient in the app
```

App methods can also use ambient deps. The app itself has access to everything it registers:

```
app MyApp[svc: OrderService] {
    ambient Logger

    fn main(self) {
        logger.info("Starting application")
        self.svc.process()!
    }
}
```

## Compile-Time Resolution

When the compiler encounters an `app` declaration, it:

1. Collects all classes and their bracket deps and ambient deps
2. Builds a dependency graph
3. Topologically sorts the graph
4. Checks for cycles -- a cycle is a compile error
5. Checks that every dependency has a provider
6. Generates a synthetic `main()` that allocates singletons in order, wires them, and calls the app's `main(self)`

This happens at compile time. The generated binary has no DI container, no reflection, no service locator. Just a sequence of allocations and pointer assignments, then your code runs.

A missing dependency is a compile error. A cycle is a compile error. An ambiguous type is a compile error. You never discover DI problems in production.

## Specific Types for Unambiguity

Dependencies are resolved by exact type. If you have two databases, you use two distinct types:

```
class OrderService[api_db: APIDatabase, accounts_db: AccountsDatabase] {
    fn process(self) {
        let orders = self.api_db.query("SELECT * FROM orders")!
        let user = self.accounts_db.query("SELECT * FROM users")!
    }
}
```

No `@Qualifier("api")`. No named bindings. The type *is* the identifier. `APIDatabase` and `AccountsDatabase` are different types, so there is no ambiguity.

## Shared Singletons

By default, every class in the DI graph is a singleton. If `ServiceA[db: Database]` and `ServiceB[db: Database]` both depend on `Database`, they get the same instance -- one allocation, shared across the graph. No special annotation needed; singleton is the default lifecycle.

## Environment Opacity

The code does not know what environment it runs in. A `Database` class in development might wrap SQLite. In staging, it wraps Postgres. In production, it wraps a replicated Aurora cluster. The code that depends on `Database` never changes. The runtime and orchestration layer determine what gets constructed. No `if (env == "prod")` conditionals, no Spring profiles, no environment variables scattered through business logic.

## Config as a Class

Configuration in Pluto is not a special construct. It is a class with fields, methods, and invariants:

```
class DatabaseConfig {
    host: string
    port: int
    max_connections: int

    invariant self.port > 0
    invariant self.port < 65536
    invariant self.max_connections > 0
    invariant self.host.len() > 0

    fn connection_string(self) string {
        return "{self.host}:{self.port}"
    }
}
```

Invariants are checked at construction. If someone provides `port: -1`, the program aborts immediately -- not 20 minutes later when the first request hits the database.

Config participates in DI like any other class:

```
class Database[config: DatabaseConfig] {
    fn connect(self) string {
        return self.config.connection_string()
    }
}
```

No `@Value("${database.host}")`. No `config.get("database.host")`. No `process.env.DATABASE_HOST`. A typed field on a class, validated by invariants, injected by the compiler.

## Secret\<T\> for Sensitive Values

`Secret<T>` is a built-in generic that wraps a value and prevents accidental leakage at compile time:

```
class DatabaseConfig {
    host: string
    port: int
    password: Secret<string>
}
```

The compiler enforces protection:

```
let pw = self.config.password
print(pw)           // COMPILE ERROR: cannot print Secret<string>
"{pw}"              // COMPILE ERROR: cannot interpolate Secret<string>

pw.expose()         // string -- explicit unwrap, auditable
authenticate(pw)    // OK: pass to functions accepting Secret<string>
```

Every place secrets are unwrapped is visible by searching for `.expose()`. This is a compiler guarantee, not a convention.

## Env as a DI Singleton

There is no `System.getenv()` in Pluto. Environment variable access is a dependency, injected like everything else:

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
}
```

`Env` is a singleton provided by the runtime. Why DI instead of a global function:

- **Testable.** Inject a mock `Env`. No `setenv()` hacks, no global state pollution between tests.
- **Visible.** The compiler knows which classes use env vars -- `Env` is in the dependency graph. You can trace every env var dependency by checking who injects `Env`.
- **No scatter.** You cannot have 47 files each calling `getenv("DATABASE_URL")` independently. Env access is concentrated in config classes.

## Lifecycle Scopes

Not every dependency should live for the entire process. A `RequestContext` is per-request. A database transaction is per-request. Pluto supports three lifecycles:

| Lifecycle | Duration | Created | Shared? |
|-----------|----------|---------|---------|
| **singleton** | Entire process | Once at startup | Yes, globally |
| **scoped** | One scope block | Once per scope entry | Yes, within scope |
| **transient** | Injection point | Every time needed | No |

Singleton is the default. Scoped and transient are declared on the class:

```
scoped class RequestCtx {
    request_id: string
    user_id: int
}

transient class UUIDGenerator {
    seed: int
}

class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}
```

The compiler infers lifecycle when you do not declare one. The rule: **a class's lifecycle is the shortest-lived lifecycle among its dependencies.** If `UserService` depends on singleton `Logger` and scoped `RequestCtx`, it is inferred as scoped.

A singleton class explicitly depending on a scoped class is a compile error -- the compiler catches captive dependency bugs that Spring silently allows and .NET only catches at runtime.

## Scope Blocks

Scoped instances are created inside `scope` blocks:

```
app MyApp {
    fn main(self) {
        scope(RequestCtx { request_id: "abc", user_id: 42 }) |svc: UserService| {
            let user = svc.get_user(1)
            print(user)
        }
    }
}
```

The scope block takes **seeds** (struct literals for scoped classes that need external values) and **bindings** (which scoped instances the body needs). The compiler creates all scoped instances at scope entry, wires their dependencies, runs the body, and then the scoped instances become eligible for garbage collection.

Scoped classes that depend only on other injected deps (no regular fields) are auto-created without needing to be seeds. If `UserService[ctx: RequestCtx]` has no regular fields, the compiler creates it automatically when the scope provides `RequestCtx`.

Scopes nest, with inner scopes shadowing outer scopes for the same type. Scoped classes can depend on singletons (wired from the global graph), but singletons cannot depend on scoped classes.

## App-Level Lifecycle Overrides

The app can shorten a class's lifecycle without modifying the class itself:

```
app MyApp {
    scoped Database
    transient Logger

    fn main(self) {
        scope(RequestCtx { id: "r1" }, Database { url: "pg://db" }) |svc: Service| {
            print(svc.run())
        }
    }
}
```

Overrides can only shorten: singleton to scoped, singleton to transient, or scoped to transient. Lengthening (transient to singleton) is a compile error -- it would violate the lifetime guarantees of the original declaration.

## Comparison

| | Spring Boot | Go | NestJS | Pluto |
|---|---|---|---|---|
| Declaration | `@Service` + `@Autowired` | Manual construction | `@Injectable()` + modules | `class Foo[dep: Type]` |
| Resolution | Runtime container | Explicit in `main()` | Runtime container | Compile-time codegen |
| Missing dep | Runtime `BeanCreationException` | Compile error (manual) | Runtime error | Compile error |
| Cycles | Runtime error | Not possible (manual) | Runtime error | Compile error |
| Scope handling | `@Scope("request")`, runtime | Manual per-request setup | `@Injectable({ scope: Scope.REQUEST })` | `scoped class`, compile-time |
| Captive deps | Silent bug | Not applicable | Silent bug | Compile error |
| Config | `@Value`, properties files | `os.Getenv`, viper | `@Inject(CONFIG)`, dotenv | Class with invariants |
| Secrets | Hope you don't log them | Hope you don't log them | Hope you don't log them | `Secret<T>`, compile-enforced |
| Env access | `System.getenv()` anywhere | `os.Getenv()` anywhere | `process.env` anywhere | `Env` DI singleton |
| Ambient/cross-cutting | `@Autowired` everywhere | Pass through every call | `@Inject` everywhere | `uses Logger`, bare access |

## What This Means in Practice

DI in Pluto is not a framework feature you opt into. It is a language feature the compiler enforces. The dependency graph is fully visible at compile time: every class, every dependency, every lifecycle, every ambient registration. Missing deps, cycles, scope violations, and captive dependencies are all compile errors.

The generated binary has no container. No reflection. No proxy objects. No classpath scanning. Just a sequence of allocations and pointer assignments generated by the compiler from a graph it fully understands. The code you write looks like normal classes with normal methods. The dependencies arrive automatically, validated and wired before a single line of your code runs.

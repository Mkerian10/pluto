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

## Open Questions

- [ ] How are DI providers registered per environment? (currently no environment-specific config)
- [ ] Lifecycle management — singleton vs per-request vs per-process (currently all singletons)
- [ ] Scoped overrides — `with` blocks for providing alternative implementations in tests

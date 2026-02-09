# Dependency Injection

## Overview

Pluto has language-level dependency injection. Dependencies are declared with `inject` and are auto-wired by the runtime based on type.

## Declaring Dependencies

```
class OrderService {
    inject db: APIDatabase
    inject accounts: AccountsService
}
```

## Specific Types, Not Generic Types

Dependencies are injected by **specific** type, not generic type. If you have two databases, you define distinct types:

```
class APIDatabase impl Database { ... }
class AccountsDatabase impl Database { ... }

class OrderService {
    inject db: APIDatabase            // unambiguous
    inject accounts: AccountsDatabase // unambiguous
}
```

This ensures DI resolution is always unambiguous — no `@Qualifier` annotations or named bindings needed.

## Environment Opacity

The code does not know what concrete implementation backs its dependency. The same `inject db: APIDatabase` works whether `APIDatabase` is:
- A local SQLite in development
- A shared Postgres in staging
- A replicated Aurora cluster in production

The environment configuration (which lives outside the language, in the runtime/orchestration layer) determines what gets injected. Code never changes between environments.

## Multi-Environment Support

Because dependencies are resolved by type and provided upstream, switching environments is a configuration change, not a code change:

```
// This code is identical in dev, staging, and production.
// The only thing that changes is what the runtime provides
// when it sees "inject db: APIDatabase".

class OrderService {
    inject db: APIDatabase

    fn get_order(self, id: string) Order {
        return self.db.query("SELECT * FROM orders WHERE id = ?", id)?
    }
}
```

## Compile-Time Verification

The whole-program compiler verifies that every `inject` declaration has a provider in the dependency graph. Missing dependencies are compile-time errors, not runtime surprises.

## Current Implementation

DI is implemented with compile-time wiring:

- **Bracket deps:** Classes declare dependencies with `class Foo[dep: Type]` syntax. These are stored before regular fields in memory.
- **App as root:** The `app` declaration is the DI root. All dependencies are declared as bracket deps on classes.
- **Topological sort:** The compiler orders singletons by dependency, detects cycles at compile time.
- **Synthetic main:** Codegen generates a `main()` that allocates all singletons, wires dependencies, then calls the app's `main(self)`.
- **Struct literal blocking:** Classes with injected fields cannot be constructed manually via struct literals.

```
class Logger {
    fn info(self, msg: string) {
        print(msg)
    }
}

class OrderService[logger: Logger] {
    fn process(self) {
        self.logger.info("processing")
    }
}

app MyApp[svc: OrderService] {
    fn main(self) {
        self.svc.process()
    }
}
```

## Open Questions

- [ ] How are DI providers registered per environment? (currently no environment-specific config)
- [ ] Lifecycle management — singleton vs per-request vs per-process (currently all singletons)
- [ ] Can `inject` appear in nested classes not directly referenced by the app?

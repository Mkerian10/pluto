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

## Scope

> **Status:** Open design question.
>
> Topics to resolve:
> - Can `inject` appear in any class, or only at certain scopes (app-level, service-level)?
> - How deep can DI go? Can a class three layers deep in the call stack declare `inject`?
> - How are DI providers registered? Is there a separate binding configuration?
> - Lifecycle management — singleton vs per-request vs per-process

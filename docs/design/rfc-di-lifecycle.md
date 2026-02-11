# RFC: DI Lifecycle Scopes

**Status:** Implemented (Phases 1–3, 5a–5e). Phase 4 (Transient) deferred. Phase 6 (polish) complete.
**Author:** Matt Kerian
**Date:** 2026-02-10

## Summary

Extend Pluto's dependency injection system with **lifecycle scopes** — the ability to declare that a class should be created once per process (singleton, the current default), once per scope block (scoped), or fresh every time it's injected (transient). The compiler **infers** lifecycle for classes that don't declare one, based on their dependencies. A class that depends on a scoped class is automatically scoped. This is verified at compile time with zero runtime overhead for scope resolution.

## Motivation

Today, every DI-managed class in Pluto is a singleton. The synthetic `main()` creates one instance of each class at startup, wires them together, and they live for the entire process.

This works for stateless services but breaks down for real backend workloads:

- **Per-request state.** A `RequestContext` holding the current user, request ID, or trace context shouldn't be shared across requests. Today you'd have to thread it manually through every function call.
- **Per-request resources.** A database transaction should be opened at request start, committed or rolled back at request end, and never shared across requests.
- **Isolation.** Mutable state accumulated during one request (metrics counters, caches, buffers) should not leak into the next request.

Every mainstream DI framework solves this — Spring has `@Scope("request")`, .NET has `AddScoped<T>()`, Dagger has custom scopes. But they all share the same limitation: **scope violations are detected at runtime** (or not at all). A singleton accidentally depending on a scoped service is a silent bug in Spring and a runtime crash in .NET.

Pluto can do better. The whole-program compiler already has the complete dependency graph at compile time. Scope inference and validation is just another graph analysis pass — similar to how Pluto already infers error-ability from call graphs.

## Design

### Lifecycle Kinds

Three lifecycles, ordered by duration:

| Lifecycle | Duration | Created | Shared? |
|-----------|----------|---------|---------|
| **singleton** | Entire process | Once at startup | Yes, globally |
| **scoped** | One `scope` block | Once per scope entry | Yes, within the scope |
| **transient** | Injection point | Every time it's needed | No |

**Singleton** is the default (preserving backward compatibility). Classes that need a different lifecycle declare it explicitly.

### Declaration Syntax

Lifecycle is declared on the class:

```
scoped class RequestContext {
    request_id: string
    user_id: int
}

transient class UUIDGenerator {
    seed: int
}

class Logger {                 // singleton (default, no keyword)
    fn info(self, msg: string) {
        print(msg)
    }
}
```

**Rationale for class-site declaration:** The lifecycle is an intrinsic property of the class. A `RequestContext` is inherently per-request — that doesn't change based on which app uses it. Declaring it on the class makes it discoverable and self-documenting. (The app can also override the scope for specific classes — see [App-Level Overrides](#app-level-overrides).)

### Scope Inference

The compiler infers lifecycle for any class that doesn't explicitly declare one. The rule is simple:

> **A class's lifecycle is the shortest-lived lifecycle among its dependencies.**

If `UserService` depends on singleton `Logger` and scoped `RequestContext`:

```
class UserService[ctx: RequestContext, logger: Logger] {
    fn get_user(self, id: int) string {
        logger.info("Request {self.ctx.request_id}: getting user {id}")
        return "user-{id}"
    }
}
// Inferred: scoped (because RequestContext is scoped)
```

`UserService` is inferred as `scoped` because `RequestContext` is scoped. The compiler will create a fresh `UserService` for each scope, wired to that scope's `RequestContext`.

#### Inference Rules

```
inferred_lifecycle(class) = min(lifecycle(dep) for dep in class.dependencies)
```

Where the ordering is: `transient < scoped < singleton`.

If a class has no dependencies, it defaults to `singleton`.

#### Captive Dependency Detection

A **captive dependency** occurs when a longer-lived class depends on a shorter-lived one. This is always a bug — the longer-lived instance would hold a stale reference after the shorter-lived scope ends.

With inference, captive dependencies only occur when a class **explicitly** declares a lifecycle that conflicts with its dependencies:

```
singleton class BadService[ctx: RequestContext] { ... }
//         ^ compile error: singleton class 'BadService' depends on
//           scoped class 'RequestContext'
```

If `BadService` didn't say `singleton`, it would be inferred as `scoped` and everything would work. The error only fires when the programmer explicitly claims a lifecycle the compiler can prove is wrong.

**This is the key advantage over framework-based DI.** In Spring, this is a silent bug. In .NET, it's a runtime crash on the first request (if you remembered to enable `ValidateScopes`). In Pluto, it's a compile-time error with a clear message.

### Scope Blocks

A `scope` block is the entry point for scoped DI. It defines the lifetime boundary for all scoped instances:

```
scope(RequestContext { request_id: "abc", user_id: 42 }) |userSvc: UserService| {
    let user = userSvc.get_user(1)
    print(user)
}
// All scoped instances created above are now eligible for GC
```

**Anatomy of a scope block:**

1. **Seeds** — `scope(Foo { ... }, Bar { ... })` — struct literals for scoped classes that need external values. These are the "inputs" to the scope.
2. **Bindings** — `|name: Type, ...|` — which scoped instances the block body needs direct access to. The compiler creates these (and their transitive deps) automatically.
3. **Body** — normal code. Methods called on bound instances use their injected deps via `self` as usual.

The compiler, at the scope entry point:
1. Creates seed instances from the provided literals
2. Topologically sorts all scoped classes (same algorithm as singleton DI today)
3. Creates all other scoped instances, wiring deps from seeds and other scoped instances
4. Binds the requested variables
5. Executes the body
6. On exit, scoped instances become unreachable (GC reclaims them)

**Scoped classes with no external inputs** (no regular fields or all zero-initializable) are auto-created without needing to be listed as seeds:

```
scoped class RequestMetrics {
    count: int          // zero-initialized

    fn increment(mut self) { self.count = self.count + 1 }
    fn get(self) int { return self.count }
}

// No seed needed — RequestMetrics is auto-created
scope() |metrics: RequestMetrics, handler: RequestHandler| {
    handler.process()
    print(metrics.get())
}
```

### How Scoped Instances Flow Through Code

A common concern: "How does a function called from within the scope block access scoped instances?"

Answer: **exactly the same way it works today for singletons** — through DI fields on `self`.

```
scoped class RequestCtx { request_id: string }

// Inferred scoped
class UserService[ctx: RequestCtx] {
    fn get_user(self, id: int) string {
        // self.ctx is the scoped RequestCtx — wired at scope entry
        return "user-{id} (request {self.ctx.request_id})"
    }
}

// Inferred scoped (depends on UserService which is scoped)
class OrderService[users: UserService] {
    fn process(self, order_id: int) {
        let user = self.users.get_user(order_id)
        print(user)
    }
}

scope(RequestCtx { request_id: "req-1" }) |orders: OrderService| {
    orders.process(42)
    // OrderService.process uses self.users (scoped UserService)
    // UserService.get_user uses self.ctx (scoped RequestCtx)
    // All wired automatically at scope entry
}
```

No implicit parameters. No TLS. No runtime container. The compiler creates all instances with the right pointers at scope entry, and method calls follow pointers as usual.

### Transient Lifecycle

A `transient` class gets a fresh instance at every injection point:

```
transient class RequestId {
    value: string
}
```

Every class that depends on `RequestId` gets its own instance. Two classes in the same scope with `[id: RequestId]` get different `RequestId` instances.

**Transient + scoped interaction:** A transient class within a scope is created fresh per injection point *within that scope*. A transient class in singleton context is created once at startup per injection point.

**Transient classes must be auto-constructible** (no regular fields requiring external values). If a transient class needs initialization, it must get it from its own DI dependencies. This ensures the compiler can always create them without user input.

### Interaction with Concurrency

Scope blocks interact with `spawn`:

```
scope(RequestCtx { request_id: "r1" }) |handler: RequestHandler| {
    let task = spawn handler.process_async()
    // task captures scoped instances by reference
    task.get()   // wait for completion
}
```

**Rule: A scope block must not exit while tasks spawned within it are still running if those tasks reference scoped instances.** The compiler can enforce this statically: if a `scope` block contains `spawn` on scoped instances and doesn't `.get()` all tasks before the block exits, it's a compile error.

This aligns with structured concurrency — scoped resources have a defined lifetime, and concurrent tasks must respect it.

### Nested Scopes

Scope blocks can nest. Inner scopes shadow outer scopes for the same type:

```
scope(RequestCtx { request_id: "outer" }) |outerSvc: UserService| {
    print(outerSvc.get_user(1))  // request_id = "outer"

    scope(RequestCtx { request_id: "inner" }) |innerSvc: UserService| {
        print(innerSvc.get_user(1))  // request_id = "inner"
    }

    print(outerSvc.get_user(1))  // request_id = "outer" (restored)
}
```

Each scope block creates its own independent set of scoped instances.

### Ambient + Scoped Interaction

Ambient deps (`uses`) work naturally with scoped classes:

```
scoped class RequestCtx { request_id: string }

class UserService uses RequestCtx {
    fn get_user(self, id: int) string {
        return "user-{id} (request {requestCtx.request_id})"
    }
}

app MyApp {
    ambient RequestCtx

    fn main(self) {
        // ...
        scope(RequestCtx { request_id: "abc" }) |svc: UserService| {
            svc.get_user(1)
        }
    }
}
```

The ambient desugaring already converts `uses` to injected fields. Scope inference sees the injected `RequestCtx` dep and infers `UserService` as scoped. No special handling needed.

## Compiler Implementation

### Phase 1: Scope inference (typeck)

Extend `validate_di_graph()` with a scope inference pass after topological sort:

```
For each class in di_order:
    if class has explicit lifecycle annotation:
        class.lifecycle = annotation
    else:
        dep_lifecycles = [lifecycle(dep) for dep in class.injected_deps]
        class.lifecycle = min(dep_lifecycles) or Singleton (if no deps)

    // Validate: no captive dependencies
    for dep in class.injected_deps:
        if lifecycle(dep) < class.lifecycle:
            error: "singleton class X depends on scoped class Y"
```

Store `lifecycle: Lifecycle` in `ClassInfo` alongside existing fields.

### Phase 2: Scope block typeck

New expression: `Expr::Scope { seeds, bindings, body }`.

Type checking:
1. Validate each seed is a struct literal for a scoped class
2. Validate each binding type is scoped (or inferred scoped)
3. Build scoped DI sub-graph from seeds + bindings
4. Verify all scoped deps can be satisfied (either from seeds, auto-constructed, or singleton deps)
5. Type-check body with bindings in scope as local variables

### Phase 3: Codegen

At a scope block:
1. Emit code to create seed instances (same as struct literal codegen)
2. For each scoped class needed (in topological order):
   - Allocate via `__pluto_alloc`
   - Wire injected fields from seeds, other scoped instances, or singleton pointers
3. Bind local variables to the requested instances
4. Emit body code
5. No explicit cleanup needed — GC handles it

For transient classes:
- At each injection point, emit fresh allocation + wiring inline

### Phase 4: Scope validation for spawn

Walk scope block bodies. If `spawn` captures a scoped binding:
- Require corresponding `.get()` before scope block exit
- Or: require all tasks to be `.get()`'d (simpler rule)

## Examples

### HTTP Request Handler

```
import std.http

scoped class RequestCtx {
    request_id: string
    path: string
}

scoped class RequestMetrics {
    query_count: int

    fn record_query(mut self) {
        self.query_count = self.query_count + 1
    }
}

class UserRepository uses RequestCtx, RequestMetrics {
    fn find(self, id: int) string {
        requestMetrics.record_query()
        return "user-{id}"
    }
}

class UserController[users: UserRepository] {
    fn get(self, id: int) string {
        return self.users.find(id)
    }
}

class Router {
    fn handle(self, req: http.Request) http.Response {
        scope(RequestCtx { request_id: req.header("X-Request-Id"), path: req.path }) |ctrl: UserController, metrics: RequestMetrics| {
            let body = ctrl.get(1)
            print("Queries executed: {metrics.query_count}")
            return http.Response { status: 200, body: body }
        }
    }
}

app MyApp[router: Router] {
    fn main(self) {
        http.listen(8080, self.router)
    }
}
```

### Testing with Scope Overrides

```
class Database {
    fn query(self, sql: string) string {
        // real database query
    }
}

class UserService[db: Database] {
    fn get_user(self, id: int) string {
        return self.db.query("SELECT name FROM users WHERE id = {id}")
    }
}

test "user service returns user" {
    let mock_db = Database { /* mock config */ }
    scope(mock_db) |svc: UserService| {
        let user = svc.get_user(1)
        expect(user).to_equal("Alice")
    }
}
```

## App-Level Overrides

A class declares its natural lifecycle, but the app can override it:

```
class ConnectionPool { ... }   // default: singleton

app MyApp {
    scoped ConnectionPool       // override: per-request pool
    fn main(self) { ... }
}
```

This is useful when the same class is used differently in different contexts. The override must respect the captive dependency rule — you can make a singleton scoped (shorter lifetime, always safe) but not make a scoped class singleton (would violate deps).

## Migration / Backward Compatibility

- **No breaking changes.** All existing code continues to work — `singleton` is the default.
- **Opt-in.** Teams add `scoped` annotations only when they need per-request semantics.
- **Gradual.** Scope inference means you only annotate the "source" classes (`RequestCtx`), and everything that depends on them is automatically inferred.

## Open Questions

1. **Scope block syntax** — Is `scope(seeds) |bindings| { body }` the right syntax? Alternatives: `scope { provide Foo { ... } }`, `with Foo { ... } as foo { }`, `request { }`.

2. **Transient initialization** — How do transient classes get non-trivial initial values? Factory functions? Builder pattern?

3. **Scope nesting rules** — Can a `scoped` class in an inner scope depend on a `scoped` class from an outer scope? (Probably yes — outer scope instances are still alive.)

4. **Named scopes** — Should scopes be named/typed? e.g., `scope<Request>(...)` vs `scope<Connection>(...)` to allow multiple independent scope hierarchies.

5. **Thread safety of scoped instances** — If a scope block spawns multiple tasks, scoped instances are shared across those tasks (within the same scope). Should the compiler warn about mutable scoped state + concurrent access?

6. **Scope and closures** — If a closure captures a scoped binding and escapes the scope block, that's a use-after-free in spirit. Should this be a compile error?

## Future Work

- **Protocol contracts on scope boundaries** — `ensures` on scope exit (e.g., "transaction was committed or rolled back")
- **Scope-aware error handling** — Automatic rollback/cleanup on error within a scope
- **Custom scope types** — User-defined lifecycle beyond singleton/scoped/transient
- **Scope visualization** — Tooling that shows the scope graph alongside the DI graph

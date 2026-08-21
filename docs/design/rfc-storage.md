# RFC: Storage Declarations

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-14
**Depends on:** `rfc-schema.md`

## Summary

Introduce `storage` as a first-class language construct that binds schemas to persistent backends. The compiler knows what data is stored where, enabling type-safe migrations, deployment planning, and compile-time validation of data access patterns.

## Motivation

Today, database interactions in any language are stringly-typed:
- SQL queries are strings — no compile-time type checking
- ORM mappings are configuration — disconnected from the type system
- Schema changes require manual migration files — no compiler involvement
- The compiler has no idea what's stored where

In Pluto, whole-program compilation means the compiler can see everything. But if storage is just "call some database library," the compiler can't help. Storage declarations make persistence a first-class concept the compiler understands.

## Design

### Declaration Syntax

```
storage orders: Table<Order>
storage users: Table<User>
storage sessions: KeyValue<string, Session>
storage events: Append<Event>
```

Storage declarations are top-level constructs (like classes, schemas, functions). Each declares:
- A **name** — how code refers to this storage
- A **storage kind** — the abstract data model
- A **schema type** — what data is stored

### Storage Kinds

Storage kinds are abstract data models. The compiler understands their semantics; the runtime binds them to concrete backends.

| Kind | Semantics | Typical Backend |
|------|-----------|----------------|
| `Table<T>` | Relational table with CRUD operations, queryable | PostgreSQL, MySQL |
| `KeyValue<K, V>` | Key-value store with get/set/delete | Redis, DynamoDB |
| `Append<T>` | Append-only log, sequential reads | Kafka, event log |
| `Document<T>` | Document store with nested queries | MongoDB |
| `Queue<T>` | FIFO with enqueue/dequeue | SQS, RabbitMQ |

Each kind provides a fixed set of operations. The compiler type-checks calls against these operations.

### Operations by Kind

**`Table<T>`:**
```
storage users: Table<User>

class UserService {
    fn create(mut self, user: User) {
        users.insert(user)!
    }

    fn find(self, id: string) User {
        return users.get(id)!?
    }

    fn search(self, name: string) [User] {
        return users.query(
            (u: User) => u.name == name
        )!
    }

    fn update(mut self, id: string, user: User) {
        users.put(id, user)!
    }

    fn delete(mut self, id: string) {
        users.remove(id)!
    }
}
```

Operations: `insert`, `get`, `put`, `remove`, `query`, `count`, `exists`.

**`KeyValue<K, V>`:**
```
storage sessions: KeyValue<string, Session>

// Operations: get, set, delete, exists
sessions.set(session_id, session)!
let s = sessions.get(session_id)!?
sessions.delete(session_id)!
```

**`Append<T>`:**
```
storage events: Append<Event>

// Operations: append, read (sequential), tail
events.append(event)!
for event in events.tail(100)! {
    process(event)
}
```

**`Document<T>`:**
```
storage profiles: Document<UserProfile>

// Operations: insert, get, update, remove, query
profiles.insert(profile)!
let p = profiles.get(id)!?
```

**`Queue<T>`:**
```
storage tasks: Queue<WorkItem>

// Operations: enqueue, dequeue, peek
tasks.enqueue(work_item)!
let next = tasks.dequeue()!?
```

### All Storage Operations Are Fallible

Every storage operation can fail (network errors, timeout, not found). The compiler infers these as part of the error inference system. Storage operations automatically contribute to a function's error set:

```
fn get_user(id: string) User {
    return users.get(id)!?    // ! propagates StorageError, ? propagates none
}
// Compiler infers: get_user can raise StorageError and is nullable
```

### Storage and DI

Storage declarations are accessible from any class in the compilation unit — they're effectively globals, but the compiler tracks all access sites:

```
storage orders: Table<Order>

class OrderService {
    fn create(mut self, data: OrderData) string {
        let id = generate_id()
        let order = Order { id: id, ...data }
        orders.insert(order)!       // direct access to storage
        return id
    }
}
```

The compiler knows which classes access which storage declarations. This feeds into:
- **Migration planning** — which services are affected by a schema change
- **Concurrency analysis** — storage operations are inherently synchronized by the backend
- **Deployment ordering** — migrate storage before deploying services that use it

### Schema Requirement

Storage declarations must use **schema types**, not class types:

```
schema User {
    id: string
    name: string
    email: string
}

storage users: Table<User>          // OK — User is a schema

class UserService { ... }
storage services: Table<UserService> // COMPILE ERROR: storage requires schema type
```

This enforces the schema/class separation: schemas are data that crosses boundaries (including the persistence boundary), classes are behavioral.

### Backend Configuration

The storage declaration specifies the abstract data model. The concrete backend is configured at deployment time, not in source code:

```
// Source code — abstract
storage users: Table<User>

// Deployment config (not Pluto syntax — orchestration layer)
// users -> postgres://db.prod.internal:5432/myapp
// sessions -> redis://cache.prod.internal:6379/0
```

This means the same source code works with different backends in different environments (PostgreSQL in production, SQLite in tests, in-memory for unit tests).

### Indexes and Constraints

Storage declarations can include index and constraint hints that the migration system uses:

```
storage users: Table<User> {
    index email unique
    index name
    index created_at
}
```

These are declarative — the compiler and migration system handle the DDL:
- `index field` — create an index on the field
- `index field unique` — create a unique index
- `index field1, field2` — composite index

Constraints from schema invariants and `requires` clauses also inform the generated DDL:

```
schema User {
    email: string
    age: int
    invariant self.age >= 0
    invariant self.age <= 200
}

storage users: Table<User>
// Generated DDL includes: CHECK (age >= 0 AND age <= 200)
```

## Compiler Integration

### Type Checking

The type checker validates:
- Storage schema types exist and are schemas (not classes)
- Operations called on storage match the storage kind's API
- Schema types used in operations match the storage's declared type
- Index fields exist on the declared schema

### Migration System

Storage declarations are the input to the migration system (`rfc-migration.md`). When the compiler diffs the current source against a deployed snapshot:
- **Added storage** → generate CREATE TABLE / create collection / etc.
- **Removed storage** → flag for manual review (data loss)
- **Changed schema** → generate ALTER TABLE / migration based on `from` clauses
- **Changed indexes** → generate CREATE INDEX / DROP INDEX

### Concurrency Analysis

Storage operations go through external backends, so they don't participate in the rwlock-based synchronization of DI singletons. However, the compiler tracks which singletons access which storage for:
- Identifying concurrent write patterns (multiple tasks writing to the same storage)
- Flagging potential consistency issues (read-then-write without transactions)
- Ensuring storage is available before dependent services start

## Implementation

### Parser Changes

- New keyword: `storage`
- Storage declarations parsed as top-level items
- Syntax: `storage <name>: <Kind><TypeArgs> { <index-hints>? }`
- New AST node: `StorageDecl { name, kind, type_args, indexes }`

### Type System Changes

- New builtin types: `Table<T>`, `KeyValue<K, V>`, `Append<T>`, `Document<T>`, `Queue<T>`
- Methods on each type corresponding to the operations listed above
- All methods implicitly fallible (contribute `StorageError` to error sets)

### Codegen Changes

- Storage operations compiled to calls into the runtime's storage abstraction layer
- Runtime provides concrete backend implementations (initially: PostgreSQL for Table, Redis for KeyValue)
- Backend selection happens at runtime startup based on configuration

## Examples

### Example 1: CRUD Service

```
schema Todo {
    id: string
    title: string
    done: bool
    created_at: int
}

storage todos: Table<Todo> {
    index done
    index created_at
}

class TodoService {
    fn create(mut self, title: string) Todo {
        let todo = Todo {
            id: generate_id(),
            title: title,
            done: false,
            created_at: timestamp(),
        }
        todos.insert(todo)!
        return todo
    }

    fn complete(mut self, id: string) {
        let mut todo = todos.get(id)!?
        todo.done = true
        todos.put(id, todo)!
    }

    fn list_pending(self) [Todo] {
        return todos.query((t: Todo) => t.done == false)!
    }
}
```

### Example 2: Event Sourcing

```
enum EventType {
    OrderCreated,
    OrderConfirmed,
    OrderShipped,
}

schema OrderEvent {
    order_id: string
    type: EventType
    timestamp: int
    data: string
}

storage events: Append<OrderEvent>
storage order_state: KeyValue<string, Order>

class OrderService {
    fn create(mut self, items: [OrderItem]) string {
        let id = generate_id()
        let event = OrderEvent {
            order_id: id,
            type: EventType.OrderCreated,
            timestamp: timestamp(),
            data: serialize_items(items),
        }
        events.append(event)!

        let order = Order { id: id, items: items, status: OrderStatus.Pending }
        order_state.set(id, order)!
        return id
    }

    fn get(self, id: string) Order {
        return order_state.get(id)!?
    }
}
```

## Open Questions

- [ ] **Transactions.** How do multi-storage operations compose? Should there be a `transaction { ... }` block that ensures atomicity across storage operations? Deferred — important but orthogonal to the storage declaration itself.
- [ ] **Query language.** The `query` method takes a closure predicate. How expressive should this be? Should the compiler translate it to SQL/backend-native queries? Initially: in-memory filtering after full fetch. Later: query planning.
- [ ] **Migrations for non-Table kinds.** `ALTER TABLE` makes sense for relational storage. What does migration look like for key-value or append-only stores? Schema evolution applies to the value type regardless of storage kind.
- [ ] **Storage in modules.** Can imported modules declare storage? How do storage namespaces work across modules? Initially: all storage is per-compilation-unit.

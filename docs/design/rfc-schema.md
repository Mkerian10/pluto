# RFC: Schema Construct

**Status:** Draft
**Author:** Matt Kerian
**Date:** 2026-02-14
**Depends on:** None (foundational)

## Summary

Introduce a `schema` keyword for pure, value-typed data declarations that cross every boundary in a Pluto system: wire (RPC), storage (databases), and time (migrations). Schemas are distinct from classes — classes are behavioral reference types with DI and side effects; schemas are pure data with value semantics.

## Motivation

Pluto currently has one construct for structured data: `class`. Classes serve double duty — they model both behavioral services (with DI, methods, side effects) and plain data (DTOs, wire types, database rows). This conflation causes problems:

1. **Serialization ambiguity.** Which classes can be serialized? The ones without DI? Without side-effecting methods? There's no way to tell from the declaration.
2. **Migration opacity.** How does the compiler know which types represent stored data vs. ephemeral state? It can't track schema evolution if data types look identical to service types.
3. **Purity confusion.** A class method might have side effects, mutate global state, or depend on injected services. You can't call class methods in a contract expression or a `from` clause because purity isn't guaranteed.
4. **Value vs. reference.** Classes are reference types (identity, heap allocation, GC). Data that crosses boundaries should be value types (copy semantics, no identity, equality by content).

Schemas solve all four problems by introducing a dedicated construct for pure data.

## Design

### Declaration

```
schema Point {
    x: float
    y: float
}

schema Order {
    id: string
    items: [OrderItem]
    status: OrderStatus
    total_cents: int
}

schema OrderItem {
    product_id: string
    quantity: int
    unit_price_cents: int
}

enum OrderStatus {
    Pending,
    Confirmed,
    Shipped,
    Delivered,
}
```

Schemas look like classes but with the `schema` keyword. They have fields but fundamentally different semantics.

### Value Semantics

Schemas are **value types**, like integers or floats but composite:

```
let p1 = Point { x: 1.0, y: 2.0 }
let p2 = p1                          // copy, not reference
p2.x = 3.0                          // does not affect p1
print(p1.x)                         // 1.0

// Equality by content
let p3 = Point { x: 1.0, y: 2.0 }
print(p1 == p3)                      // true (same content)
```

No identity. No heap allocation (compiler may stack-allocate small schemas). Two schemas with the same field values are equal. Assignment copies.

### Pure Functions

Schemas can have functions, but they must be **pure** — no DI, no side effects, no `mut self` that isn't local:

```
schema Rectangle {
    width: float
    height: float

    fn area(self) float {
        return self.width * self.height
    }

    fn scale(self, factor: float) Rectangle {
        return Rectangle {
            width: self.width * factor,
            height: self.height * factor,
        }
    }

    fn perimeter(self) float {
        return 2.0 * (self.width + self.height)
    }
}
```

**Allowed in schema functions:**
- Field access (`self.field`)
- Arithmetic, comparisons, logical operators
- Calling other pure functions (schema methods, stdlib pure functions)
- Constructing schema instances
- Pattern matching
- Local `let`/`let mut` bindings

**Not allowed:**
- DI bracket deps (`schema Foo[dep: Type]` is a compile error)
- Calling class methods (classes can have side effects)
- I/O (print, file, network)
- `spawn` (concurrency)
- `raise` (schemas can't fail — they're pure data)
- Channel operations

The purity restriction means schema functions can be used in contract expressions, `from` clauses, and any other context that requires deterministic evaluation.

### No DI

Schemas have no bracket dependencies. The schema construct is explicitly not part of the DI system:

```
// COMPILE ERROR: schemas cannot have dependencies
schema BadSchema[db: Database] {
    name: string
}
```

### Generics

Schemas support generics, monomorphized like class generics:

```
schema Pair<A, B> {
    first: A
    second: B

    fn swap(self) Pair<B, A> {
        return Pair<B, A> { first: self.second, second: self.first }
    }
}

schema Envelope<T> {
    payload: T
    timestamp: int
    trace_id: string
}

let pair = Pair<int, string> { first: 42, second: "hello" }
let msg = Envelope<Order> { payload: order, timestamp: now(), trace_id: id }
```

### Conditional Fields

Schema fields can be conditional on a **discriminator** — an enum or bool field whose value determines which other fields exist:

```
enum PaymentMethod {
    Card,
    BankTransfer,
    Crypto,
}

schema Payment {
    amount_cents: int
    method: PaymentMethod

    // These fields exist only when method == Card
    card_number: string when method == PaymentMethod.Card
    card_expiry: string when method == PaymentMethod.Card
    card_cvv: string when method == PaymentMethod.Card

    // These fields exist only when method == BankTransfer
    routing_number: string when method == PaymentMethod.BankTransfer
    account_number: string when method == PaymentMethod.BankTransfer

    // These fields exist only when method == Crypto
    wallet_address: string when method == PaymentMethod.Crypto
    chain: string when method == PaymentMethod.Crypto
}
```

**Discriminator rules:**
- Discriminators must be `enum` or `bool` typed fields
- No string discriminators (strings are unbounded — can't enumerate cases)
- Discriminators must be unconditional fields (no conditional discriminators)
- Each conditional field specifies exactly one `when` clause

**Flow-sensitive field access:**

The type checker uses the discriminator value to determine which fields are accessible:

```
fn process(payment: Payment) {
    // Always accessible
    print(payment.amount_cents)
    print(payment.method)

    // Conditional access requires match or if
    match payment.method {
        PaymentMethod.Card {
            // card_number, card_expiry, card_cvv accessible here
            charge_card(payment.card_number, payment.card_expiry)
        }
        PaymentMethod.BankTransfer {
            // routing_number, account_number accessible here
            initiate_transfer(payment.routing_number, payment.account_number)
        }
        PaymentMethod.Crypto {
            // wallet_address, chain accessible here
            send_crypto(payment.wallet_address, payment.chain)
        }
    }

    // COMPILE ERROR: card_number not accessible without matching on method
    print(payment.card_number)
}
```

**Bool discriminator:**

```
schema User {
    name: string
    is_admin: bool

    admin_level: int when is_admin == true
    admin_notes: string when is_admin == true
}
```

**Construction:**

When constructing a schema with conditional fields, the compiler checks that all fields required by the discriminator value are provided:

```
let payment = Payment {
    amount_cents: 5000,
    method: PaymentMethod.Card,
    card_number: "4111111111111111",
    card_expiry: "12/28",
    card_cvv: "123",
}
// routing_number, account_number, wallet_address, chain not needed (method == Card)
```

### `from` Clauses

Schema fields can have `from` clauses that describe how to compute the field's value from a previous data shape. These are structural migration hints, not version annotations:

```
schema Order {
    total_cents: int from total: float => int(total * 100.0)
    customer_email: string from email: string => email
}
```

The `from` clause says: "If the old data had a field named `total` of type `float`, compute `total_cents` as `int(total * 100.0)`."

**Semantics:**
- `from` clauses are evaluated by the migration system, not at runtime
- The migration system diffs the current schema against a snapshot and matches `from` clauses structurally
- Multiple `from` clauses can exist on one field (for handling multiple possible old shapes)
- The `from` expression must be pure (same rules as schema functions)

See `rfc-migration.md` for how `from` clauses interact with the snapshot diffing system.

### Spread Composition

Schemas compose via spread syntax, not inheritance:

```
schema Timestamped {
    created_at: int
    updated_at: int
}

schema Identifiable {
    id: string
}

schema Order {
    ...Identifiable
    ...Timestamped
    items: [OrderItem]
    total_cents: int
}

// Order has fields: id, created_at, updated_at, items, total_cents
```

**Rules:**
- Spread copies fields into the target schema (flattening, not nesting)
- Field name conflicts are a compile error (no shadowing)
- Conditional fields from spread schemas retain their conditions
- Spread schemas' functions are NOT copied (only fields)
- No diamond problem — if two spreads would introduce the same field name, it's an error

**Why not inheritance:**
- Inheritance creates subtyping, which complicates value semantics (slicing, coercion)
- Spread is explicit — you can see exactly what fields a schema has
- No method dispatch ambiguity — schema functions are per-schema
- Composition is more flexible than single inheritance and simpler than multiple inheritance

### Trait Implementation

Schemas can implement traits, using the same nominal `impl` mechanism as classes:

```
trait Serializable {
    fn to_bytes(self) bytes
}

trait Printable {
    fn to_string(self) string
}

schema Point impl Printable {
    x: float
    y: float

    fn to_string(self) string {
        return "({self.x}, {self.y})"
    }
}
```

Trait methods on schemas must be pure (same rules as all schema functions).

### Schemas and Classes

Schemas and classes are distinct constructs with no conformance or inheritance relationship. Classes use schemas through composition:

```
schema OrderData {
    id: string
    items: [OrderItem]
    total_cents: int
}

class OrderService[db: Database, payment: PaymentGateway] {
    fn create(mut self, data: OrderData) string {
        let id = self.db.insert(data)!
        self.payment.charge(data.total_cents)!
        return id
    }

    fn get(self, id: string) OrderData? {
        return self.db.find(id) catch none
    }
}
```

The schema defines the data shape. The class provides behavior and side effects. They compose naturally through typed fields and parameters.

**What you cannot do:**
- A class cannot "conform to" or "extend" a schema
- A schema cannot contain a class-typed field (classes have identity; schemas don't)
- A schema cannot have DI dependencies
- A class method cannot be called from a schema function (impure)

## Serialization

Schemas are the only types that can cross boundaries:

- **Wire (RPC):** Schema types are automatically serializable. The compiler generates marshaling code for schema parameters and return types on pub stage/app methods.
- **Storage:** `storage` declarations bind schemas to backends (see `rfc-storage.md`).
- **Channels:** Schemas sent through channels use value copy (already the case for all types in copy-on-spawn).

Classes cannot be serialized. If a method needs to accept or return data across a boundary, it uses a schema type.

## Implementation

### Parser Changes

- New keyword: `schema`
- `schema` declarations parsed like class declarations but with:
  - No bracket deps
  - `when` clauses on fields
  - `from` clauses on fields
- New AST node: `SchemaDecl` (parallel to `ClassDecl`)

### Type System Changes

- New type: `PlutoType::Schema(name)`
- Schemas are value types (pass by copy, not by reference)
- Equality: structural (compare all fields)
- Assignment: deep copy
- Schemas are not subtypes of anything (no coercion except `Schema → Schema?`)

### Type Checker Changes

- Register schemas in `TypeEnv` (like classes)
- Enforce purity in schema functions (no DI, no I/O, no spawn, no raise)
- Flow-sensitive conditional field access based on discriminator values
- Validate `from` clause expressions are pure
- Validate spread composition (no field conflicts)
- Validate `when` clause discriminators (must be enum or bool, must be unconditional field)

### Codegen Changes

- Schema instances: stack-allocated when possible, heap-allocated with GC when escaping
- Assignment: emit deep copy
- Equality: emit field-by-field comparison
- Function calls: pass by value (copy)

### Migration System Integration

- The migration system reads schema declarations and their `from` clauses
- Snapshots capture schema shapes at deployment time
- The compiler diffs current schemas against snapshots to generate migrations
- See `rfc-migration.md` for details

## Examples

### Example 1: API Request/Response

```
schema CreateUserRequest {
    email: string
    name: string
    password: Secret<string>
}

schema UserResponse {
    id: string
    email: string
    name: string
    created_at: int
}

class UserService[db: Database] {
    fn create(mut self, req: CreateUserRequest) UserResponse {
        let id = generate_id()
        let now = timestamp()
        self.db.insert_user(req.email, req.name, req.password)!
        return UserResponse {
            id: id,
            email: req.email,
            name: req.name,
            created_at: now,
        }
    }
}
```

### Example 2: Conditional Fields for Polymorphic Data

```
enum NotificationType {
    Email,
    SMS,
    Push,
}

schema Notification {
    id: string
    type: NotificationType
    message: string

    // Email-specific
    to_address: string when type == NotificationType.Email
    subject: string when type == NotificationType.Email

    // SMS-specific
    phone_number: string when type == NotificationType.SMS

    // Push-specific
    device_token: string when type == NotificationType.Push
    badge_count: int when type == NotificationType.Push
}

fn send(notification: Notification) {
    match notification.type {
        NotificationType.Email {
            send_email(notification.to_address, notification.subject, notification.message)
        }
        NotificationType.SMS {
            send_sms(notification.phone_number, notification.message)
        }
        NotificationType.Push {
            send_push(notification.device_token, notification.message, notification.badge_count)
        }
    }
}
```

### Example 3: Schema Evolution with `from`

```
// Current version — prices in cents
schema Product {
    id: string
    name: string
    price_cents: int from price: float => int(price * 100.0)
    category: string from department: string => department
}
```

If the deployed snapshot had `price: float` and `department: string`, the migration system uses the `from` clauses to compute the transformation. No version numbers, no migration files — the diff tells the story.

### Example 4: Generic Envelope

```
schema Envelope<T> {
    ...Identifiable
    ...Timestamped
    payload: T
    version: int

    fn age_seconds(self, now: int) int {
        return now - self.created_at
    }
}

schema Identifiable {
    id: string
}

schema Timestamped {
    created_at: int
    updated_at: int
}

// Usage
let msg = Envelope<Order> {
    id: "msg-001",
    created_at: now(),
    updated_at: now(),
    payload: order,
    version: 1,
}
```

## Open Questions

- [ ] **Schema in schema:** Can a schema field be another schema type? (Yes — schemas compose naturally. Nested schemas are value-copied like all other schema fields.)
- [ ] **Enum fields in schemas:** Can schemas use enum types for fields? (Yes — enums are value types already. They work naturally as schema fields and as discriminators.)
- [ ] **Optional fields:** How do `T?` fields interact with conditional fields? (They're orthogonal — a field can be both optional and conditional. `name: string? when is_active == true` means the field exists when active but may still be `none`.)
- [ ] **Default values:** Should schemas support default field values? (Deferred — not needed for v1. Can be added later without breaking changes.)
- [ ] **Recursive schemas:** Can a schema contain a field of its own type? (Only through arrays or optionals — `children: [TreeNode]` works, `child: TreeNode` would be infinitely sized as a value type.)

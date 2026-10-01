# Objects and Entities

Every backend program contains two kinds of things, and most languages refuse to distinguish them.

A `Point { x: 1, y: 2 }` is **data**. It is its bytes. Copy it and nothing is lost; compare it field-by-field and you have compared everything there is. If reading a field hit the network, you would be shocked.

A payment service, an open file, a counter shared between threads — these are not data. Each one is an **entity**: it represents *the actual thing*, not a snapshot of it. Calling a method is a message to the thing; its state may change between calls; copying it would be semantically wrong (two payment services? two cursors into one file descriptor?). Identity is the point.

Mainstream languages give you one construct for both and leave the distinction in your head. Java makes everything an identity-bearing object and then bolts on `equals()`, records, and value-type proposals to claw data semantics back. Go and Rust give you structs and leave "is this a value or a service?" as a convention. Pluto makes the distinction a declaration:

- **`class`** declares a value — structural equality, deep-copied across concurrency boundaries.
- **`object`** declares an entity — reference identity, shared across concurrency boundaries, methods serialized.

## Declaring an object

An `object` body is syntactically a class body: fields, methods, bracket deps, invariants. The difference is entirely in the semantics the compiler attaches to instances.

```
object Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}
```

| | `class` | `object` |
|---|---|---|
| Represents | data (a value) | an entity (the thing itself) |
| `==` | structural — fields compared recursively | identity — same entity? |
| Crossing `spawn` | deep-copied | shared — it is the same entity |
| Concurrent method calls | caller's problem (or DI-inferred locks) | serialized per instance, by construct |
| Crossing a placement boundary | copied by value (wire-shaped) | by reference — an identity handle |

## Equality says what you mean

Value equality is structural. Two points with the same coordinates are the same point — there is nothing else to a value than its content. This works recursively through classes, enums, arrays, maps, sets, and nullables.

Entity equality is identity. Two counters that both read zero are still two different counters:

```
class Point {
    x: int
    y: int
}

object Counter {
    value: int

    fn get(self) int {
        return self.value
    }
}

fn main() {
    let a = Point { x: 1, y: 2 }
    let b = Point { x: 1, y: 2 }
    print(a == b)        // true — same content, same value

    let c = Counter { value: 0 }
    let d = Counter { value: 0 }
    let alias = c
    print(c == d)        // false — same fields, different entities
    print(c == alias)    // true — one entity, two names
}
```

This composes: a structural comparison of two class values that each hold a `Counter` field compares those fields *by identity*. Equal state in a different entity is not the same entity.

> **The pattern in the wild.** Transfusion medicine enforces entity-not-description with a wristband: blood is crossmatched for *the patient wearing the band*, not for anyone matching the chart's description — identity, never field equality. The type-and-crossmatch result expires after 72 hours (evidence with a validity window, not a permanent fact about the patient), and the two-person bedside check is validation at the point of effect — the moment the blood hangs, not the moment it was ordered. Every piece of that protocol is a construct in this book; [Verified Distribution](../vision/verified-distribution.md) makes the case that this is no coincidence.

## Spawn: values copy, entities share

Pluto's baseline concurrency rule is that `spawn` deep-copies its arguments. Each task gets its own world; data races on values are impossible by construction:

```
class Data {
    value: int

    fn bump(mut self) {
        self.value = self.value + 1
    }
}

fn work(mut d: Data) {
    d.bump()
}

fn main() {
    let mut d = Data { value: 0 }
    let t = spawn work(d)
    t.get()
    print(d.value)    // 0 — the task mutated its own deep copy
}
```

Objects invert the rule. The entity is *one thing*; a copy would mint a second identity, which is exactly wrong. So spawn shares entities — and sharing is safe because objects carry a stronger concurrency contract: **an object's methods are serialized**. Each instance processes one message at a time (a per-instance lock; distinct instances of the same type run concurrently). You get shared state without a heuristic "did both threads touch it?" analysis — the guarantee is attached to the construct that means shared identity.

Serialized means one *message* at a time, not one stack frame. An entity method that calls another method on the same instance — directly, through an alias, through mutual recursion, even via `at` placement on itself — is still processing the same message, and the call proceeds. Reentrancy within a message is not a deadlock and not a second message; the serialization boundary sits between messages from *outside* the activation.

```
object Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn work(mut self) {
        let mut i = 0
        while i < 1000 {
            self.increment()
            i = i + 1
        }
    }

    fn get(self) int {
        return self.value
    }
}

fn main() {
    let mut c = Counter { value: 0 }
    let t = spawn c.work()
    let mut i = 0
    while i < 1000 {
        c.increment()
        i = i + 1
    }
    t.get()
    print(c.get())    // 2000 — both sides hit the same entity, no lost updates
}
```

Write this with a `class` and the spawned task increments a private copy: you get 1000 and 1000, isolated. Write it with an `object` and you get one entity, 2000, and no data race — serialization is the construct's guarantee, not your discipline.

The sharing rule follows the entity wherever it is nested: deep-copying a class value into a task *shares* any entity stored inside it. Values copy; entities ride along by reference.

> **The pattern in the wild.** The famous LMAX architecture — an entire financial exchange matched on a single thread — is entity method serialization deployed at world scale. The matching engine is one entity: every order, every cancel, from every trader on earth, is a message to the same referent, processed one at a time. That is exactly why it needs no locks and loses no updates — the industry arrived at "serialize the authority, don't lock the data" through measurement and production pain, and here it is the semantics of the `object` keyword. [Verified Distribution](../vision/verified-distribution.md) collects this pattern library across industries.

## Generic objects

Objects can take type parameters. Each monomorphized instantiation is a distinct entity type with its own identity space and its own per-instance serialization:

```
object Topic<T> {
    name: string
    latest: T?
    published: int

    fn publish(mut self, msg: T) {
        self.latest = msg
        self.published = self.published + 1
    }

    fn count(self) int {
        return self.published
    }
}

fn main() {
    let mut news = Topic<string> { name: "news", latest: none, published: 0 }
    let mut metrics = Topic<int> { name: "metrics", latest: none, published: 0 }
    news.publish("breaking")
    metrics.publish(42)
    print(news.count())      // 1
    print(metrics.count())   // 1
}
```

`Topic<int>` and `Topic<string>` are different entity types entirely — their values cannot even be compared.

## Entities cross boundaries as handles

A class value crossing a placement boundary is copied — it is wire-shaped data. An entity cannot be copied without destroying what it means, so an entity crosses **by reference**: what serializes is an identity handle — the entity's home process, its type, and a per-home id. The wire stays a closed, compiler-derived surface; a handle is just schema-level data.

Identity survives the round trip. Send an entity to a server that stores it and hands it back, and the caller gets *the same entity* — `sent == returned` is `true`.

Calling through an entity is a placement expression — `at` runs the call **where the entity lives**:

```
object Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

fn main() {
    let mut c = Counter { value: 0 }
    at c { increment() } catch err {
        print("boundary failure")
        return
    }
    let n = at c { get() } catch -1
    print(n)    // 1
}
```

If the entity is local, `at` is a direct call. If it is a handle to an entity in another process, the same expression dials the entity's home, and the entity's own interface hash rejects version skew. Either way the boundary contract is identical — which is why every `at` must be handled with `!` or `catch`: a domain boundary can fail in *some* deployment plan, even one that happens to be colocated today.

Distribution stays explicit. A plain method call on a foreign handle outside `at` is a runtime error — the visible `at` boundary is the point. The [Distribution chapter](stages.md) covers placement, domains, and serving in full.

## Which one do you want?

The standard library answers this consistently and it is a good guide:

- **Resource holders are objects.** `fs.File`, `net.TcpListener`, `net.TcpConnection`, `http.HttpServer`, `http.HttpConnection` — each owns an OS resource with identity (a descriptor, a bound port, a cursor). Two copies of one `File` would share an offset and double-close a descriptor; as entities they are spawn-shared and their methods serialize.
- **Service singletons are objects.** A `PaymentService`, a served `BillingService` — one pool of funds with identity, mutations that must stick.
- **Wire-shaped data stays classes.** `Request`, `Response`, JSON values — snapshots, copied freely, compared structurally.
- **Transient local cursors stay classes.** Parsers and encoders in hot loops, where per-instance method serialization would cost and identity adds nothing.

A useful test from the DI system: anything that needs *observable per-injection identity* is, by definition, an entity. Values have no identity to observe.

## No inheritance, still

Objects do not add inheritance. Entity semantics are about identity, sharing, and messaging — not a second polymorphism mechanism. Traits (including default methods) cover behavior specialization and subtype polymorphism for objects exactly as they do for classes.

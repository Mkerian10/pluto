# Typestates

Every API has a protocol. A partition must be acquired before it is consumed. A lease must be released. A connection must be opened before it is read. Most languages document the protocol in prose, check it at runtime if you are lucky, and let you find out in production if you are not.

Pluto makes protocol violations **inexpressible**. A state is a type parameter; a method that is only legal in some state only *exists* in that state; a transition consumes the old binding so stale references cannot be reused. Calling out of order is not a runtime error — it is a method that does not exist, or a binding that is no longer there.

## State-gated methods

A typestate is an ordinary generic class whose type parameter encodes the state. States are plain marker classes. A `where S == State` clause after a method's return type restricts the method to one state:

```
class Unowned { tag: int }
class Owned { tag: int }

class Partition<S> {
    id: int

    fn acquire(self) Partition<Owned> where S == Unowned {
        return Partition<Owned> { id: self.id }
    }

    fn consume(self) int where S == Owned {
        return self.id
    }

    fn release(self) Partition<Unowned> where S == Owned {
        return Partition<Unowned> { id: self.id }
    }

    fn describe(self) string {       // no clause: exists in every state
        return f"partition {self.id}"
    }
}

fn main() {
    let u = Partition<Unowned> { id: 7 }
    let o = u.acquire()
    print(o.consume())    // 7
}
```

Method existence is per-instantiation. When `Partition<Owned>` is instantiated, only methods whose constraints are satisfied get registered; monomorphization never even generates the excluded ones. Calling `consume()` on a `Partition<Unowned>` is not a "constraint violation" — the method is simply not there, and the diagnostic says why:

```
error: method 'consume' does not exist on 'Partition<Unowned>':
on 'Partition' it exists only where S == Owned
```

Functions express state requirements with ordinary parameter types — `fn process(p: Partition<Owned>)` needs no special syntax. Passing a `Partition<Unowned>` is a plain type mismatch.

## Transitions consume their receiver

Gating methods by state is only half the guarantee. After `let o = u.acquire()`, what stops you from calling `u.acquire()` again — a stale alias in the pre-transition state?

Linearity does. A **transition method** is one whose return type is the same class with a state parameter changed (`Partition<Unowned>` → `Partition<Owned>`). Calling a transition through a local binding *consumes* that binding; later uses are compile errors:

```
let u = Partition<Unowned> { id: 7 }
let o = u.acquire()
let o2 = u.acquire()    // COMPILE ERROR
```

```
error: 'u' was consumed by the transition '.acquire()' (it is now
Partition<Owned>); use the transition's result, or rebind 'u'
```

The discrimination is automatic and surgical: only type parameters that appear in a `where` clause count as state parameters, so data generics never participate — `Box<T>.map()` returning `Box<U>` consumes nothing, and a class with no `where` clauses is untouched by the analysis. Reassignment or a fresh `let` revives a consumed binding. Branch joins are conservative (consumed on any path means consumed after the join), loops are analyzed to a fixpoint (a transition in iteration one is caught as a use in iteration two), and capturing a consumed value in a closure or `spawn` is a use.

## Must-release states

Some states are not just restrictive — they are *obligations*. Holding a lease means you owe a release. Pluto lets the class say so:

```
class Idle { tag: int }
class Held { tag: int }

class Lease<S> {
    id: int

    must_release Held

    fn acquire(self) Lease<Held> where S == Idle {
        return Lease<Held> { id: self.id }
    }

    fn release(self) Lease<Idle> where S == Held {
        return Lease<Idle> { id: self.id }
    }
}
```

A binding in a `must_release` state is fully linear. Dropping it — by fall-through, `return`, `break`, rebinding, or just never touching it again — is a compile error:

```
fn main() {
    let l = Lease<Idle> { id: 7 }
    let h = l.acquire()
    print("done")        // COMPILE ERROR: h was never discharged
}
```

```
error: 'h' still holds Lease<Held>, a must_release state, when it goes
out of scope; transition it out of 'Held' (e.g. .release()), move it
onward, or return it
```

The diagnostic computes the suggested exits from the class's own transitions. The rules:

- **Moves travel the obligation.** `let b = a`, passing as an argument, returning, and raising inside an error payload each move the value — the caller is clean, and the receiver now owes the discharge. One obligation, one owner, always.
- **Capture is rejected.** A closure or `spawn` capturing a live must-release binding would duplicate the evidence.
- **Field and container stores are rejected.** The obligation would escape the analysis.
- **Discharge** is a consuming transition out of the state, moving the value onward, or returning it.

> **The pattern in the wild.** Electrical lockout-tagout is must-release linearity made of steel. Before working on a circuit, a worker hangs a personal padlock and tag on the breaker: a token you hold in your hand, and re-energization is impossible until *you* remove it — nobody else's key fits. The multi-padlock hasp goes further: every worker on the job locks the same hasp, and the breaker cannot close until **all** locks come off. That shape — conjunctive linear obligations, N holders, all must release — is one Pluto does not have yet; `must_release` tracks a single owner. Breaker interlocks ("cannot close unless proven de-energized") are the other half of the pattern: the effect site dominated by a validity check. [Verified Distribution](../vision/verified-distribution.md) traces both shapes across the industries that invented them.

## Degradation: when the world moves you out of a state

Transitions so far are things *you* do. Distribution adds transitions the world does: the coordinator revokes your lease between two instructions, and no local type can prevent it. Pluto's answer is **state-carrying errors** — a fallible transition whose error carries the value in its post-failure state:

```
class Idle { tag: int }
class Held { tag: int }
class Revoked { tag: int }

error Degraded { lease: Lease<Revoked> }

class Lease<S> {
    id: int
    epoch: int

    must_release Held

    fn acquire(self) Lease<Held> where S == Idle {
        return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
    }

    fn renew(self) Lease<Held> where S == Held {
        if self.epoch > 2 {
            raise Degraded { lease: Lease<Revoked> { id: self.id, epoch: self.epoch } }
        }
        return Lease<Held> { id: self.id, epoch: self.epoch + 1 }
    }

    fn release(self) Lease<Idle> where S == Held {
        return Lease<Idle> { id: self.id, epoch: self.epoch }
    }

    fn describe(self) string {
        return f"lease {self.id} (epoch {self.epoch})"
    }
}
```

Because `Degraded`'s payload is the receiver's class at a *changed* state (`Lease<Revoked>` vs the receiver's `Held`), the compiler discriminates it as a **degradation error** — the exact mirror of the transition rule. That drives the error-path bookkeeping:

```
fn main() {
    let l = Lease<Idle> { id: 7, epoch: 0 }
    let h = l.acquire()

    let h2 = h.renew() catch e: Degraded {
        print(f"degraded: {e.lease.describe()}")
        return
    }

    let done = h2.release()
    print(f"released: {done.describe()}")
}
```

Raising a degradation error moves the receiver into the payload. Catching one therefore consumes the receiver **on the error path only**: inside the handler the old binding is gone and the degraded value lives in `e.lease`; a handler that terminates (`return`, `raise`, `break`) leaves the success path's binding valid with no ceremony. Recovery is just reading the payload out — `let stale = e.lease`.

The obligation survives the failure. If the payload's state is itself must-release, wildcard and shorthand catches are rejected (they would strand the payload); a typed handler takes the obligation on as `e.field`. In practice degraded states like `Revoked` are droppable — "the world took it from you" should not pile ceremony on the recovery path — while only `Held`-like states carry obligations. Note the design cut: `release()` transitions out of `Held` and the degraded value needs no release, because "clean up your local belief" is the one duty the world cannot revoke.

## What a typestate honestly claims

`Lease<Held>` does **not** mean "the coordinator still considers me the holder." It means "I acquired and have not released" — a record of your own actions, which nothing outside the process can falsify. That is the only kind of fact a static type can honestly carry in a distributed system; remote agreement can be revoked between any two instructions, which is exactly why revocation surfaces as a typed, catchable `Degraded` error rather than being promised away by the type.

The same reasoning explains a rule you will notice: **typestate never lives on an [object](objects.md)**. Entities are the sharing construct — if another task transitions a shared entity, your binding's static state would be a lie. The factoring that works: the entity is the *authority* (shared, always answering with its current opinion), and typestated values are *evidence* it issues — grants, leases, tokens — linear, possibly must-release, honest about their staleness, validated by the authority at the point of effect. The [Verified Distribution](../vision/verified-distribution.md) chapter develops this into Pluto's larger direction.

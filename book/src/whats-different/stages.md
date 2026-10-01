# Distribution: Placement, Domains, and Stages

Pluto is not an RPC language. It does not try to make network calls look local — that trick papers over exactly the things distributed programmers must design for: placement, data movement, partial failure, and the call that fails without telling you whether it executed.

Pluto is a **distributed language**. The central principle:

> **Logical distribution is part of the program. Physical deployment is not.**

Most systems collapse three concerns: what computation happens, which domain owns it, and which process/pod/region runs it. Pluto puts the first two in the program and leaves the third to the deployment binding — the way a database separates the logical query from its physical execution plan. A program can be split apart or collapsed together without rewriting business logic, because the code never said "make an HTTP call"; it said *where the computation belongs*.

## Placement: `at`

A placement expression evaluates its body in another logical execution domain:

```
import std.wire

error PaymentDeclined {
    code: int
}

// The payment service is an entity — THE payment domain's capability,
// not a copyable value — so it is an object (see Objects and Entities).
object PaymentService {
    fn charge(self, amount: int) int {
        if amount > 100 {
            raise PaymentDeclined { code: 402 }
        }
        return amount * 2
    }
}

app Shop[pay: domain PaymentService] {
    fn main(self) {
        let charged = at self.pay {
            charge(21)
        } catch err: PaymentDeclined {
            0 - err.code
        } catch err {
            -1
        }
        print(f"charged: {charged}")
    }
}
```

`pay: domain PaymentService` declares a dependency on the payment domain. `at self.pay { charge(21) }` means: evaluate this call *in that domain*. It does not mean "make an RPC call."

How the boundary is physically crossed is the deployment binding's decision, made at startup — not a code change:

```
# Plan A — colocated: the domain is wired in-process, at is a direct call
$ ./shop

# Plan B — distributed: the domain lives in another process
$ PLUTO_DOMAIN_PAYMENTSERVICE=127.0.0.1:9000 ./shop
```

Same binary. Same semantics. The compiler checks the boundary contract identically in both plans: values crossing must be wire-shaped, errors are typed and inferred across the boundary, and the `at` **must** be handled with `!` or `catch` — because a domain boundary can fail in *some* deployment plan, even one that happens to be colocated today. Distribution is explicit — `at` is a visible, syntactic boundary — but transport is never the programming model.

## Serving a domain

The other side of Plan B is a process that serves the domain's entity:

```
import std.wire

error PaymentDeclined {
    code: int
}

object PaymentService {
    fn charge(self, amount: int) int {
        if amount > 100 {
            raise PaymentDeclined { code: 402 }
        }
        return amount * 2
    }
}

fn main() {
    let svc = PaymentService {}
    serve svc on 0    // port 0: OS-assigned, printed at startup
}
```

`serve` runs one thread per connection, so state mutated by a handler sticks — which is the point of calling an entity at home. The served object's methods serialize per instance (they always do — it is an entity), so concurrent connections cannot interleave mid-method.

Typed errors cross the boundary: the client's `catch err: PaymentDeclined` above works identically whether the decline came from an in-process call or over a socket. And every boundary is guarded by **interface hashing** — a caller and callee built from skewed versions of the interface are rejected at the boundary instead of silently mis-decoding each other's data.

## Entities cross as handles

Wire-shaped class values cross a boundary by copy. Entities cross **by reference**: what serializes is an identity handle (home process, type, id), and calling through a handle is placement — `at entity { method() }` runs the call where the entity lives. A server can accept an entity handle, store it, hand it back, and the caller gets the same entity — identity survives the round trip. Plain method calls on a foreign handle outside `at` are runtime errors: the boundary stays visible. See [Objects and Entities](objects.md).

## Stages: lifecycle shells

A `stage` is a declaration for a deployable unit's lifecycle: a shell with DI dependencies and a `main`, but no state of its own. Stages support `requires fn` members — lifecycle templates that concrete stages fill in:

```
class Config {
    fn db_url(self) string {
        return "postgres://localhost/mydb"
    }
}

class Database[config: Config] {
    fn query(self, sql: string) string {
        return f"result: {sql} (db={self.config.db_url()})"
    }
}

// Abstract base stage — defines the lifecycle template
stage Daemon {
    requires fn start(self)
    requires fn run(self)
    requires fn stop(self)

    fn main(self) {
        self.start()
        self.run()
        self.stop()
    }
}

// Concrete stage — inherits the lifecycle, adds DI
stage Worker : Daemon [db: Database] {
    override fn start(self) {
        print("Worker starting...")
    }

    override fn run(self) {
        print(self.db.query("SELECT * FROM jobs"))
    }

    override fn stop(self) {
        print("Worker stopped.")
    }
}
```

Each stage gets its own dependency graph, allocated when its process starts. Stages are one piece of the distribution story — the lifecycle/packaging piece — not the whole of it; the programming model for crossing boundaries is `at` over domains.

## Systems: the whole program, checked

A `system` declaration names a set of modules that deploy together, and the compiler checks them *against each other* before any binary exists:

```
// main.pt
import billing
import orders

system Shop {
    billing: billing
    orders: orders
}
```

The `orders` module declares a remote dependency (`app OrdersApp[billing: remote BillingService]`) against a local interface mirror; the `billing` module serves the real `BillingService`. Compiling the system verifies that the dependency is actually served, that the interfaces match, and that the error contracts line up — then produces one binary per member:

```
$ pluto compile main.pt -o build --stdlib stdlib
  compiled billing → build/billing
  compiled orders → build/orders
system: 2 member(s) compiled
```

There is no gRPC schema to drift out of date, no Swagger spec to trust. The types match because one compiler checked both sides.

A note on `remote`: it is the first physical transport, shipped and working — but it couples the transport decision into the code, which is exactly what the placement model removes. `at` over logical domains is the target programming model; expect `remote`-style explicit wiring to be subsumed by it.

## What `at` buys you that RPC frameworks don't

Because the boundary is a language construct, the compiler checks what programmers otherwise track by convention:

- **Value transfer** — only wire-shaped data crosses by copy; entities cross as handles; anything else is rejected at compile time.
- **Typed failure** — the callee's error set flows into the caller's, plus the boundary's own failure modes, and handling is mandatory. There is no "forgot to handle the network error." And the boundary's own failure is honest about what is known: every `NetworkError` carries a [`definite` classification](errors.md#errors-at-a-distance-networkerrordefinite) — the request provably never dispatched, or sent with no answer.
- **Version skew** — interface hashes are checked at the boundary, not discovered as corrupted decodes.
- **Semantics under colocation** — fusing two domains into one process is an optimization, and it is only legal if it preserves boundary semantics: no shared mutable references appear across a boundary that would have copied, and code written to survive an unreachable domain keeps that failure mode in its contract.

An `at` boundary can also fail *ambiguously* — the request left, no answer returned, and the effect may or may not have applied. No local check can resolve that; it is part of the boundary's contract, and it is precisely what "RPC looks local" hides. Two pieces of making ambiguity first-class have shipped: the definite/ambiguous split itself, carried truthfully on every boundary failure as [`NetworkError.definite`](errors.md#errors-at-a-distance-networkerrordefinite), and compiler-proven fencing via [`guarded_by`](contracts.md#guarded_by-every-write-provably-fenced) — the `examples/blob` store's single-writer safety theorem is now fully compiler-checked, not held by inspection. The remaining layer — idempotency and its kin as *library-defined properties* a retry combinator can demand — is the subject of [Verified Distribution](../vision/verified-distribution.md).

## Where this is going

Shipped and tested today: whole-program compilation across service code, `at` placement over domain dependencies with deployment-plan binding, `serve`, typed errors across boundaries with definite/ambiguous failure classification, the schema-level wire format, interface hashing, entity handles, stages, and systems.

Deliberately open, and stated as such: what exactly declares a logical domain, the deployment-plan artifact that binds domains to physical placement, deadline and cancellation propagation, the precise legality rules for fusing domains, and structured distributed computation (parallel `at`, scatter/gather). The model these will land in — logical placement, physical execution — is settled; the pieces arrive incrementally, each checked by the same compiler that sees both sides of every boundary.

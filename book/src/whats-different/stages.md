# Stages: Distributed Systems as Programs

*(This chapter is aspirational — stages are designed but not fully implemented.)*

Right now, building a distributed system means building multiple services in multiple repos, with hand-written RPC code, manual serialization, and orchestration scripts to wire them together. Pluto's vision is to make the distributed system **the program**, with stages as independently deployable units and the compiler generating all the wiring.

## The Problem with Microservices

When you build a distributed backend today, you're really building several systems:

**The services themselves.** Order service, payment service, inventory service. Each is a separate codebase, separate repo, separate deployment.

**The communication layer.** gRPC schemas, REST APIs, message queues. You write the same serialization code in every service. You handle network errors manually. You retry, you add circuit breakers, you log everything.

**The orchestration.** Kubernetes manifests, Terraform configs, deployment scripts. These define how services discover each other, how they scale, where they run. This configuration is *separate* from your code — the compiler never sees it.

The result: your distributed system is **implicit**. The compiler sees isolated services. It doesn't know they communicate. It can't check that a function call across service boundaries has compatible types. It can't generate the RPC code. It can't verify that your error handling is complete.

## Pluto's Approach: Stages

In Pluto, a **stage** is a unit of deployment. It's like a service, but it's declared in your program, not in YAML:

```
stage api {
    fn handle_order(req: OrderRequest) OrderResponse {
        let user = auth.verify_token(req.token)!
        let order = orders.create_order(user.id, req.items)!
        let payment = billing.charge(user.id, order.total)!
        return OrderResponse { order_id: order.id }
    }
}

stage auth {
    pub fn verify_token(token: string) User {
        // validate JWT, look up user
    }
}

stage orders {
    pub fn create_order(user_id: int, items: Array<Item>) Order {
        // persist order to database
    }
}

stage billing {
    pub fn charge(user_id: int, amount: int) Payment {
        // call Stripe, record transaction
    }
}
```

This is **one program**. The compiler sees all four stages. It sees that `api` calls `auth.verify_token`, `orders.create_order`, and `billing.charge`. It knows these calls cross stage boundaries.

**The compiler generates RPC code automatically.**

## What the Compiler Generates

When you call a function in another stage, the compiler sees the call and generates:

**Serialization.** The arguments are serialized into a wire format (JSON, Protobuf, or a future Pluto binary format). The compiler knows the types of the arguments, so it generates the exact serialization code needed.

**HTTP transport.** The compiler generates an HTTP client call to the target stage. The endpoint is discovered from configuration (environment variables, service discovery, etc.).

**Deserialization.** The response is deserialized back into the return type. The compiler knows the return type, so it generates the exact deserialization code needed.

**Error propagation.** If the remote function can raise errors, the compiler includes those errors in the call site's error set. Network errors (timeout, connection failure) are automatically added as a built-in `NetworkError` type. The `!` operator handles both remote errors and network errors uniformly.

From your perspective, it's a function call:

```
let user = auth.verify_token(req.token)!
```

From the compiler's perspective, it generates (pseudocode):

```
let request_body = serialize_json({ token: req.token })
let response = http_post("http://auth-service/verify_token", request_body) catch err {
    raise NetworkError { reason: "auth service unreachable" }
}
let user = deserialize_json(response.body, User) catch err {
    raise DeserializationError { reason: "invalid user format" }
}
if response.has_error {
    raise deserialize_error(response.error)  // remote raised an error
}
return user!  // propagate to caller
```

You write one line. The compiler generates 10+ lines of serialization, HTTP, error handling, and deserialization. **Because it sees the whole program, it knows what to generate.**

## Stages Define Deployment Boundaries

When you compile a Pluto program with stages, the compiler generates **one binary per stage**:

```
$ pluto compile-stages my_app.pluto --output-dir ./build

Generated:
  build/api
  build/auth
  build/orders
  build/billing
```

Each binary is a self-contained native executable. You deploy them independently:

```
# Deploy to separate containers
docker build -t my_app/api ./build/api
docker build -t my_app/auth ./build/auth
docker build -t my_app/orders ./build/orders
docker build -t my_app/billing ./build/billing

# Or deploy to different regions
api -> us-east-1
auth -> us-east-1
orders -> us-west-2
billing -> eu-west-1
```

But here's the key: **they were compiled from one program**. The compiler saw all four stages, type-checked all the cross-stage calls, inferred all the error sets, and generated all the RPC code. There's no manual gRPC schema. There's no Swagger spec that might be out of date. The types are guaranteed to match because the compiler checked them.

## Configuration Separates Code from Environment

The stages above call each other by name (`auth.verify_token`), but they don't hardcode URLs. The compiler generates code that reads endpoint locations from configuration:

```
# config/production.toml
[stages.auth]
endpoint = "https://auth.prod.mycompany.com"

[stages.orders]
endpoint = "https://orders.prod.mycompany.com"

[stages.billing]
endpoint = "https://billing.prod.mycompany.com"
```

At runtime, the `api` binary reads this config and knows where to send RPC calls. In dev, you might point everything to `localhost`. In staging, you might point to staging URLs. In production, you might use service discovery (Consul, Kubernetes DNS) to find endpoints dynamically.

**The code is the same. The config changes.**

This is the "environment opacity" principle: the same compiled binary works in dev, staging, and prod. Only the configuration changes.

## Error Handling Across Stages

Errors compose naturally across stage boundaries. If a remote stage can raise `NotFound`, that error flows through the RPC layer and into the caller's error set:

```
stage orders {
    error OrderNotFound { order_id: string }

    pub fn get_order(id: string) Order {
        if !exists(id) {
            raise OrderNotFound { order_id: id }
        }
        return load_order(id)
    }
}

stage api {
    fn handle_get_order(id: string) Response {
        let order = orders.get_order(id)!  // might raise OrderNotFound or NetworkError
        return Response { body: order }
    }
}
```

The compiler sees that `orders.get_order` can raise `OrderNotFound`. When `api` calls `get_order` across the stage boundary, the compiler:
1. Generates serialization of `OrderNotFound` in the `orders` binary
2. Generates deserialization of `OrderNotFound` in the `api` binary
3. Adds `OrderNotFound` to the error set of `handle_get_order`
4. Enforces that `api` handles the error with `!` or `catch`

Network errors (timeout, connection refused) are added automatically as a built-in error type. You don't handle them separately — they're just part of the error set.

## Stages vs. Pods vs. Services

A stage is **not** a Kubernetes pod. A stage is **not** a Docker container. A stage is a **logical unit of deployment** in your program.

How you deploy stages is up to you:
- One stage per container (microservices)
- Multiple stages in one container (monolith)
- Stages spread across regions (geo-distributed)
- Stages scaled independently (autoscaling)

The compiler doesn't dictate deployment topology. It just generates the binaries and the RPC code. You decide where to run them.

## Stages and Dependency Injection

Stages integrate with Pluto's dependency injection system. Each stage is an `app` with its own dependency graph:

```
stage api[db: Database, cache: Cache] {
    fn handle_request(req: Request) Response {
        let user = self.db.query("SELECT * FROM users WHERE id = {req.user_id}")!
        return build_response(user)
    }
}

stage workers[db: Database, queue: JobQueue] {
    fn process_jobs() {
        for job in self.queue.poll() {
            self.db.update(job.id, job.result)!
        }
    }
}
```

Each stage has its own `Database` instance, its own `Cache` instance. They're allocated when the stage's binary starts. The DI system is per-stage, not global.

This means you can:
- Use different database configs per stage
- Use different cache backends per stage
- Inject mocks for testing individual stages

The stages communicate via `pub` functions (RPC), not via shared DI state. This is deliberate: stages are **loosely coupled** at runtime, but **tightly coupled** at compile time (the compiler sees all of them).

## Orchestration: The Layer Above Pluto

Pluto compiles stages into binaries. It doesn't deploy them. It doesn't manage their lifecycle. That's the job of an **orchestration layer** built on top of Pluto.

The vision:
1. **Pluto compiles your program** into per-stage binaries with all RPC generated
2. **You write deployment specs** (YAML, HCL, or a future Pluto-native format) that define:
   - Which stages run where (regions, availability zones)
   - How they scale (replicas, autoscaling rules)
   - What resources they need (CPU, memory, disk)
3. **An orchestrator deploys them** (Kubernetes, Nomad, a future Pluto-native orchestrator)
4. **Service discovery wires them together** at runtime (DNS, Consul, environment variables)

The key: **the orchestrator is not part of the language**. It's a separate system. Pluto's job is to compile correct, efficient binaries. The orchestrator's job is to run them.

This is the opposite of frameworks like Spring Cloud or Akka, where deployment concerns leak into your code via annotations and framework APIs. In Pluto, deployment is **configuration**, not code.

## The Stages You Don't Write

Eventually, Pluto will support **generated stages** — stages that the compiler creates automatically based on your program structure.

Example: database migrations. Instead of writing migration scripts, you declare your schema in your program:

```
stage db {
    schema User {
        id: int primary key
        email: string unique
        created_at: timestamp
    }

    pub fn get_user(id: int) User {
        return query("SELECT * FROM users WHERE id = {id}")!
    }
}
```

The compiler sees the `schema` declaration and generates:
1. A migration stage that creates/updates the `users` table
2. SQL queries for `get_user` that match the schema
3. Type-checked queries (if you try to access a field that doesn't exist, compilation fails)

You never write SQL. You declare the schema in Pluto. The compiler generates the SQL, the migrations, and the query code. **Because it sees the whole program, it can do this safely.**

## The End Goal: One Program, Many Deployments

The vision is a world where:
- You write **one program** with multiple stages
- The compiler **sees all of it** and generates per-stage binaries with all RPC code
- You write **deployment configuration** (not code) to define where stages run
- An orchestrator **deploys the binaries** and wires them together
- Service discovery and config management **connect the stages** at runtime

From your perspective, you wrote a program with function calls. The compiler turned it into a distributed system.

No gRPC schemas. No manual serialization. No out-of-date API docs. No wondering if a service's error handling changed. **The compiler checked it all.**

This is what whole-program compilation enables at scale: distributed systems that are **programs**, not collections of loosely coordinated services.

## Where We Are Today

Stages are designed but not implemented. Here's what exists:
- The `app` model (one app per program)
- Whole-program compilation and analysis
- Package dependencies (source-level composition)
- Error inference across function calls
- Dependency injection

Here's what's coming:
- The `stage` keyword and multi-stage programs
- Cross-stage RPC generation
- Per-stage binary output
- Configuration-based endpoint resolution
- Orchestration integration

The foundation is in place. Stages are the next step. And when they land, Pluto will be the first language where **your distributed system is your program, and the compiler is your infrastructure.**

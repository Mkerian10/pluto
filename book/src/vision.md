# AI-Native Development

Whole-program compilation means the Pluto compiler builds a complete understanding of your program: resolved types for every expression, inferred error sets for every function, the full call graph, the dependency wiring topology, cross-references between every declaration and its usage sites.

Most compilers throw this knowledge away after code generation. Pluto keeps it.

The compiler's internal representation — the AST, the type information, the analysis results — is a first-class artifact: serializable, queryable, and designed for AI agents to read and write directly. The compiler sees your entire program and understands it deeply. AI agents get access to that same understanding through a structured API.

This chapter covers what is implemented today and where the design is headed.

## What Exists Now

### Binary AST Format (PLTO)

Pluto source files can be compiled to a binary AST format that preserves the full semantic graph:

```
$ plutoc emit-ast main.pt -o main.pluto       # text -> binary AST
$ plutoc generate-pt main.pluto                # binary AST -> text (stdout)
$ plutoc generate-pt main.pluto -o main.pt     # binary AST -> text file
$ plutoc sync main.pt --output main.pluto      # merge text edits back, preserving UUIDs
```

The binary format (`.pluto`) contains the complete parsed AST, the original source text, and derived analysis data. The text format (`.pt`) is standard Pluto syntax that humans read and edit.

### Stable UUIDs

Every declaration in a Pluto program has a stable UUID: functions, classes, enums, traits, methods, fields, parameters, error declarations, enum variants, and app declarations. UUIDs are assigned at creation time and survive renames, moves, and refactors.

This is the foundation. When an AI agent renames a function, the UUID stays the same. Every call site, struct literal, enum usage, and raise site that references that declaration tracks the UUID, not the name string. The name is display text. The UUID is identity.

### Cross-References

The compiler resolves and stores cross-references by UUID:

- **Call sites** -- which functions call which other functions, tracked by the callee's UUID
- **Struct literals** -- where each class is constructed, tracked by the class UUID
- **Enum usages** -- where each enum variant is used, tracked by enum and variant UUIDs
- **Raise sites** -- where each error type is raised, tracked by the error UUID

These are not heuristics or text search. They are exact, compiler-resolved references that survive across renames.

### The SDK (`plutoc-sdk`)

The SDK is a Rust crate for reading and writing Pluto programs as structured data. It is what AI agents (and any tooling) use to interact with Pluto code without parsing text.

**Loading a module:**

```rust
use plutoc_sdk::Module;

// From binary format
let module = Module::from_bytes(&bytes)?;

// From source text (parse without full compilation)
let module = Module::from_source(source)?;

// From a .pluto source file (full front-end pipeline with analysis)
let module = Module::from_source_file("main.pluto")?;
```

**Querying:**

```rust
// List all functions, classes, enums, traits, errors
for f in module.functions() {
    println!("{}: {}", f.name(), f.id());
}

// Look up by name or UUID
let decls = module.find("process_order");
let decl = module.get(some_uuid);

// Cross-references
let callers = module.callers_of(function_uuid);
let constructors = module.constructors_of(class_uuid);
let usages = module.enum_usages_of(enum_uuid);
let raises = module.raise_sites_of(error_uuid);
```

**Editing:**

```rust
let mut editor = module.edit();

// Add declarations
let id = editor.add_from_source("fn greet() {\n    print(\"hello\")\n}\n")?;

// Replace (preserves UUID)
editor.replace_from_source(existing_id, "fn greet() {\n    print(\"goodbye\")\n}\n")?;

// Rename (updates all references)
editor.rename(function_id, "hello")?;

// Add methods and fields to classes
editor.add_method_from_source(class_id, "fn area(self) float {\n    return self.w * self.h\n}\n")?;
editor.add_field(class_id, "z", "float")?;

// Delete (reports dangling references)
let result = editor.delete(function_id)?;
for d in &result.dangling {
    eprintln!("warning: dangling reference to '{}' at {:?}", d.name, d.span);
}

// Commit: re-serializes source, rebuilds index and xrefs
let module = editor.commit();
```

The key property: edits are UUID-stable. Renaming a function updates the declaration's name and every reference site in a single operation. The UUID never changes.

### The MCP Server

The MCP (Model Context Protocol) server exposes the SDK's capabilities as structured tools that AI agents call directly. This is how Claude, and other LLM-based agents, interact with Pluto codebases.

**Read tools:**

| Tool | Purpose |
|------|---------|
| `load_module` | Load and analyze a `.pluto` source file or binary |
| `list_declarations` | List all declarations, optionally filtered by kind |
| `inspect` | Deep inspection of a declaration: params, types, error sets, methods, fields |
| `xrefs` | Cross-reference queries: callers, constructors, enum usages, raise sites |
| `errors` | Error handling info for a function: fallibility and error set |
| `source` | Get source text, optionally at a byte range |

**Write tools:**

| Tool | Purpose |
|------|---------|
| `add_declaration` | Add a function, class, enum, trait, or error |
| `replace_declaration` | Replace a declaration's body (preserves UUID) |
| `delete_declaration` | Remove a declaration (reports dangling refs) |
| `rename_declaration` | Rename with automatic reference updates |
| `add_method` | Add a method to an existing class |
| `add_field` | Add a field to an existing class |

**Build tools:**

| Tool | Purpose |
|------|---------|
| `check` | Type-check without producing a binary |
| `compile` | Compile to a native binary |
| `run` | Compile and execute |
| `test` | Compile in test mode and run tests |

The agent workflow is: load a module, query its structure, understand types and error sets, make a targeted edit, validate with the type checker, iterate.

## The Vision: Programs, Not Services

The tools above are implemented and working. The larger vision takes them further — toward a world where distributed systems are **programs**, not collections of services.

### The Four Pillars

**1. Whole-program compilation.** The compiler sees your entire distributed system: every stage, every function call, every error that could be raised. It generates per-stage binaries with all RPC code.

**2. Source-level composition.** There are no library binaries, no ABI boundaries. All dependencies are source code. The compiler integrates them into your program and optimizes across module boundaries.

**3. Stages as deployment units.** You declare `stage api`, `stage workers`, `stage analytics` in your program. The compiler knows which calls cross stage boundaries and generates the RPC code.

**4. AI agents as first-class developers.** The compiler's internal representation (AST, types, call graph, error sets) is a queryable artifact. AI agents read and write programs at the semantic level, not as text.

When these four pillars are complete, you'll write a Pluto program that defines your entire backend — all services, all communication, all schemas — and the compiler will generate deployable binaries, migration scripts, API documentation, and monitoring hooks automatically.

**You write logic. The compiler generates infrastructure.**

## The Vision

### .pluto as Canonical Source

Today, `.pt` text files are the source of truth and `.pluto` binary files are derived. The plan is to invert this: `.pluto` binary becomes canonical, `.pt` text files become generated views for human review.

In this model, a git repository contains `.pluto` binaries (committed, source of truth) and `.pt` text files (generated, for review and code search). The `plutoc sync` command already exists to merge human `.pt` edits back into `.pluto` files, preserving UUIDs where declarations match by name.

### Derived Data

The compiler produces valuable analysis that is currently discarded after each build: resolved types for every expression, inferred error sets, the full call graph, DI wiring topology. The derived data layer stores this analysis in the `.pluto` binary so that agents and tools can query it without re-compiling.

The SDK already surfaces derived data (resolved signatures, class info, enum info, error info) when a module is loaded via `from_source_file`. The plan is to extend this to the full set of compiler analysis.

### The Feedback Loop

The core idea is that the compiler and AI agents form a feedback loop:

1. **Agent writes code** -- using the SDK or MCP server, the agent makes structured edits with stable UUIDs.
2. **Compiler analyzes** -- type checking, error inference, call graph construction. The analysis is stored, not discarded.
3. **Agent reads analysis** -- the agent queries resolved types, error sets, cross-references. It learns what the compiler knows about the code.
4. **Agent writes better code** -- informed by the compiler's analysis, the agent makes its next edit with full knowledge of the type system, error propagation, and dependency graph.

This is not AI replacing developers. It is the compiler's understanding of your program being accessible to every tool in the chain -- AI agents, editors, code review systems, refactoring tools -- through a structured interface with stable identities.

The text file is a view. The semantic graph is the source.

## What This Looks Like in Practice

Imagine building a production backend in 2027 with Pluto. Here's what the workflow looks like:

### You Write One Program

```pluto
// main.pluto
stage api[db: Database] {
    fn handle_order(req: OrderRequest) OrderResponse {
        let user = auth.verify_user(req.token)!
        let inventory = warehouse.check_stock(req.items)!
        let order = self.db.create_order(user.id, req.items)!
        payments.charge_async(order.id, order.total)
        return OrderResponse { order_id: order.id }
    }
}

stage auth {
    pub fn verify_user(token: string) User {
        // JWT validation
    }
}

stage warehouse {
    pub fn check_stock(items: Array<Item>) Inventory {
        // inventory lookup
    }
}

stage payments {
    pub fn charge_async(order_id: string, amount: int) {
        // async payment processing
    }
}
```

This is your entire backend. Four stages, defined in one program.

### The Compiler Sees Everything

When you run `plutoc compile main.pluto`, the compiler:

1. **Parses and type-checks** all four stages together
2. **Infers error sets** — sees that `verify_user` can raise `InvalidToken`, `check_stock` can raise `OutOfStock`, `create_order` can raise `DatabaseError`
3. **Analyzes call graph** — sees that `api` calls `auth`, `warehouse`, and `payments` across stage boundaries
4. **Generates RPC code** — serialization for `OrderRequest`, `User`, `Item`, `Inventory`; HTTP client calls; error propagation for both remote errors and network failures
5. **Generates per-stage binaries** — `build/api`, `build/auth`, `build/warehouse`, `build/payments`

### An AI Agent Adds a Feature

You ask Claude: "Add fraud detection before charging payments."

Claude uses the MCP server to:
1. **Load the program** — `load_module("main.pluto")`
2. **Query the structure** — `list_declarations()` shows all four stages and their functions
3. **Inspect the payments stage** — `inspect("payments.charge_async")` shows params, return type, error set
4. **Add a new stage**:

```pluto
stage fraud {
    error FraudDetected { order_id: string, reason: string }

    pub fn check_order(order_id: string, amount: int) {
        if amount > 10000 {
            raise FraudDetected { order_id: order_id, reason: "high value order" }
        }
    }
}
```

5. **Modify the api stage** to call `fraud.check_order`:

```pluto
stage api[db: Database] {
    fn handle_order(req: OrderRequest) OrderResponse {
        let user = auth.verify_user(req.token)!
        let inventory = warehouse.check_stock(req.items)!
        let order = self.db.create_order(user.id, req.items)!
        fraud.check_order(order.id, order.total)!  // NEW
        payments.charge_async(order.id, order.total)
        return OrderResponse { order_id: order.id }
    }
}
```

6. **Type-check** — `check()` verifies the edit is correct
7. **Commit** — the SDK updates the `.pluto` binary with the new stage and the modified function

The compiler re-compiles. It sees that `handle_order` now calls `fraud.check_order`, which can raise `FraudDetected`. It:
- Adds `FraudDetected` to the error set of `handle_order`
- Generates RPC code for the `api → fraud` call
- Generates a new `build/fraud` binary
- Updates `build/api` to include the RPC client for fraud

You never wrote serialization code. You never wrote error handling boilerplate. The compiler generated it.

### You Deploy to Production

You write a deployment spec (YAML, HCL, or future Pluto-native format):

```yaml
# deploy.yaml
stages:
  api:
    replicas: 5
    region: us-east-1
    cpu: 2
    memory: 4GB

  auth:
    replicas: 2
    region: us-east-1
    cpu: 1
    memory: 2GB

  warehouse:
    replicas: 3
    region: us-west-2
    cpu: 2
    memory: 4GB

  payments:
    replicas: 2
    region: us-east-1
    cpu: 1
    memory: 2GB

  fraud:
    replicas: 1
    region: us-east-1
    cpu: 1
    memory: 1GB
```

You run `pluto deploy --spec deploy.yaml` (hypothetical command for a future orchestrator). It:
1. Builds Docker images for each stage binary
2. Pushes them to a container registry
3. Deploys them to Kubernetes (or Nomad, or EC2, or wherever)
4. Configures service discovery so stages can find each other
5. Injects config (database URLs, API keys) as environment variables

The stages start up. They discover each other via DNS or Consul. The `api` stage handles requests and calls `auth`, `warehouse`, `fraud`, and `payments` via HTTP. The RPC code handles serialization, retries, timeouts, error propagation — all generated by the compiler.

### A Schema Change Propagates Automatically

You realize `OrderRequest` needs a new field: `discount_code`.

You edit `main.pluto`:

```pluto
class OrderRequest {
    token: string
    items: Array<Item>
    discount_code: string?  // NEW: optional discount code
}
```

You re-compile. The compiler:
- Updates the serialization code for `OrderRequest` in the `api` stage
- Updates the deserialization code in any stage that receives `OrderRequest` (none in this case, but if `warehouse` needed it, the compiler would update `warehouse` too)
- Type-checks all usages of `OrderRequest` to ensure they handle the new field correctly

No manual Protobuf schema update. No "rebuild all the services that depend on this." The compiler saw the change, propagated it, and verified it.

### The Database Schema is Part of Your Program

*(Aspirational — schema-as-code isn't implemented yet.)*

Eventually, your database schema will be declared in your program:

```pluto
stage db {
    schema Order {
        id: string primary key
        user_id: int foreign key User(id)
        items: json
        total: int
        created_at: timestamp
    }

    pub fn create_order(user_id: int, items: Array<Item>) Order {
        return insert_into!(Order, {
            id: generate_uuid(),
            user_id: user_id,
            items: items,
            total: calculate_total(items),
            created_at: now()
        })
    }
}
```

The compiler generates:
- SQL migrations (CREATE TABLE, ALTER TABLE) based on schema changes
- Type-checked queries (`insert_into!` macro validates that all required fields are present)
- Query result types (the return type of `create_order` matches the `Order` schema)

When you change the schema (add a field, change a type, add an index), the compiler generates the migration. When you deploy, the migration runs before the stage starts. **Schema changes are code changes.** They're type-checked, versioned, and deployed atomically.

### Observability is Generated

*(Aspirational — observability hooks aren't implemented yet.)*

The compiler knows every function call, every error, every stage boundary. It can generate observability hooks automatically:

```pluto
stage api[db: Database] {
    @traced  // compiler generates tracing spans
    @metrics  // compiler generates latency/error metrics
    fn handle_order(req: OrderRequest) OrderResponse {
        let user = auth.verify_user(req.token)!
        let inventory = warehouse.check_stock(req.items)!
        let order = self.db.create_order(user.id, req.items)!
        fraud.check_order(order.id, order.total)!
        payments.charge_async(order.id, order.total)
        return OrderResponse { order_id: order.id }
    }
}
```

The compiler generates code that:
- Creates a trace span for `handle_order`
- Creates child spans for each cross-stage call (`auth.verify_user`, `warehouse.check_stock`, etc.)
- Emits metrics for latency (p50, p95, p99) and error rates
- Logs errors with structured context (order_id, user_id, error type)

You write business logic. The compiler generates the plumbing.

### This is the Vision

A world where:
- **You write one program** with stages, not services in separate repos
- **The compiler sees everything** and generates RPC, serialization, migrations, metrics
- **AI agents write code** using the SDK, not text editors
- **Deployment is configuration**, not code
- **Your distributed system is type-checked, error-checked, and optimized** as a whole

This is not a fantasy. The pieces exist:
- Whole-program compilation ✅
- Error inference ✅
- Dependency injection ✅
- AI SDK and MCP server ✅
- Binary AST format ✅

What's left:
- Stages and cross-stage RPC generation
- Schema-as-code and migration generation
- Observability hooks
- Orchestration integration

The foundation is solid. The vision is clear. Pluto is building toward a world where **your infrastructure is your compiler, not your YAML files.**

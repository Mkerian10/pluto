# Source-Level Libraries — No Boundaries

Pluto has libraries. You can write them, publish them, depend on them, and import them into your programs. But they work fundamentally differently than libraries in other languages.

**Traditional libraries** are compiled separately into binary artifacts (`.so`, `.dll`, `.jar`, `.rlib`). The compiler sees an opaque interface. The library is a black box.

**Pluto libraries** are always source code. When you depend on a package, the compiler pulls in the source and sees everything: every function, every type, every implementation detail (that's marked `pub` or used by your code).

There is no binary artifact. There is no ABI boundary. There is no separate compilation. **The compiler always sees the source.**

This isn't about whether libraries exist. It's about **how they're compiled**. In Rust, you compile a crate to an `.rlib`, and consumers link against it. In Go, you compile a package to a `.a` archive. In Java, you compile to a `.jar`. The consumer never sees your source code — only the binary interface.

In Pluto, when you depend on a library, **you depend on source code**. The Pluto compiler pulls that source into your build and sees everything. It can inline functions across library boundaries. It can eliminate dead code from the library. It can specialize generic types based on how you use them. The library isn't a black box — the compiler sees through it.

## Why Other Languages Have Binary Libraries

The library/executable split exists for three reasons:

**1. Build times.** If you had to recompile all your dependencies every time you changed one line, builds would be unbearably slow. Separate compilation means you compile each library once, emit a binary artifact, and reuse it across builds.

**2. Distribution.** If you want to share code without sharing source, you need a binary format. C libraries ship as `.so` or `.dll` files. Rust crates compile to `.rlib` files. The consumer links against the binary without seeing the original source.

**3. ABI stability.** Once you ship a binary library, you can't change its interface without breaking everyone who depends on it. The ABI (Application Binary Interface) is a contract: "here's how to call my functions in machine code." This enables separate compilation and dynamic linking, but it also locks you into your decisions.

The result: compilers can't see across library boundaries. They can't inline functions from dependencies. They can't eliminate dead code from libraries. They can't specialize generics based on how you use them. The library is opaque.

## Pluto's Model: Everything is Source

In Pluto, there are no binary artifacts. When you depend on a package, you depend on **source code**. The compiler pulls that source into your program and sees everything.

```
# pluto.toml
[dependencies]
http = { git = "https://github.com/pluto-lang/http", tag = "v1.0" }
db = { path = "../shared/database" }
```

When you compile your program, the compiler:
1. Fetches the source for `http` and `db`
2. Parses them
3. Flattens them into your program's namespace
4. Type-checks the whole thing together
5. Performs whole-program optimizations
6. Generates a single native binary

The `http` package isn't a black box. The compiler sees every function in it. If you only call `http.get` and never use `http.post`, the compiler doesn't generate code for `post`. If `http.parse_headers` is only called once and it's small, the compiler inlines it. If you instantiate `http.Client<MyConfig>`, the compiler generates a specialized version of the client for your config type.

**This is only possible because the compiler sees the source.**

## No ABI Means No Breakage

Because there's no binary artifact, there's no ABI to break. If you update a dependency, the compiler re-analyzes the whole program. If the types changed incompatibly, you get a compile error. If the error sets changed, you get a compile error. If a function you called was removed, you get a compile error.

There's no such thing as "runtime dependency resolution failure." There's no `LD_LIBRARY_PATH` to configure. There's no version of a library loaded at runtime that's incompatible with what you compiled against. **If it compiles, the dependencies are correct.**

## How Pluto Libraries Work

A Pluto library (or package) is a collection of source files with `pub` declarations. It's not a compiled artifact. It's not a binary. It's source code that the compiler integrates into your program.

```
// In package "payments"
pub fn charge(amount: int) Receipt {
    return process_transaction(amount)!
}

fn process_transaction(amount: int) Receipt {
    // internal implementation
}
```

When you import this library, the compiler sees both functions — `charge` (public) and `process_transaction` (internal). It knows that `process_transaction` is only called from within the `payments` library. If it's small, it might inline it into `charge`. If it's never called, it won't generate code for it.

The consumer sees:

```
import payments

fn main() {
    let receipt = payments.charge(100)!
}
```

The compiler knows that `charge` can fail (it contains `!`), infers the error set, and enforces handling. It knows the return type is `Receipt`. It knows whether `Receipt` has any fields you're accessing. **It sees everything.**

## Apps, Not Executables

The unit you compile is not an "executable." It's an **app**. The app is a first-class language construct:

```
app PaymentService[db: Database, cache: Cache] {
    fn main(self) {
        // entry point
    }
}
```

The compiler generates a native binary for this app. The binary is **self-contained**: all dependencies are compiled in, all wiring is done, all error handling is checked. There's no external configuration file that changes the dependency graph. There's no classpath. There's no dynamic linking. It's a single binary that does one thing.

## Stages: The Future of Deployment

*(This section is aspirational — stages are designed but not fully implemented.)*

Right now, a Pluto program is one app. In the future, a Pluto program will be a collection of **stages** — independently deployable units that communicate via function calls.

```
stage api {
    pub fn handle_request(req: Request) Response {
        let user = db_stage.fetch_user(req.user_id)!
        return build_response(user)
    }
}

stage db_stage {
    pub fn fetch_user(id: int) User {
        return query("SELECT * FROM users WHERE id = {id}")!
    }
}
```

The compiler sees both stages. It sees that `api` calls `db_stage.fetch_user`. It knows this crosses a stage boundary. **It generates RPC code automatically.**

From your perspective, it's a function call. From the compiler's perspective, it's:
1. Serialize the arguments (`id: int`)
2. Make an HTTP request to the `db_stage` service
3. Deserialize the response (`User`)
4. Propagate any errors (network failure, remote raise, deserialization failure)

You write:

```
let user = db_stage.fetch_user(42)!
```

The compiler generates the serialization, the HTTP client, the error handling, the retry logic (based on your configuration), and the deserialization. **Because it sees the whole program, it knows which calls cross stage boundaries.**

This is the opposite of libraries. Libraries hide details from the compiler. Stages expose details to the compiler so it can generate the right code for cross-service communication.

## No Dynamic Linking, Ever

Pluto will never support dynamic linking. It will never have `.so` files or `.dll` files. It will never load code at runtime that wasn't seen at compile time.

This is a deliberate trade-off:
- **Pro:** The compiler can do aggressive whole-program optimizations. Dead code elimination. Inlining across module boundaries. Monomorphization of generics. Static analysis of error propagation.
- **Con:** You can't ship a "Pluto library" as a binary artifact. You can only share source.

For backend services, this is the right trade-off. Backend services aren't plugins. They're not loaded dynamically. They're deployed as containers or binaries, and they do one thing. Pluto optimizes for that use case.

## The Package Ecosystem

When Pluto has a package registry, it won't be like npm or crates.io where packages are versioned binaries. It will be a registry of **source repositories with tagged versions**.

Publishing a package means:
1. Tag a commit in your git repo
2. Register the repo + tag with the registry
3. Document the `pub` API surface

Depending on a package means:
1. The compiler fetches the source at the specified tag
2. The compiler parses and type-checks it
3. The compiler integrates it into your program

There's no "build the package, upload the artifact, download the artifact, link against it" step. The compiler always works with source.

## Binary AST as an Optimization

*(This section is aspirational — the binary AST format exists but isn't the default yet.)*

Eventually, Pluto will support a **binary AST format** (`.pluto` files) as an optimization for large codebases. This isn't a binary library in the traditional sense — it's a serialized parse tree with type information.

When you compile a program, the compiler can cache the parsed and type-checked AST for each module. On the next build, if a module hasn't changed, the compiler reuses the cached AST instead of re-parsing the source.

But the key difference from traditional libraries: **the compiler still sees the AST**. It can still inline functions, eliminate dead code, specialize generics. The binary AST is an optimization for build times, not an ABI boundary.

The workflow:
1. Write `.pt` source files (Pluto text format)
2. Compiler parses them into `.pluto` binary AST
3. Compiler caches the `.pluto` files
4. On incremental builds, compiler reuses cached `.pluto` files for unchanged modules
5. Final whole-program analysis and codegen happens across all modules, cached or not

You never ship a `.pluto` file to users as a "library." You always share source. The binary AST is a local build cache.

## What This Means for You

If you're coming from Go, Java, or Rust, this is a mental shift:

**You DO write libraries.** But you write them as collections of `.pluto` source files with `pub` declarations. You don't compile them into standalone binary artifacts.

**You don't link binaries.** The compiler generates one self-contained binary for your app. All dependencies are compiled in (from source). There's no runtime linker, no `LD_LIBRARY_PATH`, no classpath.

**You don't publish binary artifacts.** If you want to share a library, you publish source code (via git repos or a future package registry). The consumer's compiler pulls your source and integrates it into their program.

**You don't version ABIs.** There's no ABI because there's no binary artifact. If you change your library's `pub` API, consumers recompile and get immediate feedback if the change broke them. Semantic versioning still matters (breaking changes vs. non-breaking changes), but there's no "runtime version mismatch" or "DLL hell."

The benefit: **the compiler always sees your entire program, from main() to the deepest dependency, and can optimize accordingly.** Libraries aren't black boxes. They're transparent to the compiler.

## The Vision

The endgame for Pluto is a programming model where:

- **You write source.** Functions, classes, errors, contracts.
- **You declare dependencies.** Packages from git repos, local paths, or a future registry.
- **You define stages.** Independently deployable units within your program.
- **The compiler sees everything.** Every function, every call site, every error, every dependency, every stage boundary.
- **The compiler generates deployment artifacts.** One binary per stage, with all RPC, serialization, and error handling generated automatically.

No libraries. No dynamic linking. No reflection. No runtime containers. No frameworks discovering your code at startup.

Just source code, whole-program analysis, and native binaries that do exactly what you wrote.

This is what justifies rejecting the library/executable distinction. The compiler becomes your infrastructure.

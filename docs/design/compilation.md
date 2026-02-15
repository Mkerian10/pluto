# Compilation Model

## Whole-Program Compilation

All source code must be available at compile time. There is no dynamic loading or runtime code generation.

## What This Enables

Having the entire program visible at compile time enables analysis that is impossible with separate compilation:

- **Error inference:** The compiler walks the entire call graph to determine which functions can error. No annotations needed. *(Implemented)*
- **DI verification:** Every `inject` declaration is checked against the full dependency graph. *(Implemented)*
- **Cross-pod type safety:** The compiler verifies that both sides of every RPC and channel agree on types. *(Not yet implemented)*
- **Topology analysis:** The compiler builds the full communication graph — which processes talk to which, how data flows. It can detect deadlocks, unused channels, or impossible message patterns. *(Not yet implemented)*
- **Placement optimization:** The compiler can inform co-location decisions based on communication frequency between processes. *(Not yet implemented)*
- **Serialization elimination:** Same-pod communication skips serialization entirely because the compiler knows both ends are local. *(Not yet implemented)*
- **Protocol verification:** Distributed communication patterns can be statically verified for correctness. *(Not yet implemented)*

## Incremental Compilation

> Not yet implemented. The current compiler does full compilation every time.

The design calls for a two-phase approach:

1. **Edit phase:** Modules are compiled independently as they change. Type checking and basic analysis happen here. This is fast — subsecond feedback.
2. **Link phase:** The final compilation step performs whole-program analysis — cross-boundary type checking, error inference, topology optimization, serialization strategy, native code generation. This is slower but only runs for builds/deploys.

This is analogous to Link-Time Optimization (LTO) in C/C++/Rust but with much deeper semantic analysis.

## Constraints

Whole-program compilation imposes one key constraint: **no dynamically loaded plugins or hot-swapping arbitrary code at runtime.** For a distributed systems language where correctness matters more than runtime flexibility, this is a feature, not a bug.

## Libraries

Libraries work within this model — they are source code (or pre-compiled IR) that gets included in the whole-program compilation step. The compiler can analyze library code the same way it analyzes application code.

> **Status:** Open design question — exact library distribution format. The [AI-native representation](ai-native-representation.md) RFC proposes `.pluto` binary files as the distribution format, with full or signature-only variants.

## Future: AI-Native Representation

> See [AI-Native Representation RFC](ai-native-representation.md) for the full design.

The compilation model is designed to evolve toward an AI-native representation where:

- **`.pluto` files become binary** — a canonical semantic graph with stable UUIDs per declaration, authored content (AST), and compiler-derived analysis (types, errors, call graphs)
- **`pluto analyze`** enriches `.pluto` files with derived data on demand (separate from compilation, which is non-mutating)
- **`pluto-sdk`** (Rust crate) provides programmatic read/write access for AI agents
- **Incremental compilation** becomes more precise — stable UUIDs enable exact change tracking at the declaration level rather than file-level heuristics

This preserves the whole-program compilation model while making the compiler's analysis persistent and accessible to AI tooling.

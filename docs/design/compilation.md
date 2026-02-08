# Compilation Model

## Whole-Program Compilation

All source code must be available at compile time. There is no dynamic loading or runtime code generation.

## What This Enables

Having the entire program visible at compile time enables analysis that is impossible with separate compilation:

- **Cross-pod type safety:** The compiler verifies that both sides of every RPC and channel agree on types.
- **Error inference:** The compiler walks the entire call graph to determine which functions can error. No annotations needed.
- **Topology analysis:** The compiler builds the full communication graph — which processes talk to which, how data flows. It can detect deadlocks, unused channels, or impossible message patterns.
- **Placement optimization:** The compiler can inform co-location decisions based on communication frequency between processes.
- **Serialization elimination:** Same-pod communication skips serialization entirely because the compiler knows both ends are local.
- **Protocol verification:** Distributed communication patterns can be statically verified for correctness.
- **DI verification:** Every `inject` declaration is checked against the full dependency graph.

## Incremental Compilation

While the final compilation requires the whole program, development uses incremental compilation for fast iteration:

1. **Edit phase:** Modules are compiled independently as they change. Type checking and basic analysis happen here. This is fast — subsecond feedback.
2. **Link phase:** The final compilation step performs whole-program analysis — cross-boundary type checking, error inference, topology optimization, serialization strategy, native code generation. This is slower but only runs for builds/deploys.

This is analogous to Link-Time Optimization (LTO) in C/C++/Rust but with much deeper semantic analysis.

## Constraints

Whole-program compilation imposes one key constraint: **no dynamically loaded plugins or hot-swapping arbitrary code at runtime.** For a distributed systems language where correctness matters more than runtime flexibility, this is a feature, not a bug.

## Libraries

Libraries work within this model — they are source code (or pre-compiled IR) that gets included in the whole-program compilation step. The compiler can analyze library code the same way it analyzes application code.

> **Status:** Open design question — exact library distribution format.

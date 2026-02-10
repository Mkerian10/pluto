# Pluto Benchmark Suite

## About

This benchmark suite measures Pluto's runtime performance against C, Go, and Python
on identical workloads running on the same hardware. All benchmarks run in the
`compare.sh` script and in the GitHub Actions `benchmarks.yml` workflow.

Pluto has **no optimization passes** yet (Cranelift emits unoptimized native code), so
these numbers represent a baseline. As we add optimizations, we expect to see
improvement over time — that's the whole point of tracking this.

### Methodology

Each benchmark is implemented independently in Pluto, C, Go, and Python with the
same algorithm and parameters. Timing is measured inside each program (wall-clock,
monotonic). We report single-run milliseconds — not averages, not warmed up, not
cherry-picked.

**Important caveats:**
- These are **our own ports**, not canonical implementations. The algorithms match
  the published specifications, but our implementations may differ in ways that
  affect performance (e.g., using 1D arrays to simulate 2D, different data layout
  choices). Results are a **sanity check**, not a rigorous benchmark publication.
- C is compiled with `-O2`. Go and Python use default settings.
- Pluto's codegen target is detected at compile time (aarch64-darwin, x86_64-linux, etc.)
  but there are no target-specific optimizations.

---

## Benchmark Sources

We draw from three well-known, published benchmark suites plus a handful of custom
micro-benchmarks for Pluto-specific features.

### Computer Language Benchmarks Game (CLBG)

Website: https://benchmarksgame-team.pages.debian.net/benchmarksgame/

The most widely used cross-language performance comparison. Maintained since 2004.
Contains 10 benchmark programs with canonical implementations in 20+ languages.
Results are publicly visible and frequently cited in language comparisons.

**Benchmarks from CLBG in our suite:**

| Benchmark | Status | What it tests |
|-----------|--------|---------------|
| **binary-trees** | ✅ | Allocate/traverse/discard binary trees of depth 4–16. Classic GC stress test (based on Boehm's GCBench). |
| **fannkuch-redux** | ✅ | Pancake-flip counting over all permutations of N=10 elements. Tight array reversal loops. |
| **n-body** | ✅ | Jovian planet gravitational simulation, 50M steps. Float-heavy with sqrt. |
| **spectral-norm** | ✅ | Power method on an infinite matrix (N=500, 10 iterations). Nested-loop float multiply-accumulate. |
| **mandelbrot** | ✅ | Mandelbrot set fractal computation (2000×2000, max 50 iterations). Float iteration with early exit. |
| **k-nucleotide** | 🔴 | DNA k-mer frequency counting. Needs file I/O, string slicing, sort. |
| **fasta** | 🔴 | DNA sequence generation with LCG PRNG. Needs formatted stdout output. |
| **reverse-complement** | 🔴 | Reverse-complement a FASTA sequence. Needs stdin/stdout. |
| **pidigits** | 🔴 | Digits of pi via spigot algorithm. Needs arbitrary-precision integers. |
| **regex-redux** | 🔴 | Regex matching and substitution on DNA data. Needs regex library. |

### Are We Fast Yet (AWFY)

Paper: "Cross-Language Compiler Benchmarking: Are We Fast Yet?" (Marr et al., 2016)
Repository: https://github.com/smarr/are-we-fast-yet

Academic benchmark suite designed specifically for comparing language implementations.
14 benchmarks limited to features present in most languages (objects, closures, arrays).
No stdlib dependencies, no threads, no file I/O. Reference implementations exist in
Java, JavaScript, Smalltalk, Ruby, and others.

**Benchmarks from AWFY in our suite:**

| Benchmark | Status | What it tests |
|-----------|--------|---------------|
| **bounce** | ✅ | Ball bouncing simulation, 10M steps. Float arithmetic + conditionals. |
| **sieve** | ✅ | Sieve of Eratosthenes, primes below 500K. Boolean array operations. |
| **permute** | ✅ | Heap's algorithm, all 10! permutations. Recursion + array swaps. |
| **queens** | ✅ | 12-Queens backtracking solver. Recursion + constraint arrays. |
| **towers** | ✅ | Towers of Hanoi, 20 discs × 100 iters. Recursion + array mutation. |
| **storage** | ✅ | Tree of arrays (depth 8, 4 children), count leaves. GC stress with nested allocations. |
| **list** | ✅ | Array-backed linked-list create/traverse/reverse (5000 nodes × 500 iters). Pointer-chasing simulation. |
| **n-body** | ✅ | N-body simulation with Body class + methods. (Same as CLBG version — uses OOP patterns.) |
| **mandelbrot** | ✅ | Mandelbrot computation. (Same as CLBG version — compute only, no I/O.) |
| **richards** | 🟡 | OS task scheduler simulation (12 classes). Tests polymorphic dispatch + state machines. |
| **CD** | 🟡 | Collision detection via kd-tree (16 classes). Complex spatial OOP. |
| **json** | ✅ | Recursive-descent JSON parser. Character-by-character string processing. |
| **deltablue** | 🔴 | Incremental constraint solver (20 classes, 99 methods). Needs inheritance-like dispatch. |
| **havlak** | 🔴 | Loop detection on control-flow graphs. Needs complex OOP + collections. |

### SciMark 2.0

Website: https://math.nist.gov/scimark2/
Source: NIST (National Institute of Standards and Technology)

Standard benchmark for scientific/numerical computing. 5 computational kernels.
Reference implementations in C and Java. All are purely numerical — arrays and float
math only, no objects, no strings, no I/O.

**Benchmarks from SciMark in our suite:**

| Benchmark | Status | What it tests |
|-----------|--------|---------------|
| **FFT** | ✅ | Fast Fourier Transform on 2^16 complex numbers (100 iterations). Bit-reversal + butterfly operations. Uses sin/cos. |
| **SOR** | ✅ | Jacobi successive over-relaxation on 500×500 grid (100 iterations). Stencil access pattern (1D array simulating 2D). |
| **monte-carlo** | ✅ | Estimate pi via random sampling, 100M points. LCG PRNG + float comparison. |
| **sparse-matrix-multiply** | ✅ | Sparse matrix (CSR format, 5000×5000, 50K nonzeros) × dense vector, 1000 iterations. Indirect array indexing. |
| **LU-decomposition** | ✅ | LU factorization with partial pivoting, 500×500 matrix, 10 iterations. Row swapping + float arithmetic. |

### Custom Micro-Benchmarks

These are **not from any published suite**. They test Pluto-specific features
(closures, traits, DI, GC introspection) that don't have equivalents in the suites
above. Useful for tracking Pluto's own progress but not meaningful for cross-language
comparison.

| Benchmark | Status | What it tests |
|-----------|--------|---------------|
| **fib** | ✅ | Naive recursive fib(35). Pure function call overhead. |
| **loop-sum** | ✅ | Sum 0..100M. Raw loop + integer add overhead. |
| **string-concat** | ✅ | 100K string concatenations via interpolation. |
| **array-push** | ✅ | Push 1M elements to dynamic array. |
| **array-iter** | ✅ | Iterate and sum 1M-element array. |
| **class-method** | ✅ | 10M direct method calls. |
| **closure-call** | ✅ | 10M closure invocations with capture. |
| **trait-dispatch** | ✅ | 10M calls through trait vtable. |
| **gc-churn** | ✅ | Allocate 1M short-lived objects. GC throughput. |
| **gc-string-pressure** | ✅ | 100K intermediate strings. String GC. |

---

## Status Summary

| Status | Count | Description |
|--------|-------|-------------|
| ✅ Implemented | 27 | In the suite today |
| 🟢 Ready | 0 | Can implement with current Pluto features |
| 🟡 Stretch | 2 | Needs workarounds (complex trait mapping) |
| 🔴 Blocked | 7 | Needs language features not yet available |
| **Total** | **36** | |

All benchmarks implementable with current language features are now complete.

**From published suites:** 27 (CLBG: 10, AWFY: 14, SciMark: 5) — note: n-body and mandelbrot appear in both CLBG and AWFY but are counted once; json uses stdlib
**Custom/Pluto-specific:** 10

---

## Cross-Language Comparison (compare.sh)

The `compare.sh` script and GitHub Actions workflow run 11 algorithm benchmarks
(the ones with reference implementations in C, Go, and Python) head-to-head:

    fib, loop_sum, sieve, bounce, towers, permute, queens,
    fannkuch_redux, spectral_norm, nbody, mandelbrot

Reference implementations live in `reference/{c,go,python}/`. Each does the same
work with the same parameters and prints `elapsed: {ms} ms`.

These are all **our own ports** of well-known algorithms. The C/Go/Python versions
use idiomatic code for each language. Results won't match numbers published elsewhere
(different hardware, different workload sizes, different implementation choices) but
they're valid **relative comparisons on the same machine in the same run**.

---

## Implementation Priorities

**Needs language work first:**
richards (complex trait dispatch), CD (kd-tree OOP),
deltablue (inheritance), havlak (complex OOP + collections),
k-nucleotide / fasta / reverse-complement / pidigits / regex-redux (I/O, stdlib)

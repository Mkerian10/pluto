# Pluto Benchmark Suite — Target List

50 benchmarks from reputable suites for cross-language comparison.

**Status key:**
- ✅ Implemented — already in the benchmark suite
- 🟢 Ready — can implement with current Pluto features
- 🟡 Stretch — needs minor workarounds or features almost available
- 🔴 Blocked — needs features Pluto doesn't have yet

**Source key:** CLBG = Computer Language Benchmarks Game, AWFY = Are We Fast Yet, SM = SciMark 2.0, Classic = well-known PL benchmarks

---

## Recursion / Call Overhead

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 1 | **fib** | Classic | ✅ | Naive recursive fib(35). Pure function call overhead. |
| 2 | **ackermann** | Classic | 🟢 | A(3,12) — ~100K deep recursive calls. Tests stack/call overhead at extreme depth. |
| 3 | **tak** | Classic | 🟢 | Takeuchi function tak(18,12,6) — triple recursion, ~63K calls. Classic Lisp benchmark. |
| 4 | **towers** | AWFY | ✅ | Towers of Hanoi, 20 discs × 100 iters. Recursion + array mutation. |

## Array / Loop / Integer

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 5 | **loop-sum** | Classic | ✅ | Sum 0..100M in a while loop. Raw loop + integer add overhead. |
| 6 | **sieve** | AWFY | ✅ | Sieve of Eratosthenes, primes below 500K. Bool array marking. |
| 7 | **permute** | AWFY | ✅ | Heap's algorithm, all permutations of 10 elements. Recursion + array swaps. |
| 8 | **queens** | AWFY | ✅ | 12-Queens backtracking solver. Recursion + constraint arrays. |
| 9 | **fannkuch-redux** | CLBG | 🟢 | Pancake flipping over all permutations of 10 elements. Array reversal in tight loop. |
| 10 | **quicksort** | Classic | 🟢 | Sort 1M random integers (LCG-generated). Partition + recursive sort. |
| 11 | **mergesort** | Classic | 🟢 | Sort 1M random integers. Divide-and-conquer with auxiliary array. |
| 12 | **insertion-sort** | Classic | 🟢 | Sort 50K random integers. O(n²) with shifts. Tests raw array move performance. |
| 13 | **binary-search** | Classic | 🟢 | 10M lookups in a sorted 1M-element array. Tests array indexing + branching. |
| 14 | **knapsack** | Classic | 🟢 | 0/1 knapsack via dynamic programming. 2D table (flattened to 1D array). |
| 15 | **levenshtein** | Classic | 🟢 | Edit distance between two long strings. 2D DP table, tests array access patterns. |

## Floating-Point / Numerical

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 16 | **bounce** | AWFY | ✅ | Ball bouncing simulation, 10M steps. Float add + conditional branches. |
| 17 | **n-body** | CLBG | 🟢 | Jovian planet gravitational sim, 50M steps. Float-heavy with sqrt. 5 bodies as class instances. |
| 18 | **spectral-norm** | CLBG | 🟢 | Compute spectral norm via power method on 5500×5500 matrix. Float multiply-accumulate + sqrt. |
| 19 | **mandelbrot** | AWFY | 🟢 | Mandelbrot set computation, 750×750 grid. Float iteration with early exit. (No bitmap output — just count iterations.) |
| 20 | **matrix-multiply** | Classic | 🟢 | Multiply two 500×500 float matrices (naive O(n³)). 1D array simulating 2D. Tests cache + float throughput. |
| 21 | **pi-summation** | Classic | 🟢 | Leibniz series for pi, 100M terms. Single-loop float accumulator. |
| 22 | **monte-carlo-pi** | SM | 🟢 | Estimate pi via random sampling, 100M points. LCG PRNG + float comparison. |
| 23 | **SOR** | SM | 🟢 | Jacobi successive over-relaxation on 500×500 grid, 100 iterations. Stencil access pattern. 1D array simulating 2D. |
| 24 | **sparse-matrix-multiply** | SM | 🟢 | Sparse matrix (CSR format) × dense vector. Indirect array indexing, tests irregular memory access. |
| 25 | **LU-decomposition** | SM | 🟢 | LU factorization with partial pivoting, 500×500 matrix. Float-heavy, row swapping. |
| 26 | **FFT** | SM | 🟢 | Fast Fourier Transform on 2^16 complex numbers. Bit-reversal + butterfly ops. Uses sin/cos builtins. |
| 27 | **euler** | Classic | 🟢 | Solve ODE via Euler method, 10M steps. Simple float loop with repeated multiply-add. |

## GC / Memory Allocation

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 28 | **gc-churn** | Custom | ✅ | Allocate 1M short-lived class instances. Pure allocation throughput + GC reclaim. |
| 29 | **gc-binary-trees** | CLBG | ✅ | Build/check/discard binary trees depth 4–16. Classic GC stress benchmark (Boehm's GCBench). |
| 30 | **gc-string-pressure** | Custom | ✅ | 100K intermediate strings via interpolation. Tests string GC. |
| 31 | **storage** | AWFY | 🟢 | Build tree of arrays (depth 6), count leaf elements. Stresses GC with nested array structures. |
| 32 | **gc-linked-list** | Classic | 🟢 | Build and traverse 1M-node linked list (class with next field). Tests GC with long-lived pointer chains. |

## OOP / Polymorphism / Dispatch

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 33 | **class-method** | Custom | ✅ | 10M method calls on a single class instance. Direct dispatch overhead. |
| 34 | **trait-dispatch** | Custom | ✅ | 10M method calls through trait interface. Vtable indirect dispatch overhead. |
| 35 | **closure-call** | Custom | ✅ | 10M closure invocations with captured variable. Indirect call + capture access overhead. |
| 36 | **n-body-oop** | AWFY | 🟢 | Same physics as n-body but with Body class + methods. Tests OOP field access vs raw variables. |
| 37 | **richards** | AWFY | 🟡 | OS task scheduler simulation (12 classes). Classic OOP benchmark — tests polymorphic dispatch, queue management, state machines. Needs trait-based polymorphism mapping. |
| 38 | **list** | AWFY | 🟢 | Linked-list operations (create, traverse, compare) using class nodes. Tests pointer-chasing + recursion. |
| 39 | **CD** | AWFY | 🟡 | Collision detection with kd-tree (16 classes). Complex OOP + spatial algorithms. Needs careful trait hierarchy mapping. |
| 40 | **deltablue** | AWFY | 🔴 | Incremental constraint solver (20 classes, 99 methods). Requires inheritance-like patterns not available in Pluto. |

## String / Text Processing

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 41 | **string-concat** | Custom | ✅ | Concatenate 100K strings via interpolation. Tests string allocation + copy. |
| 42 | **json-parse** | AWFY | 🟡 | Hand-written recursive-descent JSON parser. Tests character-by-character string processing. Needs string indexing (s[i] or char-at). |
| 43 | **brainfuck-interp** | Classic | 🟡 | Interpret a Brainfuck program (e.g., mandelbrot.bf). Tests interpreter dispatch loop + array ops. Needs string indexing for instruction fetch. |

## Hash Table / Map / Set

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 44 | **map-insert-lookup** | Classic | 🟢 | Insert 1M key-value pairs into Map, then look up each. Tests Pluto's built-in Map performance. |
| 45 | **set-operations** | Classic | 🟢 | Insert/contains/remove on Set with 1M elements. Tests Pluto's built-in Set performance. |
| 46 | **k-nucleotide** | CLBG | 🔴 | Count DNA k-mer frequencies in a large sequence. Needs file I/O (stdin), string slicing, sort. |
| 47 | **havlak** | AWFY | 🔴 | Loop detection on control-flow graph (18 classes, uses maps/sets). Needs complex OOP + potentially inheritance. |

## I/O / System (Future)

| # | Benchmark | Source | Status | What it tests |
|---|-----------|--------|--------|---------------|
| 48 | **fasta** | CLBG | 🔴 | Generate DNA sequences with PRNG. Needs stdout line output. |
| 49 | **reverse-complement** | CLBG | 🔴 | Reverse-complement a FASTA sequence. Needs stdin/stdout, string/byte manipulation. |
| 50 | **pidigits** | CLBG | 🔴 | Compute N digits of pi. Needs arbitrary-precision (big integer) arithmetic. |

---

## Summary

| Status | Count | Description |
|--------|-------|-------------|
| ✅ Implemented | 14 | Already in the suite |
| 🟢 Ready | 25 | Can implement now with current features |
| 🟡 Stretch | 5 | Needs minor workarounds or near-future features |
| 🔴 Blocked | 6 | Needs features not yet in Pluto |
| **Total** | **50** | |

## Priority Order for Implementation

**Phase 1 — Quick wins (pure compute, no new features needed):**
ackermann, tak, pi-summation, monte-carlo-pi, euler, binary-search, insertion-sort, fannkuch-redux

**Phase 2 — Array-heavy (1D array tricks for 2D problems):**
matrix-multiply, SOR, sparse-matrix-multiply, LU-decomposition, FFT, quicksort, mergesort, knapsack, levenshtein

**Phase 3 — Object/GC-oriented:**
n-body, n-body-oop, spectral-norm, mandelbrot, storage, gc-linked-list, list, map-insert-lookup, set-operations

**Phase 4 — Complex OOP (may need trait enhancements):**
richards, CD, json-parse, brainfuck-interp

**Phase 5 — After new language features:**
deltablue, k-nucleotide, fasta, reverse-complement, havlak, pidigits

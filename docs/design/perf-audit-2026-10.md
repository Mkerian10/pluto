# Compiler Performance Audit — October 2026

Post-verification-engine baseline. Measures the aggregate compile-time cost of
the analyses added to the typecheck phase since PR #333 (commit `885b934`,
"Handle-call routing"): flow-fact engine (`facts.rs`), strict invariant
discharge (`discharge.rs`), dominance (`dominance.rs`), raise summaries +
call-site shrinking (`shrink.rs`), two-state ensures, linearity extensions, and
DI zero-state proofs. Report-only: no optimization work was done.

**Verdict up front:** no alarms. Plain code (no contracts, no errors) compiles
at the same speed as before the verification engine landed (+0.5% on a
10k-line stress corpus; within noise on real examples). The full contract
surface — invariants, fallible calls, two-state ensures at saturation density —
costs +11.6% wall-clock on that corpus, proportional to what is declared: you
only pay for what you declare. Error-heavy and invariant-carrying code is
actually 2–3× *faster* to compile than at the baseline. One oddity is flagged
(a fixed ~1.5–2 ms cost on programs containing closures) along with two
bit-rotted benchmarks found and repaired along the way.

## Method

All measurements on one machine (Apple Silicon macOS, Darwin 24.6.0), release
builds (`cargo build --release`) of two commits:

- **baseline** — `885b934` (merge of PR #333), in a detached throwaway worktree
- **master** — `6228ed5` (merge of PR #356, two-state proofs), 81 commits later

Three instruments:

1. **Criterion micro-benchmarks** (`benches/compile_time.rs`, identical at both
   commits — no benchmark-set drift). Run per-worktree with
   `cargo bench --bench compile_time`; medians compared manually from each
   worktree's `target/criterion/*/new/estimates.json` rather than trusting
   criterion's cross-run change detection.
2. **Wall-clock macro measurements** — `pluto compile <file> -o <out> --stdlib
   stdlib`, best-of-3 after a warmup pass (warmup also populates the runtime cc
   cache in `~/.pluto/cache`). Wall-clock includes linking (cc + ld), a roughly
   constant additive cost bounded below by the hello-world floor.
   Corpora:
   - the 44 example programs whose directories are byte-identical at both
     commits (of 54 shared; `git-packages`/`testing` excluded per CI's
     `scripts/check_examples.sh`) — the apples-to-apples set;
   - each commit's own full example set, reported separately (counts differ:
     master adds `blob`, `generic-objects`, `lease`, and 8 shared examples'
     sources changed between the commits);
   - a synthetic contract-heavy corpus (below).
   Stdlib drift between the commits is minor (5 files, +50/-7 lines) and is
   included in what each compiler compiles, as in real use.
3. **Synthetic corpus with feature stripping** — the compiler has no
   phase-timing flag, so per-pass attribution is done coarsely: the committed
   generator `scripts/perf_audit_gen.py` emits a large single-file program in
   four modes whose function/method bodies are kept textually identical
   wherever possible, so mode-vs-mode deltas isolate the marginal cost of the
   declared features. The measured corpus used `--classes 120 --funcs 240`
   (~10k lines; defaults give ~3.4k):
   - `plain` — guards + arithmetic; no errors/invariants/ensures (compiles at
     both commits)
   - `errors` — + 8 error decls; half of all function pairs become fallible:
     raise-under-guard, `!` propagation in a wrapper, `catch` in main (~240
     fallible functions, ~240 handled call sites at the measured size; both
     commits)
   - `invariants` — + 2 bounds invariants per class (240 total), all writes
     guarded so master's strict discharge can prove them: ~240 proven
     constructions + ~360 mutating methods (both commits; runtime-checked at
     baseline, statically discharged on master)
   - `full` — + proof-form `ensures ... old(...)` on one method per class
     (120 methods; master only — baseline rejects `ensures` at parse)
   This corpus is deliberately shaped to exercise the new passes (every class
   carries invariants, half the functions are fallible, every write is
   guarded); it is a stress shape, not representative user code.

`cargo test --test stdlib_tests` was not used: its compile portion is not
separable from process spawn + execution in the harness, and the examples set
already covers stdlib-heavy compiles.

## Criterion results

`cargo bench --bench compile_time`, medians from `estimates.json` (the full
`cargo bench` does not build — see infrastructure findings). These measure
`pluto::compile_to_object` on tiny single-source programs with **no stdlib and
no linking**; absolute times are dominated by fixed per-compile work (prelude
injection + typecheck of the injected prelude).

| benchmark             | 885b934 median | master median | change |
|-----------------------|---------------:|--------------:|-------:|
| compile_hello_world   | 1173 µs        | 936 µs        | −20%   |
| compile_generics      | 1590 µs        | 1184 µs       | −26%   |
| compile_closures      | 1346 µs        | 2890 µs       | **+115%** |
| compile_errors        | 27 µs          | 36 µs         | +35% (parse-error path, see below) |
| compile_large_program | 1653 µs        | 1841 µs       | +11%   |

Mixed: the fixed per-compile cost went *down* (hello world, generics), the
small multi-class program is +11%, and the closures microbenchmark more than
doubled. The closures number is investigated under Hotspots — it does **not**
reproduce through the CLI compile path and does not scale with closure count.

## Wall-clock results

Full `pluto compile` (frontend + codegen + cc/ld link). The link floor — a
3-line hello world — is ~40 ms at both commits (master 42 ms, baseline 39 ms),
so small-program times are link-dominated and percentage deltas on them
understate/overstate frontend changes; the synthetic corpus below gives the
scaled view.

**Identical-examples corpus** (44 example programs byte-identical at both
commits, best-of-3 per file, totals of per-file bests; run twice to gauge
run-to-run noise):

| corpus                      | 885b934 | master  | change |
|-----------------------------|--------:|--------:|-------:|
| 44 identical examples, run 1 | 1.938 s | 2.072 s | +6.9%  |
| 44 identical examples, run 2 | 1.909 s | 1.858 s | −2.7%  |

The two runs straddle zero: on real example programs the commit-to-commit
difference is **within measurement noise** (each file is 38–50 ms, i.e. mostly
link floor; per-file deltas were ±5–16%, a few ms). Run 1 contained one
spectacular outlier — `examples/uuid` at 94 ms vs 40 ms — which did not
reproduce (re-probed at 37.1 ms master / 36.7 ms baseline over 10 reps): a
scheduling flake, not a regression.

**Own example sets** (CI `check_examples.sh` set, each commit compiling its own
examples with its own stdlib; not directly comparable — master has 3 more
examples and 8 shared examples' sources changed; 4 non-compiling/network
entries skipped identically at both commits):

| corpus                | total   | files |
|-----------------------|--------:|------:|
| 885b934 own examples  | 2.103 s | 48    |
| master own examples   | 2.085 s | 51    |

Master compiles **more** examples in slightly less total time.

**Synthetic contract-heavy corpus** (~10k lines, `--classes 120 --funcs 240`;
median of 10 runs after warmup):

| mode       | 885b934  | master   | master vs base |
|------------|---------:|---------:|---------------:|
| plain      | 98.4 ms  | 98.9 ms  | +0.5%          |
| errors     | 242.0 ms | 103.7 ms | **−57%**       |
| invariants | 314.6 ms | 108.2 ms | **−66%**       |
| full       | n/a (rejects `ensures`) | 110.4 ms | — |

Two things fall out:

1. **Plain code at scale: no regression.** 10k lines of guard-heavy,
   contract-free code compiles in the same time at both commits (98–99 ms),
   even though the flow-fact engine now runs over every body.
2. **Master is 2–3× faster than the baseline on error-heavy and
   invariant-carrying code.** The old compiler paid heavily for error-set
   inference (+144 ms over plain for ~240 fallible functions / ~240 handled
   call sites) and for runtime invariant-check codegen (+73 ms more). The new
   raise-summary/shrinking machinery and the removal of runtime invariant
   checks (strict static discharge emits no code) more than paid for the added
   analyses. The baseline's error-heavy times were also far noisier
   (min 188 ms / median 242 ms vs master's 100/104).

## Marginal cost of declared contracts

The number that matters: what do you pay on the *current* compiler for
declaring contracts, versus not? Measured on the 10k-line synthetic corpus,
where bodies are textually identical across modes (same guards, same
arithmetic) and only the declared features differ:

| increment                                  | marginal cost | what it buys |
|--------------------------------------------|--------------:|--------------|
| plain → errors (8 error types, ~240 fallible fns, ~240 handled call sites) | +4.8 ms (+4.9%) | error inference, raise summaries, call-site shrinking |
| errors → invariants (240 invariants; ~240 constructions + ~360 mutating methods proven) | +4.5 ms (+4.3%) | strict static discharge, dominance, facts-backed proofs |
| invariants → full (120 `ensures`/`old()` methods) | +2.2 ms (+2.0%) | two-state exit proofs + caller-side assumption |
| **plain → full (everything)**              | **+11.5 ms (+11.6%)** | the whole verification surface |

This is the working-as-intended shape: cost is proportional to declared
contracts, at roughly 10–50 µs per proof obligation on this corpus, and code
that declares nothing pays nothing measurable. Remember the corpus is
deliberately contract-saturated (every class carries invariants, half of all
functions are fallible); real programs sit below this density.

## Benchmark infrastructure findings

Two pre-existing defects in `benches/`, present at **both** commits (so they do
not affect the comparison, but they mean `cargo bench` does not do what CI or a
developer would expect):

1. **`cargo bench` does not build.** `benches/visitor_overhead.rs` has
   bit-rotted against the current AST: `Expr::Catch { handler }` is now
   `handlers`, and the `Stmt` match is missing `Assert` and `Serve` arms
   (E0026 + E0004). Last touched in PR #201; already broken at `885b934`.
   Only `cargo bench --bench compile_time` runs.
2. **`bench_compile_errors` benchmarks a parse failure.** Its source declares
   `fn might_fail() int!` — but `!` on a *declared* return type is not Pluto
   syntax (fallibility is inferred; `!` belongs to function types and call
   sites). `compile_to_object` returns a syntax error in ~30–50 µs and the
   benchmark happily measures that error path. It has never measured
   error-handling compilation. (Same at baseline.) After the repair it
   measures a real raise/propagate/catch compile at ~2.8 ms on master — treat
   that as a fresh baseline; its history is not comparable.

## Regression verdict by area

| area | verdict |
|------|---------|
| Lex/parse + fixed per-compile cost | **Improved.** Hello-world and generics microbenches are 20–26% faster; the CLI link floor is unchanged (39→42 ms, within noise). |
| Typecheck, plain code (no contracts, no errors) | **No regression.** 10k-line guard-heavy corpus: 98.4 → 98.9 ms (+0.5%), despite the flow-fact engine now running over every body. Real examples: within noise across two runs (+6.9% / −2.7%). |
| Typecheck, error-heavy code | **Much faster.** 10k-line corpus with ~480 fallible call sites: 242 → 104 ms (−57%). The baseline's error-set inference cost far more than the new raise-summary + shrinking machinery does. |
| Typecheck, invariant-carrying code | **Much faster**, with a caveat. 315 → 108 ms (−66%) — but the baseline number includes *runtime* invariant-check codegen, which strict discharge removed. The honest like-for-like statement is the marginal-cost table above: discharge+dominance+facts cost ~4.5 ms per 240 invariants / ~840 proof sites on this corpus. |
| Two-state ensures | **Cheap.** +2.2 ms for 120 `ensures`/`old()` methods (~18 µs per proven method). No baseline comparison possible (new syntax). |
| Codegen/link | Unchanged within noise (link floor identical; small-program criterion +11% on `compile_large_program` is µs-scale, see below). |

Nothing crosses the alarm threshold (>20% on plain code without contracts).
The one number over 20% — `compile_closures` +115% — is a fixed ~1.5–2 ms
cost, not proportional to code size or closure count; flagged below, not
alarming at current absolute scale.

## Hotspots flagged (no fixes attempted)

1. **Fixed "program contains closures" cost of ~1.5–2 ms** (the
   `compile_closures` +115% criterion result). Characterization:
   - Reproducible and stable in the `compile_to_object` path (no stdlib, no
     link): 1.35 ms at 885b934 → 2.89 ms on master for a 1-closure program,
     while a 0-closure hello world got *faster* (1.17 → 0.94 ms).
   - **Not per-closure**: the new `compile_closures_8` bench (8 closures)
     compiles in 2.31 ms — less than the 1-closure program; and CLI compiles
     of 50/200-closure files show ~10 µs/closure at both commits with no
     master-side divergence.
   - Invisible in CLI deltas because every CLI compile includes the stdlib
     prelude (which contains closures), so the fixed cost is paid — and
     cancelled out — in every measurement pair. It is consistent with the
     ~3 ms drift observed on the CLI hello-world floor.
   - Candidate origins (unverified, from the audit window's feature list):
     closure nodes in the error-inference graph, closure-type fallibility
     contract checking, or eta-expansion machinery for function references.
   Verdict: suspicious but small and flat. Worth a look next time someone is
   in `src/closures.rs`/error inference; not worth an optimization pass today.
2. **Criterion `compile_large_program` +11%** (1.65 → 1.84 ms). Five classes +
   functions, no contracts. Plausibly the same fixed effect as (1) plus noise;
   the 10k-line corpus shows scaling is fine, so this is a µs-scale fixed cost,
   not a growth-rate problem.
3. **Baseline-era error inference was the real hotspot** (242 ms → 104 ms on
   the errors corpus). Called out so nobody "optimizes" the new machinery back
   toward the old shape: the shrink/summaries work is a large net win.

## Infrastructure changes made by this audit (benches only, no src/)

- `benches/visitor_overhead.rs` repaired (compiles again; new walker arms for
  `Assert`/`Serve`/`Typed` catch/`NullCoalesce`/if-expr/`At`/match-expr).
- `benches/compile_time.rs`: `bench_compile_errors` source fixed to actually
  compile (its history is not comparable — it used to measure a parse error);
  `compile_closures_8` added as a scaling probe for hotspot (1).
- `scripts/perf_audit_gen.py` committed so the synthetic corpus is
  reproducible: `perf_audit_gen.py --mode {plain|errors|invariants|full}
  [--classes N --funcs N]`.

## Follow-ups suggested (measurement, not optimization)

- A phase-timing flag (`PLUTO_TIMINGS=1` printing per-pass wall times from
  `run_frontend`) would have made attribution direct instead of subtractive;
  this audit had to infer pass costs from corpus deltas.
- CI never runs `cargo bench`, which is how `visitor_overhead` rotted and
  `compile_errors` measured a parse error for its entire life. A
  compile-only `cargo bench --no-run` (or `-- --test`) in CI would catch rot.

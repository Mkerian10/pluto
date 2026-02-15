# RFC: Compiler Metrics & Instrumentation

> **Status:** Planning
> **Priority:** Medium — Enables performance optimization and usage analytics
> **Effort:** 1-2 weeks
> **Date:** February 2026

## Executive Summary

This RFC proposes a comprehensive metrics collection system for the Pluto compiler. The system will track compilation performance, code characteristics, feature usage, and error patterns to enable data-driven optimization and inform language design decisions.

**Objective:** Build an instrumentation framework that provides actionable insights into compiler performance and Pluto language usage without impacting compilation speed or developer experience.

---

## Background

### Motivation

As Pluto moves toward production use, we need visibility into:

1. **Performance bottlenecks** — Which compilation phases are slow? Where should we optimize?
2. **Feature adoption** — Which language features are heavily used vs. rarely touched?
3. **Error patterns** — What errors do developers hit most often? Where is the language confusing?
4. **Code complexity** — How large are typical Pluto programs? How deep are call graphs?
5. **Compilation reliability** — What's the success rate? Where do crashes occur?

### Current State

- No structured metrics collection
- Performance profiling requires external tools (Instruments, perf)
- No visibility into feature usage or developer pain points
- Optimization is guesswork without data

### Design Principles

1. **Zero overhead by default** — Metrics collection must be opt-in or compile-time gated
2. **Privacy-preserving** — Never collect source code, identifiers, or sensitive data
3. **Actionable data** — Focus on metrics that inform concrete decisions
4. **Minimal dependencies** — Avoid heavy metric libraries; keep implementation simple
5. **Composable** — Support multiple backends (JSON files, stderr, future: remote telemetry)

---

## Phase 1: Core Metrics Framework (3-4 days)

### Goals
- Define metrics data model and collection API
- Implement phase timing instrumentation
- Add compilation outcome tracking
- Support JSON export for analysis

### Tasks

#### 1. Define Metrics Schema

Create `src/metrics/mod.rs` with core types:

```rust
pub struct CompilationMetrics {
    // Metadata
    pub timestamp: SystemTime,
    pub compiler_version: String,
    pub target_triple: String,
    pub outcome: CompilationOutcome,

    // Phase timings (microseconds)
    pub timing: PhaseTimings,

    // Code characteristics
    pub code_stats: CodeStats,

    // Resource usage
    pub resources: ResourceUsage,
}

pub struct PhaseTimings {
    pub total_us: u64,
    pub lex_us: u64,
    pub parse_us: u64,
    pub module_resolve_us: u64,
    pub module_flatten_us: u64,
    pub closure_lift_us: u64,
    pub typecheck_us: u64,
    pub monomorphize_us: u64,
    pub codegen_us: u64,
    pub link_us: u64,
}

pub struct CodeStats {
    pub source_bytes: usize,
    pub source_lines: usize,
    pub num_functions: usize,
    pub num_classes: usize,
    pub num_traits: usize,
    pub num_enums: usize,
    pub num_modules: usize,
}

pub struct ResourceUsage {
    pub peak_memory_bytes: usize,
    pub object_size_bytes: usize,
    pub binary_size_bytes: usize,
}

pub enum CompilationOutcome {
    Success,
    LexError,
    ParseError,
    TypeError,
    CodegenError,
    LinkError,
    InternalError(String),
}
```

#### 2. Implement Collection API

Add instrumentation helpers:

```rust
// Start timing a phase
pub fn start_phase(phase: Phase) -> PhaseGuard;

// Record outcome
pub fn record_outcome(outcome: CompilationOutcome);

// Flush metrics to backend
pub fn flush(backend: MetricsBackend);

// Thread-local storage for current metrics
thread_local! {
    static CURRENT_METRICS: RefCell<Option<CompilationMetrics>>;
}
```

#### 3. Instrument Compilation Pipeline

Modify `src/lib.rs::compile_file()`:

```rust
pub fn compile_file(path: &Path) -> Result<Vec<u8>> {
    metrics::init_collection();

    let _lex_timer = metrics::start_phase(Phase::Lex);
    let tokens = lex(source)?;
    drop(_lex_timer);

    let _parse_timer = metrics::start_phase(Phase::Parse);
    let ast = parse(tokens)?;
    drop(_parse_timer);

    // ... continue for all phases

    metrics::record_outcome(CompilationOutcome::Success);
    metrics::flush(MetricsBackend::from_env());

    Ok(object_bytes)
}
```

#### 4. Add CLI Flag

Extend `src/main.rs`:

```rust
#[derive(Parser)]
struct CompileArgs {
    // Existing flags...

    #[arg(long, help = "Enable metrics collection")]
    metrics: bool,

    #[arg(long, help = "Metrics output file (default: stderr)")]
    metrics_output: Option<PathBuf>,
}
```

Environment variable support:
```bash
PLUTO_METRICS=1 cargo run -- compile foo.pluto
PLUTO_METRICS_FILE=metrics.json cargo run -- compile foo.pluto
```

### Deliverables

- `src/metrics/mod.rs` — Core metrics types and API
- Updated `src/lib.rs` — Phase instrumentation
- JSON export support
- CLI flag + environment variable support
- Unit tests for metrics collection
- Documentation: `docs/metrics.md`

---

## Phase 2: Code Characteristic Metrics (2-3 days)

### Goals
- Collect detailed code statistics
- Track feature usage patterns
- Measure code complexity

### Metrics to Add

#### Code Statistics
```rust
pub struct CodeStats {
    // Existing...

    // Declaration counts
    pub num_generic_functions: usize,
    pub num_generic_classes: usize,
    pub num_closures: usize,
    pub num_tests: usize,
    pub num_error_types: usize,

    // App & DI
    pub has_app: bool,
    pub num_injected_classes: usize,
    pub di_graph_size: usize,

    // Expression complexity
    pub max_call_depth: usize,
    pub total_expressions: usize,
    pub total_statements: usize,

    // Type usage
    pub type_usage: HashMap<PlutoType, usize>,
}
```

#### Feature Usage
```rust
pub struct FeatureUsage {
    // Language features
    pub uses_generics: bool,
    pub uses_closures: bool,
    pub uses_async: bool, // spawn/Task
    pub uses_channels: bool,
    pub uses_nullable: bool,
    pub uses_error_handling: bool,
    pub uses_contracts: bool,
    pub uses_di: bool,

    // Stdlib usage
    pub stdlib_modules: HashSet<String>,

    // Advanced features
    pub uses_traits: bool,
    pub uses_enums: bool,
    pub uses_maps: bool,
    pub uses_sets: bool,
}
```

### Implementation

1. **AST Walker for Stats**
   - Add `collect_stats()` pass after parsing
   - Walk AST and count declarations, expressions
   - Track max nesting depth for complexity

2. **Type Usage Tracking**
   - Hook into typechecker's type inference
   - Count occurrences of each PlutoType
   - Identify most common types

3. **Feature Detection**
   - Check for presence of specific AST nodes (Spawn, GenericInvocation, etc.)
   - Track imported stdlib modules
   - Boolean flags for feature presence

### Deliverables

- Extended `CodeStats` and `FeatureUsage` structs
- AST walker in `src/metrics/stats.rs`
- Integration with typecheck phase
- Tests verifying accurate counting

---

## Phase 3: Error & Diagnostic Metrics (2-3 days)

### Goals
- Track compilation errors and warnings
- Identify common error patterns
- Measure error message quality

### Metrics to Add

```rust
pub struct ErrorMetrics {
    // Error counts by phase
    pub lex_errors: usize,
    pub parse_errors: usize,
    pub type_errors: usize,
    pub codegen_errors: usize,

    // Error types (anonymized)
    pub error_kinds: HashMap<String, usize>, // e.g., "UndefinedVariable" -> 3

    // Error locations (line/column distributions, not actual code)
    pub error_phases: Vec<String>, // ["Parse", "Typecheck", "Typecheck"]

    // Warnings
    pub warning_count: usize,
    pub warning_kinds: HashMap<String, usize>,
}
```

### Implementation

1. **Error Interceptor**
   - Wrap error reporting in `src/errors.rs`
   - Record error kind without exposing source code
   - Track error location spans

2. **Error Classification**
   - Map compiler errors to categories:
     - Syntax errors
     - Type mismatches
     - Undefined references
     - Contract violations
     - Import errors

3. **Privacy Guarantees**
   - NEVER log identifiers, literals, or code snippets
   - Only log error type (e.g., "UndefinedVariable")
   - Only log span positions (line/column numbers)

### Deliverables

- `src/metrics/errors.rs` — Error tracking
- Integration with error reporting
- Privacy audit and tests
- Documentation on what IS and IS NOT collected

---

## Phase 4: Performance Profiling Support (2-3 days)

### Goals
- Identify slow compilation units
- Track memory allocation patterns
- Support external profiler integration

### Metrics to Add

```rust
pub struct PerformanceMetrics {
    // Per-function timings (top 10 slowest)
    pub slow_functions: Vec<FunctionTiming>,

    // Memory allocations by phase
    pub allocations_by_phase: HashMap<Phase, AllocationStats>,

    // Monomorphization stats
    pub monomorphization: MonomorphizationStats,

    // Codegen stats
    pub codegen: CodegenStats,
}

pub struct FunctionTiming {
    pub name_hash: u64, // Hash, not actual name
    pub typecheck_us: u64,
    pub codegen_us: u64,
}

pub struct MonomorphizationStats {
    pub num_instantiations: usize,
    pub num_iterations: usize,
    pub total_clones: usize,
    pub largest_instantiation: usize, // Number of instances of one generic
}

pub struct CodegenStats {
    pub num_cranelift_insts: usize,
    pub num_basic_blocks: usize,
    pub optimization_passes: usize,
}
```

### Implementation

1. **Per-Function Tracking**
   - Record time spent typechecking each function
   - Record time spent in codegen per function
   - Report top 10 slowest (by hash, not name)

2. **Memory Profiling**
   - Hook into allocator if possible
   - Track allocations per phase
   - Report peak memory usage

3. **Monomorphization Insights**
   - Count generic instantiations
   - Track iteration count for fixed-point
   - Identify generics with many instances

4. **External Profiler Hooks**
   - Add tracing spans compatible with `tracing` crate (optional dependency)
   - Support flame graph generation
   - JSON format for import into analysis tools

### Deliverables

- Extended performance metrics
- Optional `tracing` integration (feature-gated)
- Analysis tools/scripts in `tools/metrics/`
- Documentation on profiling workflows

---

## Phase 5: Aggregation & Analysis Tools (2-3 days)

### Goals
- Build CLI tools to analyze metrics
- Aggregate metrics across multiple compilations
- Generate reports and visualizations

### Tools to Build

#### 1. `pluto-metrics` CLI Tool

```bash
# Analyze single compilation
pluto-metrics analyze metrics.json

# Aggregate multiple runs
pluto-metrics aggregate metrics/*.json -o summary.json

# Compare two compilations
pluto-metrics diff baseline.json current.json

# Generate report
pluto-metrics report summary.json --format html
```

#### 2. Analysis Capabilities

- **Regression detection** — Compare timings across versions
- **Feature adoption trends** — Track feature usage over time
- **Performance profiling** — Identify slow phases and functions
- **Error analysis** — Find common error patterns

#### 3. Output Formats

- **JSON** — Machine-readable, for CI integration
- **Markdown** — Human-readable reports
- **HTML** — Interactive visualizations (charts for timings, feature usage)
- **CSV** — For import into spreadsheets

### Implementation

```rust
// New binary in src/bin/pluto-metrics.rs
fn main() {
    let args = Args::parse();

    match args.command {
        Command::Analyze { file } => analyze_single(file),
        Command::Aggregate { files, output } => aggregate_many(files, output),
        Command::Diff { baseline, current } => diff_compilations(baseline, current),
        Command::Report { input, format } => generate_report(input, format),
    }
}
```

### Deliverables

- `src/bin/pluto-metrics.rs` — Standalone tool
- Aggregation and diffing logic
- Report generation (markdown + HTML)
- CI integration guide
- Example usage in `docs/metrics.md`

---

## Implementation Strategy

### Dependencies

```toml
[dependencies]
# Existing deps...

# Metrics (all optional, feature-gated)
serde = { version = "1.0", features = ["derive"], optional = true }
serde_json = { version = "1.0", optional = true }

[dev-dependencies]
# For testing metrics collection
similar-asserts = "1.5"

[features]
default = []
metrics = ["serde", "serde_json"]
```

### Feature Gates

- Metrics collection only compiled when `--features metrics`
- Zero runtime cost when disabled
- Environment variable still works if feature enabled at build time

### Testing Strategy

1. **Unit tests** — Test individual metric collectors
2. **Integration tests** — Compile known programs, verify metrics
3. **Regression tests** — Ensure metrics don't break compilation
4. **Performance tests** — Verify <1% overhead when enabled

---

## Open Questions

### 1. Privacy & Telemetry

**Question:** Should we support remote telemetry (sending metrics to a server)?

**Options:**
- **Option A:** Local-only metrics, no network transmission (current proposal)
- **Option B:** Opt-in telemetry with explicit user consent
- **Option C:** Anonymous aggregated metrics for language improvement

**Decision:** Start with local-only (Option A), revisit telemetry after privacy policy established.

### 2. Metrics Granularity

**Question:** How fine-grained should per-function metrics be?

**Trade-offs:**
- More granular = better insights, but larger metric payloads
- Less granular = smaller overhead, but less actionable

**Proposal:** Default to phase-level only. Add `--metrics=detailed` flag for per-function profiling.

### 3. Historical Storage

**Question:** Should the compiler maintain a historical database of metrics?

**Options:**
- **Option A:** Each compilation writes to new file (user manages history)
- **Option B:** Append to SQLite database for automatic history tracking
- **Option C:** External service handles aggregation

**Proposal:** Start with Option A (stateless), add Option B in future if needed.

### 4. CI Integration

**Question:** How should CI systems consume metrics?

**Proposal:**
- Export JSON to `$OUT_DIR/metrics.json`
- CI can parse JSON and fail on regressions
- Example GitHub Actions workflow provided

---

## Success Metrics

After implementation, we should be able to answer:

1. **Performance:** "Which phase is the bottleneck for large codebases?"
2. **Adoption:** "What % of Pluto programs use generics? Closures? DI?"
3. **Errors:** "What are the top 5 most common compilation errors?"
4. **Optimization:** "Did monomorphization changes improve compile time?"
5. **Complexity:** "How large are typical Pluto programs compared to last quarter?"

---

## Future Work

### Phase 6: Runtime Metrics (Later)

- Integrate with Pluto runtime for execution metrics
- Track memory usage, GC pressure, concurrency patterns
- Requires runtime instrumentation (separate RFC)

### Phase 7: Language Server Integration

- Expose metrics via LSP for IDE integration
- Show performance hints in editor
- Track which files are slow to typecheck

### Phase 8: Cross-Compilation Metrics

- Track metrics across different targets (aarch64, x86_64)
- Compare performance characteristics
- Identify target-specific bottlenecks

---

## References

- [Rust compiler performance tracking](https://github.com/rust-lang/rustc-perf)
- [Go compiler metrics](https://go.dev/doc/diagnostics#profiling)
- [TypeScript compiler tracing](https://github.com/microsoft/TypeScript/wiki/Performance-Tracing)

---

## Changelog

- **2026-02-11:** Initial draft

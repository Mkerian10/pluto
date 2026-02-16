pub mod span;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod visit;
pub mod typeck;
pub mod codegen;
pub mod modules;
pub mod closures;
pub mod monomorphize;
pub mod prelude;
pub mod reflection;
pub mod ambient;
pub mod spawn;
pub mod contracts;
pub mod marshal;
pub mod concurrency;
pub mod manifest;
pub mod git_cache;
pub mod binary;
pub mod derived;
pub mod pretty;
pub mod xref;
pub mod sync;
pub mod plto_store;
pub mod stages;
pub mod cache;
pub mod watch;
pub mod coverage;
pub mod toolchain;
pub mod server;
pub mod docs;

use diagnostics::{CompileError, CompileWarning};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resolve the effective stdlib root path from an explicit argument or PLUTO_STDLIB env var.
fn resolve_stdlib(stdlib_root: Option<&Path>) -> Option<PathBuf> {
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib)
}

struct FrontendResult {
    env: typeck::env::TypeEnv,
    warnings: Vec<CompileWarning>,
}

/// Run the shared frontend pipeline: prelude → ambient → spawn → [strip tests] →
/// contracts → typeck → monomorphize → trait conformance → closures → xref.
/// Run the frontend for editing/analysis: prelude → stages → ambient → type check.
/// Stops BEFORE transformations (spawn desugar, monomorphize, closure lift, reflection).
/// This preserves the canonical (pre-transformation) AST for emit-ast and analyze.
fn run_frontend_for_editing(program: &mut Program) -> Result<FrontendResult, CompileError> {
    prelude::inject_prelude(program)?;
    stages::flatten_stage_hierarchy(program)?;
    ambient::desugar_ambient(program)?;
    contracts::validate_contracts(program)?;
    let (env, warnings) = typeck::type_check(program)?;
    Ok(FrontendResult { env, warnings })
}

/// Run the full frontend pipeline for compilation: editing pipeline + transformations.
/// This mutates the AST with spawn desugaring, monomorphization, closure lifting, etc.
fn run_frontend(program: &mut Program, test_mode: bool) -> Result<FrontendResult, CompileError> {
    prelude::inject_prelude(program)?;
    stages::flatten_stage_hierarchy(program)?;
    ambient::desugar_ambient(program)?;
    spawn::desugar_spawn(program)?;
    if !test_mode {
        let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
            .map(|t| t.fn_name.clone()).collect();
        program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
        program.test_info.clear();
        program.tests = None;
    }
    contracts::validate_contracts(program)?;
    marshal::generate_marshalers_phase_a(program)?;
    let (mut env, warnings) = typeck::type_check(program)?;
    reflection::generate_type_info_impls(program, &env)?;
    monomorphize::monomorphize(program, &mut env)?;
    marshal::generate_marshalers_phase_b(program, &env)?;
    typeck::check_trait_conformance(program, &mut env)?;
    typeck::serializable::validate_serializable_types(program, &env)?;
    closures::lift_closures(program, &mut env)?;
    xref::resolve_cross_refs(program);
    Ok(FrontendResult { env, warnings })
}

/// Resolve extern rust crates and inject their extern fn declarations into the program.

/// Add Rust FFI artifact static libs and native lib flags to a link config.

/// Parse a source string and reject extern rust declarations (which need file-based compilation).
pub fn parse_source(source: &str) -> Result<Program, CompileError> {
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let program = parser.parse_program()?;
    Ok(program)
}

/// Parse source for editing — no transforms (no monomorphize, no closure lift, no spawn desugar).
/// Does NOT inject prelude (avoids serializing Option<T> etc. into user source).
/// Resolves cross-references so xref IDs are available for user-defined declarations.
pub fn parse_for_editing(source: &str) -> Result<parser::ast::Program, CompileError> {
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;
    xref::resolve_cross_refs(&mut program);
    Ok(program)
}

/// Compile a source string to object bytes (lex → parse → prelude → typeck → monomorphize → closures → codegen).
/// No file I/O or linking. Useful for compile-fail tests that only need to check errors.
pub fn compile_to_object(source: &str) -> Result<Vec<u8>, CompileError> {
    // Run compilation on a thread with a larger stack (16MB) to handle deeply nested expressions
    // like classes with 100+ fields where the sum expression creates a deeply nested BinOp tree.
    // Default stack size (typically 2-8MB) can overflow with ~100 levels of recursion.
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let mut program = parse_source(&source)?;
            // Resolve QualifiedAccess for single-file programs (no module flattening)
            modules::resolve_qualified_access_single_file(&mut program)?;
            let result = run_frontend(&mut program, false)?;
            codegen::codegen(&program, &result.env, &source, None)
        })
        .expect("failed to spawn compilation thread")
        .join()
        .expect("compilation thread panicked")
}

/// Compile a source string and return both the object bytes and any compiler warnings.
pub fn compile_to_object_with_warnings(source: &str) -> Result<(Vec<u8>, Vec<CompileWarning>), CompileError> {
    // Run compilation on a thread with a larger stack (16MB) to handle deeply nested expressions
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let mut program = parse_source(&source)?;
            // Resolve QualifiedAccess for single-file programs (no module flattening)
            modules::resolve_qualified_access_single_file(&mut program)?;
            let result = run_frontend(&mut program, false)?;
            let obj = codegen::codegen(&program, &result.env, &source, None)?;
            Ok((obj, result.warnings))
        })
        .expect("failed to spawn compilation thread")
        .join()
        .expect("compilation thread panicked")
}

/// Compile a source string directly (single-file, no module resolution).
/// Used by tests and backward-compatible API.
pub fn compile(source: &str, output_path: &Path) -> Result<(), CompileError> {
    let object_bytes = compile_to_object(source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    link(&obj_path, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

/// Compile a source string in test mode (lex → parse → prelude → typeck → monomorphize → closures → codegen).
/// Tests are preserved and a test runner main is generated.
pub fn compile_to_object_test_mode(source: &str) -> Result<Vec<u8>, CompileError> {
    // Run compilation on a thread with a larger stack (16MB) to handle deeply nested expressions
    let source = source.to_string();
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let mut program = parse_source(&source)?;
            // Resolve QualifiedAccess for single-file programs (no module flattening)
            modules::resolve_qualified_access_single_file(&mut program)?;
            let result = run_frontend(&mut program, true)?;
            codegen::codegen(&program, &result.env, &source, None)
        })
        .expect("failed to spawn compilation thread")
        .join()
        .expect("compilation thread panicked")
}

/// Compile a source string in test mode directly (single-file, no module resolution).
/// Links against the test runtime (sequential task execution, no-mutex channels).
pub fn compile_test(source: &str, output_path: &Path) -> Result<(), CompileError> {
    let object_bytes = compile_to_object_test_mode(source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let config = LinkConfig::test_config(&obj_path, GcBackend::default())?;
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

/// Compile from a file path with full module resolution.
/// Loads all .pluto files in the entry file's directory, resolves imports, flattens, then compiles.
///
/// `stdlib_root`: optional path to stdlib directory. If None, will try PLUTO_STDLIB env var,
/// then `./stdlib` relative to entry file.
pub fn compile_file(entry_file: &Path, output_path: &Path) -> Result<(), CompileError> {
    compile_file_with_stdlib(entry_file, output_path, None)
}

/// Compile with an explicit stdlib root path.
pub fn compile_file_with_stdlib(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>) -> Result<(), CompileError> {
    compile_file_impl(entry_file, output_path, stdlib_root, false, GcBackend::default(), false).map(|_| ())
}

/// Compile with an explicit stdlib root path and GC backend.
pub fn compile_file_with_options(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>, gc: GcBackend, standalone: bool) -> Result<(), CompileError> {
    compile_file_impl(entry_file, output_path, stdlib_root, standalone, gc, false).map(|_| ())
}

/// Compile with coverage instrumentation. Returns the coverage map.
pub fn compile_file_with_coverage(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>) -> Result<coverage::CoverageMap, CompileError> {
    compile_file_impl(entry_file, output_path, stdlib_root, false, GcBackend::default(), true)?
        .ok_or_else(|| CompileError::codegen("coverage map should have been generated".to_string()))
}

fn compile_file_impl(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>, skip_siblings: bool, gc: GcBackend, coverage: bool) -> Result<Option<coverage::CoverageMap>, CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = if skip_siblings {
        modules::resolve_modules_no_siblings(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?
    } else {
        modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?
    };


    let (mut program, source_map) = modules::flatten_modules(graph)?;


    let result = run_frontend(&mut program, false)?;
    for w in &result.warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }

    let cov_map = if coverage {
        Some(coverage::build_coverage_map(&program, &source_map))
    } else {
        None
    };
    let object_bytes = codegen::codegen(&program, &result.env, &source, cov_map.as_ref())?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let config = LinkConfig::default_config(&obj_path, gc)?;
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(cov_map)
}

use parser::ast::Program;

/// Analyze a source file: run the full front-end pipeline (parse → modules → desugar →
/// typeck → monomorphize → closures → xref) but stop before codegen.
/// Returns the fully resolved Program, the entry file's source text, and derived analysis data.
/// Parse a .pt file for editing/analysis without AST transformations.
/// Returns the canonical (pre-transformation) AST + type-checked derived data.
/// Use this for emit-ast and analyze commands.
pub fn parse_file_for_editing(entry_file: &Path, stdlib_root: Option<&Path>) -> Result<(Program, String, derived::DerivedInfo), CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    // Type check without transformations (preserves canonical AST)
    let result = run_frontend_for_editing(&mut program)?;
    let derived = derived::DerivedInfo::build(&result.env, &program, &source);

    Ok((program, source, derived))
}

pub fn analyze_file(entry_file: &Path, stdlib_root: Option<&Path>) -> Result<(Program, String, derived::DerivedInfo), CompileError> {
    let (program, source, derived, _warnings) = analyze_file_with_warnings(entry_file, stdlib_root)?;
    Ok((program, source, derived))
}

/// Like `analyze_file`, but also returns compiler warnings.
pub fn analyze_file_with_warnings(entry_file: &Path, stdlib_root: Option<&Path>) -> Result<(Program, String, derived::DerivedInfo, Vec<CompileWarning>), CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;


    let (mut program, source_map) = modules::flatten_modules(graph)?;

    let result = run_frontend(&mut program, false)?;
    let derived = derived::DerivedInfo::build(&result.env, &program, &source);

    // Filter warnings to only include those from the entry file
    // Find the entry file's ID in the source map
    let entry_file_id = source_map.files.iter()
        .position(|(path, _)| path == &entry_file)
        .map(|pos| pos as u32);

    let filtered_warnings: Vec<CompileWarning> = if let Some(file_id) = entry_file_id {
        result.warnings.into_iter()
            .filter(|w| w.span.file_id == file_id)
            .collect()
    } else {
        // Fallback: if we can't find the entry file in source map, return all warnings
        // This shouldn't happen but is safer than returning no warnings
        result.warnings
    };

    Ok((program, source, derived, filtered_warnings))
}

/// Analyze an existing .pluto file and update it with fresh derived data.
///
/// This function:
/// 1. Reads a .pluto binary file
/// 2. Runs full type analysis on the AST
/// 3. Builds fresh DerivedInfo
/// 4. Writes the updated .pluto file with new derived data
///
/// The AST and source text are preserved unchanged.
///
/// If the input is a .pt text file, it will be parsed first.
pub fn analyze_and_update(
    file_path: &Path,
    stdlib_root: Option<&Path>,
) -> Result<(), CompileError> {
    // Canonicalize path
    let file_path = file_path.canonicalize().map_err(|e| {
        CompileError::codegen(format!("could not resolve path '{}': {e}", file_path.display()))
    })?;

    // Read the file
    let data = std::fs::read(&file_path).map_err(|e| {
        CompileError::codegen(format!("failed to read {}: {e}", file_path.display()))
    })?;

    // Determine if it's a binary .pluto or text .pt file
    let (mut program, source) = if binary::is_binary_format(&data) {
        // Binary .pluto file - deserialize it (already flattened)
        let (program, source, _old_derived) = binary::deserialize_program(&data)
            .map_err(|e| CompileError::codegen(format!("failed to deserialize .pluto: {e}")))?;
        (program, source)
    } else {
        // Text file - resolve and flatten modules
        let source = std::fs::read_to_string(&file_path)
            .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
        let effective_stdlib = resolve_stdlib(stdlib_root);
        let entry_dir = file_path.parent().unwrap_or(Path::new("."));
        let pkg_graph = manifest::find_and_resolve(entry_dir)?;

        // Resolve module graph (discover imports, parse all files)
        // Use no_siblings variant to avoid parsing the .pluto output file as a sibling
        let graph = modules::resolve_modules_no_siblings(&file_path, effective_stdlib.as_deref(), &pkg_graph)?;

        // Flatten modules into single program
        let (program, _source_map) = modules::flatten_modules(graph)?;

        // TODO: Store merged source from source_map instead of just entry file
        (program, source)
    };

    // Run analysis pipeline without transformations (preserves canonical AST)
    let result = run_frontend_for_editing(&mut program)?;
    let derived = derived::DerivedInfo::build(&result.env, &program, &source);

    // Serialize with fresh derived data
    let bytes = binary::serialize_program(&program, &source, &derived)
        .map_err(|e| CompileError::codegen(format!("failed to serialize .pluto: {e}")))?;

    // Determine output path - always write .pluto
    let output_path = file_path.with_extension("pluto");

    // Write updated file
    std::fs::write(&output_path, &bytes).map_err(|e| {
        CompileError::codegen(format!("failed to write {}: {e}", output_path.display()))
    })?;

    Ok(())
}

/// Filter tests based on cache - keeps only tests with changed dependencies.
/// Returns the number of tests to run (after filtering).
fn filter_tests_by_cache(
    entry_file: &Path,
    source: &str,
    program: &mut parser::ast::Program,
    derived_info: &derived::DerivedInfo,
) -> Result<usize, CompileError> {
    // Try to load cache
    let cached = cache::load_cache(entry_file, source);

    if let Some(cache_entry) = cached {
        // Compare current hashes with cached hashes
        let mut tests_to_run = Vec::new();

        for test_info in &program.test_info {
            let current_hash = derived_info
                .test_dep_hashes
                .get(&test_info.display_name);
            let cached_hash = cache_entry.test_hashes.get(&test_info.display_name);

            // Run test if:
            // 1. It's a new test (not in cache)
            // 2. Its hash has changed
            // 3. Current hash is missing (shouldn't happen, but be safe)
            let should_run = match (current_hash, cached_hash) {
                (Some(curr), Some(cached)) => curr != cached,
                _ => true, // New test or missing hash
            };

            if should_run {
                tests_to_run.push(test_info.clone());
            }
        }

        let count = tests_to_run.len();
        // Replace test_info with filtered list
        program.test_info = tests_to_run;
        Ok(count)
    } else {
        // No cache, run all tests
        Ok(program.test_info.len())
    }
}

/// Compile a file in test mode. Tests are preserved and a test runner main is generated.
/// If `use_cache` is true, only tests with changed dependencies will be run.
pub fn compile_file_for_tests(
    entry_file: &Path,
    output_path: &Path,
    stdlib_root: Option<&Path>,
    use_cache: bool,
) -> Result<(), CompileError> {
    compile_file_for_tests_with_coverage(entry_file, output_path, stdlib_root, use_cache, false).map(|_| ())
}

/// Compile a file in test mode with a specific GC backend.
pub fn compile_file_for_tests_with_gc(
    entry_file: &Path,
    output_path: &Path,
    stdlib_root: Option<&Path>,
    use_cache: bool,
    gc: GcBackend,
) -> Result<(), CompileError> {
    compile_file_for_tests_impl(entry_file, output_path, stdlib_root, use_cache, gc, false).map(|_| ())
}

/// Compile a file in test mode with optional coverage instrumentation.
/// Returns Option<CoverageMap> when coverage is enabled.
pub fn compile_file_for_tests_with_coverage(
    entry_file: &Path,
    output_path: &Path,
    stdlib_root: Option<&Path>,
    use_cache: bool,
    coverage: bool,
) -> Result<Option<coverage::CoverageMap>, CompileError> {
    compile_file_for_tests_impl(entry_file, output_path, stdlib_root, use_cache, GcBackend::default(), coverage)
}

fn compile_file_for_tests_impl(
    entry_file: &Path,
    output_path: &Path,
    stdlib_root: Option<&Path>,
    use_cache: bool,
    gc: GcBackend,
    coverage: bool,
) -> Result<Option<coverage::CoverageMap>, CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    // Use resolve_modules_no_siblings to compile test files in isolation and prevent test ID collisions
    let graph = modules::resolve_modules_no_siblings(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;


    let (mut program, source_map) = modules::flatten_modules(graph)?;

    if program.test_info.is_empty() {
        return Err(CompileError::codegen(format!(
            "no tests found in '{}'", entry_file.display()
        )));
    }
    if program.app.is_some() {
        return Err(CompileError::codegen(
            "test files should not contain an app declaration".to_string()
        ));
    }
    if !program.stages.is_empty() {
        return Err(CompileError::codegen(
            "test files should not contain a stage declaration".to_string()
        ));
    }


    let result = run_frontend(&mut program, true)?;
    for w in &result.warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }

    // Build derived info to get test dependency hashes
    let derived_info = derived::DerivedInfo::build(&result.env, &program, &source);

    // Load cache and filter tests if caching is enabled
    let original_test_count = program.test_info.len();
    let tests_to_run = if use_cache {
        filter_tests_by_cache(&entry_file, &source, &mut program, &derived_info)?
    } else {
        // Run all tests
        program.test_info.len()
    };

    // If all tests are skipped, exit early with success
    if tests_to_run == 0 {
        eprintln!("All {} tests unchanged, skipping execution", original_test_count);
        // Still save the cache for next run
        let _ = cache::save_cache(
            &entry_file,
            &source,
            derived_info.test_dep_hashes.clone().into_iter().collect(),
        );
        return Ok(None);
    }

    if use_cache && tests_to_run < original_test_count {
        eprintln!(
            "Running {} of {} tests ({} skipped, unchanged)",
            tests_to_run,
            original_test_count,
            original_test_count - tests_to_run
        );
    }

    let cov_map = if coverage {
        Some(coverage::build_coverage_map(&program, &source_map))
    } else {
        None
    };
    let object_bytes = codegen::codegen(&program, &result.env, &source, cov_map.as_ref())?;

    // Save cache after successful compilation
    if use_cache {
        let _ = cache::save_cache(
            &entry_file,
            &source,
            derived_info.test_dep_hashes.into_iter().collect(),
        );
    }

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let config = LinkConfig::test_config(&obj_path, gc)?;
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(cov_map)
}

/// Which garbage collector backend to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GcBackend {
    /// Conservative mark-and-sweep collector (default).
    #[default]
    MarkSweep,
    /// No-op allocator that never collects. Useful for benchmarking.
    Noop,
}

impl GcBackend {
    fn name(&self) -> &'static str {
        match self {
            GcBackend::MarkSweep => "marksweep",
            GcBackend::Noop => "noop",
        }
    }

    fn gc_source(&self) -> &'static str {
        match self {
            GcBackend::MarkSweep => include_str!("../runtime/gc/marksweep.c"),
            GcBackend::Noop => include_str!("../runtime/gc/noop.c"),
        }
    }
}

/// Compute a content-addressed cache key for the runtime object file.
/// The key incorporates all C source content, compilation flags, GC backend,
/// and host platform so that any change triggers a cache miss.
fn runtime_cache_key(test_mode: bool, gc: GcBackend) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    gc.gc_source().hash(&mut hasher);
    include_str!("../runtime/threading.c").hash(&mut hasher);
    include_str!("../runtime/builtins.c").hash(&mut hasher);
    include_str!("../runtime/builtins.h").hash(&mut hasher);
    test_mode.hash(&mut hasher);
    gc.name().hash(&mut hasher);
    std::env::consts::ARCH.hash(&mut hasher);
    std::env::consts::OS.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Check the persistent disk cache for a pre-compiled runtime object.
/// Returns the cached path if it exists and is non-empty, None otherwise.
fn check_disk_cache(cache_key: &str) -> Option<PathBuf> {
    let cache_dir = git_cache::cache_root().join("runtime");
    let cached_path = cache_dir.join(format!("{cache_key}.o"));
    match std::fs::metadata(&cached_path) {
        Ok(meta) if meta.len() > 0 => Some(cached_path),
        _ => None,
    }
}

/// Store a compiled runtime object in the persistent disk cache.
/// Uses atomic write (write to .tmp, then rename) to avoid partial reads.
fn store_disk_cache(cache_key: &str, object_path: &Path) -> Result<(), CompileError> {
    let cache_dir = git_cache::cache_root().join("runtime");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| CompileError::link(format!("failed to create runtime cache dir: {e}")))?;
    let final_path = cache_dir.join(format!("{cache_key}.o"));
    let tmp_path = cache_dir.join(format!("{cache_key}.o.tmp"));
    std::fs::copy(object_path, &tmp_path)
        .map_err(|e| CompileError::link(format!("failed to write runtime cache: {e}")))?;
    std::fs::rename(&tmp_path, &final_path)
        .map_err(|e| CompileError::link(format!("failed to finalize runtime cache: {e}")))?;
    Ok(())
}

/// Compile gc, threading, and builtins C sources to a single linked object file.
/// Uses a three-tier cache: OnceLock (in-process) → disk cache → full compilation.
fn compile_runtime_object(test_mode: bool, gc: GcBackend) -> Result<PathBuf, CompileError> {
    let cache_key = runtime_cache_key(test_mode, gc);

    // Tier 2: Check persistent disk cache
    if let Some(cached) = check_disk_cache(&cache_key) {
        return Ok(cached);
    }

    // Tier 3: Full compilation
    let gc_src = gc.gc_source();
    let threading_src = include_str!("../runtime/threading.c");
    let builtins_src = include_str!("../runtime/builtins.c");
    let coverage_src = include_str!("../runtime/coverage.c");
    let header_src = include_str!("../runtime/builtins.h");

    let dir_suffix = if test_mode { "pluto_test_runtime" } else { "pluto_runtime" };
    let dir = std::env::temp_dir().join(format!("{}_{}_{}", dir_suffix, gc.name(), std::process::id()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| CompileError::link(format!("failed to create runtime build dir: {e}")))?;

    // Write all source files
    let header_h = dir.join("builtins.h");
    let gc_c = dir.join("gc.c");
    let threading_c = dir.join("threading.c");
    let builtins_c = dir.join("builtins.c");
    let coverage_c = dir.join("coverage.c");

    std::fs::write(&header_h, header_src)
        .map_err(|e| CompileError::link(format!("failed to write header: {e}")))?;
    std::fs::write(&gc_c, gc_src)
        .map_err(|e| CompileError::link(format!("failed to write gc.c: {e}")))?;
    std::fs::write(&threading_c, threading_src)
        .map_err(|e| CompileError::link(format!("failed to write threading.c: {e}")))?;
    std::fs::write(&builtins_c, builtins_src)
        .map_err(|e| CompileError::link(format!("failed to write builtins.c: {e}")))?;
    std::fs::write(&coverage_c, coverage_src)
        .map_err(|e| CompileError::link(format!("failed to write coverage.c: {e}")))?;

    let gc_o = dir.join("gc.o");
    let threading_o = dir.join("threading.o");
    let builtins_o = dir.join("builtins.o");
    let coverage_o = dir.join("coverage.o");
    let runtime_o = dir.join("runtime.o");

    // Compile gc.c
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-c");
    if test_mode {
        cmd.arg("-DPLUTO_TEST_MODE").arg("-Wno-deprecated-declarations");
    }
    cmd.arg("-I").arg(&dir);
    cmd.arg(&gc_c).arg("-o").arg(&gc_o);
    #[cfg(target_os = "linux")]
    if !test_mode {
        cmd.arg("-pthread");
    }
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to compile gc.c: {e}")))?;
    if !status.success() {
        return Err(CompileError::link("failed to compile gc.c"));
    }

    // Compile threading.c
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-c");
    if test_mode {
        cmd.arg("-DPLUTO_TEST_MODE").arg("-Wno-deprecated-declarations");
    }
    cmd.arg("-I").arg(&dir);
    cmd.arg(&threading_c).arg("-o").arg(&threading_o);
    #[cfg(target_os = "linux")]
    if !test_mode {
        cmd.arg("-pthread");
    }
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to compile threading.c: {e}")))?;
    if !status.success() {
        return Err(CompileError::link("failed to compile threading.c"));
    }

    // Compile builtins.c
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-c");
    if test_mode {
        cmd.arg("-DPLUTO_TEST_MODE").arg("-Wno-deprecated-declarations");
    }
    cmd.arg("-I").arg(&dir);
    cmd.arg(&builtins_c).arg("-o").arg(&builtins_o);
    #[cfg(target_os = "linux")]
    if !test_mode {
        cmd.arg("-pthread");
    }
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to compile builtins.c: {e}")))?;
    if !status.success() {
        return Err(CompileError::link("failed to compile builtins.c"));
    }

    // Compile coverage.c
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-c");
    if test_mode {
        cmd.arg("-DPLUTO_TEST_MODE").arg("-Wno-deprecated-declarations");
    }
    cmd.arg("-I").arg(&dir);
    cmd.arg(&coverage_c).arg("-o").arg(&coverage_o);
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to compile coverage.c: {e}")))?;
    if !status.success() {
        return Err(CompileError::link("failed to compile coverage.c"));
    }

    // Link all object files into one
    let mut cmd = std::process::Command::new("ld");
    cmd.arg("-r");
    cmd.arg(&gc_o).arg(&threading_o).arg(&builtins_o).arg(&coverage_o).arg("-o").arg(&runtime_o);
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to link runtime: {e}")))?;
    if !status.success() {
        return Err(CompileError::link("failed to link runtime"));
    }

    // Store in persistent disk cache before cleaning up
    let _ = store_disk_cache(&cache_key, &runtime_o);

    // Cleanup intermediate files
    let _ = std::fs::remove_file(&header_h);
    let _ = std::fs::remove_file(&gc_c);
    let _ = std::fs::remove_file(&threading_c);
    let _ = std::fs::remove_file(&builtins_c);
    let _ = std::fs::remove_file(&coverage_c);
    let _ = std::fs::remove_file(&gc_o);
    let _ = std::fs::remove_file(&threading_o);
    let _ = std::fs::remove_file(&builtins_o);
    let _ = std::fs::remove_file(&coverage_o);

    // Return the disk-cached path if it was stored successfully, otherwise the temp path
    if let Some(cached) = check_disk_cache(&cache_key) {
        let _ = std::fs::remove_file(&runtime_o);
        let _ = std::fs::remove_dir(&dir);
        Ok(cached)
    } else {
        Ok(runtime_o)
    }
}

/// Compile the runtime once per process (per backend) and cache the resulting .o path.
/// Tier 1 (OnceLock) wraps Tier 2 (disk) and Tier 3 (full compile).
fn cached_runtime_object(gc: GcBackend) -> Result<&'static Path, CompileError> {
    match gc {
        GcBackend::MarkSweep => {
            static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
            let result = CACHE.get_or_init(|| compile_runtime_object(false, GcBackend::MarkSweep).map_err(|e| e.to_string()));
            match result {
                Ok(path) => Ok(path.as_path()),
                Err(msg) => Err(CompileError::link(msg.clone())),
            }
        }
        GcBackend::Noop => {
            static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
            let result = CACHE.get_or_init(|| compile_runtime_object(false, GcBackend::Noop).map_err(|e| e.to_string()));
            match result {
                Ok(path) => Ok(path.as_path()),
                Err(msg) => Err(CompileError::link(msg.clone())),
            }
        }
    }
}

/// Compile the test runtime once per process (per backend) and cache the resulting .o path.
fn cached_test_runtime_object(gc: GcBackend) -> Result<&'static Path, CompileError> {
    match gc {
        GcBackend::MarkSweep => {
            static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
            let result = CACHE.get_or_init(|| compile_runtime_object(true, GcBackend::MarkSweep).map_err(|e| e.to_string()));
            match result {
                Ok(path) => Ok(path.as_path()),
                Err(msg) => Err(CompileError::link(msg.clone())),
            }
        }
        GcBackend::Noop => {
            static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
            let result = CACHE.get_or_init(|| compile_runtime_object(true, GcBackend::Noop).map_err(|e| e.to_string()));
            match result {
                Ok(path) => Ok(path.as_path()),
                Err(msg) => Err(CompileError::link(msg.clone())),
            }
        }
    }
}

struct LinkConfig {
    objects: Vec<PathBuf>,
    static_libs: Vec<PathBuf>,
    flags: Vec<String>,
}

impl LinkConfig {
    fn default_config(pluto_obj: &Path, gc: GcBackend) -> Result<Self, CompileError> {
        let runtime_o = cached_runtime_object(gc)?;
        #[allow(unused_mut)]
        let mut flags = vec!["-lm".to_string()];
        #[cfg(target_os = "linux")]
        flags.push("-pthread".to_string());
        Ok(Self {
            objects: vec![pluto_obj.to_path_buf(), runtime_o.to_path_buf()],
            static_libs: vec![],
            flags,
        })
    }

    fn test_config(pluto_obj: &Path, gc: GcBackend) -> Result<Self, CompileError> {
        let runtime_o = cached_test_runtime_object(gc)?;
        let flags = vec!["-lm".to_string()];
        // No -pthread in test mode (single-threaded)
        Ok(Self {
            objects: vec![pluto_obj.to_path_buf(), runtime_o.to_path_buf()],
            static_libs: vec![],
            flags,
        })
    }
}

fn link_from_config(config: &LinkConfig, output: &Path) -> Result<(), CompileError> {
    let mut cmd = std::process::Command::new("cc");
    for obj in &config.objects {
        cmd.arg(obj);
    }
    for lib in &config.static_libs {
        cmd.arg(lib);
    }
    cmd.arg("-o").arg(output);
    for flag in &config.flags {
        cmd.arg(flag);
    }

    let status = cmd
        .status()
        .map_err(|e| CompileError::link(format!("failed to invoke linker: {e}")))?;

    if !status.success() {
        return Err(CompileError::link("linker failed"));
    }

    Ok(())
}

fn link(obj_path: &Path, output_path: &Path) -> Result<(), CompileError> {
    let config = LinkConfig::default_config(obj_path, GcBackend::default())?;
    link_from_config(&config, output_path)
}

/// Quick-parse a file to check if it contains a system declaration.
/// Returns the parsed program if it does, None if it doesn't.
pub fn detect_system_file(entry_file: &Path) -> Result<Option<parser::ast::Program>, CompileError> {
    let source = std::fs::read_to_string(entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read file: {e}")))?;
    let tokens = lexer::lex(&source)?;
    let mut parser = parser::Parser::new(&tokens, &source);
    let program = parser.parse_program()?;
    if program.system.is_some() {
        Ok(Some(program))
    } else {
        Ok(None)
    }
}

/// Compile a system file: parse the system declaration, validate members,
/// and compile each member app as a standalone binary.
///
/// Returns a list of (member_name, binary_path) on success.
pub fn compile_system_file_with_stdlib(
    system_file: &Path,
    output_dir: &Path,
    stdlib_root: Option<&Path>,
) -> Result<Vec<(String, PathBuf)>, CompileError> {
    let system_file = system_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", system_file.display())))?;
    let system_dir = system_file.parent().ok_or_else(||
        CompileError::codegen("system file has no parent directory"))?;

    let source = std::fs::read_to_string(&system_file)
        .map_err(|e| CompileError::codegen(format!("failed to read system file: {e}")))?;
    let tokens = lexer::lex(&source)?;
    let mut parser = parser::Parser::new(&tokens, &source);
    let program = parser.parse_program()?;

    let system_decl = program.system.as_ref().ok_or_else(||
        CompileError::codegen("file does not contain a system declaration"))?;

    // Validate: no top-level fn main() in system file
    for func in &program.functions {
        if func.node.name.node == "main" {
            return Err(CompileError::syntax(
                "system files must not contain a top-level fn main()",
                func.node.name.span,
            ));
        }
    }

    // Validate each member references an imported module
    let import_names: std::collections::HashSet<String> = program.imports.iter()
        .map(|i| i.node.binding_name().to_string())
        .collect();

    for member in &system_decl.node.members {
        if !import_names.contains(&member.module_name.node) {
            return Err(CompileError::syntax(
                format!("system member '{}' references module '{}' which is not imported",
                    member.name.node, member.module_name.node),
                member.module_name.span,
            ));
        }
    }

    // Resolve modules to validate that member modules contain apps
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let pkg_graph = manifest::find_and_resolve(system_dir)?;
    let graph = modules::resolve_modules(&system_file, effective_stdlib.as_deref(), &pkg_graph)?;

    for member in &system_decl.node.members {
        let module_name = &member.module_name.node;
        let module_prog = graph.imports.iter()
            .find(|(name, _, _)| name == module_name)
            .map(|(_, prog, _)| prog);
        match module_prog {
            Some(prog) if prog.app.is_none() => {
                return Err(CompileError::syntax(
                    format!("system member '{}' references module '{}' which does not contain an app declaration",
                        member.name.node, module_name),
                    member.module_name.span,
                ));
            }
            None => {
                return Err(CompileError::syntax(
                    format!("system member '{}' references module '{}' which could not be resolved",
                        member.name.node, module_name),
                    member.module_name.span,
                ));
            }
            _ => {} // Has app, valid
        }
    }

    // Create output directory
    std::fs::create_dir_all(output_dir)
        .map_err(|e| CompileError::codegen(format!("failed to create output directory: {e}")))?;

    // Compile each member as a standalone program
    let mut results = Vec::new();
    for member in &system_decl.node.members {
        let module_name = &member.module_name.node;
        let member_name = &member.name.node;

        // Find the module's entry point
        let dir_path = system_dir.join(module_name);
        let file_path = system_dir.join(format!("{}.pluto", module_name));

        let entry_file = if dir_path.is_dir() {
            // Directory module: use main.pluto if it exists, otherwise first .pluto file
            let main_pluto = dir_path.join("main.pluto");
            if main_pluto.is_file() {
                main_pluto
            } else {
                let mut files: Vec<PathBuf> = std::fs::read_dir(&dir_path)
                    .map_err(|e| CompileError::codegen(format!("could not read directory '{}': {e}", dir_path.display())))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|ext| ext == "pluto"))
                    .collect();
                files.sort();
                files.into_iter().next().ok_or_else(||
                    CompileError::codegen(format!("no .pluto files found in module directory '{}'", dir_path.display())))?
            }
        } else if file_path.is_file() {
            file_path
        } else {
            return Err(CompileError::codegen(format!(
                "cannot find module '{}': no directory or file found", module_name
            )));
        };

        let output_path = output_dir.join(member_name);
        compile_file_impl(&entry_file, &output_path, stdlib_root, true, GcBackend::default(), false)?;
        results.push((member_name.clone(), output_path));
    }

    Ok(results)
}

/// Fetch latest versions of all git dependencies declared in pluto.toml.
pub fn update_git_deps(dir: &Path) -> Result<(), CompileError> {
    let updated = manifest::update_git_deps(dir)?;
    if updated.is_empty() {
        eprintln!("no git dependencies to update");
    } else {
        for name in &updated {
            eprintln!("updated: {name}");
        }
    }
    Ok(())
}


pub mod span;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod typeck;
pub mod codegen;
pub mod modules;
pub mod closures;
pub mod monomorphize;
pub mod prelude;
pub mod ambient;
pub mod rust_ffi;
pub mod spawn;
pub mod contracts;
pub mod concurrency;
pub mod manifest;
pub mod git_cache;
pub mod lsp;
pub mod binary;
pub mod derived;
pub mod pretty;
pub mod xref;
pub mod sync;
pub mod stages;
pub mod cache;

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
    let (mut env, warnings) = typeck::type_check(program)?;
    monomorphize::monomorphize(program, &mut env)?;
    typeck::check_trait_conformance(program, &mut env)?;
    closures::lift_closures(program, &mut env)?;
    xref::resolve_cross_refs(program);
    Ok(FrontendResult { env, warnings })
}

/// Resolve extern rust crates and inject their extern fn declarations into the program.
fn resolve_rust_artifacts(program: &mut Program, entry_dir: &Path) -> Result<Vec<rust_ffi::RustCrateArtifact>, CompileError> {
    let artifacts = if program.extern_rust_crates.is_empty() {
        vec![]
    } else {
        rust_ffi::resolve_rust_crates(program, entry_dir)?
    };
    rust_ffi::inject_extern_fns(program, &artifacts);
    Ok(artifacts)
}

/// Add Rust FFI artifact static libs and native lib flags to a link config.
fn add_rust_artifact_flags(config: &mut LinkConfig, artifacts: &[rust_ffi::RustCrateArtifact]) {
    for artifact in artifacts {
        config.static_libs.push(artifact.static_lib.clone());
        config.flags.extend(artifact.native_libs.clone());
    }
}

/// Parse a source string and reject extern rust declarations (which need file-based compilation).
fn parse_source(source: &str) -> Result<Program, CompileError> {
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let program = parser.parse_program()?;
    if !program.extern_rust_crates.is_empty() {
        return Err(CompileError::syntax(
            "extern rust declarations require file-based compilation (use compile_file instead)",
            program.extern_rust_crates[0].span,
        ));
    }
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
    let mut program = parse_source(source)?;
    let result = run_frontend(&mut program, false)?;
    codegen::codegen(&program, &result.env, source)
}

/// Compile a source string and return both the object bytes and any compiler warnings.
pub fn compile_to_object_with_warnings(source: &str) -> Result<(Vec<u8>, Vec<CompileWarning>), CompileError> {
    let mut program = parse_source(source)?;
    let result = run_frontend(&mut program, false)?;
    let obj = codegen::codegen(&program, &result.env, source)?;
    Ok((obj, result.warnings))
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
    let mut program = parse_source(source)?;
    let result = run_frontend(&mut program, true)?;
    codegen::codegen(&program, &result.env, source)
}

/// Compile a source string in test mode directly (single-file, no module resolution).
/// Links against the test runtime (sequential task execution, no-mutex channels).
pub fn compile_test(source: &str, output_path: &Path) -> Result<(), CompileError> {
    let object_bytes = compile_to_object_test_mode(source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let config = LinkConfig::test_config(&obj_path)?;
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
    compile_file_impl(entry_file, output_path, stdlib_root, false)
}

fn compile_file_impl(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>, skip_siblings: bool) -> Result<(), CompileError> {
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

    check_extern_rust_import_collisions(&graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    let rust_artifacts = resolve_rust_artifacts(&mut program, entry_dir)?;

    let result = run_frontend(&mut program, false)?;
    for w in &result.warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }
    let object_bytes = codegen::codegen(&program, &result.env, &source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let mut config = LinkConfig::default_config(&obj_path)?;
    add_rust_artifact_flags(&mut config, &rust_artifacts);
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

use parser::ast::Program;

/// Analyze a source file: run the full front-end pipeline (parse → modules → desugar →
/// typeck → monomorphize → closures → xref) but stop before codegen.
/// Returns the fully resolved Program, the entry file's source text, and derived analysis data.
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

    check_extern_rust_import_collisions(&graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    resolve_rust_artifacts(&mut program, entry_dir)?;

    let result = run_frontend(&mut program, false)?;
    let derived = derived::DerivedInfo::build(&result.env, &program);

    Ok((program, source, derived, result.warnings))
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
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let effective_stdlib = resolve_stdlib(stdlib_root);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;

    check_extern_rust_import_collisions(&graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

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

    let rust_artifacts = resolve_rust_artifacts(&mut program, entry_dir)?;

    let result = run_frontend(&mut program, true)?;
    for w in &result.warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }

    // Build derived info to get test dependency hashes
    let derived_info = derived::DerivedInfo::build(&result.env, &program);

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
        return Ok(());
    }

    if use_cache && tests_to_run < original_test_count {
        eprintln!(
            "Running {} of {} tests ({} skipped, unchanged)",
            tests_to_run,
            original_test_count,
            original_test_count - tests_to_run
        );
    }

    let object_bytes = codegen::codegen(&program, &result.env, &source)?;

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

    let mut config = LinkConfig::test_config(&obj_path)?;
    add_rust_artifact_flags(&mut config, &rust_artifacts);
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

/// Compile builtins.c to an object file. In test mode, adds -DPLUTO_TEST_MODE
/// for sequential task execution and no-mutex channels.
fn compile_runtime_object(test_mode: bool) -> Result<PathBuf, CompileError> {
    let runtime_src = include_str!("../runtime/builtins.c");
    let dir_suffix = if test_mode { "pluto_test_runtime" } else { "pluto_runtime" };
    let dir = std::env::temp_dir().join(format!("{}_{}", dir_suffix, std::process::id()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| CompileError::link(format!("failed to create runtime cache dir: {e}")))?;
    let runtime_c = dir.join("builtins.c");
    let o_name = if test_mode { "builtins_test.o" } else { "builtins.o" };
    let runtime_o = dir.join(o_name);
    std::fs::write(&runtime_c, runtime_src)
        .map_err(|e| CompileError::link(format!("failed to write runtime source: {e}")))?;
    let mut cmd = std::process::Command::new("cc");
    cmd.arg("-c");
    if test_mode {
        cmd.arg("-DPLUTO_TEST_MODE").arg("-Wno-deprecated-declarations");
    }
    cmd.arg(&runtime_c).arg("-o").arg(&runtime_o);
    #[cfg(target_os = "linux")]
    if !test_mode {
        cmd.arg("-pthread");
    }
    let status = cmd.status()
        .map_err(|e| CompileError::link(format!("failed to compile runtime: {e}")))?;
    let _ = std::fs::remove_file(&runtime_c);
    if !status.success() {
        return Err(CompileError::link("failed to compile runtime"));
    }
    Ok(runtime_o)
}

/// Compile builtins.c once per process and cache the resulting .o path.
fn cached_runtime_object() -> Result<&'static Path, CompileError> {
    static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let result = CACHE.get_or_init(|| compile_runtime_object(false).map_err(|e| e.to_string()));
    match result {
        Ok(path) => Ok(path.as_path()),
        Err(msg) => Err(CompileError::link(msg.clone())),
    }
}

/// Compile builtins.c with -DPLUTO_TEST_MODE once per process and cache the resulting .o path.
fn cached_test_runtime_object() -> Result<&'static Path, CompileError> {
    static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let result = CACHE.get_or_init(|| compile_runtime_object(true).map_err(|e| e.to_string()));
    match result {
        Ok(path) => Ok(path.as_path()),
        Err(msg) => Err(CompileError::link(msg.clone())),
    }
}

struct LinkConfig {
    objects: Vec<PathBuf>,
    static_libs: Vec<PathBuf>,
    flags: Vec<String>,
}

impl LinkConfig {
    fn default_config(pluto_obj: &Path) -> Result<Self, CompileError> {
        let runtime_o = cached_runtime_object()?;
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

    fn test_config(pluto_obj: &Path) -> Result<Self, CompileError> {
        let runtime_o = cached_test_runtime_object()?;
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
    let config = LinkConfig::default_config(obj_path)?;
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
        compile_file_impl(&entry_file, &output_path, stdlib_root, true)?;
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

/// Check that extern rust aliases don't collide with import aliases.
/// Must be called before flatten_modules (which clears import data).
fn check_extern_rust_import_collisions(graph: &modules::ModuleGraph) -> Result<(), CompileError> {
    let import_aliases: std::collections::HashSet<&str> = graph
        .imports
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();

    for ext_rust in &graph.root.extern_rust_crates {
        let alias = &ext_rust.node.alias.node;
        if import_aliases.contains(alias.as_str()) {
            return Err(CompileError::syntax(
                format!(
                    "extern rust alias '{}' conflicts with import alias of the same name",
                    alias
                ),
                ext_rust.node.alias.span,
            ));
        }
    }
    Ok(())
}

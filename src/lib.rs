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
pub mod manifest;
pub mod git_cache;
pub mod lsp;
pub mod binary;
pub mod derived;
pub mod pretty;
pub mod xref;
pub mod sync;

use diagnostics::{CompileError, CompileWarning};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;
    // Reject extern rust in single-string compilation
    if !program.extern_rust_crates.is_empty() {
        return Err(CompileError::syntax(
            "extern rust declarations require file-based compilation (use compile_file instead)",
            program.extern_rust_crates[0].span,
        ));
    }
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    // Strip test functions in non-test mode
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    contracts::validate_contracts(&program)?;
    let (mut env, _warnings) = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    codegen::codegen(&program, &env, source)
}

/// Compile a source string and return both the object bytes and any compiler warnings.
pub fn compile_to_object_with_warnings(source: &str) -> Result<(Vec<u8>, Vec<CompileWarning>), CompileError> {
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;
    if !program.extern_rust_crates.is_empty() {
        return Err(CompileError::syntax(
            "extern rust declarations require file-based compilation (use compile_file instead)",
            program.extern_rust_crates[0].span,
        ));
    }
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    contracts::validate_contracts(&program)?;
    let (mut env, warnings) = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    let obj = codegen::codegen(&program, &env, source)?;
    Ok((obj, warnings))
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
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;
    // Reject extern rust in single-string compilation
    if !program.extern_rust_crates.is_empty() {
        return Err(CompileError::syntax(
            "extern rust declarations require file-based compilation (use compile_file instead)",
            program.extern_rust_crates[0].span,
        ));
    }
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    // test_info is NOT stripped in test mode
    contracts::validate_contracts(&program)?;
    let (mut env, _warnings) = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    codegen::codegen(&program, &env, source)
}

/// Compile a source string in test mode directly (single-file, no module resolution).
pub fn compile_test(source: &str, output_path: &Path) -> Result<(), CompileError> {
    let object_bytes = compile_to_object_test_mode(source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    link(&obj_path, output_path)?;

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
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;

    // Check extern rust aliases don't collide with import aliases
    check_extern_rust_import_collisions(&graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    // Resolve extern rust crates (build glue, extract signatures)
    let rust_artifacts = if program.extern_rust_crates.is_empty() {
        vec![]
    } else {
        rust_ffi::resolve_rust_crates(&program, entry_dir)?
    };
    rust_ffi::inject_extern_fns(&mut program, &rust_artifacts);

    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    // Strip test functions in non-test mode
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    contracts::validate_contracts(&program)?;
    let (mut env, warnings) = typeck::type_check(&program)?;
    for w in &warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    let object_bytes = codegen::codegen(&program, &env, &source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let mut config = LinkConfig::default_config(&obj_path)?;
    for artifact in &rust_artifacts {
        config.static_libs.push(artifact.static_lib.clone());
        config.flags.extend(artifact.native_libs.clone());
    }
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
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;

    check_extern_rust_import_collisions(&graph)?;

    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    let rust_artifacts = if program.extern_rust_crates.is_empty() {
        vec![]
    } else {
        rust_ffi::resolve_rust_crates(&program, entry_dir)?
    };
    rust_ffi::inject_extern_fns(&mut program, &rust_artifacts);

    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    // Strip test functions in non-test mode
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    contracts::validate_contracts(&program)?;
    let (mut env, warnings) = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    let derived = derived::DerivedInfo::build(&env, &program);

    Ok((program, source, derived, warnings))
}

/// Compile a file in test mode. Tests are preserved and a test runner main is generated.
pub fn compile_file_for_tests(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>) -> Result<(), CompileError> {
    let entry_file = entry_file.canonicalize().map_err(|e|
        CompileError::codegen(format!("could not resolve path '{}': {e}", entry_file.display())))?;
    let source = std::fs::read_to_string(&entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir)?;
    let graph = modules::resolve_modules(&entry_file, effective_stdlib.as_deref(), &pkg_graph)?;

    // Check extern rust aliases don't collide with import aliases
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

    // Resolve extern rust crates
    let rust_artifacts = if program.extern_rust_crates.is_empty() {
        vec![]
    } else {
        rust_ffi::resolve_rust_crates(&program, entry_dir)?
    };
    rust_ffi::inject_extern_fns(&mut program, &rust_artifacts);

    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    spawn::desugar_spawn(&mut program)?;
    contracts::validate_contracts(&program)?;
    let (mut env, warnings) = typeck::type_check(&program)?;
    for w in &warnings {
        diagnostics::render_warning(&source, &entry_file.display().to_string(), w);
    }
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    xref::resolve_cross_refs(&mut program);
    let object_bytes = codegen::codegen(&program, &env, &source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    let mut config = LinkConfig::default_config(&obj_path)?;
    for artifact in &rust_artifacts {
        config.static_libs.push(artifact.static_lib.clone());
        config.flags.extend(artifact.native_libs.clone());
    }
    link_from_config(&config, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

/// Compile builtins.c once per process and cache the resulting .o path.
fn cached_runtime_object() -> Result<&'static Path, CompileError> {
    static CACHE: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    let result = CACHE.get_or_init(|| {
        (|| -> Result<PathBuf, CompileError> {
            let runtime_src = include_str!("../runtime/builtins.c");
            let dir = std::env::temp_dir().join(format!("pluto_runtime_{}", std::process::id()));
            std::fs::create_dir_all(&dir)
                .map_err(|e| CompileError::link(format!("failed to create runtime cache dir: {e}")))?;
            let runtime_c = dir.join("builtins.c");
            let runtime_o = dir.join("builtins.o");
            std::fs::write(&runtime_c, runtime_src)
                .map_err(|e| CompileError::link(format!("failed to write runtime source: {e}")))?;
            let mut cmd = std::process::Command::new("cc");
            cmd.arg("-c").arg(&runtime_c).arg("-o").arg(&runtime_o);
            #[cfg(target_os = "linux")]
            cmd.arg("-pthread");
            let status = cmd.status()
                .map_err(|e| CompileError::link(format!("failed to compile runtime: {e}")))?;
            let _ = std::fs::remove_file(&runtime_c);
            if !status.success() {
                return Err(CompileError::link("failed to compile runtime"));
            }
            Ok(runtime_o)
        })()
        .map_err(|e| e.to_string())
    });
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
        let mut flags = vec!["-lm".to_string()];
        #[cfg(target_os = "linux")]
        flags.push("-pthread".to_string());
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

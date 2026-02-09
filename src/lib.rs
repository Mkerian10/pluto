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

use diagnostics::CompileError;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Compile a source string to object bytes (lex → parse → prelude → typeck → monomorphize → closures → codegen).
/// No file I/O or linking. Useful for compile-fail tests that only need to check errors.
pub fn compile_to_object(source: &str) -> Result<Vec<u8>, CompileError> {
    let tokens = lexer::lex(source)?;
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    // Strip test functions in non-test mode
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    let mut env = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    codegen::codegen(&program, &env, source)
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
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    // test_info is NOT stripped in test mode
    let mut env = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
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
    let source = std::fs::read_to_string(entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    let graph = modules::resolve_modules(entry_file, effective_stdlib.as_deref())?;
    let (mut program, _source_map) = modules::flatten_modules(graph)?;
    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    // Strip test functions in non-test mode
    let test_fn_names: std::collections::HashSet<String> = program.test_info.iter()
        .map(|(_, fn_name)| fn_name.clone()).collect();
    program.functions.retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    let mut env = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    let object_bytes = codegen::codegen(&program, &env, &source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    link(&obj_path, output_path)?;

    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

/// Compile a file in test mode. Tests are preserved and a test runner main is generated.
pub fn compile_file_for_tests(entry_file: &Path, output_path: &Path, stdlib_root: Option<&Path>) -> Result<(), CompileError> {
    let source = std::fs::read_to_string(entry_file)
        .map_err(|e| CompileError::codegen(format!("failed to read entry file: {e}")))?;
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    let graph = modules::resolve_modules(entry_file, effective_stdlib.as_deref())?;
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

    prelude::inject_prelude(&mut program)?;
    ambient::desugar_ambient(&mut program)?;
    let mut env = typeck::type_check(&program)?;
    monomorphize::monomorphize(&mut program, &mut env)?;
    closures::lift_closures(&mut program, &mut env)?;
    let object_bytes = codegen::codegen(&program, &env, &source)?;

    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    link(&obj_path, output_path)?;

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
            let status = std::process::Command::new("cc")
                .arg("-c")
                .arg(&runtime_c)
                .arg("-o")
                .arg(&runtime_o)
                .status()
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

fn link(obj_path: &Path, output_path: &Path) -> Result<(), CompileError> {
    let runtime_o = cached_runtime_object()?;

    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .arg(runtime_o)
        .arg("-lm")
        .arg("-o")
        .arg(output_path)
        .status()
        .map_err(|e| CompileError::link(format!("failed to invoke linker: {e}")))?;

    if !status.success() {
        return Err(CompileError::link("linker failed"));
    }

    Ok(())
}

pub mod span;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod typeck;
pub mod codegen;
pub mod modules;
pub mod closures;
pub mod monomorphize;

use diagnostics::CompileError;
use std::path::{Path, PathBuf};

/// Compile a source string directly (single-file, no module resolution).
/// Used by tests and backward-compatible API.
pub fn compile(source: &str, output_path: &Path) -> Result<(), CompileError> {
    // 1. Lex
    let tokens = lexer::lex(source)?;

    // 2. Parse
    let mut parser = parser::Parser::new(&tokens, source);
    let mut program = parser.parse_program()?;

    // 3. Type check
    let mut env = typeck::type_check(&program)?;

    // 3b. Monomorphize generics
    monomorphize::monomorphize(&mut program, &mut env)?;

    // 3c. Lift closures
    closures::lift_closures(&mut program, &mut env)?;

    // 4. Codegen → object bytes
    let object_bytes = codegen::codegen(&program, &env)?;

    // 5. Write .o file
    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    // 6. Link
    link(&obj_path, output_path)?;

    // 7. Clean up .o file
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
    // Check PLUTO_STDLIB env var as fallback
    let env_stdlib = std::env::var("PLUTO_STDLIB").ok().map(PathBuf::from);
    let effective_stdlib = stdlib_root.map(|p| p.to_path_buf()).or(env_stdlib);

    // 1. Resolve modules
    let graph = modules::resolve_modules(entry_file, effective_stdlib.as_deref())?;

    // 2. Flatten
    let (mut program, _source_map) = modules::flatten_modules(graph)?;

    // 3. Type check
    let mut env = typeck::type_check(&program)?;

    // 3b. Monomorphize generics
    monomorphize::monomorphize(&mut program, &mut env)?;

    // 3c. Lift closures
    closures::lift_closures(&mut program, &mut env)?;

    // 4. Codegen → object bytes
    let object_bytes = codegen::codegen(&program, &env)?;

    // 5. Write .o file
    let obj_path = output_path.with_extension("o");
    std::fs::write(&obj_path, &object_bytes)
        .map_err(|e| CompileError::codegen(format!("failed to write object file: {e}")))?;

    // 6. Link
    link(&obj_path, output_path)?;

    // 7. Clean up .o file
    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}

fn link(obj_path: &Path, output_path: &Path) -> Result<(), CompileError> {
    // Compile the Pluto runtime (builtins.c)
    let runtime_src = include_str!("../runtime/builtins.c");
    let runtime_c = obj_path.with_file_name("pluto_runtime.c");
    let runtime_o = obj_path.with_file_name("pluto_runtime.o");
    std::fs::write(&runtime_c, runtime_src)
        .map_err(|e| CompileError::link(format!("failed to write runtime source: {e}")))?;

    let cc_status = std::process::Command::new("cc")
        .arg("-c")
        .arg(&runtime_c)
        .arg("-o")
        .arg(&runtime_o)
        .status()
        .map_err(|e| CompileError::link(format!("failed to compile runtime: {e}")))?;

    if !cc_status.success() {
        return Err(CompileError::link("failed to compile runtime"));
    }

    // Link user code + runtime
    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .arg(&runtime_o)
        .arg("-o")
        .arg(output_path)
        .status()
        .map_err(|e| CompileError::link(format!("failed to invoke linker: {e}")))?;

    // Clean up runtime temp files
    let _ = std::fs::remove_file(&runtime_c);
    let _ = std::fs::remove_file(&runtime_o);

    if !status.success() {
        return Err(CompileError::link("linker failed"));
    }

    Ok(())
}

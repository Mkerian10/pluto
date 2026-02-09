pub mod span;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod typeck;
pub mod codegen;
pub mod modules;

use diagnostics::CompileError;
use std::path::Path;

/// Compile a source string directly (single-file, no module resolution).
/// Used by tests and backward-compatible API.
pub fn compile(source: &str, output_path: &Path) -> Result<(), CompileError> {
    // 1. Lex
    let tokens = lexer::lex(source)?;

    // 2. Parse
    let mut parser = parser::Parser::new(&tokens, source);
    let program = parser.parse_program()?;

    // 3. Type check
    let env = typeck::type_check(&program)?;

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
pub fn compile_file(entry_file: &Path, output_path: &Path) -> Result<(), CompileError> {
    // 1. Resolve modules
    let graph = modules::resolve_modules(entry_file)?;

    // 2. Flatten
    let (program, _source_map) = modules::flatten_modules(graph)?;

    // 3. Type check
    let env = typeck::type_check(&program)?;

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

pub mod span;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod typeck;
pub mod codegen;

use diagnostics::CompileError;
use std::path::Path;

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

fn link(obj_path: &Path, output_path: &Path) -> Result<(), CompileError> {
    let status = std::process::Command::new("cc")
        .arg(obj_path)
        .arg("-o")
        .arg(output_path)
        .status()
        .map_err(|e| CompileError::link(format!("failed to invoke linker: {e}")))?;

    if !status.success() {
        return Err(CompileError::link("linker failed"));
    }

    Ok(())
}

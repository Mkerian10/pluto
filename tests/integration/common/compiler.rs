//! Fluent API for testing compiler pipeline stages.
//!
//! Provides programmatic access to each compiler stage (lex, parse, typecheck, codegen)
//! for testing and property-based testing.

use pluto::diagnostics::{CompileError, CompileWarning};
use pluto::lexer::lex;
use pluto::parser::ast::Program;
use pluto::typeck::{env::TypeEnv, type_check};
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Fluent API for testing the compiler pipeline.
///
/// # Example
/// ```ignore
/// let compiler = TestCompiler::new("fn main() { print(\"hello\") }");
/// let tokens = compiler.lex().unwrap();
/// let ast = compiler.parse().unwrap();
/// let output = compiler.run().unwrap();
/// assert_eq!(output.stdout, "hello\n");
/// ```
pub struct TestCompiler {
    source: String,
    stdlib_path: Option<PathBuf>,
}

/// Output from running a compiled Pluto program.
#[derive(Debug)]
pub struct TestOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl TestCompiler {
    /// Create a new compiler test from source code.
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            stdlib_path: None,
        }
    }

    /// Set an explicit stdlib path (overrides PLUTO_STDLIB env var).
    pub fn with_stdlib(mut self, path: PathBuf) -> Self {
        self.stdlib_path = Some(path);
        self
    }

    /// Run the lexer stage only.
    /// Returns the number of tokens (Token type is not exposed publicly).
    pub fn lex(&self) -> Result<usize, CompileError> {
        let tokens = lex(&self.source)?;
        Ok(tokens.len())
    }

    /// Run lex + parse stages.
    pub fn parse(&self) -> Result<Program, CompileError> {
        let tokens = lex(&self.source)?;
        let mut parser = pluto::parser::Parser::new(&tokens, &self.source);
        parser.parse_program()
    }

    /// Run lex + parse + typecheck stages.
    /// Returns TypeEnv and any warnings produced.
    pub fn typecheck(&self) -> Result<(TypeEnv, Vec<CompileWarning>), CompileError> {
        let tokens = lex(&self.source)?;
        let mut parser = pluto::parser::Parser::new(&tokens, &self.source);
        let program = parser.parse_program()?;

        // Note: This is a simplified typecheck that skips the full frontend pipeline.
        // For full testing including prelude, spawn desugaring, etc., use compile() or run().
        type_check(&program)
    }

    /// Run full compilation pipeline to produce object code.
    pub fn codegen(&self) -> Result<Vec<u8>, CompileError> {
        pluto::compile_to_object(&self.source)
    }

    /// Compile to an executable binary.
    /// Returns the path to the compiled binary (in a temp directory).
    pub fn compile(&self) -> Result<CompiledTestBinary, CompileError> {
        let dir = TempDir::new()
            .map_err(|e| CompileError::codegen(format!("failed to create temp dir: {e}")))?;
        let bin_path = dir.path().join("test_bin");

        pluto::compile(&self.source, &bin_path)?;

        Ok(CompiledTestBinary {
            _dir: dir,
            path: bin_path,
        })
    }

    /// Compile and run the program, capturing stdout/stderr/exit code.
    pub fn run(&self) -> Result<TestOutput, CompileError> {
        let binary = self.compile()?;
        Ok(binary.run())
    }

    /// Compile in test mode (preserves test functions, generates test runner).
    pub fn compile_test(&self) -> Result<CompiledTestBinary, CompileError> {
        let dir = TempDir::new()
            .map_err(|e| CompileError::codegen(format!("failed to create temp dir: {e}")))?;
        let bin_path = dir.path().join("test_bin");

        pluto::compile_test(&self.source, &bin_path)?;

        Ok(CompiledTestBinary {
            _dir: dir,
            path: bin_path,
        })
    }

    /// Compile in test mode and run.
    pub fn run_test(&self) -> Result<TestOutput, CompileError> {
        let binary = self.compile_test()?;
        Ok(binary.run())
    }
}

/// A compiled test binary in a temporary directory.
/// The tempdir is kept alive for the struct's lifetime.
pub struct CompiledTestBinary {
    _dir: TempDir,
    pub path: PathBuf,
}

impl CompiledTestBinary {
    /// Run the compiled binary and capture output.
    pub fn run(&self) -> TestOutput {
        let output = Command::new(&self.path)
            .output()
            .unwrap_or_else(|e| panic!("failed to run binary at {:?}: {}", self.path, e));

        TestOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }

    /// Run with custom environment variables.
    pub fn run_with_env(&self, envs: &[(&str, &str)]) -> TestOutput {
        let mut cmd = Command::new(&self.path);
        for (key, val) in envs {
            cmd.env(key, val);
        }

        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("failed to run binary at {:?}: {}", self.path, e));

        TestOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }
    }
}

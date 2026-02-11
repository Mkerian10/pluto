use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::diagnostics::CompileError;
use crate::modules::SourceMap;
use crate::parser::ast::Program;
use crate::typeck::env::TypeEnv;
use crate::{ambient, manifest, modules, prelude, spawn};

use super::line_index::LineIndex;

/// Result of running the check pipeline (no codegen).
pub struct AnalysisResult {
    pub program: Program,
    pub env: TypeEnv,
    pub source_map: SourceMap,
    /// LineIndex per file_id.
    pub line_indices: HashMap<u32, LineIndex>,
    /// Maps file_id to canonical file path.
    pub file_paths: HashMap<u32, PathBuf>,
}

/// Run lex → parse → module resolve → flatten → prelude → desugar → typeck.
/// Stops before monomorphize/codegen for speed.
/// Returns Ok(result) on success, Err(error) if the pipeline fails at any stage.
pub fn check_file(
    entry_file: &Path,
    stdlib_root: Option<&Path>,
) -> Result<AnalysisResult, AnalysisError> {
    let entry_file = entry_file.canonicalize().map_err(|e| {
        AnalysisError::compile(CompileError::codegen(format!(
            "could not resolve path '{}': {e}",
            entry_file.display()
        )))
    })?;

    let entry_dir = entry_file.parent().unwrap_or(Path::new("."));
    let pkg_graph = manifest::find_and_resolve(entry_dir).map_err(AnalysisError::compile)?;
    let graph =
        modules::resolve_modules(&entry_file, stdlib_root, &pkg_graph).map_err(AnalysisError::compile)?;

    let (mut program, source_map) = modules::flatten_modules(graph).map_err(AnalysisError::compile)?;

    // Build line indices and file path map from source_map
    let mut line_indices = HashMap::new();
    let mut file_paths = HashMap::new();
    for (file_id, (path, source)) in source_map.files.iter().enumerate() {
        let fid = file_id as u32;
        line_indices.insert(fid, LineIndex::new(source));
        file_paths.insert(fid, path.clone());
    }

    // Skip extern rust resolution — not needed for check-only
    // (would require building Rust crates)

    prelude::inject_prelude(&mut program).map_err(AnalysisError::compile)?;
    ambient::desugar_ambient(&mut program).map_err(AnalysisError::compile)?;
    spawn::desugar_spawn(&mut program).map_err(AnalysisError::compile)?;

    // Strip test functions (same as normal compilation)
    let test_fn_names: std::collections::HashSet<String> = program
        .test_info
        .iter()
        .map(|t| t.fn_name.clone())
        .collect();
    program
        .functions
        .retain(|f| !test_fn_names.contains(&f.node.name.node));
    program.test_info.clear();
    program.tests = None;

    let (env, _warnings) = crate::typeck::type_check(&program).map_err(AnalysisError::compile)?;

    Ok(AnalysisResult {
        program,
        env,
        source_map,
        line_indices,
        file_paths,
    })
}

/// Wraps CompileError with the source_map context needed to locate errors in files.
pub struct AnalysisError {
    pub error: CompileError,
}

impl AnalysisError {
    pub fn compile(error: CompileError) -> Self {
        Self { error }
    }
}

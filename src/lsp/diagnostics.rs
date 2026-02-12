use std::collections::HashMap;
use std::path::PathBuf;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Uri};

use crate::diagnostics::CompileError;

use super::line_index::LineIndex;
use super::path_to_uri;

/// Convert a CompileError into a list of (file_uri, Diagnostic) pairs.
pub fn compile_error_to_diagnostics(
    error: &CompileError,
    line_indices: &HashMap<u32, LineIndex>,
    file_paths: &HashMap<u32, PathBuf>,
) -> Vec<(Uri, Diagnostic)> {
    match error {
        CompileError::Syntax { msg, span } | CompileError::Type { msg, span } => {
            let file_id = span.file_id;
            let source_label = match error {
                CompileError::Syntax { .. } => "plutoc(syntax)",
                CompileError::Type { .. } => "plutoc(type)",
                _ => unreachable!(),
            };

            let (start, end) = if let Some(idx) = line_indices.get(&file_id) {
                (idx.offset_to_position(span.start), idx.offset_to_position(span.end))
            } else {
                (Position { line: 0, character: 0 }, Position { line: 0, character: 0 })
            };

            let uri = file_paths
                .get(&file_id)
                .map(|p| path_to_uri(p))
                .unwrap_or_else(|| "file:///unknown".parse().unwrap());

            let diag = Diagnostic {
                range: Range { start, end },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(source_label.to_string()),
                message: msg.clone(),
                ..Default::default()
            };

            vec![(uri, diag)]
        }
        CompileError::Codegen { msg } | CompileError::Link { msg } => {
            let uri = file_paths
                .get(&0)
                .map(|p| path_to_uri(p))
                .unwrap_or_else(|| "file:///unknown".parse().unwrap());

            let diag = Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("plutoc".to_string()),
                message: msg.clone(),
                ..Default::default()
            };

            vec![(uri, diag)]
        }
        CompileError::SiblingFile { source, .. } => {
            // Recursively convert the wrapped error
            compile_error_to_diagnostics(source, line_indices, file_paths)
        }
        CompileError::Manifest { msg, path } => {
            let uri = path_to_uri(path);

            let diag = Diagnostic {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("plutoc(manifest)".to_string()),
                message: msg.clone(),
                ..Default::default()
            };

            vec![(uri, diag)]
        }
    }
}

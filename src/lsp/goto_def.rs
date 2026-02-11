use std::collections::HashMap;

use lsp_types::{GotoDefinitionResponse, Location, Position, Range, Uri};

use crate::parser::ast::Program;
use crate::span::Span;

use super::analysis::AnalysisResult;
use super::{path_to_uri, uri_to_path};

/// Index of top-level declaration names → their definition spans.
pub struct DefinitionIndex {
    /// name → (file_id, span)
    pub defs: HashMap<String, (u32, Span)>,
}

impl DefinitionIndex {
    /// Walk the Program AST and index all top-level declarations.
    pub fn build(program: &Program) -> Self {
        let mut defs = HashMap::new();

        for f in &program.functions {
            defs.insert(f.node.name.node.clone(), (f.node.name.span.file_id, f.node.name.span));
        }

        for c in &program.classes {
            let fid = c.node.name.span.file_id;
            defs.insert(c.node.name.node.clone(), (fid, c.span));
            for m in &c.node.methods {
                let key = format!("{}.{}", c.node.name.node, m.node.name.node);
                defs.insert(key, (m.node.name.span.file_id, m.node.name.span));
            }
        }

        for t in &program.traits {
            defs.insert(t.node.name.node.clone(), (t.node.name.span.file_id, t.span));
        }

        for e in &program.enums {
            let fid = e.node.name.span.file_id;
            defs.insert(e.node.name.node.clone(), (fid, e.span));
            for v in &e.node.variants {
                let key = format!("{}.{}", e.node.name.node, v.name.node);
                defs.insert(key, (fid, v.name.span));
            }
        }

        for err in &program.errors {
            defs.insert(err.node.name.node.clone(), (err.node.name.span.file_id, err.span));
        }

        if let Some(app) = &program.app {
            defs.insert(app.node.name.node.clone(), (app.node.name.span.file_id, app.span));
        }

        for stage in &program.stages {
            defs.insert(stage.node.name.node.clone(), (stage.node.name.span.file_id, stage.span));
        }

        Self { defs }
    }
}

/// Extract the identifier word at the given byte offset in source.
fn word_at_offset(source: &str, offset: usize) -> Option<&str> {
    if offset >= source.len() {
        return None;
    }
    let bytes = source.as_bytes();
    if !is_ident_char(bytes[offset]) {
        return None;
    }
    let mut start = offset;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }
    Some(&source[start..end])
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Handle a textDocument/definition request.
pub fn handle_goto_definition(
    result: &AnalysisResult,
    file_uri: &Uri,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let file_path = uri_to_path(file_uri)?;
    let file_id = result
        .file_paths
        .iter()
        .find(|(_, p)| **p == file_path)
        .map(|(id, _)| *id)?;

    let (_, source) = result.source_map.files.get(file_id as usize)?;
    let line_index = result.line_indices.get(&file_id)?;
    let offset = line_index.position_to_offset(position);

    let word = word_at_offset(source, offset)?;

    let def_index = DefinitionIndex::build(&result.program);

    let (def_file_id, def_span) = def_index.defs.get(word)?;

    let def_line_index = result.line_indices.get(def_file_id)?;
    let def_path = result.file_paths.get(def_file_id)?;
    let def_uri = path_to_uri(def_path);

    let start = def_line_index.offset_to_position(def_span.start);
    let end = def_line_index.offset_to_position(def_span.end);

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: def_uri,
        range: Range { start, end },
    }))
}

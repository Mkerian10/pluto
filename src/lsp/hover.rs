use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Uri};

use crate::typeck::env::{ClassInfo, EnumInfo, ErrorInfo, FuncSig, TraitInfo};
use crate::typeck::types::PlutoType;

use super::analysis::AnalysisResult;
use super::uri_to_path;

/// Handle a textDocument/hover request.
pub fn handle_hover(
    result: &AnalysisResult,
    file_uri: &Uri,
    position: Position,
) -> Option<Hover> {
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

    // Try functions
    if let Some(sig) = result.env.functions.get(word) {
        return Some(make_hover(&format_function_hover(word, sig)));
    }

    // Try classes
    if let Some(info) = result.env.classes.get(word) {
        return Some(make_hover(&format_class_hover(word, info)));
    }

    // Try traits
    if let Some(info) = result.env.traits.get(word) {
        return Some(make_hover(&format_trait_hover(word, info)));
    }

    // Try enums
    if let Some(info) = result.env.enums.get(word) {
        return Some(make_hover(&format_enum_hover(word, info)));
    }

    // Try errors
    if let Some(info) = result.env.errors.get(word) {
        return Some(make_hover(&format_error_hover(word, info)));
    }

    None
}

fn make_hover(content: &str) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content.to_string(),
        }),
        range: None,
    }
}

fn format_function_hover(name: &str, sig: &FuncSig) -> String {
    let params: Vec<String> = sig.params.iter().map(|p| p.to_string()).collect();
    let ret = if sig.return_type == PlutoType::Void {
        String::new()
    } else {
        format!(" {}", sig.return_type)
    };
    format!("```pluto\nfn {}({}){}\n```", name, params.join(", "), ret)
}

fn format_class_hover(name: &str, info: &ClassInfo) -> String {
    let mut lines = vec![format!("class {} {{", name)];
    for (fname, ftype, _) in &info.fields {
        lines.push(format!("  {}: {}", fname, ftype));
    }
    lines.push("}".to_string());
    if !info.methods.is_empty() {
        lines.push(String::new());
        lines.push(format!("Methods: {}", info.methods.join(", ")));
    }
    if !info.impl_traits.is_empty() {
        lines.push(format!("Implements: {}", info.impl_traits.join(", ")));
    }
    format!("```pluto\n{}\n```", lines.join("\n"))
}

fn format_trait_hover(name: &str, info: &TraitInfo) -> String {
    let mut lines = vec![format!("trait {} {{", name)];
    for (mname, sig) in &info.methods {
        let params: Vec<String> = sig.params.iter().map(|p| p.to_string()).collect();
        let ret = if sig.return_type == PlutoType::Void {
            String::new()
        } else {
            format!(" {}", sig.return_type)
        };
        lines.push(format!("  fn {}({}){}", mname, params.join(", "), ret));
    }
    lines.push("}".to_string());
    format!("```pluto\n{}\n```", lines.join("\n"))
}

fn format_enum_hover(name: &str, info: &EnumInfo) -> String {
    let mut lines = vec![format!("enum {} {{", name)];
    for (vname, fields) in &info.variants {
        if fields.is_empty() {
            lines.push(format!("  {}", vname));
        } else {
            let fs: Vec<String> = fields.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
            lines.push(format!("  {} {{ {} }}", vname, fs.join(", ")));
        }
    }
    lines.push("}".to_string());
    format!("```pluto\n{}\n```", lines.join("\n"))
}

fn format_error_hover(name: &str, info: &ErrorInfo) -> String {
    let mut lines = vec![format!("error {} {{", name)];
    for (fname, ftype) in &info.fields {
        lines.push(format!("  {}: {}", fname, ftype));
    }
    lines.push("}".to_string());
    format!("```pluto\n{}\n```", lines.join("\n"))
}

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

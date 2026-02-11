use lsp_types::{DocumentSymbol, Range, SymbolKind};

use crate::parser::ast::Program;

use super::line_index::LineIndex;

/// Build document symbols (outline) for declarations in the given file_id.
#[allow(deprecated)] // DocumentSymbol::deprecated is deprecated but required by the type
pub fn document_symbols(
    program: &Program,
    file_id: u32,
    line_index: &LineIndex,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for f in &program.functions {
        if f.node.name.span.file_id != file_id {
            continue;
        }
        let name = &f.node.name.node;
        // Skip synthetic/internal functions
        if name.starts_with("__") {
            continue;
        }
        let range = span_to_range(&f.span, line_index);
        let selection_range = span_to_range(&f.node.name.span, line_index);
        symbols.push(DocumentSymbol {
            name: name.clone(),
            detail: Some(format_fn_detail(&f.node)),
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: None,
        });
    }

    for c in &program.classes {
        if c.node.name.span.file_id != file_id {
            continue;
        }
        let range = span_to_range(&c.span, line_index);
        let selection_range = span_to_range(&c.node.name.span, line_index);

        let mut children = Vec::new();
        for m in &c.node.methods {
            if m.node.name.node.starts_with("__") {
                continue;
            }
            let m_range = span_to_range(&m.span, line_index);
            let m_sel = span_to_range(&m.node.name.span, line_index);
            children.push(DocumentSymbol {
                name: m.node.name.node.clone(),
                detail: Some(format_fn_detail(&m.node)),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                range: m_range,
                selection_range: m_sel,
                children: None,
            });
        }

        symbols.push(DocumentSymbol {
            name: c.node.name.node.clone(),
            detail: None,
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    for t in &program.traits {
        if t.node.name.span.file_id != file_id {
            continue;
        }
        let range = span_to_range(&t.span, line_index);
        let selection_range = span_to_range(&t.node.name.span, line_index);
        symbols.push(DocumentSymbol {
            name: t.node.name.node.clone(),
            detail: None,
            kind: SymbolKind::INTERFACE,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: None,
        });
    }

    for e in &program.enums {
        if e.node.name.span.file_id != file_id {
            continue;
        }
        let range = span_to_range(&e.span, line_index);
        let selection_range = span_to_range(&e.node.name.span, line_index);

        let children: Vec<DocumentSymbol> = e
            .node
            .variants
            .iter()
            .map(|v| {
                let v_range = span_to_range(&v.name.span, line_index);
                DocumentSymbol {
                    name: v.name.node.clone(),
                    detail: None,
                    kind: SymbolKind::ENUM_MEMBER,
                    tags: None,
                    deprecated: None,
                    range: v_range,
                    selection_range: v_range,
                    children: None,
                }
            })
            .collect();

        symbols.push(DocumentSymbol {
            name: e.node.name.node.clone(),
            detail: None,
            kind: SymbolKind::ENUM,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    for err in &program.errors {
        if err.node.name.span.file_id != file_id {
            continue;
        }
        let range = span_to_range(&err.span, line_index);
        let selection_range = span_to_range(&err.node.name.span, line_index);
        symbols.push(DocumentSymbol {
            name: err.node.name.node.clone(),
            detail: None,
            kind: SymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: None,
        });
    }

    if let Some(app) = &program.app
        && app.node.name.span.file_id == file_id
    {
        let range = span_to_range(&app.span, line_index);
        let selection_range = span_to_range(&app.node.name.span, line_index);
        symbols.push(DocumentSymbol {
            name: app.node.name.node.clone(),
            detail: Some("app".to_string()),
            kind: SymbolKind::MODULE,
            tags: None,
            deprecated: None,
            range,
            selection_range,
            children: None,
        });
    }

    for stage in &program.stages {
        if stage.node.name.span.file_id == file_id {
            let range = span_to_range(&stage.span, line_index);
            let selection_range = span_to_range(&stage.node.name.span, line_index);
            symbols.push(DocumentSymbol {
                name: stage.node.name.node.clone(),
                detail: Some("stage".to_string()),
                kind: SymbolKind::MODULE,
                tags: None,
                deprecated: None,
                range,
                selection_range,
                children: None,
            });
        }
    }

    symbols
}

fn span_to_range(span: &crate::span::Span, line_index: &LineIndex) -> Range {
    Range {
        start: line_index.offset_to_position(span.start),
        end: line_index.offset_to_position(span.end),
    }
}

fn format_fn_detail(f: &crate::parser::ast::Function) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.node, format_type_expr(&p.ty.node)))
        .collect();
    let ret = f
        .return_type
        .as_ref()
        .map(|t| format!(" {}", format_type_expr(&t.node)))
        .unwrap_or_default();
    format!("fn({}){}", params.join(", "), ret)
}

fn format_type_expr(te: &crate::parser::ast::TypeExpr) -> String {
    use crate::parser::ast::TypeExpr;
    match te {
        TypeExpr::Named(name) => name.clone(),
        TypeExpr::Array(inner) => {
            format!("[{}]", format_type_expr(&inner.node))
        }
        TypeExpr::Fn { params, return_type } => {
            let ps: Vec<String> = params.iter().map(|p| format_type_expr(&p.node)).collect();
            let r = format_type_expr(&return_type.node);
            format!("fn({}) {}", ps.join(", "), r)
        }
        TypeExpr::Generic { name, type_args } => {
            let a: Vec<String> = type_args.iter().map(|a| format_type_expr(&a.node)).collect();
            format!("{}<{}>", name, a.join(", "))
        }
        TypeExpr::Qualified { module, name } => {
            format!("{}.{}", module, name)
        }
        TypeExpr::Nullable(inner) => {
            format!("{}?", format_type_expr(&inner.node))
        }
    }
}

use crate::span::Span;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Syntax error: {msg}")]
    Syntax { msg: String, span: Span },

    #[error("Type error: {msg}")]
    Type { msg: String, span: Span },

    #[error("Codegen error: {msg}")]
    Codegen { msg: String },

    #[error("Link error: {msg}")]
    Link { msg: String },

    #[error("Manifest error: {msg}")]
    Manifest { msg: String, path: PathBuf },

    /// Error from a sibling file that was auto-included during compilation
    #[error("{source}")]
    SiblingFile {
        path: PathBuf,
        source: Box<CompileError>
    },
}

impl CompileError {
    pub fn syntax(msg: impl Into<String>, span: Span) -> Self {
        Self::Syntax { msg: msg.into(), span }
    }

    pub fn type_err(msg: impl Into<String>, span: Span) -> Self {
        Self::Type { msg: msg.into(), span }
    }

    pub fn codegen(msg: impl Into<String>) -> Self {
        Self::Codegen { msg: msg.into() }
    }

    pub fn link(msg: impl Into<String>) -> Self {
        Self::Link { msg: msg.into() }
    }

    pub fn manifest(msg: impl Into<String>, path: PathBuf) -> Self {
        Self::Manifest { msg: msg.into(), path }
    }

    pub fn sibling_file(path: PathBuf, source: CompileError) -> Self {
        Self::SiblingFile { path, source: Box::new(source) }
    }
}

#[derive(Debug, Clone)]
pub struct CompileWarning {
    pub msg: String,
    pub span: Span,
    pub kind: WarningKind,
}

#[derive(Debug, Clone)]
pub enum WarningKind {
    UnusedVariable,
}

/// Render a CompileWarning with ariadne for nice terminal output (yellow).
pub fn render_warning(source: &str, _filename: &str, warning: &CompileWarning) {
    use ariadne::{Label, Report, ReportKind, Source};

    Report::build(ReportKind::Warning, (), warning.span.start)
        .with_message("warning")
        .with_label(
            Label::new(warning.span.start..warning.span.end)
                .with_message(&warning.msg),
        )
        .finish()
        .eprint(Source::from(source))
        .unwrap();
}

/// Render a CompileError with ariadne for nice terminal output.
pub fn render_error(source: &str, _filename: &str, err: &CompileError) {
    use ariadne::{Label, Report, ReportKind, Source};

    match err {
        CompileError::Syntax { msg, span } | CompileError::Type { msg, span } => {
            let kind_str = if matches!(err, CompileError::Syntax { .. }) { "syntax" } else { "type" };
            Report::build(ReportKind::Error, (), span.start)
                .with_message(format!("{kind_str} error"))
                .with_label(
                    Label::new(span.start..span.end)
                        .with_message(msg),
                )
                .finish()
                .eprint(Source::from(source))
                .unwrap();
        }
        CompileError::Codegen { msg } | CompileError::Link { msg } => {
            eprintln!("error: {msg}");
        }
        CompileError::Manifest { msg, path } => {
            eprintln!("error[manifest]: {msg}");
            eprintln!("  --> {}", path.display());
        }
        CompileError::SiblingFile { path, source } => {
            // Load the sibling file's source to render the error correctly
            if let Ok(sibling_source) = std::fs::read_to_string(path) {
                // Render the underlying error with the sibling file's source
                render_error(&sibling_source, &path.display().to_string(), source);
                eprintln!("note: this file was auto-included as a sibling of the entry point");
            } else {
                // Fallback if we can't read the sibling file
                eprintln!("error in sibling file: {}", path.display());
                eprintln!("{source}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span { start: 10, end: 20, file_id: 0 }
    }

    #[test]
    fn test_syntax_error_constructor() {
        let err = CompileError::syntax("unexpected token", dummy_span());
        match err {
            CompileError::Syntax { msg, span } => {
                assert_eq!(msg, "unexpected token");
                assert_eq!(span.start, 10);
                assert_eq!(span.end, 20);
            }
            _ => panic!("Expected Syntax error"),
        }
    }

    #[test]
    fn test_syntax_error_constructor_string_conversion() {
        let err = CompileError::syntax(String::from("test"), dummy_span());
        match err {
            CompileError::Syntax { msg, .. } => assert_eq!(msg, "test"),
            _ => panic!("Expected Syntax error"),
        }
    }

    #[test]
    fn test_type_error_constructor() {
        let err = CompileError::type_err("type mismatch", dummy_span());
        match err {
            CompileError::Type { msg, span } => {
                assert_eq!(msg, "type mismatch");
                assert_eq!(span.start, 10);
                assert_eq!(span.end, 20);
            }
            _ => panic!("Expected Type error"),
        }
    }

    #[test]
    fn test_codegen_error_constructor() {
        let err = CompileError::codegen("failed to generate IR");
        match err {
            CompileError::Codegen { msg } => {
                assert_eq!(msg, "failed to generate IR");
            }
            _ => panic!("Expected Codegen error"),
        }
    }

    #[test]
    fn test_link_error_constructor() {
        let err = CompileError::link("linker failed");
        match err {
            CompileError::Link { msg } => {
                assert_eq!(msg, "linker failed");
            }
            _ => panic!("Expected Link error"),
        }
    }

    #[test]
    fn test_manifest_error_constructor() {
        let path = PathBuf::from("/tmp/pluto.toml");
        let err = CompileError::manifest("invalid manifest", path.clone());
        match err {
            CompileError::Manifest { msg, path: p } => {
                assert_eq!(msg, "invalid manifest");
                assert_eq!(p, path);
            }
            _ => panic!("Expected Manifest error"),
        }
    }

    #[test]
    fn test_sibling_file_error_constructor() {
        let path = PathBuf::from("/tmp/sibling.pluto");
        let inner = CompileError::syntax("bad syntax", dummy_span());
        let err = CompileError::sibling_file(path.clone(), inner);
        match err {
            CompileError::SiblingFile { path: p, source } => {
                assert_eq!(p, path);
                assert!(matches!(*source, CompileError::Syntax { .. }));
            }
            _ => panic!("Expected SiblingFile error"),
        }
    }

    #[test]
    fn test_syntax_error_display() {
        let err = CompileError::syntax("oops", dummy_span());
        let display = format!("{}", err);
        assert!(display.contains("Syntax error"));
        assert!(display.contains("oops"));
    }

    #[test]
    fn test_type_error_display() {
        let err = CompileError::type_err("wrong type", dummy_span());
        let display = format!("{}", err);
        assert!(display.contains("Type error"));
        assert!(display.contains("wrong type"));
    }

    #[test]
    fn test_codegen_error_display() {
        let err = CompileError::codegen("IR generation failed");
        let display = format!("{}", err);
        assert!(display.contains("Codegen error"));
        assert!(display.contains("IR generation failed"));
    }

    #[test]
    fn test_link_error_display() {
        let err = CompileError::link("linking failed");
        let display = format!("{}", err);
        assert!(display.contains("Link error"));
        assert!(display.contains("linking failed"));
    }

    #[test]
    fn test_manifest_error_display() {
        let err = CompileError::manifest("bad toml", PathBuf::from("/tmp/pluto.toml"));
        let display = format!("{}", err);
        assert!(display.contains("Manifest error"));
        assert!(display.contains("bad toml"));
    }

    #[test]
    fn test_sibling_file_error_display() {
        let inner = CompileError::syntax("syntax error", dummy_span());
        let err = CompileError::sibling_file(PathBuf::from("/tmp/test.pluto"), inner);
        let display = format!("{}", err);
        assert!(display.contains("Syntax error"));
    }

    #[test]
    fn test_warning_kind_unused_variable() {
        let warning = CompileWarning {
            msg: "unused variable x".to_string(),
            span: dummy_span(),
            kind: WarningKind::UnusedVariable,
        };
        assert_eq!(warning.msg, "unused variable x");
        assert!(matches!(warning.kind, WarningKind::UnusedVariable));
    }

    #[test]
    fn test_warning_clone() {
        let warning = CompileWarning {
            msg: "test warning".to_string(),
            span: dummy_span(),
            kind: WarningKind::UnusedVariable,
        };
        let cloned = warning.clone();
        assert_eq!(cloned.msg, warning.msg);
        assert_eq!(cloned.span.start, warning.span.start);
    }

    #[test]
    fn test_compile_error_debug() {
        let err = CompileError::syntax("test", dummy_span());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Syntax"));
    }

    #[test]
    fn test_warning_debug() {
        let warning = CompileWarning {
            msg: "test".to_string(),
            span: dummy_span(),
            kind: WarningKind::UnusedVariable,
        };
        let debug = format!("{:?}", warning);
        assert!(debug.contains("CompileWarning"));
    }

    #[test]
    fn test_render_syntax_error() {
        let source = "fn main() { let x = 1 }";
        let err = CompileError::syntax("unexpected token", Span { start: 8, end: 12, file_id: 0 });
        // Just ensure it doesn't panic - output goes to stderr
        render_error(source, "test.pluto", &err);
    }

    #[test]
    fn test_render_type_error() {
        let source = "fn main() { let x: int = \"string\" }";
        let err = CompileError::type_err("type mismatch", Span { start: 25, end: 33, file_id: 0 });
        render_error(source, "test.pluto", &err);
    }

    #[test]
    fn test_render_codegen_error() {
        let source = "fn main() {}";
        let err = CompileError::codegen("failed to generate code");
        render_error(source, "test.pluto", &err);
    }

    #[test]
    fn test_render_link_error() {
        let source = "fn main() {}";
        let err = CompileError::link("linker error");
        render_error(source, "test.pluto", &err);
    }

    #[test]
    fn test_render_manifest_error() {
        let source = "";
        let err = CompileError::manifest("invalid manifest", PathBuf::from("/tmp/pluto.toml"));
        render_error(source, "", &err);
    }

    #[test]
    fn test_render_sibling_file_error_fallback() {
        let source = "fn main() {}";
        let inner = CompileError::syntax("syntax error", dummy_span());
        let err = CompileError::sibling_file(PathBuf::from("/nonexistent/file.pluto"), inner);
        // Fallback path when file doesn't exist
        render_error(source, "test.pluto", &err);
    }

    #[test]
    fn test_render_warning() {
        let source = "fn main() { let x = 1 }";
        let warning = CompileWarning {
            msg: "unused variable x".to_string(),
            span: Span { start: 16, end: 17, file_id: 0 },
            kind: WarningKind::UnusedVariable,
        };
        // Just ensure it doesn't panic
        render_warning(source, "test.pluto", &warning);
    }

    #[test]
    fn test_render_syntax_error_matches_detection() {
        let source = "fn main() {}";
        let err = CompileError::syntax("test", dummy_span());
        // Test that the matches! macro correctly identifies Syntax variant
        assert!(matches!(err, CompileError::Syntax { .. }));
        render_error(source, "test.pluto", &err);
    }
}

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

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
}

/// Render a CompileError with ariadne for nice terminal output.
pub fn render_error(source: &str, _filename: &str, err: &CompileError) {
    use ariadne::{Label, Report, ReportKind, Source};

    match err {
        CompileError::Syntax { msg, span } | CompileError::Type { msg, span } => {
            let kind_str = match err {
                CompileError::Syntax { .. } => "syntax",
                CompileError::Type { .. } => "type",
                _ => unreachable!(),
            };
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
    }
}

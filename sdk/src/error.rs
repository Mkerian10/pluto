#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Binary format error: {0}")]
    Binary(#[from] pluto::binary::BinaryError),
    #[error("Compile error: {0}")]
    Compile(#[from] pluto::diagnostics::CompileError),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Edit error: {0}")]
    Edit(String),
}

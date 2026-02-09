/// Byte-offset span in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub file_id: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end, file_id: 0 }
    }

    pub fn with_file(start: usize, end: usize, file_id: u32) -> Self {
        Self { start, end, file_id }
    }

    pub fn dummy() -> Self {
        Self { start: 0, end: 0, file_id: 0 }
    }
}

/// A value annotated with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

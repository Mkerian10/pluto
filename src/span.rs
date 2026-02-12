use serde::{Serialize, Deserialize};

/// Byte-offset span in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }

    pub fn dummy(node: T) -> Self {
        Self { node, span: Span::dummy() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Span tests =====

    #[test]
    fn test_span_new() {
        let span = Span::new(10, 20);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.file_id, 0);
    }

    #[test]
    fn test_span_with_file() {
        let span = Span::with_file(5, 15, 42);
        assert_eq!(span.start, 5);
        assert_eq!(span.end, 15);
        assert_eq!(span.file_id, 42);
    }

    #[test]
    fn test_span_dummy() {
        let span = Span::dummy();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
        assert_eq!(span.file_id, 0);
    }

    #[test]
    fn test_span_equality() {
        let span1 = Span::new(10, 20);
        let span2 = Span::new(10, 20);
        let span3 = Span::new(10, 21);
        assert_eq!(span1, span2);
        assert_ne!(span1, span3);
    }

    #[test]
    fn test_span_clone() {
        let span = Span::new(10, 20);
        let cloned = span.clone();
        assert_eq!(span, cloned);
    }

    #[test]
    fn test_span_copy() {
        let span = Span::new(10, 20);
        let copied = span; // Copy trait
        assert_eq!(span, copied);
        // Both should still be usable (proving Copy)
        assert_eq!(span.start, 10);
        assert_eq!(copied.start, 10);
    }

    #[test]
    fn test_span_debug() {
        let span = Span::new(10, 20);
        let debug_str = format!("{:?}", span);
        assert!(debug_str.contains("start"));
        assert!(debug_str.contains("10"));
        assert!(debug_str.contains("end"));
        assert!(debug_str.contains("20"));
    }

    #[test]
    fn test_span_with_different_file_ids() {
        let span1 = Span::with_file(10, 20, 1);
        let span2 = Span::with_file(10, 20, 2);
        assert_ne!(span1, span2);
    }

    // ===== Spanned tests =====

    #[test]
    fn test_spanned_new() {
        let span = Span::new(5, 10);
        let spanned = Spanned::new(42, span);
        assert_eq!(spanned.node, 42);
        assert_eq!(spanned.span, span);
    }

    #[test]
    fn test_spanned_dummy() {
        let spanned = Spanned::dummy("hello");
        assert_eq!(spanned.node, "hello");
        assert_eq!(spanned.span, Span::dummy());
    }

    #[test]
    fn test_spanned_with_string() {
        let span = Span::new(0, 5);
        let spanned = Spanned::new("test".to_string(), span);
        assert_eq!(spanned.node, "test");
        assert_eq!(spanned.span.start, 0);
        assert_eq!(spanned.span.end, 5);
    }

    #[test]
    fn test_spanned_equality() {
        let span = Span::new(10, 20);
        let spanned1 = Spanned::new(42, span);
        let spanned2 = Spanned::new(42, span);
        let spanned3 = Spanned::new(43, span);
        assert_eq!(spanned1, spanned2);
        assert_ne!(spanned1, spanned3);
    }

    #[test]
    fn test_spanned_clone() {
        let span = Span::new(10, 20);
        let spanned = Spanned::new(vec![1, 2, 3], span);
        let cloned = spanned.clone();
        assert_eq!(spanned, cloned);
    }

    #[test]
    fn test_spanned_debug() {
        let span = Span::new(10, 20);
        let spanned = Spanned::new(42, span);
        let debug_str = format!("{:?}", spanned);
        assert!(debug_str.contains("node"));
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("span"));
    }

    #[test]
    fn test_spanned_different_spans() {
        let span1 = Span::new(0, 5);
        let span2 = Span::new(5, 10);
        let spanned1 = Spanned::new(42, span1);
        let spanned2 = Spanned::new(42, span2);
        assert_ne!(spanned1, spanned2);
    }

    // ===== Serialization tests =====

    #[test]
    fn test_span_serialize() {
        let span = Span::new(10, 20);
        let json = serde_json::to_string(&span).unwrap();
        assert!(json.contains("10"));
        assert!(json.contains("20"));
    }

    #[test]
    fn test_span_deserialize() {
        let json = r#"{"start":10,"end":20,"file_id":0}"#;
        let span: Span = serde_json::from_str(json).unwrap();
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.file_id, 0);
    }

    #[test]
    fn test_span_roundtrip() {
        let span = Span::with_file(5, 15, 42);
        let json = serde_json::to_string(&span).unwrap();
        let deserialized: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(span, deserialized);
    }

    #[test]
    fn test_spanned_serialize() {
        let span = Span::new(10, 20);
        let spanned = Spanned::new(42, span);
        let json = serde_json::to_string(&spanned).unwrap();
        assert!(json.contains("node"));
        assert!(json.contains("42"));
        assert!(json.contains("span"));
    }

    #[test]
    fn test_spanned_deserialize() {
        let json = r#"{"node":42,"span":{"start":10,"end":20,"file_id":0}}"#;
        let spanned: Spanned<i32> = serde_json::from_str(json).unwrap();
        assert_eq!(spanned.node, 42);
        assert_eq!(spanned.span.start, 10);
        assert_eq!(spanned.span.end, 20);
    }

    #[test]
    fn test_spanned_roundtrip() {
        let span = Span::new(5, 10);
        let spanned = Spanned::new("test".to_string(), span);
        let json = serde_json::to_string(&spanned).unwrap();
        let deserialized: Spanned<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(spanned, deserialized);
    }

    #[test]
    fn test_spanned_with_complex_type() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct ComplexType {
            value: i32,
            name: String,
        }

        let span = Span::new(0, 10);
        let complex = ComplexType { value: 42, name: "test".to_string() };
        let spanned = Spanned::new(complex.clone(), span);

        assert_eq!(spanned.node, complex);
        assert_eq!(spanned.span, span);
    }

    // ===== Edge cases =====

    #[test]
    fn test_span_zero_length() {
        let span = Span::new(10, 10);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 10);
    }

    #[test]
    fn test_span_large_offsets() {
        let span = Span::new(1000000, 2000000);
        assert_eq!(span.start, 1000000);
        assert_eq!(span.end, 2000000);
    }

    #[test]
    fn test_span_large_file_id() {
        let span = Span::with_file(10, 20, u32::MAX);
        assert_eq!(span.file_id, u32::MAX);
    }

    #[test]
    fn test_multiple_spanned_with_same_span() {
        let span = Span::new(10, 20);
        let spanned1 = Spanned::new(1, span);
        let spanned2 = Spanned::new(2, span);
        assert_eq!(spanned1.span, spanned2.span);
        assert_ne!(spanned1.node, spanned2.node);
    }
}

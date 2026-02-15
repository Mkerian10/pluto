// Comprehensive lexer testing module
//
// This test suite systematically explores lexer edge cases based on
// industry research from Python, Rust, Go, JavaScript, and Ruby lexers.
//
// Categories:
// - whitespace: Token boundaries, newlines, tabs, EOF handling
// - numbers: Integer and float literals with edge cases
// - strings: String literals, escapes, interpolation
// - unicode: UTF-8 handling, emoji, BOM, invalid sequences
// - identifiers: Valid/invalid identifiers, keywords
// - comments: Line/block comments, nesting
// - operators: Multi-char operators, ambiguous sequences
// - errors: Error recovery, invalid tokens
// - spans: Position tracking accuracy
// - stress: Large inputs, boundary conditions

use pluto::lexer::{lex, token::Token};
use pluto::span::Span;

/// Lex source and expect success
pub fn lex_ok(source: &str) -> Vec<(Token, Span)> {
    let result = lex(source).expect("lexing should succeed");
    result.into_iter().map(|t| (t.node, t.span)).collect()
}

/// Lex source and expect failure
pub fn lex_fails(source: &str) {
    assert!(lex(source).is_err(), "lexing should fail for: {}", source);
}

/// Assert tokens match expected kinds (ignoring spans)
pub fn assert_tokens(source: &str, expected: &[Token]) {
    let tokens = lex_ok(source);
    let actual: Vec<Token> = tokens.iter().map(|(t, _)| t.clone()).collect();
    assert_eq!(
        actual, expected,
        "Token mismatch for source: {}\nExpected: {:?}\nActual: {:?}",
        source, expected, actual
    );
}

/// Assert specific token at index has expected span
pub fn assert_span(source: &str, token_idx: usize, start: usize, end: usize) {
    let tokens = lex_ok(source);
    assert!(
        token_idx < tokens.len(),
        "Token index {} out of bounds (len={})",
        token_idx,
        tokens.len()
    );
    let (_, span) = &tokens[token_idx];
    assert_eq!(
        *span,
        Span::new(start, end),
        "Span mismatch for token {} in source: {}\nExpected: {:?}\nActual: {:?}",
        token_idx,
        source,
        Span::new(start, end),
        span
    );
}

/// Get token count (useful for quick checks)
pub fn token_count(source: &str) -> usize {
    lex_ok(source).len()
}

mod whitespace;
mod numbers;
mod strings;
mod unicode;
mod identifiers;
mod comments;
mod operators;
mod errors;
mod spans;
mod stress;
mod real_world;
mod edge_cases;

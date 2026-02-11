//! Property-based tests for AST invariants.
//!
//! These tests use proptest to verify compiler invariants hold across
//! a wide variety of generated programs.

use proptest::prelude::*;
use plutoc::lexer::lex;

// Simple program generators - start with basic constructs
fn arb_simple_program() -> impl Strategy<Value = String> {
    prop::collection::vec(arb_statement(), 1..5).prop_map(|stmts| stmts.join("\n"))
}

fn arb_statement() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_let_statement(),
        arb_function_call(),
        arb_return_statement(),
    ]
}

fn arb_let_statement() -> impl Strategy<Value = String> {
    (arb_identifier(), arb_simple_expr()).prop_map(|(name, expr)| format!("let {name} = {expr}"))
}

fn arb_function_call() -> impl Strategy<Value = String> {
    arb_identifier().prop_map(|name| format!("{name}()"))
}

fn arb_return_statement() -> impl Strategy<Value = String> {
    arb_simple_expr().prop_map(|expr| format!("return {expr}"))
}

fn arb_simple_expr() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_int_lit(),
        arb_string_lit(),
        arb_bool_lit(),
        arb_identifier(),
    ]
}

fn arb_identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,10}".prop_map(|s| s.to_string())
}

fn arb_int_lit() -> impl Strategy<Value = String> {
    (0i64..1000).prop_map(|n| n.to_string())
}

fn arb_string_lit() -> impl Strategy<Value = String> {
    "[a-zA-Z ]{0,20}".prop_map(|s| format!("\"{s}\""))
}

fn arb_bool_lit() -> impl Strategy<Value = String> {
    prop_oneof![Just("true".to_string()), Just("false".to_string())]
}

proptest! {
    /// Property: Lexer spans are monotonic (non-overlapping and ordered)
    #[test]
    fn spans_are_monotonic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let spans: Vec<_> = tokens.iter().map(|t| t.span).collect();
            for window in spans.windows(2) {
                assert!(
                    window[0].end <= window[1].start,
                    "Spans not monotonic: {:?} followed by {:?} in source: {}",
                    window[0],
                    window[1],
                    source
                );
            }
        }
    }

    /// Property: Lex-parse roundtrip produces valid AST or error (no panics)
    #[test]
    fn lex_parse_no_panic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let mut parser = plutoc::parser::Parser::new(&tokens, &source);
            let _ = parser.parse_program();
            // Just verify it doesn't panic - result can be Ok or Err
        }
    }

    /// Property: All token spans are within source bounds
    #[test]
    fn spans_within_bounds(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let source_len = source.len();
            for token in &tokens {
                assert!(
                    token.span.start <= source_len,
                    "Span start {} exceeds source length {} in: {}",
                    token.span.start,
                    source_len,
                    source
                );
                assert!(
                    token.span.end <= source_len,
                    "Span end {} exceeds source length {} in: {}",
                    token.span.end,
                    source_len,
                    source
                );
            }
        }
    }

    /// Property: Lexer is deterministic (same input produces same output)
    #[test]
    fn lexer_deterministic(source in arb_simple_program()) {
        let result1 = lex(&source);
        let result2 = lex(&source);

        match (result1, result2) {
            (Ok(tokens1), Ok(tokens2)) => {
                assert_eq!(tokens1.len(), tokens2.len(), "Token count differs for: {}", source);
                for (t1, t2) in tokens1.iter().zip(tokens2.iter()) {
                    // Compare token types (not Debug repr, which may be unstable)
                    assert_eq!(
                        std::mem::discriminant(&t1.node),
                        std::mem::discriminant(&t2.node),
                        "Token type differs for: {}",
                        source
                    );
                    assert_eq!(t1.span, t2.span, "Span differs for: {}", source);
                }
            }
            (Err(_), Err(_)) => {
                // Both errored - that's fine, just verify it's consistent
            }
            _ => panic!("Lexer non-deterministic: one succeeded, one failed for: {}", source),
        }
    }

    /// Property: Parser is deterministic (same tokens produce same result)
    #[test]
    fn parser_deterministic(source in arb_simple_program()) {
        if let Ok(tokens) = lex(&source) {
            let mut parser1 = plutoc::parser::Parser::new(&tokens, &source);
            let mut parser2 = plutoc::parser::Parser::new(&tokens, &source);

            let result1 = parser1.parse_program();
            let result2 = parser2.parse_program();

            match (result1, result2) {
                (Ok(_ast1), Ok(_ast2)) => {
                    // Both succeeded - verify both are Ok (full structural equality hard without PartialEq)
                }
                (Err(e1), Err(e2)) => {
                    // Both errored - verify error messages match
                    assert_eq!(
                        e1.to_string(),
                        e2.to_string(),
                        "Parser error non-deterministic for: {}",
                        source
                    );
                }
                _ => panic!("Parser non-deterministic: one succeeded, one failed for: {}", source),
            }
        }
    }
}

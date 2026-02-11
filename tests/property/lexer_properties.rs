// Property-Based Testing for Pluto Lexer
//
// Property-based testing validates INVARIANTS (properties that should ALWAYS hold)
// across randomly generated inputs, rather than testing specific examples.
//
// This file demonstrates:
// 1. Basic properties (safety, determinism)
// 2. Structural properties (span consistency, token structure)
// 3. Semantic properties (round-tripping, valid syntax)
// 4. Custom generators (producing valid Pluto syntax)
// 5. Shrinking (finding minimal failing cases)

use proptest::prelude::*;
use plutoc::lexer::{lex, token::Token};

// =============================================================================
// SECTION 1: BASIC PROPERTIES - Safety & Determinism
// =============================================================================

/// Property: Lexer NEVER panics, even on garbage input
///
/// This tests robustness - the lexer should handle ANY string gracefully,
/// returning either Ok (valid tokens) or Err (syntax error), but NEVER panic.
///
/// Generator: "\\PC*" = any Unicode string (0-1000 chars)
/// Runs: 100 random test cases by default
#[test]
fn prop_lexer_never_panics() {
    proptest!(|(source in "\\PC{0,1000}")| {
        // The act of calling lex() should never panic
        let _result = lex(&source);
        // If we get here, no panic occurred ✓
    });
}

/// Property: Lexing is deterministic
///
/// The same input should ALWAYS produce the same output.
/// This catches issues like:
/// - Using uninitialized memory
/// - Depending on global state
/// - Non-deterministic algorithms
///
/// Generator: any UTF-8 string up to 500 chars
#[test]
fn prop_lexing_is_deterministic() {
    proptest!(|(source in "\\PC{0,500}")| {
        let result1 = lex(&source);
        let result2 = lex(&source);

        // Both should succeed or both should fail
        assert_eq!(result1.is_ok(), result2.is_ok());

        // If successful, token streams should be identical
        if let (Ok(tokens1), Ok(tokens2)) = (result1, result2) {
            assert_eq!(tokens1.len(), tokens2.len());
            for (t1, t2) in tokens1.iter().zip(tokens2.iter()) {
                assert_eq!(t1.node, t2.node);
                assert_eq!(t1.span, t2.span);
            }
        }
    });
}

/// Property: Empty input produces empty token stream
///
/// Edge case that should always work - lexing empty string should succeed
/// with zero tokens (no EOF token or similar).
#[test]
fn prop_empty_input_is_valid() {
    let result = lex("");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

// =============================================================================
// SECTION 2: STRUCTURAL PROPERTIES - Spans & Token Structure
// =============================================================================

/// Property: Token spans NEVER overlap
///
/// If tokens are adjacent, token[i].end <= token[i+1].start
/// This ensures source positions are well-formed.
///
/// Generator: Any valid UTF-8 string
#[test]
fn prop_spans_never_overlap() {
    proptest!(|(source in "\\PC{0,500}")| {
        if let Ok(tokens) = lex(&source) {
            for i in 0..tokens.len().saturating_sub(1) {
                let current_end = tokens[i].span.end;
                let next_start = tokens[i+1].span.start;

                prop_assert!(
                    current_end <= next_start,
                    "Overlapping spans: token {} ends at {}, token {} starts at {}",
                    i, current_end, i+1, next_start
                );
            }
        }
    });
}

/// Property: All spans are within source bounds
///
/// Every token span should point to valid positions in the source string.
/// span.start < span.end and span.end <= source.len()
#[test]
fn prop_spans_within_bounds() {
    proptest!(|(source in "\\PC{0,500}")| {
        if let Ok(tokens) = lex(&source) {
            for (i, token) in tokens.iter().enumerate() {
                prop_assert!(
                    token.span.start < token.span.end,
                    "Token {} has invalid span: start ({}) >= end ({})",
                    i, token.span.start, token.span.end
                );

                prop_assert!(
                    token.span.end <= source.len(),
                    "Token {} span.end ({}) exceeds source length ({})",
                    i, token.span.end, source.len()
                );
            }
        }
    });
}

/// Property: Spans are byte-aligned (UTF-8 safety)
///
/// In UTF-8, character boundaries must be at valid byte positions.
/// This ensures we never split a multi-byte character.
#[test]
fn prop_spans_are_utf8_aligned() {
    proptest!(|(source in "\\PC{0,500}")| {
        if let Ok(tokens) = lex(&source) {
            for (i, token) in tokens.iter().enumerate() {
                // Check that we can safely slice at these positions
                prop_assert!(
                    source.is_char_boundary(token.span.start),
                    "Token {} start ({}) is not a UTF-8 char boundary",
                    i, token.span.start
                );

                prop_assert!(
                    source.is_char_boundary(token.span.end),
                    "Token {} end ({}) is not a UTF-8 char boundary",
                    i, token.span.end
                );
            }
        }
    });
}

/// Property: Concatenating all token slices reproduces original source
///
/// This is the "round-trip" property - if we extract each token's text
/// from the source using its span, concatenating them should give us
/// back the original source (for successful lexing).
///
/// Note: This only holds if there are no skipped characters (like filtered
/// whitespace in some lexers, but Pluto includes all significant whitespace).
#[test]
fn prop_round_trip_via_spans() {
    proptest!(|(source in "\\PC{0,300}")| {
        if let Ok(tokens) = lex(&source) {
            if tokens.is_empty() {
                // Empty tokens means source had only skipped content (comments)
                // or was truly empty
                return Ok(());
            }

            // Reconstruct source from token spans
            let mut reconstructed = String::new();
            let mut pos = 0;

            for token in &tokens {
                // Add any gap between tokens (whitespace/comments)
                if token.span.start > pos {
                    reconstructed.push_str(&source[pos..token.span.start]);
                }
                // Add the token itself
                reconstructed.push_str(&source[token.span.start..token.span.end]);
                pos = token.span.end;
            }

            // Add any remaining source after last token
            if pos < source.len() {
                reconstructed.push_str(&source[pos..]);
            }

            prop_assert_eq!(reconstructed, source);
        }
    });
}

// =============================================================================
// SECTION 3: SEMANTIC PROPERTIES - Valid Syntax Always Works
// =============================================================================

/// Custom generator: Valid decimal integers
///
/// Strategy: Generate non-negative i64 values, convert to string
/// Note: Negative numbers are handled at parse-time (lexer treats '-' as operator)
/// Shrinking: proptest will automatically try smaller values on failure
fn valid_integers() -> impl Strategy<Value = String> {
    (0i64..=i64::MAX).prop_map(|n| n.to_string())
}

/// Property: All valid integers lex successfully
#[test]
fn prop_valid_integers_always_lex() {
    proptest!(|(int_str in valid_integers())| {
        let result = lex(&int_str);
        prop_assert!(result.is_ok(), "Failed to lex integer: {}", int_str);

        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1, "Integer should produce exactly 1 token");

        // Check it's actually an IntLit token
        prop_assert!(
            matches!(tokens[0].node, Token::IntLit(_)),
            "Expected IntLit, got {:?}", tokens[0].node
        );
    });
}

/// Custom generator: Valid hex literals
///
/// Strategy: Generate i64 (positive only), format as 0xHEX
fn valid_hex_literals() -> impl Strategy<Value = String> {
    (0i64..=i64::MAX).prop_map(|n| format!("0x{:X}", n))
}

/// Property: All valid hex literals lex successfully
#[test]
fn prop_valid_hex_always_lexes() {
    proptest!(|(hex_str in valid_hex_literals())| {
        let result = lex(&hex_str);
        prop_assert!(result.is_ok(), "Failed to lex hex: {}", hex_str);

        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        prop_assert!(matches!(tokens[0].node, Token::IntLit(_)));
    });
}

/// Custom generator: Valid float literals
///
/// Strategy: Generate non-negative f64 values (finite only), convert to string
/// Note: Negative numbers are handled at parse-time (lexer treats '-' as operator)
fn valid_floats() -> impl Strategy<Value = String> {
    (0.0f64..=f64::MAX)
        .prop_filter("must be finite", |f| f.is_finite())
        .prop_map(|f| {
            let s = f.to_string();
            // Ensure it has a decimal point (Pluto requires digits on both sides)
            if s.contains('.') {
                s
            } else {
                format!("{}.0", s)
            }
        })
}

/// Property: All valid floats lex successfully
#[test]
fn prop_valid_floats_always_lex() {
    proptest!(|(float_str in valid_floats())| {
        let result = lex(&float_str);
        prop_assert!(result.is_ok(), "Failed to lex float: {}", float_str);

        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        prop_assert!(matches!(tokens[0].node, Token::FloatLit(_)));
    });
}

/// Custom generator: Valid identifiers
///
/// Strategy: Start with [a-zA-Z_], followed by [a-zA-Z0-9_]*
fn valid_identifiers() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z_][a-zA-Z0-9_]{0,50}")
        .expect("valid regex")
        .prop_filter("not a keyword", |s| {
            !is_pluto_keyword(s)
        })
}

/// Helper: Check if string is a Pluto keyword
fn is_pluto_keyword(s: &str) -> bool {
    matches!(s,
        "fn" | "let" | "mut" | "return" | "if" | "else" | "while" | "true" | "false" |
        "class" | "trait" | "app" | "error" | "raise" | "catch" | "spawn" | "enum" |
        "impl" | "self" | "pub" | "for" | "in" | "break" | "continue" | "match" |
        "import" | "as" | "extern" | "uses" | "ambient" | "test" | "invariant" |
        "requires" | "ensures" | "select" | "default" | "scope" | "scoped" |
        "transient" | "none" | "system" | "stage" | "override" | "yield" | "stream"
    )
}

/// Property: All valid identifiers lex successfully
#[test]
fn prop_valid_identifiers_always_lex() {
    proptest!(|(ident in valid_identifiers())| {
        let result = lex(&ident);
        prop_assert!(result.is_ok(), "Failed to lex identifier: {}", ident);

        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        prop_assert!(matches!(tokens[0].node, Token::Ident));
    });
}

/// Custom generator: Valid string literals
///
/// Strategy: Generate printable ASCII (avoiding special cases), wrap in quotes
fn valid_strings() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ,.:;!?()\\[\\]{}+=*/-]{0,100}")
        .expect("valid regex")
        .prop_map(|s| format!("\"{}\"", s))
}

/// Property: All valid strings lex successfully
#[test]
fn prop_valid_strings_always_lex() {
    proptest!(|(str_lit in valid_strings())| {
        let result = lex(&str_lit);
        prop_assert!(result.is_ok(), "Failed to lex string: {}", str_lit);

        let tokens = result.unwrap();
        prop_assert_eq!(tokens.len(), 1);
        prop_assert!(matches!(tokens[0].node, Token::StringLit(_)));
    });
}

// =============================================================================
// SECTION 4: COMPLEX GENERATORS - Valid Pluto Expressions
// =============================================================================

/// Custom generator: Simple arithmetic expressions
///
/// This demonstrates recursive generation - expressions can contain
/// other expressions. We limit depth to prevent infinite recursion.
fn arith_expr(depth: u32) -> impl Strategy<Value = String> {
    let leaf = prop::strategy::Union::new(vec![
        valid_integers().boxed(),
        valid_floats().boxed(),
        valid_identifiers().boxed(),
    ]);

    leaf.prop_recursive(
        depth,  // max depth
        256,    // max total nodes
        10,     // items per collection
        move |inner| {
            prop::strategy::Union::new(vec![
                // Binary operations
                (inner.clone(), prop::sample::select(vec!["+", "-", "*", "/"]), inner.clone())
                    .prop_map(|(l, op, r)| format!("{} {} {}", l, op, r))
                    .boxed(),
                // Parenthesized
                inner.clone()
                    .prop_map(|e| format!("({})", e))
                    .boxed(),
            ])
        }
    )
}

/// Property: Valid arithmetic expressions lex successfully
#[test]
fn prop_arith_expressions_lex() {
    proptest!(|(expr in arith_expr(3))| {
        let result = lex(&expr);
        prop_assert!(result.is_ok(), "Failed to lex expression: {}", expr);

        // Should produce at least one token
        let tokens = result.unwrap();
        prop_assert!(tokens.len() > 0, "Expression should produce at least one token");

        // All tokens should be valid (no error tokens)
        // The lexer doesn't have an explicit Error token type, so if lex() succeeded,
        // all tokens are valid by definition
    });
}

/// Custom generator: Variable declarations
///
/// Generates: let <ident> = <expr>
fn var_declaration() -> impl Strategy<Value = String> {
    (valid_identifiers(), arith_expr(2))
        .prop_map(|(name, expr)| format!("let {} = {}", name, expr))
}

/// Property: Variable declarations lex successfully
#[test]
fn prop_var_declarations_lex() {
    proptest!(|(decl in var_declaration())| {
        let result = lex(&decl);
        prop_assert!(result.is_ok(), "Failed to lex declaration: {}", decl);

        let tokens = result.unwrap();
        // Should have: let, ident, =, expr_tokens
        prop_assert!(tokens.len() >= 3);
        prop_assert!(matches!(tokens[0].node, Token::Let));
    });
}

/// Custom generator: Function signatures
///
/// Generates: fn <name>() <ret_type>
fn function_signature() -> impl Strategy<Value = String> {
    (valid_identifiers(), prop::sample::select(vec!["int", "float", "bool", "string"]))
        .prop_map(|(name, ret_type)| format!("fn {}() {}", name, ret_type))
}

/// Property: Function signatures lex successfully
#[test]
fn prop_function_signatures_lex() {
    proptest!(|(sig in function_signature())| {
        let result = lex(&sig);
        prop_assert!(result.is_ok(), "Failed to lex function: {}", sig);

        let tokens = result.unwrap();
        prop_assert!(tokens.len() >= 5); // fn, name, (, ), type
        prop_assert!(matches!(tokens[0].node, Token::Fn));
    });
}

// =============================================================================
// SECTION 5: NEGATIVE PROPERTIES - Invalid Input Should Fail
// =============================================================================

/// Custom generator: Invalid hex literals (with non-hex digits)
fn invalid_hex_literals() -> impl Strategy<Value = String> {
    prop::string::string_regex("0x[G-Z]{1,5}")
        .expect("valid regex")
}

/// Property: Invalid hex literals should fail to lex
#[test]
fn prop_invalid_hex_fails() {
    proptest!(|(bad_hex in invalid_hex_literals())| {
        let result = lex(&bad_hex);
        prop_assert!(result.is_err(), "Should reject invalid hex: {}", bad_hex);
    });
}

/// Custom generator: Multiple decimal points
fn multiple_decimal_points() -> impl Strategy<Value = String> {
    (1u32..=5u32)
        .prop_map(|n| {
            let mut s = String::from("1");
            for _ in 0..n {
                s.push_str(".0");
            }
            s
        })
}

/// Property: Multiple decimal points should fail
#[test]
fn prop_multiple_decimals_fail() {
    proptest!(|(bad_float in multiple_decimal_points())| {
        // Only fail if it's actually multiple decimals (not just "1.0")
        if bad_float.matches('.').count() > 1 {
            let result = lex(&bad_float);
            prop_assert!(result.is_err(), "Should reject multiple decimals: {}", bad_float);
        }
    });
}

/// Custom generator: Unterminated strings
fn unterminated_strings() -> impl Strategy<Value = String> {
    prop::string::string_regex("\"[a-zA-Z0-9 ]{1,20}")
        .expect("valid regex")
}

/// Property: Unterminated strings should fail
#[test]
fn prop_unterminated_strings_fail() {
    proptest!(|(bad_str in unterminated_strings())| {
        let result = lex(&bad_str);
        prop_assert!(result.is_err(), "Should reject unterminated string: {}", bad_str);
    });
}

// =============================================================================
// SECTION 6: STATEFUL PROPERTIES - Token Stream Consistency
// =============================================================================

/// Property: Total span coverage
///
/// The first token should start at 0 (or later if there's leading whitespace),
/// and the last token should end at or before source.len().
#[test]
fn prop_spans_cover_source() {
    proptest!(|(source in "\\PC{1,500}")| {
        if let Ok(tokens) = lex(&source) {
            if !tokens.is_empty() {
                let first_start = tokens[0].span.start;
                let last_end = tokens[tokens.len() - 1].span.end;

                prop_assert!(first_start <= source.len());
                prop_assert!(last_end <= source.len());

                // If source starts with non-whitespace, first token should start at 0
                if !source.starts_with(char::is_whitespace) && !source.starts_with("//") {
                    prop_assert_eq!(first_start, 0, "First token should start at 0");
                }
            }
        }
    });
}

/// Property: No duplicate spans
///
/// No two tokens should have exactly the same span (would indicate
/// lexer emitting duplicate tokens).
#[test]
fn prop_no_duplicate_spans() {
    proptest!(|(source in "\\PC{0,500}")| {
        if let Ok(tokens) = lex(&source) {
            for i in 0..tokens.len() {
                for j in (i+1)..tokens.len() {
                    prop_assert!(
                        tokens[i].span != tokens[j].span,
                        "Duplicate spans at indices {} and {}: {:?}",
                        i, j, tokens[i].span
                    );
                }
            }
        }
    });
}

/// Property: Newlines are preserved or skipped consistently
///
/// If the source contains newlines, either:
/// - They produce Newline tokens, OR
/// - They're consistently skipped (in comments, etc.)
#[test]
fn prop_newline_handling_consistent() {
    proptest!(|(source in "\\PC{0,500}")| {
        if let Ok(tokens) = lex(&source) {
            let newline_count = source.matches('\n').count();

            if newline_count > 0 {
                let newline_tokens = tokens.iter()
                    .filter(|t| matches!(t.node, Token::Newline))
                    .count();

                // Either we have newline tokens, or all newlines are in comments
                let comment_newlines = source.matches("//").count();

                prop_assert!(
                    newline_tokens > 0 || comment_newlines > 0 || newline_count == source.split("//").count() - 1,
                    "Newlines should either produce tokens or be in comments"
                );
            }
        }
    });
}

// =============================================================================
// SECTION 7: SHRINKING DEMONSTRATION
// =============================================================================

/// This test is designed to FAIL on purpose to demonstrate shrinking.
/// When it fails, proptest will find the MINIMAL failing input.
///
/// Uncomment to see shrinking in action!
/*
#[test]
fn prop_demonstrate_shrinking() {
    proptest!(|(s in "\\PC{10,100}")| {
        // Fail if string contains "bug"
        prop_assert!(
            !s.contains("bug"),
            "Found the word 'bug' in: {}", s
        );
    });
}
*/

// =============================================================================
// SECTION 8: PERFORMANCE PROPERTIES
// =============================================================================

/// Property: Lexing time is bounded
///
/// This is a regression test - lexing should be fast even for large inputs.
/// We don't have strict timing bounds, but we can verify it completes.
#[test]
fn prop_lexing_completes_in_reasonable_time() {
    proptest!(|(source in "\\PC{0,10000}")| {
        use std::time::{Duration, Instant};

        let start = Instant::now();
        let _result = lex(&source);
        let elapsed = start.elapsed();

        // Lexing 10K chars should take < 1 second
        prop_assert!(
            elapsed < Duration::from_secs(1),
            "Lexing took {:?} for {} chars", elapsed, source.len()
        );
    });
}

/// Property: Token count is proportional to source size
///
/// Very rough heuristic: token count should be at most source.len()
/// (one token per character is the maximum density).
#[test]
fn prop_token_count_bounded() {
    proptest!(|(source in "\\PC{0,1000}")| {
        if let Ok(tokens) = lex(&source) {
            prop_assert!(
                tokens.len() <= source.len(),
                "Token count ({}) exceeds source length ({})",
                tokens.len(), source.len()
            );
        }
    });
}

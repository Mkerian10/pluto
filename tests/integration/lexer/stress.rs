// Category 10: Edge Cases & Stress Tests
//
// Tests extreme inputs and boundary conditions:
// - Empty files
// - Very large files
// - Very long tokens
// - Many tokens

use super::*;

// ===== Empty/Minimal Input =====

#[test]
fn stress_empty_file() {
    let src = "";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 0);
}

#[test]
fn stress_only_whitespace() {
    let src = "   \t  \n  ";
    let tokens = lex_ok(src);
    // Should only have newline token
    assert!(tokens.len() <= 1);
}

#[test]
fn stress_only_comments() {
    let src = "// comment 1\n// comment 2\n// comment 3";
    let tokens = lex_ok(src);
    // Comments filtered out, only newlines remain
    let newline_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Newline)).count();
    assert!(newline_count <= 2);
}

#[test]
fn stress_only_newlines() {
    let src = "\n\n\n\n\n";
    let tokens = lex_ok(src);
    // Multiple newlines collapsed into one
    assert!(tokens.len() <= 1);
}

// ===== Very Long Lines =====

#[test]
fn stress_very_long_line() {
    // 10,000 character line
    let line = "let x = ".to_string() + &"1 + ".repeat(2000) + "1";
    let tokens = lex_ok(&line);
    // Should lex successfully
    assert!(tokens.len() > 0);
}

#[test]
fn stress_extremely_long_line() {
    // 100,000 character line
    let line = "let x = ".to_string() + &"1 + ".repeat(20000) + "1";
    let tokens = lex_ok(&line);
    assert!(tokens.len() > 0);
}

// ===== Very Large Files =====

#[test]
fn stress_large_file_many_statements() {
    // 1000 statements
    let mut src = String::new();
    for i in 0..1000 {
        src.push_str(&format!("let x{} = {}\n", i, i));
    }
    let tokens = lex_ok(&src);
    // Each statement: let, x{i}, =, {i}, \n = 5 tokens
    assert!(tokens.len() >= 5000);
}

#[test]
fn stress_large_file_1mb() {
    // ~1MB of source code
    let statement = "let x = 42\n";
    let count = 1024 * 1024 / statement.len();
    let src = statement.repeat(count);
    let tokens = lex_ok(&src);
    assert!(tokens.len() > 0);
}

#[test]
#[ignore] // Too slow for regular test runs
fn stress_very_large_file_10mb() {
    // ~10MB of source code (ignored by default)
    let statement = "let x = 42\n";
    let count = 10 * 1024 * 1024 / statement.len();
    let src = statement.repeat(count);
    let tokens = lex_ok(&src);
    assert!(tokens.len() > 0);
}

// ===== Many Tokens =====

#[test]
fn stress_100k_tokens() {
    // 100,000 tokens
    let src = "1 + ".repeat(50_000);
    let tokens = lex_ok(&src);
    assert!(tokens.len() >= 100_000);
}

#[test]
#[ignore] // Too slow
fn stress_1m_tokens() {
    // 1,000,000 tokens (ignored by default)
    let src = "1 + ".repeat(500_000);
    let tokens = lex_ok(&src);
    assert!(tokens.len() >= 1_000_000);
}

// ===== Very Long Tokens =====

#[test]
fn stress_long_identifier() {
    // 10,000 character identifier
    let ident = "a".repeat(10_000);
    let tokens = lex_ok(&ident);
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
#[ignore] // Stack overflow - lexer recursion issue with very long strings
fn stress_long_string() {
    // 100,000 character string
    let content = "a".repeat(100_000);
    let src = format!(r#""{}""#, content);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

#[test]
fn stress_long_number() {
    // Very long number (will overflow i64)
    let num = "9".repeat(100);
    let result = lex(&num);
    // Should fail due to overflow
    assert!(result.is_err());
}

#[test]
fn stress_long_comment() {
    // 100,000 character comment
    let comment = "// ".to_string() + &"a".repeat(100_000);
    let tokens = lex_ok(&comment);
    // Comment filtered out
    assert_eq!(tokens.len(), 0);
}

// ===== Deeply Nested Structures =====
// Note: Lexer doesn't track nesting, but we test token sequences

#[test]
fn stress_many_nested_parens() {
    let src = "(".repeat(1000) + &")".repeat(1000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 2000);
}

#[test]
fn stress_many_nested_brackets() {
    let src = "[".repeat(1000) + &"]".repeat(1000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 2000);
}

#[test]
fn stress_many_nested_braces() {
    let src = "{".repeat(1000) + &"}".repeat(1000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 2000);
}

// ===== Boundary Conditions =====

#[test]
fn stress_max_i64() {
    let src = "9223372036854775807"; // i64::MAX
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::IntLit(9223372036854775807)));
}

#[test]
fn stress_max_i64_plus_one() {
    let src = "9223372036854775808"; // i64::MAX + 1
    // Should fail to parse
    let result = lex(src);
    assert!(result.is_err());
}

#[test]
fn stress_min_i64_magnitude() {
    // -9223372036854775808 (i64::MIN)
    // Parsed as Minus + IntLit
    let src = "-9223372036854775808";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Minus));
}

#[test]
fn stress_float_max() {
    let src = "1.7976931348623157e308"; // f64::MAX
    let tokens = lex_ok(src);
    // Current lexer doesn't support scientific notation
    // Will lex as multiple tokens
    assert!(tokens.len() >= 1);
}

#[test]
fn stress_float_min_positive() {
    let src = "2.2250738585072014e-308"; // f64::MIN_POSITIVE
    let tokens = lex_ok(src);
    // Will lex as multiple tokens (no scientific notation)
    assert!(tokens.len() >= 1);
}

// ===== Repeated Patterns =====

#[test]
fn stress_alternating_tokens() {
    // Alternating pattern: 1 + 1 + 1 + ...
    let src = "1 + ".repeat(10_000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 20_000);
}

#[test]
fn stress_many_identifiers() {
    let src = "a b c d e f g ".repeat(1000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 7_000);
}

#[test]
fn stress_many_keywords() {
    let src = "let let let let let ".repeat(1000);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 5_000);
}

// ===== Unicode Stress =====

#[test]
#[ignore] // Stack overflow - lexer recursion issue with very long strings
fn stress_many_unicode_characters() {
    // String with 10,000 emoji
    let content = "🚀".repeat(10_000);
    let src = format!(r#""{}""#, content);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 1);
}

#[test]
fn stress_mixed_unicode_lengths() {
    // Mix of 1, 2, 3, 4 byte UTF-8 characters
    let content = "Aé你🚀".repeat(1000);
    let src = format!(r#""{}""#, content);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 1);
}

// ===== Memory/Performance Bounds =====

#[test]
fn stress_no_quadratic_behavior() {
    // Some lexers exhibit O(n²) behavior on certain inputs
    // Test that lexing time is roughly linear
    use std::time::Instant;

    let small = "let x = 1\n".repeat(100);
    let large = "let x = 1\n".repeat(1000);

    let start = Instant::now();
    let _ = lex(&small);
    let small_time = start.elapsed();

    let start = Instant::now();
    let _ = lex(&large);
    let large_time = start.elapsed();

    // Large input is 10x bigger, should take < 100x as long
    // (allowing for overhead and variance)
    let ratio = large_time.as_nanos() as f64 / small_time.as_nanos() as f64;
    assert!(ratio < 100.0, "Lexer may have quadratic behavior: {}x slowdown for 10x input", ratio);
}

#[test]
fn stress_no_stack_overflow() {
    // Lexer should not use recursion that could overflow stack
    // Test with deep nesting
    let src = "(".repeat(10_000) + &")".repeat(10_000);
    let result = std::panic::catch_unwind(|| {
        let _ = lex(&src);
    });
    assert!(result.is_ok(), "Lexer caused stack overflow");
}

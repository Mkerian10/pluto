// Category 9: Position & Span Tracking
//
// Tests accuracy of source position tracking:
// - Span boundaries
// - Line/column tracking
// - Multi-byte character handling

use super::*;

// ===== Basic Span Accuracy =====

#[test]
fn span_single_character_token() {
    let src = "+";
    assert_span(src, 0, 0, 1);
}

#[test]
fn span_two_character_token() {
    let src = "==";
    assert_span(src, 0, 0, 2);
}

#[test]
fn span_three_character_token() {
    let src = "..=";
    assert_span(src, 0, 0, 3);
}

#[test]
fn span_identifier() {
    let src = "hello";
    assert_span(src, 0, 0, 5);
}

#[test]
fn span_keyword() {
    let src = "let";
    assert_span(src, 0, 0, 3);
}

#[test]
fn span_integer() {
    let src = "123";
    assert_span(src, 0, 0, 3);
}

#[test]
fn span_float() {
    let src = "3.14";
    assert_span(src, 0, 0, 4);
}

#[test]
fn span_string() {
    let src = r#""hello""#;
    // Span should include quotes
    assert_span(src, 0, 0, 7);
}

// ===== Multiple Tokens =====

#[test]
fn span_two_tokens() {
    let src = "let x";
    // let: 0..3
    // x: 4..5
    assert_span(src, 0, 0, 3); // let
    assert_span(src, 1, 4, 5); // x
}

#[test]
fn span_expression() {
    let src = "1 + 2";
    // 1: 0..1
    // +: 2..3
    // 2: 4..5
    assert_span(src, 0, 0, 1); // 1
    assert_span(src, 1, 2, 3); // +
    assert_span(src, 2, 4, 5); // 2
}

#[test]
fn span_no_spaces() {
    let src = "1+2";
    // 1: 0..1
    // +: 1..2
    // 2: 2..3
    assert_span(src, 0, 0, 1);
    assert_span(src, 1, 1, 2);
    assert_span(src, 2, 2, 3);
}

// ===== Newlines =====

#[test]
fn span_newline() {
    let src = "let x\n";
    let tokens = lex_ok(src);
    // let: 0..3, x: 4..5, \n: 5..6
    assert_eq!(tokens.len(), 3);
    let newline_span = tokens[2].1;
    assert!(newline_span.start == 5 && newline_span.end >= 6);
}

#[test]
fn span_multiple_newlines() {
    let src = "x\n\n\ny";
    let tokens = lex_ok(src);
    // x: 0..1, \n\n\n: 1..4, y: 4..5
    assert_span(src, 0, 0, 1); // x
    assert_span(src, 2, 4, 5); // y
}

#[test]
fn span_tokens_across_lines() {
    let src = "let x = 1\nlet y = 2";
    // let: 0..3, x: 4..5, =: 6..7, 1: 8..9, \n: 9..10
    // let: 10..13, y: 14..15, =: 16..17, 2: 18..19
    assert_span(src, 0, 0, 3);  // let
    assert_span(src, 1, 4, 5);  // x
    assert_span(src, 2, 6, 7);  // =
    assert_span(src, 3, 8, 9);  // 1
    assert_span(src, 5, 10, 13); // let
}

// ===== Tabs =====

#[test]
fn span_with_tabs() {
    let src = "let\tx";
    // let: 0..3, \t is skipped (not a token), x: 4..5
    assert_span(src, 0, 0, 3); // let
    assert_span(src, 1, 4, 5); // x
}

#[test]
fn span_multiple_tabs() {
    let src = "let\t\tx";
    assert_span(src, 0, 0, 3);
    assert_span(src, 1, 5, 6); // x after two tabs
}

// ===== Unicode =====

#[test]
fn span_string_with_ascii() {
    let src = r#""abc""#;
    // Span: 0..5 (including quotes)
    assert_span(src, 0, 0, 5);
}

#[test]
fn span_string_with_two_byte_utf8() {
    let src = r#""é""#;
    // "é" is 4 bytes: " (1) + é (2) + " (1) = 4
    // Span should be 0..4
    assert_span(src, 0, 0, 4);
}

#[test]
fn span_string_with_three_byte_utf8() {
    let src = r#""你""#;
    // "你" is 5 bytes: " (1) + 你 (3) + " (1) = 5
    assert_span(src, 0, 0, 5);
}

#[test]
fn span_string_with_four_byte_utf8() {
    let src = r#""🚀""#;
    // "🚀" is 6 bytes: " (1) + 🚀 (4) + " (1) = 6
    assert_span(src, 0, 0, 6);
}

#[test]
fn span_identifier_after_unicode_string() {
    let src = r#""🚀" x"#;
    // "🚀": 0..6
    // x: 7..8
    assert_span(src, 0, 0, 6); // "🚀"
    assert_span(src, 1, 7, 8); // x
}

#[test]
fn span_mixed_unicode_string() {
    let src = r#""Aé你🚀""#;
    // A (1) + é (2) + 你 (3) + 🚀 (4) = 10, plus quotes = 12
    assert_span(src, 0, 0, 12);
}

// ===== String Escapes =====

#[test]
fn span_string_with_escape() {
    let src = r#""hello\nworld""#;
    // \n is two characters in source: \ and n
    // Length: " + hello + \n + world + " = 14
    assert_span(src, 0, 0, 14);
}

#[test]
fn span_string_with_multiple_escapes() {
    let src = r#""a\nb\tc""#;
    // a + \n + b + \t + c = 9, plus quotes = 11
    assert_span(src, 0, 0, 11);
}

// ===== Comments =====

#[test]
fn span_after_comment() {
    let src = "x // comment\ny";
    // x: 0..1, \n: 12..13, y: 13..14
    // Comment is filtered out
    assert_span(src, 0, 0, 1); // x
    assert_span(src, 2, 13, 14); // y
}

// ===== Edge Cases =====

#[test]
fn span_at_eof() {
    let src = "x";
    assert_span(src, 0, 0, 1);
}

#[test]
fn span_hex_number() {
    let src = "0xFF";
    assert_span(src, 0, 0, 4);
}

#[test]
fn span_hex_with_underscores() {
    let src = "0xDE_AD";
    // Span includes underscores
    assert_span(src, 0, 0, 7);
}

#[test]
fn span_float_with_underscores() {
    let src = "1_000.5_0";
    assert_span(src, 0, 0, 9);
}

#[test]
fn span_very_long_token() {
    let ident = "a".repeat(1000);
    let tokens = lex_ok(&ident);
    assert_eq!(tokens[0].1.start, 0);
    assert_eq!(tokens[0].1.end, 1000);
}

// ===== CRLF Handling =====

#[test]
fn span_crlf_newlines() {
    let src = "x\r\ny";
    // x: 0..1, \r\n: 1..3, y: 3..4
    let tokens = lex_ok(src);
    assert_span(src, 0, 0, 1); // x
    // Newline token might include \r
    assert_span(src, 2, 3, 4); // y
}

// ===== Empty Input =====

#[test]
fn span_empty_input() {
    let src = "";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 0);
}

#[test]
fn span_only_whitespace() {
    let src = "   ";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 0);
}

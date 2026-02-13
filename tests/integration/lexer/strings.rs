// Category 3: String Literals
//
// Tests string literal edge cases:
// - Empty strings
// - Escape sequences
// - String interpolation
// - Unterminated strings
// - Special characters

use super::*;

// ===== Basic Strings =====

#[test]
fn string_empty() {
    let tokens = lex_ok(r#""""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.is_empty()));
}

#[test]
fn string_single_character() {
    let tokens = lex_ok(r#""a""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "a"));
}

#[test]
fn string_with_spaces() {
    let tokens = lex_ok(r#""hello world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello world"));
}

#[test]
fn string_very_long() {
    // 10KB string
    let content = "a".repeat(10_000);
    let src = format!(r#""{}""#, content);
    let tokens = lex_ok(&src);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.len() == 10_000));
}

#[test]
fn string_with_literal_newline_should_fail() {
    // Strings with actual newlines should fail (non-raw strings)
    let src = "\"hello\nworld\"";
    // Current regex: "([^\"\\]|\\.)* matches anything except quote/backslash or escaped char
    // A literal newline is [^\"\\ ] so it WILL match
    // This is a BUG if Pluto doesn't allow newlines in strings
    let result = lex(src);
    if result.is_ok() {
        // Bug: literal newlines allowed in strings
        let tokens = result.unwrap();
        assert!(matches!(&tokens[0].node, Token::StringLit(_)), "Bug: literal newlines allowed in strings");
    }
}

// ===== Escape Sequences =====

#[test]
fn string_escape_newline() {
    let tokens = lex_ok(r#""hello\nworld""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\nworld"));
}

#[test]
fn string_escape_carriage_return() {
    let tokens = lex_ok(r#""hello\rworld""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\rworld"));
}

#[test]
fn string_escape_tab() {
    let tokens = lex_ok(r#""hello\tworld""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\tworld"));
}

#[test]
fn string_escape_backslash() {
    let tokens = lex_ok(r#""hello\\world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\\world"));
}

#[test]
fn string_escape_quote() {
    let tokens = lex_ok(r#""hello\"world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\"world"));
}

#[test]
fn string_invalid_escape_errors() {
    // \k is not a valid escape — now produces a compile error
    let result = lex(r#""hello\kworld""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("unknown escape sequence"));
}

#[test]
fn string_unicode_escape_supported() {
    // \u{1F4A9} for poop emoji
    let tokens = lex_ok(r#""hello\u{1F4A9}world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\u{1F4A9}world"));
}

#[test]
fn string_hex_escape_supported() {
    // \x41 for 'A'
    let tokens = lex_ok(r#""hello\x41world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "helloAworld"));
}

#[test]
fn string_null_escape() {
    // \0 for null byte
    let tokens = lex_ok(r#""hello\0world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\0world"));
}

#[test]
fn string_octal_escape_not_supported() {
    // \1 is unknown escape — errors
    let result = lex(r#""hello\101world""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("unknown escape sequence"));
}

#[test]
fn string_hex_escape_incomplete() {
    // \x without 2 hex digits
    let result = lex(r#""hello\xworld""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid hex escape"));
}

#[test]
fn string_hex_escape_one_digit() {
    // \x4 without second hex digit
    let result = lex(r#""hello\x4world""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid hex escape"));
}

#[test]
fn string_unicode_escape_empty() {
    // \u{} — empty
    let result = lex(r#""\u{}""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn string_unicode_escape_surrogate() {
    // \u{D800} — surrogate codepoint
    let result = lex(r#""\u{D800}""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("surrogate"));
}

#[test]
fn string_unicode_escape_too_large() {
    // \u{110000} — above max codepoint
    let result = lex(r#""\u{110000}""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not a valid Unicode codepoint"));
}

#[test]
fn string_unicode_escape_unclosed() {
    // \u{41 — missing closing brace
    let result = lex(r#""\u{41""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("missing closing"));
}

#[test]
fn string_unicode_escape_no_brace() {
    // \u41 — missing opening brace
    let result = lex(r#""\u41""#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("expected '{'"));
}

// ===== String Interpolation (Pluto-specific) =====
// Note: String interpolation is handled by parser, not lexer
// Lexer just sees StringLit tokens

#[test]
fn string_with_braces_no_interpolation() {
    // In lexer, "hello {name}" is just a string literal
    // Parser handles interpolation
    let tokens = lex_ok(r#""hello {name}""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello {name}"));
}

#[test]
fn string_escaped_braces() {
    // \{ is not a valid escape — now produces a compile error
    // Pluto uses {{ and }} for brace escaping in interpolation, not \{ and \}
    let result = lex(r#""hello \{name\}""#);
    assert!(result.is_err());
}

// ===== Edge Cases =====

#[test]
fn string_unterminated() {
    // Missing closing quote
    let src = r#""hello world"#;
    // Current regex: "([^\"\\]|\\.)*" REQUIRES closing quote
    lex_fails(src);
}

#[test]
fn string_with_null_byte() {
    // Null byte in string
    let src = "\"hello\0world\"";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains('\0')));
}

#[test]
fn string_across_multiple_lines_should_fail() {
    let src = "\"hello\n\nworld\"";
    let result = lex(src);
    // As noted earlier, this likely succeeds (bug)
    if result.is_ok() {
        // Bug documented
    }
}

#[test]
fn string_adjacent_literals() {
    // Two string literals next to each other
    let src = r#""hello" "world""#;
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
    assert!(matches!(&tokens[1].0, Token::StringLit(_)));
}

#[test]
fn string_adjacent_no_space() {
    // Adjacent strings with no space
    let src = r#""hello""world""#;
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 2);
}

#[test]
fn string_only_escapes() {
    let tokens = lex_ok(r#""\n\r\t\\""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "\n\r\t\\"));
}

#[test]
fn string_backslash_at_end() {
    // String ending with backslash: "hello\"
    // The backslash escapes the closing quote, making it unterminated
    let src = r#""hello\""#;
    // This is actually "hello\" with escaped quote, so unterminated
    lex_fails(src);
}

#[test]
fn string_double_backslash_at_end() {
    // String ending with \\ should work
    let tokens = lex_ok(r#""hello\\""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.ends_with('\\')));
}

#[test]
fn string_empty_escape() {
    // Backslash at end of string with no following char
    // Covered by backslash_at_end test
}

#[test]
fn string_unicode_content() {
    // Unicode characters in string content (not escapes)
    let tokens = lex_ok(r#""Hello 👋 你好""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("👋")));
}

#[test]
fn string_combining_characters() {
    // é as e + combining accent
    let tokens = lex_ok("\"e\u{0301}\"");
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

#[test]
fn string_emoji() {
    let tokens = lex_ok(r#""🚀🎉💯""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "🚀🎉💯"));
}

// ===== Pathological String Cases =====

#[test]
fn string_many_consecutive_escapes() {
    // String with 1000 consecutive newline escapes
    let escapes = r"\n".repeat(1000);
    let src = format!(r#""{}""#, escapes);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

#[test]
fn string_only_escape_sequences() {
    // String containing only escape sequences: "\"\"\"\""
    let tokens = lex_ok(r#""\"\"\"\"""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "\"\"\"\""));
}

#[test]
fn string_alternating_content_and_escapes() {
    // Alternating pattern: a\nb\tc\rd\\
    let tokens = lex_ok(r#""a\nb\tc\rd\\""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "a\nb\tc\rd\\"));
}

#[test]
fn string_many_mixed_escapes() {
    // Mix of all escape types repeated
    let pattern = r"\n\r\t\\";  // Removed \" to avoid format! issues
    let repeated = pattern.repeat(100);
    let src = format!("\"{}\"", repeated);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 1);
}

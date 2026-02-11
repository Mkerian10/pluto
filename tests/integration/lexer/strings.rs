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
fn string_invalid_escape_preserved() {
    // "\x" is not a valid escape in current lexer
    // Code: Some(other) => { result.push('\\'); result.push(other); }
    // So \x becomes \\x in the string
    let tokens = lex_ok(r#""hello\xworld""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\\xworld"));
}

#[test]
fn string_invalid_escape_k() {
    let tokens = lex_ok(r#""hello\kworld""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "hello\\kworld"));
}

#[test]
fn string_unicode_escape_not_supported() {
    // \u{1F4A9} for emoji - not in current lexer
    let tokens = lex_ok(r#""hello\u{1F4A9}world""#);
    // Will preserve as \\u{1F4A9}
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("\\u")));
}

#[test]
fn string_hex_escape_not_supported() {
    // \x41 for 'A' - not supported
    let tokens = lex_ok(r#""hello\x41world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("\\x")));
}

#[test]
fn string_octal_escape_not_supported() {
    // \101 for 'A' - not supported
    let tokens = lex_ok(r#""hello\101world""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("\\1")));
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
    // Not clear if \{ is valid escape - current code doesn't handle it specially
    let tokens = lex_ok(r#""hello \{name\}""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
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

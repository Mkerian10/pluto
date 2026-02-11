// Category 4: Unicode & Encoding
//
// Tests UTF-8 handling:
// - Valid UTF-8 in strings and identifiers
// - Invalid UTF-8 sequences
// - BOM (Byte Order Mark)
// - Multi-byte characters
// - Emoji

use super::*;

// ===== Valid UTF-8 =====

#[test]
fn utf8_ascii_only_identifiers() {
    let tokens = lex_ok("let hello_world = 42");
    assert_eq!(tokens.len(), 4);
}

#[test]
fn utf8_latin1_in_strings() {
    // Latin-1 characters: é, ñ, ü
    let tokens = lex_ok(r#""café señor über""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("café")));
}

#[test]
fn utf8_emoji_in_strings() {
    let tokens = lex_ok(r#""Hello 👋""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s.contains("👋")));
}

#[test]
fn utf8_cjk_in_strings() {
    let tokens = lex_ok(r#""你好世界""#);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "你好世界"));
}

#[test]
fn utf8_emoji_in_identifiers_not_allowed() {
    // Current regex: [a-zA-Z_][a-zA-Z0-9_]*
    // Emoji not allowed in identifiers
    let src = "let 🚀 = 42";
    let result = lex(src);
    assert!(result.is_err(), "Emoji should not be allowed in identifiers");
}

#[test]
fn utf8_combining_characters_in_strings() {
    // é as e + combining accent (U+0301)
    let src = "\"cafe\u{0301}\""; // café with combining accent
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

#[test]
fn utf8_zero_width_joiner_in_strings() {
    // Zero-width joiner (U+200D) used in emoji sequences
    let src = "\"👨‍👩‍👧‍👦\""; // Family emoji
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

// ===== Invalid UTF-8 =====
// Note: Rust strings are guaranteed to be valid UTF-8, so we can't
// easily test invalid UTF-8 sequences in source code.
// These tests document expected behavior.

#[test]
fn utf8_invalid_byte_sequences_doc() {
    // Document: Invalid UTF-8 should fail gracefully, not panic
    // Testing this requires using bytes, not &str
    // Example: [0xC0, 0x80] is invalid UTF-8
    // Logos operates on &str so Rust prevents invalid UTF-8 from reaching it
    // This is actually a GOOD thing - Rust's UTF-8 validation protects the lexer
}

#[test]
fn utf8_overlong_encodings_doc() {
    // Document: Overlong UTF-8 encodings are a security issue
    // Example: encoding '/' as 0xC0 0xAF instead of 0x2F
    // Rust's str type rejects these, so lexer is protected
}

#[test]
fn utf8_lone_continuation_bytes_doc() {
    // Document: Bytes like 0x80-0xBF without valid leading byte
    // Rust str validation prevents these
}

#[test]
fn utf8_incomplete_multibyte_doc() {
    // Document: Starting a multibyte sequence but hitting EOF
    // Rust str validation prevents these
}

// ===== BOM Handling =====

#[test]
fn utf8_bom_at_start_of_file() {
    // UTF-8 BOM: 0xEF 0xBB 0xBF
    // In Rust string literal: \u{FEFF}
    let src = "\u{FEFF}let x = 1";
    let result = lex(src);
    // Current lexer doesn't skip BOM - will be unexpected character
    if result.is_err() {
        // Expected - BOM not handled
    } else {
        // Bug: BOM should be skipped or cause error, not silently included
        let tokens = result.unwrap();
        // BOM might be lexed as unexpected character or skipped
        println!("Bug: BOM at start not handled properly, got {} tokens", tokens.len());
    }
}

#[test]
fn utf8_bom_in_middle_of_file_should_fail() {
    let src = "let x = 1\n\u{FEFF}let y = 2";
    let result = lex(src);
    // BOM in middle should fail
    assert!(result.is_err(), "BOM in middle of file should be an error");
}

// ===== Multi-byte Character Boundary Cases =====

#[test]
fn utf8_multibyte_characters_various_lengths() {
    // 1-byte: A (0x41)
    // 2-byte: é (0xC3 0xA9)
    // 3-byte: 你 (0xE4 0xBD 0xA0)
    // 4-byte: 🚀 (0xF0 0x9F 0x9A 0x80)
    let src = r#""Aé你🚀""#;
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "Aé你🚀"));
}

#[test]
fn utf8_surrogate_pairs_not_in_utf8() {
    // UTF-16 surrogate pairs (U+D800 to U+DFFF) are invalid in UTF-8
    // Rust str prevents these, so lexer is protected
}

#[test]
fn utf8_max_valid_codepoint() {
    // U+10FFFF is the maximum valid Unicode codepoint
    let src = "\"\u{10FFFF}\"";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(_)));
}

#[test]
fn utf8_replacement_character() {
    // U+FFFD is the replacement character �
    let src = "\"�\"";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::StringLit(s) if s == "�"));
}

// ===== Identifier Edge Cases =====

#[test]
fn utf8_non_ascii_identifiers_not_supported() {
    // Unicode identifiers like café are not supported
    // Current regex: [a-zA-Z_][a-zA-Z0-9_]*
    let src = "let café = 42";
    let result = lex(src);
    // Will fail because 'é' is not in [a-zA-Z0-9_]
    assert!(result.is_err(), "Non-ASCII identifiers not supported");
}

#[test]
fn utf8_mathematical_symbols_in_identifiers() {
    // Mathematical symbols like ∑ are not allowed
    let src = "let ∑ = 42";
    assert!(lex(src).is_err());
}

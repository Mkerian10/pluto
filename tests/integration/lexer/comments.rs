// Category 6: Comments
//
// Tests comment handling:
// - Line comments
// - Block comments (if supported)
// - Edge cases

use super::*;

// ===== Line Comments =====

#[test]
fn comment_basic_line_comment() {
    let src = "let x = 1 // this is a comment";
    let tokens = lex_ok(src);
    // Comments should be skipped, not in token stream
    assert!(!tokens.iter().any(|(t, _)| matches!(t, Token::Comment)));
    assert_eq!(tokens.len(), 4); // let x = 1
}

#[test]
fn comment_line_comment_to_end_of_line() {
    let src = "let x = 1 // comment\nlet y = 2";
    let tokens = lex_ok(src);
    // Should have: let x = 1 \n let y = 2
    let has_comment = tokens.iter().any(|(t, _)| matches!(t, Token::Comment));
    assert!(!has_comment, "Comments should be filtered out");
}

#[test]
fn comment_empty_line_comment() {
    let src = "let x = 1 //\nlet y = 2";
    let tokens = lex_ok(src);
    assert!(!tokens.iter().any(|(t, _)| matches!(t, Token::Comment)));
}

#[test]
fn comment_only_slashes() {
    let src = "//";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 0, "Empty comment should produce no tokens");
}

#[test]
fn comment_multiple_consecutive() {
    let src = "// comment 1\n// comment 2\n// comment 3\nlet x = 1";
    let tokens = lex_ok(src);
    // Should have newlines and then let x = 1
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Let)));
}

#[test]
fn comment_at_end_of_file_no_newline() {
    let src = "let x = 1 // comment at end";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4); // let x = 1
}

#[test]
fn comment_with_special_characters() {
    let src = "let x = 1 // comment with @#$%^&*()";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4);
}

#[test]
fn comment_with_unicode() {
    let src = "let x = 1 // comment with 你好 and 🚀";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4);
}

#[test]
fn comment_entire_line() {
    let src = "// entire line is a comment\nlet x = 1";
    let tokens = lex_ok(src);
    // Should have newline, then let x = 1
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Let)));
}

// ===== Block Comments (Not Supported) =====

#[test]
fn comment_block_not_supported() {
    // /* ... */ style comments are not in current lexer
    let src = "let x = /* comment */ 1";
    let result = lex(src);
    // Will lex as: let x = / * comment * / 1
    // May fail or produce unexpected tokens
    if result.is_ok() {
        let tokens = result.unwrap();
        // Bug: block comments not supported
        // Will be lexed as individual tokens
        assert!(tokens.len() > 4, "Bug: block comments not supported, lexed as tokens");
    }
}

#[test]
fn comment_nested_block_not_supported() {
    // /* outer /* inner */ */ - not supported
    let src = "let x = /* outer /* inner */ */ 1";
    let result = lex(src);
    if result.is_ok() {
        // Bug: nested block comments not supported
    }
}

#[test]
fn comment_unterminated_block_not_supported() {
    // /* comment without closing
    let src = "let x = /* comment";
    let result = lex(src);
    // Should fail if block comments were supported
    // Currently will lex as separate tokens
}

// ===== Comment-like Strings =====

#[test]
fn comment_double_slash_in_string() {
    let src = r#"let x = "http://example.com""#;
    let tokens = lex_ok(src);
    // // inside string should not be treated as comment
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::StringLit(_))));
}

#[test]
fn comment_markers_in_string_literal() {
    let src = r#"let x = "// not a comment""#;
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[3].0, Token::StringLit(s) if s == "// not a comment"));
}

// ===== Edge Cases =====

#[test]
fn comment_very_long() {
    // 10,000 character comment
    let comment = "a".repeat(10_000);
    let src = format!("let x = 1 // {}", comment);
    let tokens = lex_ok(&src);
    assert_eq!(tokens.len(), 4); // Comment filtered out
}

#[test]
fn comment_with_newlines_not_possible() {
    // Line comments can't contain newlines (by definition)
    // Newline ends the comment
    let src = "// comment\nlet x = 1";
    let tokens = lex_ok(src);
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Let)));
}

#[test]
fn comment_triple_slash() {
    // /// is sometimes used for doc comments in other languages
    // In Pluto, it's just a comment
    let src = "/// doc comment\nlet x = 1";
    let tokens = lex_ok(src);
    assert!(!tokens.iter().any(|(t, _)| matches!(t, Token::Comment)));
}

#[test]
fn comment_slash_star_in_line_comment() {
    // // /* this looks like block comment start but it's in line comment
    let src = "// /* test\nlet x = 1";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 5); // \n let x = 1
}

#[test]
fn comment_with_url() {
    // URL in comment should not confuse lexer
    let src = "// See http://example.com for details\nlet x = 1";
    let tokens = lex_ok(src);
    // Comment filtered, should have: \n let x = 1
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Let)));
}

#[test]
fn comment_that_looks_like_code() {
    // Comment containing code-like content
    let src = "// fn foo() { let x = 1; }\nlet y = 2";
    let tokens = lex_ok(src);
    // Only the actual code should be lexed
    let let_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Let)).count();
    assert_eq!(let_count, 1); // Only the real 'let y'
}

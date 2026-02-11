// Additional edge case tests to verify gaps

use super::*;

#[test]
fn edge_leading_zeros() {
    // 007 - leading zeros in decimal
    let tokens = lex_ok("007");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::IntLit(7)));
}

#[test]
fn edge_multiple_underscores() {
    // 1__000 - multiple consecutive underscores
    let tokens = lex_ok("1__000");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::IntLit(1000)));
}

#[test]
fn edge_keyword_boundary_let() {
    // letx should be identifier, not keyword "let" + identifier
    let tokens = lex_ok("letx");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn edge_keyword_boundary_fn() {
    // fnord should be identifier
    let tokens = lex_ok("fnord");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn edge_keyword_boundary_true() {
    // truex should be identifier
    let tokens = lex_ok("truex");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn edge_single_underscore() {
    // _ alone as identifier
    let tokens = lex_ok("_");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn edge_double_underscore() {
    // __ as identifier
    let tokens = lex_ok("__");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn edge_hex_case_mixing() {
    // 0xAbCdEf - mixed case hex
    let tokens = lex_ok("0xAbCdEf");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::IntLit(_)));
}

#[test]
fn edge_float_zero() {
    // 0.0 should parse correctly
    let tokens = lex_ok("0.0");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

#[test]
fn edge_operator_triple_plus() {
    // +++ should be ++ + +
    let tokens = lex_ok("+++");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::PlusPlus));
    assert!(matches!(&tokens[1].0, Token::Plus));
}

#[test]
fn edge_operator_triple_minus() {
    // --- should be -- -
    let tokens = lex_ok("---");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::MinusMinus));
    assert!(matches!(&tokens[1].0, Token::Minus));
}

#[test]
fn edge_all_escapes_combined() {
    // String with all escape sequences
    let tokens = lex_ok(r#""\n\r\t\\\"mixed""#);
    assert_eq!(tokens.len(), 1);
    if let Token::StringLit(s) = &tokens[0].0 {
        assert_eq!(s, "\n\r\t\\\"mixed");
    } else {
        panic!("Expected StringLit");
    }
}

#[test]
fn edge_eof_without_newline() {
    // Various tokens at EOF without trailing newline
    let cases = vec![
        ("42", Token::IntLit(42)),
        ("identifier", Token::Ident),
        ("true", Token::True),
    ];

    for (src, expected_pattern) in cases {
        let tokens = lex_ok(src);
        assert!(tokens.len() >= 1);
        // Just verify it doesn't crash
    }
}

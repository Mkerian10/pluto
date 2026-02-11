// Category 1: Token Boundaries & Whitespace
//
// Tests how the lexer handles whitespace in various forms:
// - Spaces vs tabs
// - Newlines (statement terminators in Pluto)
// - CRLF vs LF
// - EOF conditions
// - Leading/trailing whitespace

use super::*;

#[test]
fn multiple_spaces_between_tokens() {
    let src = "let     x    =    42";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4); // let, x, =, 42
    assert!(matches!(&tokens[0].0, Token::Let));
    assert!(matches!(&tokens[1].0, Token::Ident));
    assert!(matches!(&tokens[2].0, Token::Eq));
    assert!(matches!(&tokens[3].0, Token::IntLit(42)));
}

#[test]
fn tabs_between_tokens() {
    let src = "let\tx\t=\t42";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4);
    assert!(matches!(&tokens[0].0, Token::Let));
}

#[test]
fn mixed_tabs_and_spaces() {
    // Pluto should accept mixed tabs and spaces (not Python)
    let src = "let \t x\t = \t42";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4);
}

#[test]
fn newlines_as_statement_terminators() {
    let src = "let x = 1\nlet y = 2";
    let tokens = lex_ok(src);
    // let x = 1 \n let y = 2
    assert!(matches!(&tokens[0].0, Token::Let));
    assert!(matches!(&tokens[4].0, Token::Newline));
    assert!(matches!(&tokens[5].0, Token::Let));
}

#[test]
fn multiple_consecutive_newlines() {
    let src = "let x = 1\n\n\nlet y = 2";
    let tokens = lex_ok(src);
    // Multiple newlines should be collapsed into one Newline token (per regex r"\n[\n]*")
    let newline_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Newline)).count();
    assert_eq!(newline_count, 1, "Multiple newlines should produce single Newline token");
}

#[test]
fn crlf_vs_lf() {
    // Windows-style CRLF (\r\n) vs Unix-style LF (\n)
    let src_lf = "let x = 1\nlet y = 2";
    let src_crlf = "let x = 1\r\nlet y = 2";

    let tokens_lf = lex_ok(src_lf);
    let tokens_crlf = lex_ok(src_crlf);

    // Both should lex successfully
    assert_eq!(tokens_lf.len(), tokens_crlf.len(), "CRLF and LF should produce same token count");
}

#[test]
fn whitespace_at_eof() {
    let src = "let x = 1   \n  ";
    let tokens = lex_ok(src);
    // Should lex successfully, trailing whitespace ignored
    assert!(matches!(&tokens.last().unwrap().0, Token::Newline));
}

#[test]
fn no_newline_at_eof() {
    let src = "let x = 1";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4); // No newline at end
}

#[test]
fn leading_whitespace_before_first_token() {
    let src = "   \n  \t  let x = 1";
    let tokens = lex_ok(src);
    // Leading whitespace and newlines should be handled
    assert!(matches!(&tokens[0].0, Token::Newline) || matches!(&tokens[0].0, Token::Let));
}

#[test]
fn trailing_whitespace_after_last_token() {
    let src = "let x = 1   ";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 4);
    assert!(matches!(&tokens[3].0, Token::IntLit(1)));
}

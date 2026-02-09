pub mod token;

use logos::Logos;
use crate::span::{Span, Spanned};
use crate::diagnostics::CompileError;
use token::Token;

pub fn lex(source: &str) -> Result<Vec<Spanned<Token>>, CompileError> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(tok) => {
                // Skip comments
                if matches!(tok, Token::Comment) {
                    continue;
                }
                tokens.push(Spanned::new(tok, Span::new(span.start, span.end)));
            }
            Err(()) => {
                return Err(CompileError::syntax(
                    format!("unexpected character '{}'", &source[span.start..span.end]),
                    Span::new(span.start, span.end),
                ));
            }
        }
    }

    tokens
        .retain(|t| !matches!(t.node, Token::Comment));

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_simple_function() {
        let src = "fn main() { }";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0].node, Token::Fn));
        assert!(matches!(tokens[1].node, Token::Ident));
        assert!(matches!(tokens[2].node, Token::LParen));
        assert!(matches!(tokens[3].node, Token::RParen));
        assert!(matches!(tokens[4].node, Token::LBrace));
        assert!(matches!(tokens[5].node, Token::RBrace));
    }

    #[test]
    fn lex_function_with_body() {
        let src = "fn add(a: int, b: int) int {\n    return a + b\n}";
        let tokens = lex(src).unwrap();
        // fn add ( a : int , b : int ) int { \n return a + b \n }
        assert!(matches!(tokens[0].node, Token::Fn));
        assert!(matches!(tokens[1].node, Token::Ident)); // add
        assert!(matches!(tokens[2].node, Token::LParen));
        assert!(matches!(tokens[3].node, Token::Ident)); // a
        assert!(matches!(tokens[4].node, Token::Colon));
        assert!(matches!(tokens[5].node, Token::Ident)); // int
    }

    #[test]
    fn lex_operators() {
        let src = "== != <= >= && || + - * / %";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::EqEq));
        assert!(matches!(tokens[1].node, Token::BangEq));
        assert!(matches!(tokens[2].node, Token::LtEq));
        assert!(matches!(tokens[3].node, Token::GtEq));
        assert!(matches!(tokens[4].node, Token::AmpAmp));
        assert!(matches!(tokens[5].node, Token::PipePipe));
    }

    #[test]
    fn lex_literals() {
        let src = r#"42 3.14 "hello" true false"#;
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::IntLit(42)));
        assert!(matches!(tokens[1].node, Token::FloatLit(_)));
        assert!(matches!(tokens[2].node, Token::StringLit(_)));
        assert!(matches!(tokens[3].node, Token::True));
        assert!(matches!(tokens[4].node, Token::False));
    }

    #[test]
    fn lex_comments_skipped() {
        let src = "let x = 1 // this is a comment\nlet y = 2";
        let tokens = lex(src).unwrap();
        // Should not contain any Comment tokens
        assert!(tokens.iter().all(|t| !matches!(t.node, Token::Comment)));
    }

    #[test]
    fn lex_reserved_keywords() {
        let src = "class trait app inject error raise catch spawn enum";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::Class));
        assert!(matches!(tokens[1].node, Token::Trait));
        assert!(matches!(tokens[2].node, Token::App));
        assert!(matches!(tokens[3].node, Token::Inject));
        assert!(matches!(tokens[4].node, Token::Error));
        assert!(matches!(tokens[5].node, Token::Raise));
        assert!(matches!(tokens[6].node, Token::Catch));
        assert!(matches!(tokens[7].node, Token::Spawn));
        assert!(matches!(tokens[8].node, Token::Enum));
    }

    #[test]
    fn lex_import_keyword() {
        let src = "import math";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::Import));
        assert!(matches!(tokens[1].node, Token::Ident));
    }

    #[test]
    fn lex_match_keyword() {
        let src = "match x";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::Match));
        assert!(matches!(tokens[1].node, Token::Ident));
    }

    #[test]
    fn lex_uses_and_ambient_keywords() {
        let src = "uses ambient";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::Uses));
        assert!(matches!(tokens[1].node, Token::Ambient));
    }

    #[test]
    fn lex_string_with_escapes() {
        let src = r#""hello \"world\"""#;
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::StringLit(_)));
    }
}

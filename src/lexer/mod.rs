pub mod token;
pub use token::is_keyword;

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

    // Validate no float immediately followed by dot (e.g., 1.2.3)
    // This catches invalid number formats like 1.2.3 which would otherwise
    // lex as FloatLit(1.2) + Dot + IntLit(3)
    for i in 0..tokens.len().saturating_sub(1) {
        if matches!(tokens[i].node, Token::FloatLit(_)) && matches!(tokens[i+1].node, Token::Dot) {
            // Check if they're adjacent (no gap)
            if tokens[i].span.end == tokens[i+1].span.start {
                return Err(CompileError::syntax(
                    "invalid number format: multiple decimal points".to_string(),
                    Span::new(tokens[i].span.start, tokens[i+1].span.end),
                ));
            }
        }
    }

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
        // Test all language keywords
        let src = "class trait app inject error raise catch spawn enum import match uses ambient";
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
        assert!(matches!(tokens[9].node, Token::Import));
        assert!(matches!(tokens[10].node, Token::Match));
        assert!(matches!(tokens[11].node, Token::Uses));
        assert!(matches!(tokens[12].node, Token::Ambient));
    }


    #[test]
    fn lex_string_with_escapes() {
        let src = r#""hello \"world\"""#;
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::StringLit(_)));
    }

    #[test]
    fn lex_bitwise_operators() {
        let src = "a & b | c ^ d ~ e << f";
        let tokens = lex(src).unwrap();
        // a & b | c ^ d ~ e << f
        // 0 1 2 3 4 5 6 7 8 9 10 11
        assert!(matches!(tokens[1].node, Token::Amp));      // &
        assert!(matches!(tokens[3].node, Token::Pipe));     // |
        assert!(matches!(tokens[5].node, Token::Caret));    // ^
        assert!(matches!(tokens[7].node, Token::Tilde));    // ~
        assert!(matches!(tokens[9].node, Token::Shl));      // <<
        // Note: >> is parsed as two Gt tokens to avoid generic syntax conflicts
    }

    #[test]
    fn lex_arithmetic_operators() {
        let src = "a + b - c * d / e % f";
        let tokens = lex(src).unwrap();
        // a + b - c * d / e % f
        // 0 1 2 3 4 5 6 7 8 9 10
        assert!(matches!(tokens[1].node, Token::Plus));     // +
        assert!(matches!(tokens[3].node, Token::Minus));    // -
        assert!(matches!(tokens[5].node, Token::Star));     // *
        assert!(matches!(tokens[7].node, Token::Slash));    // /
        assert!(matches!(tokens[9].node, Token::Percent));  // %
    }

    #[test]
    fn lex_multiple_decimal_points_adjacent_error() {
        // Test the validation that catches 1.2.3 (float followed by dot)
        let src = "1.2.3";
        let result = lex(src);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("multiple decimal points"));
    }

    #[test]
    fn lex_float_followed_by_dot_with_space_ok() {
        // Float followed by dot with space should be OK (e.g., "1.2 .method()")
        let src = "1.2 .x";
        let result = lex(src);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(matches!(tokens[0].node, Token::FloatLit(_)));
        assert!(matches!(tokens[1].node, Token::Dot));
    }

    #[test]
    fn lex_float_in_expression() {
        // Float in arithmetic expression should work fine
        let src = "1.5 + 2.5";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0].node, Token::FloatLit(_)));
        assert!(matches!(tokens[1].node, Token::Plus));
        assert!(matches!(tokens[2].node, Token::FloatLit(_)));
    }

    #[test]
    fn lex_unexpected_character_error() {
        // Test that unexpected characters produce errors
        let src = "let x = @";
        let result = lex(src);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unexpected character"));
    }

    #[test]
    fn lex_multiline_comment() {
        // Multi-line with comment should work
        let src = "let x = 1 // comment\nlet y = 2";
        let tokens = lex(src).unwrap();
        // Should have: let x = 1 \n let y = 2
        // Verify no Comment tokens remain
        for token in &tokens {
            assert!(!matches!(token.node, Token::Comment));
        }
        // Should have tokens for both lines
        assert!(tokens.len() > 5);
    }

    #[test]
    fn lex_empty_source() {
        let src = "";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn lex_only_whitespace() {
        let src = "   \n\t  \n  ";
        let tokens = lex(src).unwrap();
        // Whitespace is not tokenized, except newlines
        // Newlines are tokenized and preserved
        assert!(tokens.iter().all(|t| matches!(t.node, Token::Newline)));
    }

    #[test]
    fn lex_only_comments() {
        let src = "// comment 1\n// comment 2";
        let tokens = lex(src).unwrap();
        // Should only have newlines, no Comment tokens
        assert!(tokens.iter().all(|t| matches!(t.node, Token::Newline)));
    }

    #[test]
    fn lex_adjacent_operators() {
        // Test operators that could be confused when adjacent
        let src = "a+-b";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::Ident));  // a
        assert!(matches!(tokens[1].node, Token::Plus));   // +
        assert!(matches!(tokens[2].node, Token::Minus));  // -
        assert!(matches!(tokens[3].node, Token::Ident));  // b
    }

    #[test]
    fn lex_compound_assignment_operators() {
        let src = "+= -= *= /= %=";
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].node, Token::PlusEq));
        assert!(matches!(tokens[1].node, Token::MinusEq));
        assert!(matches!(tokens[2].node, Token::StarEq));
        assert!(matches!(tokens[3].node, Token::SlashEq));
        assert!(matches!(tokens[4].node, Token::PercentEq));
    }

    #[test]
    fn lex_nullable_type_syntax() {
        let src = "int?";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].node, Token::Ident));     // int
        assert!(matches!(tokens[1].node, Token::Question));  // ?
    }

    #[test]
    fn lex_none_keyword() {
        let src = "none";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].node, Token::None));
    }

    #[test]
    fn lex_arrow_function() {
        let src = "(x) => x + 1";
        let tokens = lex(src).unwrap();
        // ( x ) => x + 1
        assert!(matches!(tokens[0].node, Token::LParen));
        assert!(matches!(tokens[1].node, Token::Ident));
        assert!(matches!(tokens[2].node, Token::RParen));
        assert!(matches!(tokens[3].node, Token::FatArrow));  // =>
        assert!(matches!(tokens[4].node, Token::Ident));
        assert!(matches!(tokens[5].node, Token::Plus));
        assert!(matches!(tokens[6].node, Token::IntLit(1)));
    }
}

#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use pluto::lexer::Token;
use pluto::span::{Span, Spanned};

/// Minimal fuzzing-friendly token representation
#[derive(Arbitrary, Debug)]
enum FuzzToken {
    Ident,
    IntLit,
    FloatLit,
    StringLit,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Fn,
    Let,
    Return,
    If,
    Else,
    While,
    Newline,
}

impl FuzzToken {
    fn to_pluto_token(&self, offset: usize) -> Spanned<Token> {
        let token = match self {
            FuzzToken::Ident => Token::Identifier("x".to_string()),
            FuzzToken::IntLit => Token::IntLit(42),
            FuzzToken::FloatLit => Token::FloatLit(3.14),
            FuzzToken::StringLit => Token::StringLit("str".to_string()),
            FuzzToken::Plus => Token::Plus,
            FuzzToken::Minus => Token::Minus,
            FuzzToken::Star => Token::Star,
            FuzzToken::Slash => Token::Slash,
            FuzzToken::LeftParen => Token::LeftParen,
            FuzzToken::RightParen => Token::RightParen,
            FuzzToken::LeftBrace => Token::LeftBrace,
            FuzzToken::RightBrace => Token::RightBrace,
            FuzzToken::Fn => Token::Fn,
            FuzzToken::Let => Token::Let,
            FuzzToken::Return => Token::Return,
            FuzzToken::If => Token::If,
            FuzzToken::Else => Token::Else,
            FuzzToken::While => Token::While,
            FuzzToken::Newline => Token::Newline,
        };
        Spanned {
            node: token,
            span: Span {
                start: offset,
                end: offset + 1,
            },
        }
    }
}

#[derive(Arbitrary, Debug)]
struct FuzzTokens {
    tokens: Vec<FuzzToken>,
}

fuzz_target!(|input: FuzzTokens| {
    // Generate token stream
    let mut tokens: Vec<Spanned<Token>> = input
        .tokens
        .iter()
        .enumerate()
        .map(|(i, t)| t.to_pluto_token(i))
        .collect();

    // Always add EOF
    let last_offset = tokens.len();
    tokens.push(Spanned {
        node: Token::Eof,
        span: Span {
            start: last_offset,
            end: last_offset,
        },
    });

    // Create a dummy source string for the parser
    let source = "x ".repeat(tokens.len());

    // Feed to parser - should never panic
    let mut parser = pluto::parser::Parser::new(&tokens, &source);
    let _ = parser.parse_program();
});

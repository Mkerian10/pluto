#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

/// Fuzzing-friendly token vocabulary, rendered to source text and fed
/// through the real lexer. Rendering (rather than fabricating a
/// `Vec<Spanned<Token>>` directly) keeps spans honest: the parser resolves
/// identifiers and contextual keywords (`ensures`, `object`, `old`, `at`,
/// `remote`, `domain`) by slicing the source at token spans.
///
/// The vocabulary deliberately over-weights the newer grammar surface:
/// typestate `where` constraints, `must_release`, `guarded_by` field
/// clauses, `ensures`/`old()` contracts, `object` declarations, and `at`
/// placement.
#[derive(Arbitrary, Debug, Clone, Copy)]
enum FuzzToken {
    // Identifiers and literals
    IdentX,
    IdentState,
    IdentOld,     // contextual: old
    IdentEnsures, // contextual: ensures
    IdentObject,  // contextual: object
    IdentAt,      // contextual: at
    IdentDomain,  // contextual: domain
    IdentRemote,  // contextual: remote
    TypeName,
    IntLit,
    StringLit,
    // Declaration keywords
    Fn,
    Let,
    Mut,
    Class,
    Trait,
    Enum,
    Impl,
    App,
    ErrorKw,
    Pub,
    // Contract / typestate keywords
    Invariant,
    Requires,
    Where,
    MustRelease,
    GuardedBy,
    Assert,
    // Statements / control flow
    Return,
    If,
    Else,
    While,
    For,
    In,
    Match,
    Raise,
    Catch,
    Spawn,
    SelfVal,
    // Punctuation / operators
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Bang,
    Question,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    DoubleColon,
    Dot,
    FatArrow,
    Newline,
}

impl FuzzToken {
    fn text(self) -> &'static str {
        match self {
            FuzzToken::IdentX => "x",
            FuzzToken::IdentState => "Held",
            FuzzToken::IdentOld => "old",
            FuzzToken::IdentEnsures => "ensures",
            FuzzToken::IdentObject => "object",
            FuzzToken::IdentAt => "at",
            FuzzToken::IdentDomain => "domain",
            FuzzToken::IdentRemote => "remote",
            FuzzToken::TypeName => "T",
            FuzzToken::IntLit => "42",
            FuzzToken::StringLit => "\"s\"",
            FuzzToken::Fn => "fn",
            FuzzToken::Let => "let",
            FuzzToken::Mut => "mut",
            FuzzToken::Class => "class",
            FuzzToken::Trait => "trait",
            FuzzToken::Enum => "enum",
            FuzzToken::Impl => "impl",
            FuzzToken::App => "app",
            FuzzToken::ErrorKw => "error",
            FuzzToken::Pub => "pub",
            FuzzToken::Invariant => "invariant",
            FuzzToken::Requires => "requires",
            FuzzToken::Where => "where",
            FuzzToken::MustRelease => "must_release",
            FuzzToken::GuardedBy => "guarded_by",
            FuzzToken::Assert => "assert",
            FuzzToken::Return => "return",
            FuzzToken::If => "if",
            FuzzToken::Else => "else",
            FuzzToken::While => "while",
            FuzzToken::For => "for",
            FuzzToken::In => "in",
            FuzzToken::Match => "match",
            FuzzToken::Raise => "raise",
            FuzzToken::Catch => "catch",
            FuzzToken::Spawn => "spawn",
            FuzzToken::SelfVal => "self",
            FuzzToken::Plus => "+",
            FuzzToken::Minus => "-",
            FuzzToken::Star => "*",
            FuzzToken::Slash => "/",
            FuzzToken::Eq => "=",
            FuzzToken::EqEq => "==",
            FuzzToken::BangEq => "!=",
            FuzzToken::Lt => "<",
            FuzzToken::Gt => ">",
            FuzzToken::LtEq => "<=",
            FuzzToken::GtEq => ">=",
            FuzzToken::Bang => "!",
            FuzzToken::Question => "?",
            FuzzToken::LParen => "(",
            FuzzToken::RParen => ")",
            FuzzToken::LBrace => "{",
            FuzzToken::RBrace => "}",
            FuzzToken::LBracket => "[",
            FuzzToken::RBracket => "]",
            FuzzToken::Comma => ",",
            FuzzToken::Colon => ":",
            FuzzToken::DoubleColon => "::",
            FuzzToken::Dot => ".",
            FuzzToken::FatArrow => "=>",
            FuzzToken::Newline => "\n",
        }
    }
}

#[derive(Arbitrary, Debug)]
struct FuzzTokens {
    tokens: Vec<FuzzToken>,
}

fuzz_target!(|input: FuzzTokens| {
    // Cap the token count: nesting tokens (`(`, `{`) recurse in the parser,
    // and production parses on a 16MB stack (see pluto::compile_to_object);
    // the default fuzz thread is smaller.
    if input.tokens.len() > 512 {
        return;
    }
    let source: String = input
        .tokens
        .iter()
        .map(|t| t.text())
        .collect::<Vec<_>>()
        .join(" ");

    let Ok(tokens) = pluto::lexer::lex(&source) else {
        return;
    };
    // Feed to parser — must never panic.
    let mut parser = pluto::parser::Parser::new(&tokens, &source);
    let _ = parser.parse_program();
});

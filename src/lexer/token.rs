use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    // Keywords
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("return")]
    Return,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("true")]
    True,
    #[token("false")]
    False,

    // Reserved future keywords
    #[token("class")]
    Class,
    #[token("trait")]
    Trait,
    #[token("app")]
    App,
    #[token("inject")]
    Inject,
    #[token("error")]
    Error,
    #[token("raise")]
    Raise,
    #[token("catch")]
    Catch,
    #[token("spawn")]
    Spawn,
    #[token("enum")]
    Enum,
    #[token("impl")]
    Impl,
    #[token("self")]
    SelfVal,
    #[token("pub")]
    Pub,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("match")]
    Match,
    #[token("import")]
    Import,
    #[token("as")]
    As,
    #[token("extern")]
    Extern,
    #[token("uses")]
    Uses,
    #[token("ambient")]
    Ambient,
    #[token("tests")]
    Tests,
    #[token("test")]
    Test,
    #[token("invariant")]
    Invariant,
    #[token("requires")]
    Requires,
    #[token("ensures")]
    Ensures,
    #[token("select")]
    Select,
    #[token("default")]
    Default,
    #[token("scope")]
    Scope,
    #[token("scoped")]
    Scoped,
    #[token("transient")]
    Transient,
    #[token("none")]
    None,
    #[token("system")]
    System,
    #[token("stage")]
    Stage,
    #[token("override")]
    Override,
    #[token("yield")]
    Yield,
    #[token("stream")]
    Stream,

    // Literals
    #[regex(r"0[xX][0-9a-fA-F_]+|[0-9][0-9_]*", |lex| {
        let s = lex.slice();
        if s.starts_with("0x") || s.starts_with("0X") {
            let hex_part = &s[2..];
            let cleaned = hex_part.replace('_', "");
            // Reject empty hex, leading/trailing underscore
            if cleaned.is_empty() {
                return None;
            }
            i64::from_str_radix(&cleaned, 16).ok()
        } else {
            s.replace('_', "").parse::<i64>().ok()
        }
    })]
    IntLit(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        let raw = &s[1..s.len()-1];
        let mut result = String::with_capacity(raw.len());
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some(other) => { result.push('\\'); result.push(other); }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }
        Some(result)
    })]
    StringLit(String),

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // Operators
    #[token("++")]
    PlusPlus,
    #[token("+")]
    Plus,
    #[token("--")]
    MinusMinus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    BangEq,
    #[token("<<")]
    Shl,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("&&")]
    AmpAmp,
    #[token("||")]
    PipePipe,
    #[token("!")]
    Bang,

    // Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token(".")]
    Dot,
    #[token("?")]
    Question,

    // Newline (significant for statement termination)
    #[regex(r"\n[\n]*")]
    Newline,

    // Comments (skip)
    #[regex(r"//[^\n]*")]
    Comment,
}

/// Returns true if the given string is a Pluto keyword.
pub fn is_keyword(s: &str) -> bool {
    matches!(s, "fn" | "let" | "mut" | "return" | "if" | "else" | "while" | "true" | "false"
        | "class" | "trait" | "app" | "inject" | "error" | "raise" | "catch" | "spawn"
        | "enum" | "impl" | "self" | "pub" | "for" | "in" | "break" | "continue"
        | "match" | "import" | "as" | "extern" | "uses" | "ambient" | "tests" | "test"
        | "invariant" | "requires" | "ensures" | "select" | "default"
        | "scope" | "scoped" | "transient" | "none" | "system" | "stage" | "override"
        | "yield" | "stream")
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Fn => write!(f, "fn"),
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::Return => write!(f, "return"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::While => write!(f, "while"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Class => write!(f, "class"),
            Token::Trait => write!(f, "trait"),
            Token::App => write!(f, "app"),
            Token::Inject => write!(f, "inject"),
            Token::Error => write!(f, "error"),
            Token::Raise => write!(f, "raise"),
            Token::Catch => write!(f, "catch"),
            Token::Spawn => write!(f, "spawn"),
            Token::Enum => write!(f, "enum"),
            Token::Impl => write!(f, "impl"),
            Token::SelfVal => write!(f, "self"),
            Token::Pub => write!(f, "pub"),
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::Break => write!(f, "break"),
            Token::Continue => write!(f, "continue"),
            Token::Match => write!(f, "match"),
            Token::Import => write!(f, "import"),
            Token::As => write!(f, "as"),
            Token::Extern => write!(f, "extern"),
            Token::Uses => write!(f, "uses"),
            Token::Ambient => write!(f, "ambient"),
            Token::Tests => write!(f, "tests"),
            Token::Test => write!(f, "test"),
            Token::Invariant => write!(f, "invariant"),
            Token::Requires => write!(f, "requires"),
            Token::Ensures => write!(f, "ensures"),
            Token::Select => write!(f, "select"),
            Token::Default => write!(f, "default"),
            Token::Scope => write!(f, "scope"),
            Token::Scoped => write!(f, "scoped"),
            Token::Transient => write!(f, "transient"),
            Token::None => write!(f, "none"),
            Token::System => write!(f, "system"),
            Token::Stage => write!(f, "stage"),
            Token::Override => write!(f, "override"),
            Token::Yield => write!(f, "yield"),
            Token::Stream => write!(f, "stream"),
            Token::IntLit(n) => write!(f, "{n}"),
            Token::FloatLit(n) => write!(f, "{n}"),
            Token::StringLit(s) => write!(f, "\"{s}\""),
            Token::Ident => write!(f, "identifier"),
            Token::PlusPlus => write!(f, "++"),
            Token::Plus => write!(f, "+"),
            Token::MinusMinus => write!(f, "--"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::PlusEq => write!(f, "+="),
            Token::MinusEq => write!(f, "-="),
            Token::StarEq => write!(f, "*="),
            Token::SlashEq => write!(f, "/="),
            Token::PercentEq => write!(f, "%="),
            Token::Eq => write!(f, "="),
            Token::EqEq => write!(f, "=="),
            Token::BangEq => write!(f, "!="),
            Token::Shl => write!(f, "<<"),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::LtEq => write!(f, "<="),
            Token::GtEq => write!(f, ">="),
            Token::Amp => write!(f, "&"),
            Token::Pipe => write!(f, "|"),
            Token::Caret => write!(f, "^"),
            Token::Tilde => write!(f, "~"),
            Token::AmpAmp => write!(f, "&&"),
            Token::PipePipe => write!(f, "||"),
            Token::Bang => write!(f, "!"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::DotDotEq => write!(f, "..="),
            Token::DotDot => write!(f, ".."),
            Token::Dot => write!(f, "."),
            Token::Question => write!(f, "?"),
            Token::Newline => write!(f, "newline"),
            Token::Comment => write!(f, "comment"),
        }
    }
}

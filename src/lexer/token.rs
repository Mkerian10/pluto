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
    #[token("assert")]
    Assert,
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
    // Note: hex and binary patterns use \w* to match any characters after 0x/0b,
    // which are then validated by the callback for better error messages
    #[regex(r"0[xX][\w]*|0[bB][\w]*|[0-9][0-9_]*", |lex| {
        let s = lex.slice();
        if s.starts_with("0x") || s.starts_with("0X") {
            let hex_part = &s[2..];

            // Reject empty hex (just "0x")
            if hex_part.is_empty() {
                return None;
            }

            // Reject leading underscore (0x_FF)
            if hex_part.starts_with('_') {
                return None;
            }

            // Reject trailing underscore (0xFF_)
            if hex_part.ends_with('_') {
                return None;
            }

            // Validate all characters are hex digits or underscores
            if !hex_part.chars().all(|c| c.is_ascii_hexdigit() || c == '_') {
                return None;
            }

            let cleaned = hex_part.replace('_', "");
            // Parse as i128 first, then validate range
            // Accept i64::MIN..=i64::MAX, plus (i64::MAX + 1) for the i64::MIN literal special case
            match i128::from_str_radix(&cleaned, 16) {
                Ok(val) if val >= i64::MIN as i128 && val <= i64::MAX as i128 + 1 => Some(val as i64),
                _ => None,
            }
        } else if s.starts_with("0b") || s.starts_with("0B") {
            let bin_part = &s[2..];

            // Reject empty binary (just "0b")
            if bin_part.is_empty() {
                return None;
            }

            // Reject leading underscore (0b_1010)
            if bin_part.starts_with('_') {
                return None;
            }

            // Reject trailing underscore (0b1010_)
            if bin_part.ends_with('_') {
                return None;
            }

            // Validate all characters are binary digits or underscores
            if !bin_part.chars().all(|c| c == '0' || c == '1' || c == '_') {
                return None;
            }

            let cleaned = bin_part.replace('_', "");
            // Parse as i128 first, then validate range
            // Accept i64::MIN..=i64::MAX, plus (i64::MAX + 1) for the i64::MIN literal special case
            match i128::from_str_radix(&cleaned, 2) {
                Ok(val) if val >= i64::MIN as i128 && val <= i64::MAX as i128 + 1 => Some(val as i64),
                _ => None,
            }
        } else {
            // Parse as i128 first, then validate range
            // Accept i64::MIN..=i64::MAX, plus (i64::MAX + 1) for the i64::MIN literal special case
            // When -9223372036854775808 is parsed, the lexer sees:
            //   - Minus token
            //   - 9223372036854775808 (which is i64::MAX + 1)
            // We accept i64::MAX + 1 here, and it wraps to i64::MIN when cast to i64
            match s.replace('_', "").parse::<i128>() {
                Ok(val) if val >= i64::MIN as i128 && val <= i64::MAX as i128 + 1 => Some(val as i64),
                _ => None,
            }
        }
    })]
    IntLit(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)?|[0-9][0-9_]*[eE][+-]?[0-9][0-9_]*", priority = 3, callback = |lex| lex.slice().replace('_', "").parse::<f64>().ok())]
    FloatLit(f64),

    #[regex(r#"f"([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[2..s.len()-1].to_string())  // Strip f" and ", return raw content
    })]
    FStringLit(String),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())  // Strip " and ", return raw content
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
    #[token("::")]
    DoubleColon,
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
    // Supports both LF (\n) and CRLF (\r\n) line endings
    #[regex(r"(\r\n|\n)+")]
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
        | "invariant" | "requires" | "assert" | "select" | "default"
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
            Token::Assert => write!(f, "assert"),
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
            Token::FStringLit(s) => write!(f, "f\"{s}\""),
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
            Token::DoubleColon => write!(f, "::"),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== is_keyword tests =====

    #[test]
    fn test_is_keyword_all_keywords() {
        let keywords = vec![
            "fn", "let", "mut", "return", "if", "else", "while", "true", "false",
            "class", "trait", "app", "inject", "error", "raise", "catch", "spawn",
            "enum", "impl", "self", "pub", "for", "in", "break", "continue",
            "match", "import", "as", "extern", "uses", "ambient", "tests", "test",
            "invariant", "requires", "assert", "select", "default",
            "scope", "scoped", "transient", "none", "system", "stage", "override",
            "yield", "stream",
        ];
        for keyword in keywords {
            assert!(is_keyword(keyword), "Expected '{keyword}' to be a keyword");
        }
    }

    #[test]
    fn test_is_keyword_non_keywords() {
        let non_keywords = vec![
            "foo", "bar", "my_var", "MyClass", "function", "variable",
            "returns", "ifs", "whiles", "selff", "pubic", "imports",
        ];
        for word in non_keywords {
            assert!(!is_keyword(word), "Expected '{word}' to not be a keyword");
        }
    }

    #[test]
    fn test_is_keyword_case_sensitive() {
        assert!(!is_keyword("FN"));
        assert!(!is_keyword("Let"));
        assert!(!is_keyword("IF"));
        assert!(!is_keyword("Class"));
    }

    // ===== Display tests =====

    #[test]
    fn test_display_keywords() {
        assert_eq!(Token::Fn.to_string(), "fn");
        assert_eq!(Token::Let.to_string(), "let");
        assert_eq!(Token::Mut.to_string(), "mut");
        assert_eq!(Token::Return.to_string(), "return");
        assert_eq!(Token::If.to_string(), "if");
        assert_eq!(Token::Else.to_string(), "else");
        assert_eq!(Token::While.to_string(), "while");
        assert_eq!(Token::True.to_string(), "true");
        assert_eq!(Token::False.to_string(), "false");
    }

    #[test]
    fn test_display_class_related() {
        assert_eq!(Token::Class.to_string(), "class");
        assert_eq!(Token::Trait.to_string(), "trait");
        assert_eq!(Token::Impl.to_string(), "impl");
        assert_eq!(Token::SelfVal.to_string(), "self");
        assert_eq!(Token::Pub.to_string(), "pub");
    }

    #[test]
    fn test_display_app_di() {
        assert_eq!(Token::App.to_string(), "app");
        assert_eq!(Token::Inject.to_string(), "inject");
        assert_eq!(Token::Scope.to_string(), "scope");
        assert_eq!(Token::Scoped.to_string(), "scoped");
        assert_eq!(Token::Transient.to_string(), "transient");
    }

    #[test]
    fn test_display_error_handling() {
        assert_eq!(Token::Error.to_string(), "error");
        assert_eq!(Token::Raise.to_string(), "raise");
        assert_eq!(Token::Catch.to_string(), "catch");
    }

    #[test]
    fn test_display_concurrency() {
        assert_eq!(Token::Spawn.to_string(), "spawn");
        assert_eq!(Token::Select.to_string(), "select");
        assert_eq!(Token::Yield.to_string(), "yield");
        assert_eq!(Token::Stream.to_string(), "stream");
    }

    #[test]
    fn test_display_control_flow() {
        assert_eq!(Token::For.to_string(), "for");
        assert_eq!(Token::In.to_string(), "in");
        assert_eq!(Token::Break.to_string(), "break");
        assert_eq!(Token::Continue.to_string(), "continue");
        assert_eq!(Token::Match.to_string(), "match");
        assert_eq!(Token::Default.to_string(), "default");
    }

    #[test]
    fn test_display_enum() {
        assert_eq!(Token::Enum.to_string(), "enum");
    }

    #[test]
    fn test_display_module_system() {
        assert_eq!(Token::Import.to_string(), "import");
        assert_eq!(Token::As.to_string(), "as");
        assert_eq!(Token::Extern.to_string(), "extern");
        assert_eq!(Token::Uses.to_string(), "uses");
    }

    #[test]
    fn test_display_testing() {
        assert_eq!(Token::Ambient.to_string(), "ambient");
        assert_eq!(Token::Tests.to_string(), "tests");
        assert_eq!(Token::Test.to_string(), "test");
    }

    #[test]
    fn test_display_contracts() {
        assert_eq!(Token::Invariant.to_string(), "invariant");
        assert_eq!(Token::Requires.to_string(), "requires");
        assert_eq!(Token::Assert.to_string(), "assert");
    }

    #[test]
    fn test_display_future_keywords() {
        assert_eq!(Token::None.to_string(), "none");
        assert_eq!(Token::System.to_string(), "system");
        assert_eq!(Token::Stage.to_string(), "stage");
        assert_eq!(Token::Override.to_string(), "override");
    }

    #[test]
    fn test_display_literals() {
        assert_eq!(Token::IntLit(42).to_string(), "42");
        assert_eq!(Token::IntLit(-123).to_string(), "-123");
        assert_eq!(Token::FloatLit(3.14).to_string(), "3.14");
        assert_eq!(Token::FloatLit(-0.5).to_string(), "-0.5");
        assert_eq!(Token::StringLit("hello".to_string()).to_string(), "\"hello\"");
        assert_eq!(Token::FStringLit("world".to_string()).to_string(), "f\"world\"");
        assert_eq!(Token::Ident.to_string(), "identifier");
    }

    #[test]
    fn test_display_arithmetic_operators() {
        assert_eq!(Token::Plus.to_string(), "+");
        assert_eq!(Token::Minus.to_string(), "-");
        assert_eq!(Token::Star.to_string(), "*");
        assert_eq!(Token::Slash.to_string(), "/");
        assert_eq!(Token::Percent.to_string(), "%");
        assert_eq!(Token::PlusPlus.to_string(), "++");
        assert_eq!(Token::MinusMinus.to_string(), "--");
    }

    #[test]
    fn test_display_assignment_operators() {
        assert_eq!(Token::Eq.to_string(), "=");
        assert_eq!(Token::PlusEq.to_string(), "+=");
        assert_eq!(Token::MinusEq.to_string(), "-=");
        assert_eq!(Token::StarEq.to_string(), "*=");
        assert_eq!(Token::SlashEq.to_string(), "/=");
        assert_eq!(Token::PercentEq.to_string(), "%=");
    }

    #[test]
    fn test_display_comparison_operators() {
        assert_eq!(Token::EqEq.to_string(), "==");
        assert_eq!(Token::BangEq.to_string(), "!=");
        assert_eq!(Token::Lt.to_string(), "<");
        assert_eq!(Token::Gt.to_string(), ">");
        assert_eq!(Token::LtEq.to_string(), "<=");
        assert_eq!(Token::GtEq.to_string(), ">=");
    }

    #[test]
    fn test_display_bitwise_operators() {
        assert_eq!(Token::Amp.to_string(), "&");
        assert_eq!(Token::Pipe.to_string(), "|");
        assert_eq!(Token::Caret.to_string(), "^");
        assert_eq!(Token::Tilde.to_string(), "~");
        assert_eq!(Token::Shl.to_string(), "<<");
    }

    #[test]
    fn test_display_logical_operators() {
        assert_eq!(Token::AmpAmp.to_string(), "&&");
        assert_eq!(Token::PipePipe.to_string(), "||");
        assert_eq!(Token::Bang.to_string(), "!");
    }

    #[test]
    fn test_display_punctuation() {
        assert_eq!(Token::LParen.to_string(), "(");
        assert_eq!(Token::RParen.to_string(), ")");
        assert_eq!(Token::LBrace.to_string(), "{");
        assert_eq!(Token::RBrace.to_string(), "}");
        assert_eq!(Token::LBracket.to_string(), "[");
        assert_eq!(Token::RBracket.to_string(), "]");
        assert_eq!(Token::Comma.to_string(), ",");
        assert_eq!(Token::Colon.to_string(), ":");
        assert_eq!(Token::DoubleColon.to_string(), "::");
        assert_eq!(Token::Dot.to_string(), ".");
        assert_eq!(Token::DotDot.to_string(), "..");
        assert_eq!(Token::DotDotEq.to_string(), "..=");
        assert_eq!(Token::Arrow.to_string(), "->");
        assert_eq!(Token::FatArrow.to_string(), "=>");
        assert_eq!(Token::Question.to_string(), "?");
    }

    #[test]
    fn test_display_special() {
        assert_eq!(Token::Newline.to_string(), "newline");
        assert_eq!(Token::Comment.to_string(), "comment");
    }

    // ===== Token equality and clone tests =====

    #[test]
    fn test_token_equality() {
        assert_eq!(Token::Fn, Token::Fn);
        assert_ne!(Token::Fn, Token::Let);
        assert_eq!(Token::IntLit(42), Token::IntLit(42));
        assert_ne!(Token::IntLit(42), Token::IntLit(43));
        assert_eq!(Token::StringLit("hello".to_string()), Token::StringLit("hello".to_string()));
        assert_ne!(Token::StringLit("hello".to_string()), Token::StringLit("world".to_string()));
    }

    #[test]
    fn test_token_clone() {
        let tok = Token::IntLit(42);
        let cloned = tok.clone();
        assert_eq!(tok, cloned);

        let tok2 = Token::StringLit("test".to_string());
        let cloned2 = tok2.clone();
        assert_eq!(tok2, cloned2);
    }

    #[test]
    fn test_token_debug() {
        let tok = Token::Fn;
        let debug_str = format!("{:?}", tok);
        assert_eq!(debug_str, "Fn");

        let tok2 = Token::IntLit(42);
        let debug_str2 = format!("{:?}", tok2);
        assert_eq!(debug_str2, "IntLit(42)");
    }

    // ===== Hex integer parsing tests via logos =====

    #[test]
    fn test_hex_integer_basic() {
        let mut lex = Token::lexer("0xFF");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(255))));
    }

    #[test]
    fn test_hex_integer_lowercase() {
        let mut lex = Token::lexer("0xff");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(255))));
    }

    #[test]
    fn test_hex_integer_uppercase_x() {
        let mut lex = Token::lexer("0XFF");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(255))));
    }

    #[test]
    fn test_hex_integer_with_underscores() {
        let mut lex = Token::lexer("0xFF_00_AA");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(0xFF00AA))));
    }

    #[test]
    fn test_hex_integer_empty() {
        let mut lex = Token::lexer("0x");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_hex_integer_leading_underscore() {
        let mut lex = Token::lexer("0x_FF");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_hex_integer_trailing_underscore() {
        let mut lex = Token::lexer("0xFF_");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_hex_integer_invalid_chars() {
        let mut lex = Token::lexer("0xGG");
        assert_eq!(lex.next(), Some(Err(())));
    }

    // ===== Binary integer parsing tests via logos =====

    #[test]
    fn test_binary_integer_basic() {
        let mut lex = Token::lexer("0b1010");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(10))));
    }

    #[test]
    fn test_binary_integer_uppercase_prefix() {
        let mut lex = Token::lexer("0B1010");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(10))));
    }

    #[test]
    fn test_binary_integer_with_underscores() {
        let mut lex = Token::lexer("0b1111_0000");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(0xF0))));
    }

    #[test]
    fn test_binary_integer_all_zeros() {
        let mut lex = Token::lexer("0b0000");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(0))));
    }

    #[test]
    fn test_binary_integer_single_one() {
        let mut lex = Token::lexer("0b1");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(1))));
    }

    #[test]
    fn test_binary_integer_empty() {
        let mut lex = Token::lexer("0b");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_binary_integer_leading_underscore() {
        let mut lex = Token::lexer("0b_1010");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_binary_integer_trailing_underscore() {
        let mut lex = Token::lexer("0b1010_");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_binary_integer_invalid_digit() {
        let mut lex = Token::lexer("0b102");
        assert_eq!(lex.next(), Some(Err(())));
    }

    #[test]
    fn test_binary_integer_max_64_bits() {
        // 63 ones = i64::MAX
        let mut lex = Token::lexer("0b0111111111111111111111111111111111111111111111111111111111111111");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(i64::MAX))));
    }

    #[test]
    fn test_decimal_integer() {
        let mut lex = Token::lexer("12345");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(12345))));
    }

    #[test]
    fn test_decimal_integer_with_underscores() {
        let mut lex = Token::lexer("1_000_000");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(1000000))));
    }

    #[test]
    fn test_float_literal() {
        let mut lex = Token::lexer("3.14");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(3.14))));
    }

    #[test]
    fn test_float_with_underscores() {
        let mut lex = Token::lexer("1_000.5_5");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1000.55))));
    }

    // ===== Scientific notation tests =====

    #[test]
    fn test_scientific_notation_integer_base() {
        let mut lex = Token::lexer("1e6");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1e6))));
    }

    #[test]
    fn test_scientific_notation_uppercase_e() {
        let mut lex = Token::lexer("1E6");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1E6))));
    }

    #[test]
    fn test_scientific_notation_negative_exponent() {
        let mut lex = Token::lexer("1e-3");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1e-3))));
    }

    #[test]
    fn test_scientific_notation_positive_exponent() {
        let mut lex = Token::lexer("1e+6");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1e+6))));
    }

    #[test]
    fn test_scientific_notation_float_base() {
        let mut lex = Token::lexer("2.5e3");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(2.5e3))));
    }

    #[test]
    fn test_scientific_notation_float_base_negative_exp() {
        let mut lex = Token::lexer("2.5e-3");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(2.5e-3))));
    }

    #[test]
    fn test_scientific_notation_float_base_positive_exp() {
        let mut lex = Token::lexer("2.5e+3");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(2.5e+3))));
    }

    #[test]
    fn test_scientific_notation_with_underscores() {
        let mut lex = Token::lexer("1_000e6");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1000e6))));
    }

    #[test]
    fn test_scientific_notation_exponent_with_underscores() {
        let mut lex = Token::lexer("1e1_0");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(1e10))));
    }

    #[test]
    fn test_scientific_notation_infinity() {
        // 1e999 overflows f64 to infinity — we accept it
        let mut lex = Token::lexer("1e999");
        assert_eq!(lex.next(), Some(Ok(Token::FloatLit(f64::INFINITY))));
    }

    #[test]
    fn test_e_without_digits_is_not_scientific() {
        let mut lex = Token::lexer("1e");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(1))));
        assert_eq!(lex.next(), Some(Ok(Token::Ident))); // 'e' as identifier
    }

    #[test]
    fn test_e_minus_without_digits_is_not_scientific() {
        // "1e-" — logos backtracks: IntLit(1), then Ident(e), then Minus
        let mut lex = Token::lexer("1e-");
        assert_eq!(lex.next(), Some(Ok(Token::IntLit(1))));
        assert_eq!(lex.next(), Some(Ok(Token::Ident)));
        assert_eq!(lex.next(), Some(Ok(Token::Minus)));
    }

    // ===== String escape sequence tests =====

    #[test]
    fn test_string_basic() {
        let mut lex = Token::lexer("\"hello\"");
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello".to_string()))));
    }

    #[test]
    fn test_string_escape_newline_raw() {
        // Logos returns raw content; escape processing is done by lex()
        let mut lex = Token::lexer(r#""hello\nworld""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\nworld".to_string()))));
    }

    #[test]
    fn test_string_escape_tab_raw() {
        let mut lex = Token::lexer(r#""hello\tworld""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\tworld".to_string()))));
    }

    #[test]
    fn test_string_escape_carriage_return_raw() {
        let mut lex = Token::lexer(r#""hello\rworld""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\rworld".to_string()))));
    }

    #[test]
    fn test_string_escape_backslash_raw() {
        let mut lex = Token::lexer(r#""hello\\world""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\\\world".to_string()))));
    }

    #[test]
    fn test_string_escape_quote_raw() {
        let mut lex = Token::lexer(r#""hello\"world""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\\"world".to_string()))));
    }

    #[test]
    fn test_string_raw_content_no_escape_processing() {
        // Logos callback now returns raw content; escape processing happens in lex()
        let mut lex = Token::lexer(r#""hello\nworld""#);
        assert_eq!(lex.next(), Some(Ok(Token::StringLit("hello\\nworld".to_string()))));
    }

    #[test]
    fn test_fstring_basic() {
        let mut lex = Token::lexer("f\"hello\"");
        assert_eq!(lex.next(), Some(Ok(Token::FStringLit("hello".to_string()))));
    }

    #[test]
    fn test_fstring_raw_content_no_escape_processing() {
        // Logos callback now returns raw content; escape processing happens in lex()
        let mut lex = Token::lexer(r#"f"hello\nworld""#);
        assert_eq!(lex.next(), Some(Ok(Token::FStringLit("hello\\nworld".to_string()))));
    }

    // ===== Identifier tests =====

    #[test]
    fn test_identifier() {
        let mut lex = Token::lexer("my_var");
        assert_eq!(lex.next(), Some(Ok(Token::Ident)));
    }

    #[test]
    fn test_identifier_with_numbers() {
        let mut lex = Token::lexer("var123");
        assert_eq!(lex.next(), Some(Ok(Token::Ident)));
    }

    #[test]
    fn test_identifier_underscore_prefix() {
        let mut lex = Token::lexer("_private");
        assert_eq!(lex.next(), Some(Ok(Token::Ident)));
    }

    // ===== Comment and newline tests =====

    #[test]
    fn test_comment() {
        let mut lex = Token::lexer("// this is a comment");
        assert_eq!(lex.next(), Some(Ok(Token::Comment)));
    }

    #[test]
    fn test_newline_lf() {
        let mut lex = Token::lexer("\n");
        assert_eq!(lex.next(), Some(Ok(Token::Newline)));
    }

    #[test]
    fn test_newline_crlf() {
        let mut lex = Token::lexer("\r\n");
        assert_eq!(lex.next(), Some(Ok(Token::Newline)));
    }

    #[test]
    fn test_multiple_newlines() {
        let mut lex = Token::lexer("\n\n\n");
        assert_eq!(lex.next(), Some(Ok(Token::Newline)));
    }

    // ===== Operator precedence tests (multi-char tokens) =====

    #[test]
    fn test_plusplus_vs_plus() {
        let mut lex = Token::lexer("++");
        assert_eq!(lex.next(), Some(Ok(Token::PlusPlus)));

        let mut lex2 = Token::lexer("+");
        assert_eq!(lex2.next(), Some(Ok(Token::Plus)));
    }

    #[test]
    fn test_eqeq_vs_eq() {
        let mut lex = Token::lexer("==");
        assert_eq!(lex.next(), Some(Ok(Token::EqEq)));

        let mut lex2 = Token::lexer("=");
        assert_eq!(lex2.next(), Some(Ok(Token::Eq)));
    }

    #[test]
    fn test_dotdoteq_vs_dotdot_vs_dot() {
        let mut lex = Token::lexer("..=");
        assert_eq!(lex.next(), Some(Ok(Token::DotDotEq)));

        let mut lex2 = Token::lexer("..");
        assert_eq!(lex2.next(), Some(Ok(Token::DotDot)));

        let mut lex3 = Token::lexer(".");
        assert_eq!(lex3.next(), Some(Ok(Token::Dot)));
    }

    #[test]
    fn test_arrow_vs_minus() {
        let mut lex = Token::lexer("->");
        assert_eq!(lex.next(), Some(Ok(Token::Arrow)));

        let mut lex2 = Token::lexer("-");
        assert_eq!(lex2.next(), Some(Ok(Token::Minus)));
    }

    #[test]
    fn test_fatarrow_vs_eq() {
        let mut lex = Token::lexer("=>");
        assert_eq!(lex.next(), Some(Ok(Token::FatArrow)));

        let mut lex2 = Token::lexer("=");
        assert_eq!(lex2.next(), Some(Ok(Token::Eq)));
    }

    #[test]
    fn test_doublecolon_vs_colon() {
        let mut lex = Token::lexer("::");
        assert_eq!(lex.next(), Some(Ok(Token::DoubleColon)));

        let mut lex2 = Token::lexer(":");
        assert_eq!(lex2.next(), Some(Ok(Token::Colon)));
    }
}

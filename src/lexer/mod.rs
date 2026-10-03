pub mod token;
pub use token::is_keyword;

use logos::Logos;
use crate::span::{Span, Spanned};
use crate::diagnostics::CompileError;
use token::Token;

/// One piece of an f-string body, as byte ranges into the raw (unescaped) content.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FStringPart {
    /// Literal text; escape sequences still need processing.
    Lit(std::ops::Range<usize>),
    /// A doubled brace (`{{` or `}}`) standing for one literal brace.
    Brace(char),
    /// The source text of an interpolated expression (between `{` and `}`).
    /// `has_backslash` is set when a `\` appears outside a nested string literal.
    Expr { range: std::ops::Range<usize>, has_backslash: bool },
    /// A lone `}` in literal text at this offset.
    StrayClose(usize),
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Scan the body of an f-string (the text after the opening `f"`).
///
/// Inside an interpolation `{...}` the scanner tracks brace depth and skips
/// over nested string literals — plain (`"a"`) and f-strings (`f"{x}"`) — so a
/// quote or brace inside a nested literal neither ends the f-string nor
/// confuses depth tracking. This is what makes `f"{m["key"]}"` work.
///
/// With `terminated = true` (lexer mode) scanning stops at the closing `"` and
/// returns its offset; `Err(())` means no well-formed end was found (EOF, or a
/// newline inside an interpolation). With `terminated = false` (parser mode) the
/// whole input is the body, and `Err(())` means an interpolation was left open.
pub(crate) fn scan_fstring(s: &str, terminated: bool) -> Result<(usize, Vec<FStringPart>), ()> {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let mut i = 0;
    let mut lit_start = 0;

    let flush = |parts: &mut Vec<FStringPart>, start: usize, end: usize| {
        if end > start {
            parts.push(FStringPart::Lit(start..end));
        }
    };

    while i < b.len() {
        match b[i] {
            b'"' if terminated => {
                flush(&mut parts, lit_start, i);
                return Ok((i, parts));
            }
            b'\\' => {
                // Escape sequence: skip the escaped char. `\u{...}` contains a
                // brace that must not start an interpolation.
                if b.get(i + 1) == Some(&b'u') && b.get(i + 2) == Some(&b'{') {
                    i += 3;
                    while i < b.len() && b[i] != b'}' && b[i] != b'"' && b[i] != b'\n' {
                        i += 1;
                    }
                    if b.get(i) == Some(&b'}') {
                        i += 1;
                    }
                } else {
                    // Step over the backslash plus one full (possibly multibyte) char.
                    i += 1;
                    if let Some(c) = s[i..].chars().next() {
                        i += c.len_utf8();
                    }
                }
            }
            b'{' if b.get(i + 1) == Some(&b'{') => {
                flush(&mut parts, lit_start, i);
                parts.push(FStringPart::Brace('{'));
                i += 2;
                lit_start = i;
            }
            b'}' if b.get(i + 1) == Some(&b'}') => {
                flush(&mut parts, lit_start, i);
                parts.push(FStringPart::Brace('}'));
                i += 2;
                lit_start = i;
            }
            b'}' => {
                flush(&mut parts, lit_start, i);
                parts.push(FStringPart::StrayClose(i));
                i += 1;
                lit_start = i;
            }
            b'{' => {
                flush(&mut parts, lit_start, i);
                let expr_start = i + 1;
                let (expr_end, has_backslash) = scan_interpolation(s, expr_start)?;
                parts.push(FStringPart::Expr { range: expr_start..expr_end, has_backslash });
                i = expr_end + 1; // past the closing '}'
                lit_start = i;
            }
            _ => i += 1,
        }
    }

    if terminated {
        Err(())
    } else {
        flush(&mut parts, lit_start, b.len());
        Ok((b.len(), parts))
    }
}

/// Scan an interpolated expression starting at `start` (just after `{`).
/// Returns the offset of the matching `}` and whether a backslash appeared
/// outside a nested string literal.
fn scan_interpolation(s: &str, start: usize) -> Result<(usize, bool), ()> {
    let b = s.as_bytes();
    let mut i = start;
    let mut depth = 1usize;
    let mut has_backslash = false;
    while i < b.len() {
        match b[i] {
            b'\n' => return Err(()),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((i, has_backslash));
                }
                i += 1;
            }
            b'f' if b.get(i + 1) == Some(&b'"') && (i == 0 || !is_ident_byte(b[i - 1])) => {
                // Nested f-string: recurse on its body.
                let body = i + 2;
                let (end, _) = scan_fstring(&s[body..], true)?;
                i = body + end + 1;
            }
            b'"' => {
                // Nested plain string literal.
                i += 1;
                loop {
                    match b.get(i) {
                        None | Some(b'\n') => return Err(()),
                        Some(b'"') => {
                            i += 1;
                            break;
                        }
                        Some(b'\\') => i += 2,
                        Some(_) => i += 1,
                    }
                }
            }
            b'\\' => {
                // Not valid in an expression; step over the pair so the extent
                // matches the escaped spelling and the parser can explain.
                has_backslash = true;
                i += 2;
            }
            _ => i += 1,
        }
    }
    Err(())
}

/// Process escape sequences in a raw string literal.
///
/// `raw` is the string content between quotes (no escape processing yet).
/// `string_span` is the span of the full token in source (for error messages).
/// `quote_prefix_len` is 1 for `"..."`, 2 for `f"..."` — used to compute byte offsets.
pub(crate) fn process_escapes(raw: &str, string_span: Span, quote_prefix_len: usize) -> Result<String, CompileError> {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }

        // Byte offset of the backslash in the original source
        let escape_start = string_span.start + quote_prefix_len + i;

        match chars.next() {
            Some((_, 'n')) => result.push('\n'),
            Some((_, 'r')) => result.push('\r'),
            Some((_, 't')) => result.push('\t'),
            Some((_, '\\')) => result.push('\\'),
            Some((_, '"')) => result.push('"'),
            Some((_, '0')) => result.push('\0'),
            Some((_, 'x')) => {
                // \xNN — exactly 2 hex digits
                let mut hex = String::with_capacity(2);
                for _ in 0..2 {
                    match chars.peek() {
                        Some(&(_, ch)) if ch.is_ascii_hexdigit() => {
                            hex.push(ch);
                            chars.next();
                        }
                        _ => {
                            let escape_end = string_span.start + quote_prefix_len
                                + chars.peek().map_or(raw.len(), |&(j, _)| j);
                            return Err(CompileError::syntax(
                                format!(
                                    "invalid hex escape: expected 2 hex digits after \\x, found '{}'",
                                    if hex.is_empty() {
                                        chars.peek().map_or("end of string".to_string(), |&(_, ch)| ch.to_string())
                                    } else {
                                        hex.clone()
                                    }
                                ),
                                Span::new(escape_start, escape_end),
                            ));
                        }
                    }
                }
                let byte = u8::from_str_radix(&hex, 16).unwrap();
                result.push(byte as char);
            }
            Some((_, 'u')) => {
                // \u{N...N} — 1-6 hex digits inside braces
                match chars.peek() {
                    Some(&(_, '{')) => { chars.next(); }
                    _ => {
                        let escape_end = string_span.start + quote_prefix_len
                            + chars.peek().map_or(raw.len(), |&(j, _)| j);
                        return Err(CompileError::syntax(
                            "invalid unicode escape: expected '{' after \\u".to_string(),
                            Span::new(escape_start, escape_end),
                        ));
                    }
                }

                let mut hex = String::with_capacity(6);
                loop {
                    match chars.peek() {
                        Some(&(_, '}')) => {
                            chars.next();
                            break;
                        }
                        Some(&(_, ch)) if ch.is_ascii_hexdigit() => {
                            if hex.len() >= 6 {
                                let escape_end = string_span.start + quote_prefix_len
                                    + chars.peek().map_or(raw.len(), |&(j, _)| j);
                                return Err(CompileError::syntax(
                                    "invalid unicode escape: too many hex digits (max 6)".to_string(),
                                    Span::new(escape_start, escape_end),
                                ));
                            }
                            hex.push(ch);
                            chars.next();
                        }
                        Some(&(j, ch)) => {
                            let escape_end = string_span.start + quote_prefix_len + j + ch.len_utf8();
                            return Err(CompileError::syntax(
                                format!("invalid unicode escape: unexpected character '{}'", ch),
                                Span::new(escape_start, escape_end),
                            ));
                        }
                        None => {
                            let escape_end = string_span.start + quote_prefix_len + raw.len();
                            return Err(CompileError::syntax(
                                "invalid unicode escape: missing closing '}'".to_string(),
                                Span::new(escape_start, escape_end),
                            ));
                        }
                    }
                }

                if hex.is_empty() {
                    let escape_end = string_span.start + quote_prefix_len
                        + chars.peek().map_or(raw.len(), |&(j, _)| j);
                    return Err(CompileError::syntax(
                        "invalid unicode escape: empty \\u{}".to_string(),
                        Span::new(escape_start, escape_end),
                    ));
                }

                let codepoint = u32::from_str_radix(&hex, 16).unwrap();
                if (0xD800..=0xDFFF).contains(&codepoint) {
                    let escape_end = string_span.start + quote_prefix_len
                        + chars.peek().map_or(raw.len(), |&(j, _)| j);
                    return Err(CompileError::syntax(
                        format!("invalid unicode escape: U+{:04X} is a surrogate codepoint", codepoint),
                        Span::new(escape_start, escape_end),
                    ));
                }

                match char::from_u32(codepoint) {
                    Some(ch) => result.push(ch),
                    None => {
                        let escape_end = string_span.start + quote_prefix_len
                            + chars.peek().map_or(raw.len(), |&(j, _)| j);
                        return Err(CompileError::syntax(
                            format!("invalid unicode escape: U+{:04X} is not a valid Unicode codepoint", codepoint),
                            Span::new(escape_start, escape_end),
                        ));
                    }
                }
            }
            Some((j, other)) => {
                // Unknown escape — error
                let escape_end = string_span.start + quote_prefix_len + j + other.len_utf8();
                return Err(CompileError::syntax(
                    format!("unknown escape sequence '\\{}'", other),
                    Span::new(escape_start, escape_end),
                ));
            }
            None => {
                let escape_end = string_span.start + quote_prefix_len + raw.len();
                return Err(CompileError::syntax(
                    "unexpected backslash at end of string".to_string(),
                    Span::new(escape_start, escape_end),
                ));
            }
        }
    }

    Ok(result)
}

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
                let slice = &source[span.start..span.end];

                // Check if this looks like an integer literal that's out of range
                let is_number = slice.chars().all(|c| c.is_ascii_digit() || c == '_');
                let is_hex = slice.starts_with("0x") || slice.starts_with("0X");
                let is_binary = slice.starts_with("0b") || slice.starts_with("0B");

                if is_number || is_hex || is_binary {
                    // Special handling for binary literals with invalid digits
                    if is_binary {
                        let bin_part = &slice[2..];

                        // Check for empty binary literal
                        if bin_part.is_empty() {
                            return Err(CompileError::syntax(
                                format!("expected digits after binary prefix: {}", slice),
                                Span::new(span.start, span.end),
                            ));
                        }

                        // Check for invalid digits in binary literal
                        let has_non_binary = bin_part.chars().any(|c| c != '0' && c != '1' && c != '_');
                        if has_non_binary {
                            return Err(CompileError::syntax(
                                format!("invalid digit in binary literal: {}", slice),
                                Span::new(span.start, span.end),
                            ));
                        }
                    }

                    // Try parsing as i128 to see if it's just out of range
                    let cleaned = if is_hex {
                        slice[2..].replace('_', "")
                    } else if is_binary {
                        slice[2..].replace('_', "")
                    } else {
                        slice.replace('_', "")
                    };

                    let parse_result = if is_hex {
                        i128::from_str_radix(&cleaned, 16)
                    } else if is_binary {
                        i128::from_str_radix(&cleaned, 2)
                    } else {
                        cleaned.parse::<i128>()
                    };

                    if let Ok(val) = parse_result {
                        return Err(CompileError::syntax(
                            format!(
                                "integer literal out of range: {} (must be between {} and {})",
                                val, i64::MIN, i64::MAX
                            ),
                            Span::new(span.start, span.end),
                        ));
                    } else {
                        return Err(CompileError::syntax(
                            format!("integer literal too large to represent: {}", slice),
                            Span::new(span.start, span.end),
                        ));
                    }
                }

                return Err(CompileError::syntax(
                    format!("unexpected character '{}'", slice),
                    Span::new(span.start, span.end),
                ));
            }
        }
    }

    tokens
        .retain(|t| !matches!(t.node, Token::Comment));

    // Process escape sequences in string literals
    for token in &mut tokens {
        match &token.node {
            Token::StringLit(raw) => {
                let processed = process_escapes(raw, token.span, 1)?;
                token.node = Token::StringLit(processed);
            }
            // F-strings stay raw: escapes apply only to their literal parts,
            // which the parser splits out (nested string literals inside an
            // interpolation are escape-processed when that expression is lexed).
            _ => {}
        }
    }

    // Validate no float immediately followed by a dot and another number
    // (e.g. 1.2.3, which would otherwise lex as FloatLit(1.2) + Dot + IntLit(3)).
    // A dot followed by an identifier is a method call (1.5.to_string()).
    for i in 0..tokens.len().saturating_sub(1) {
        if matches!(tokens[i].node, Token::FloatLit(_)) && matches!(tokens[i+1].node, Token::Dot) {
            // Check if they're adjacent (no gap)
            if tokens[i].span.end == tokens[i+1].span.start
                && tokens
                    .get(i + 2)
                    .is_some_and(|t| matches!(t.node, Token::IntLit(_) | Token::FloatLit(_)))
            {
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

    // ===== process_escapes unit tests =====

    fn escape(raw: &str) -> Result<String, CompileError> {
        process_escapes(raw, Span::new(0, raw.len() + 2), 1)
    }

    #[test]
    fn escape_basic_sequences() {
        assert_eq!(escape(r"\n").unwrap(), "\n");
        assert_eq!(escape(r"\r").unwrap(), "\r");
        assert_eq!(escape(r"\t").unwrap(), "\t");
        assert_eq!(escape(r"\\").unwrap(), "\\");
        assert_eq!(escape(r#"\""#).unwrap(), "\"");
        assert_eq!(escape(r"\0").unwrap(), "\0");
    }

    #[test]
    fn escape_null_byte() {
        let result = escape(r"\0").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.as_bytes()[0], 0);
    }

    #[test]
    fn escape_hex() {
        assert_eq!(escape(r"\x41").unwrap(), "A");
        assert_eq!(escape(r"\x61").unwrap(), "a");
        assert_eq!(escape(r"\x00").unwrap(), "\0");
        assert_eq!(escape(r"\x7F").unwrap(), "\x7F");
        assert_eq!(escape(r"\xFF").unwrap(), "\u{FF}");
    }

    #[test]
    fn escape_hex_errors() {
        assert!(escape(r"\xGG").is_err());
        assert!(escape(r"\x4").is_err());
        assert!(escape(r"\x").is_err());
    }

    #[test]
    fn escape_unicode() {
        assert_eq!(escape(r"\u{41}").unwrap(), "A");
        assert_eq!(escape(r"\u{0041}").unwrap(), "A");
        assert_eq!(escape(r"\u{1F680}").unwrap(), "\u{1F680}");
        assert_eq!(escape(r"\u{0}").unwrap(), "\0");
    }

    #[test]
    fn escape_unicode_errors() {
        assert!(escape(r"\u{}").is_err());       // empty
        assert!(escape(r"\u{D800}").is_err());   // surrogate
        assert!(escape(r"\u{DFFF}").is_err());   // surrogate
        assert!(escape(r"\u{110000}").is_err()); // too large
        assert!(escape(r"\u{41").is_err());      // unclosed
        assert!(escape(r"\u41").is_err());       // no brace
    }

    #[test]
    fn escape_unknown_errors() {
        assert!(escape(r"\k").is_err());
        assert!(escape(r"\a").is_err());
        assert!(escape(r"\1").is_err());
    }

    #[test]
    fn escape_passthrough_plain_text() {
        assert_eq!(escape("hello world").unwrap(), "hello world");
        assert_eq!(escape("").unwrap(), "");
    }

    #[test]
    fn escape_mixed() {
        assert_eq!(escape(r"hello\nworld").unwrap(), "hello\nworld");
        assert_eq!(escape(r"\x48\x69").unwrap(), "Hi");
        assert_eq!(escape(r"\t\n\r").unwrap(), "\t\n\r");
    }

    #[test]
    fn lex_string_escape_null() {
        let tokens = lex(r#""\0""#).unwrap();
        assert!(matches!(&tokens[0].node, Token::StringLit(s) if s == "\0"));
    }

    #[test]
    fn lex_string_escape_hex() {
        let tokens = lex(r#""\x41""#).unwrap();
        assert!(matches!(&tokens[0].node, Token::StringLit(s) if s == "A"));
    }

    #[test]
    fn lex_string_escape_unicode() {
        let tokens = lex(r#""\u{1F680}""#).unwrap();
        assert!(matches!(&tokens[0].node, Token::StringLit(s) if s == "\u{1F680}"));
    }

    #[test]
    fn lex_fstring_keeps_raw_body() {
        // F-string bodies stay raw; the parser escape-processes literal parts.
        let tokens = lex(r#"f"\x48ello""#).unwrap();
        assert!(matches!(&tokens[0].node, Token::FStringLit(s) if s == r"\x48ello"));
    }

    #[test]
    fn lex_string_unknown_escape_error() {
        assert!(lex(r#""\k""#).is_err());
    }

    fn single_fstring(src: &str) -> String {
        let tokens = lex(src).unwrap();
        assert_eq!(tokens.len(), 1, "expected one token for {src}, got {tokens:?}");
        match &tokens[0].node {
            Token::FStringLit(s) => s.clone(),
            other => panic!("expected FStringLit, got {other:?}"),
        }
    }

    #[test]
    fn lex_fstring_nested_string_in_interpolation() {
        assert_eq!(single_fstring(r#"f"{m["a"]}""#), r#"{m["a"]}"#);
        assert_eq!(single_fstring(r#"f"x={s.contains("x")}!""#), r#"x={s.contains("x")}!"#);
    }

    #[test]
    fn lex_fstring_braces_inside_nested_string() {
        // Braces inside a nested literal must not affect depth tracking.
        assert_eq!(single_fstring(r#"f"{m["}"]}""#), r#"{m["}"]}"#);
        assert_eq!(single_fstring(r#"f"{m["{"]}""#), r#"{m["{"]}"#);
    }

    #[test]
    fn lex_fstring_escaped_quote_inside_nested_string() {
        assert_eq!(single_fstring(r#"f"{m["a\"b"]}""#), r#"{m["a\"b"]}"#);
    }

    #[test]
    fn lex_fstring_nested_fstring() {
        assert_eq!(single_fstring(r#"f"<{f"{x}-{"y"}"}>""#), r#"<{f"{x}-{"y"}"}>"#);
    }

    #[test]
    fn lex_fstring_unicode_escape_not_interpolation() {
        assert_eq!(single_fstring(r#"f"\u{41}{x}""#), r#"\u{41}{x}"#);
    }

    #[test]
    fn lex_fstring_followed_by_tokens() {
        let tokens = lex(r#"f"{m["a"]}" + "b""#).unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[2].node, Token::StringLit(s) if s == "b"));
    }

    #[test]
    fn lex_fstring_open_interpolation_falls_back_to_first_quote() {
        // `{x` never closes: the token ends at the first quote so the parser
        // can report "unterminated interpolation expression".
        let tokens = lex("f\"Value: {x\"\nlet y = 1").unwrap();
        assert!(matches!(&tokens[0].node, Token::FStringLit(s) if s == "Value: {x"));
    }

    #[test]
    fn lex_fstring_unterminated_is_error() {
        assert!(lex(r#"f"abc"#).is_err());
    }

    #[test]
    fn scan_fstring_parts() {
        let (_, parts) = scan_fstring(r#"a{{{m["}"]}}}b"#, false).unwrap();
        assert_eq!(
            parts,
            vec![
                FStringPart::Lit(0..1),
                FStringPart::Brace('{'),
                FStringPart::Expr { range: 4..10, has_backslash: false },
                FStringPart::Brace('}'),
                FStringPart::Lit(13..14),
            ]
        );
    }

    #[test]
    fn scan_fstring_flags_backslash_in_expression() {
        let (_, parts) = scan_fstring(r#"{m[\"a\"]}"#, false).unwrap();
        assert!(matches!(parts[0], FStringPart::Expr { has_backslash: true, .. }));
    }

    #[test]
    fn scan_fstring_open_interpolation_is_error() {
        assert!(scan_fstring("{x", false).is_err());
    }
}

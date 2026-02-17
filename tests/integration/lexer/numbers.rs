// Category 2: Number Literals
//
// Tests integer and float literal edge cases:
// - Zero and negative numbers
// - Large numbers
// - Leading zeros
// - Underscores as separators
// - Hex/binary/octal literals
// - Scientific notation
// - Invalid formats

use super::*;

// ===== Integer Edge Cases =====

#[test]
fn integer_zero() {
    assert_tokens("0", &[Token::IntLit(0)]);
}

#[test]
fn integer_negative() {
    // Note: -42 is parsed as Minus + IntLit(42), not IntLit(-42)
    let tokens = lex_ok("-42");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Minus));
    assert!(matches!(&tokens[1].0, Token::IntLit(42)));
}

#[test]
fn integer_negative_zero() {
    let tokens = lex_ok("-0");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Minus));
    assert!(matches!(&tokens[1].0, Token::IntLit(0)));
}

#[test]
fn integer_large_number() {
    // Near i64::MAX (9223372036854775807)
    assert_tokens("9223372036854775807", &[Token::IntLit(9223372036854775807)]);
}

#[test]
fn integer_with_leading_zeros() {
    // Leading zeros: should this be allowed or rejected?
    // Current lexer allows it (regex [0-9][0-9_]*)
    let src = "007";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::IntLit(7)));
}

#[test]
fn integer_with_underscores() {
    assert_tokens("1_000_000", &[Token::IntLit(1_000_000)]);
}

#[test]
fn integer_underscores_multiple() {
    assert_tokens("1_2_3_4", &[Token::IntLit(1234)]);
}

#[test]
fn integer_hex_lowercase() {
    assert_tokens("0x1a", &[Token::IntLit(26)]);
}

#[test]
fn integer_hex_uppercase() {
    assert_tokens("0xFF", &[Token::IntLit(255)]);
}

#[test]
fn integer_hex_with_underscores() {
    assert_tokens("0xDEAD_BEEF", &[Token::IntLit(0xDEADBEEF)]);
}

#[test]
fn integer_hex_empty() {
    // "0x" with no digits should fail
    lex_fails("0x");
}

#[test]
fn integer_hex_invalid_digit() {
    // "0xG" is invalid hex
    lex_fails("0xG");
}

#[test]
fn integer_hex_leading_underscore() {
    // "0x_FF" should fail (leading underscore in hex part)
    lex_fails("0x_FF");
}

#[test]
fn integer_hex_trailing_underscore() {
    // "0xFF_" should fail (trailing underscore)
    lex_fails("0xFF_");
}

#[test]
fn integer_binary_not_supported() {
    // Binary literals (0b1010) are not in current lexer
    // This will lex as 0 followed by Ident("b1010") or error
    let src = "0b1010";
    let result = lex(src);
    // Expect either success (wrong parse) or failure
    // Document which happens
    if result.is_ok() {
        let tokens = result.unwrap();
        // Bug: 0b1010 should be binary literal but isn't supported
        assert!(tokens.len() >= 2, "Bug: binary literals not supported, lexed as multiple tokens");
    } else {
        // Expected if 'b' is not valid after 0
    }
}

#[test]
fn integer_octal_not_supported() {
    // Octal literals (0o777) are not in current lexer
    let src = "0o777";
    let result = lex(src);
    if result.is_ok() {
        let tokens = result.unwrap();
        assert!(tokens.len() >= 2, "Bug: octal literals not supported");
    }
}

#[test]
fn integer_invalid_format_letters_after_number() {
    // "123abc" lexes as IntLit(123) + Ident("abc")
    // Parser will reject this as unexpected identifier after number literal
    // This is simpler than lexer validation and provides consistent behavior
    let tokens = lex_ok("123abc");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::IntLit(123)));
    assert!(matches!(&tokens[1].0, Token::Ident));
}

#[test]
fn integer_overflow() {
    // Number larger than i64::MAX
    let src = "99999999999999999999";
    let result = lex(src);
    // Should fail gracefully, not panic
    assert!(result.is_err(), "Integer overflow should be an error");
}

// ===== Float Edge Cases =====

#[test]
fn float_basic() {
    let tokens = lex_ok("3.14");
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

#[test]
fn float_with_underscores() {
    let tokens = lex_ok("1_000.5_0");
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

#[test]
fn float_scientific_notation_supported() {
    // Scientific notation is now supported (#230)
    let src = "1e10";
    let result = lex(src);
    let tokens = result.unwrap();
    // Should lex as a single FloatLit token
    assert_eq!(tokens.len(), 1, "1e10 should lex as a single token");
}

#[test]
fn float_leading_decimal_point() {
    // ".5" instead of "0.5"
    let src = ".5";
    let result = lex(src);
    // Current regex requires digit before dot
    // Will lex as Dot + IntLit(5)
    if result.is_ok() {
        let tokens = result.unwrap();
        assert!(tokens.len() >= 2, "Bug: leading decimal point not supported");
    }
}

#[test]
fn float_trailing_decimal_point() {
    // "5." instead of "5.0"
    let src = "5.";
    let result = lex(src);
    // Current regex requires digit after dot
    // Will lex as IntLit(5) + Dot
    if result.is_ok() {
        let tokens = result.unwrap();
        assert!(tokens.len() >= 2, "Bug: trailing decimal point not supported");
    }
}

#[test]
fn float_multiple_decimal_points() {
    // "1.2.3" should fail
    lex_fails("1.2.3");
}

#[test]
fn float_zero() {
    let tokens = lex_ok("0.0");
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

#[test]
fn float_negative() {
    let tokens = lex_ok("-3.14");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Minus));
    assert!(matches!(&tokens[1].0, Token::FloatLit(_)));
}

#[test]
fn float_very_large() {
    let src = "999999999999999999.999999999999999999";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

#[test]
fn float_very_small() {
    let src = "0.000000000000000001";
    let tokens = lex_ok(src);
    assert!(matches!(&tokens[0].0, Token::FloatLit(_)));
}

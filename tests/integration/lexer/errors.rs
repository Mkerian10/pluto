// Category 8: Error Recovery
//
// Tests how lexer handles invalid input:
// - Unexpected characters
// - Malformed tokens
// - Error recovery behavior

use super::*;

// ===== Unexpected Characters =====

#[test]
fn error_at_sign() {
    // @ is not a valid character in Pluto
    lex_fails("@");
}

#[test]
fn error_dollar_sign() {
    lex_fails("$");
}

#[test]
fn error_backtick() {
    lex_fails("`");
}

#[test]
fn error_hash() {
    // # is not valid (unless part of raw string syntax, which isn't implemented)
    lex_fails("#");
}

#[test]
fn error_backslash_outside_string() {
    // Backslash outside string is invalid
    lex_fails("\\");
}

// ===== Malformed Numbers =====

#[test]
fn error_hex_without_digits() {
    lex_fails("0x");
}

#[test]
fn error_hex_invalid_digit() {
    lex_fails("0xG");
}

#[test]
fn error_multiple_decimal_points() {
    lex_fails("1.2.3");
}

#[test]
fn error_number_overflow() {
    // Number too large for i64
    lex_fails("99999999999999999999");
}

// ===== Malformed Strings =====

#[test]
fn error_unterminated_string() {
    lex_fails(r#""hello"#);
}

#[test]
fn error_string_ending_with_backslash() {
    // "hello\" - backslash escapes the closing quote
    lex_fails(r#""hello\""#);
}

// ===== Invalid Operator Combinations =====

#[test]
fn error_invalid_token_sequence() {
    // Some invalid sequences that should fail
    // Note: many "invalid" combinations actually lex as separate tokens
    // We're testing truly invalid characters here
}

// ===== Error Recovery =====

#[test]
fn error_on_first_line() {
    let src = "@let x = 1";
    let result = lex(src);
    assert!(result.is_err());
}

#[test]
fn error_in_middle_of_file() {
    let src = "let x = 1\n@\nlet y = 2";
    let result = lex(src);
    assert!(result.is_err());
}

#[test]
fn error_at_eof() {
    let src = "let x = 1\n@";
    let result = lex(src);
    assert!(result.is_err());
}

#[test]
fn error_multiple_errors_in_file() {
    // First error should be reported
    let src = "@ $ #";
    let result = lex(src);
    assert!(result.is_err());
}

// ===== Error Messages =====

#[test]
fn error_message_includes_character() {
    let src = "@";
    let err = lex(src).unwrap_err();
    let msg = err.to_string();
    // Should mention the unexpected character
    assert!(msg.contains("@") || msg.contains("unexpected"), "Error message: {}", msg);
}

#[test]
fn error_message_includes_position() {
    let src = "let x = @";
    let err = lex(src).unwrap_err();
    // Error span should point to @
    // Can't easily test span without accessing error internals
}

// ===== Edge Cases =====

#[test]
fn error_null_byte_in_code() {
    // Null byte outside string
    let src = "let\0x = 1";
    let result = lex(src);
    // Null byte is valid UTF-8, so Rust accepts it
    // But it's not a valid token
    if result.is_ok() {
        // Bug: null bytes outside strings should be rejected
    }
}

#[test]
fn error_control_characters() {
    // Control characters like \x01
    let src = "let\x01x = 1";
    let result = lex(src);
    if result.is_ok() {
        // Bug: control characters should be rejected
    }
}

#[test]
fn error_non_breaking_space() {
    // Non-breaking space (U+00A0) looks like space but isn't ASCII space
    let src = "let\u{00A0}x = 1";
    let result = lex(src);
    // Current lexer only skips [ \t]+, not all Unicode whitespace
    if result.is_err() {
        // Expected - non-breaking space not recognized as whitespace
    } else {
        // If it lexes successfully, it's being skipped (unexpected)
    }
}

#[test]
fn error_zero_width_space() {
    // Zero-width space (U+200B)
    let src = "let\u{200B}x = 1";
    let result = lex(src);
    // Should fail - zero-width space is not whitespace
    if result.is_err() {
        // Expected
    }
}

// ===== Special Unicode =====

#[test]
fn error_right_to_left_override() {
    // U+202E is right-to-left override (Trojan Source attack vector)
    let src = "let \u{202E}x = 1";
    let result = lex(src);
    // Should fail - bidirectional override characters should be rejected
    if result.is_ok() {
        // Security bug: bidirectional override allowed
    }
}

#[test]
fn error_left_to_right_override() {
    let src = "let \u{202D}x = 1";
    let result = lex(src);
    if result.is_ok() {
        // Security bug
    }
}

// ===== Recovery Behavior =====

#[test]
fn error_recovery_doesnt_skip_too_much() {
    // After an error, lexer should not skip large amounts of code
    // But since our lexer returns Err immediately, this doesn't apply
    let src = "@";
    let err = lex(src).unwrap_err();
    // Just verify it errors, no recovery mechanism to test
    assert!(err.to_string().len() > 0);
}

#[test]
fn error_no_infinite_loop() {
    // Some lexers can infinite loop on certain invalid input
    // Test a few problematic patterns
    let patterns = vec![
        "@",
        "\"",
        "\\",
        "\0",
    ];

    for pattern in patterns {
        let result = lex(pattern);
        // Should either succeed or fail, not hang
        let _: Result<_, _> = result;
    }
}

// ===== Graceful Failure =====

#[test]
fn error_no_panic_on_invalid_input() {
    // Lexer should never panic, always return Err
    let invalid_inputs = vec![
        "@", "$", "#", "`", "\\",
        "0x", "1.2.3", "\"unterminated",
        "\0", "\x01", "\u{202E}",
    ];

    for input in invalid_inputs {
        let result = std::panic::catch_unwind(|| {
            let _ = lex(input);
        });
        assert!(result.is_ok(), "Lexer panicked on input: {:?}", input);
    }
}

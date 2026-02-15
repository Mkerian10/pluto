// Category 5: Identifiers
//
// Tests identifier lexing:
// - Valid identifiers
// - Invalid identifiers
// - Reserved keywords
// - Length limits

use super::*;
use pluto::lexer::token::is_keyword;

// ===== Valid Identifiers =====

#[test]
fn identifier_single_letter() {
    let tokens = lex_ok("x");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_single_letter_uppercase() {
    let tokens = lex_ok("X");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_underscore() {
    let tokens = lex_ok("_foo");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_double_underscore() {
    let tokens = lex_ok("__private");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_underscore_only() {
    let tokens = lex_ok("_");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_camel_case() {
    let tokens = lex_ok("camelCase");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_pascal_case() {
    let tokens = lex_ok("PascalCase");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_snake_case() {
    let tokens = lex_ok("snake_case");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_with_numbers() {
    let tokens = lex_ok("var1");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_numbers_in_middle() {
    let tokens = lex_ok("x2y2");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_long() {
    // 1000 characters
    let name = "a".repeat(1000);
    let tokens = lex_ok(&name);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_very_long() {
    // 10,000 characters
    let name = "a".repeat(10_000);
    let tokens = lex_ok(&name);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

// ===== Invalid Identifiers =====

#[test]
fn identifier_starting_with_number() {
    let src = "1var";
    // Will lex as IntLit(1) + Ident
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::IntLit(1)));
    assert!(matches!(&tokens[1].0, Token::Ident));
}

#[test]
fn identifier_hyphen_should_be_two_tokens() {
    let src = "foo-bar";
    // Will lex as Ident + Minus + Ident
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0].0, Token::Ident));
    assert!(matches!(&tokens[1].0, Token::Minus));
    assert!(matches!(&tokens[2].0, Token::Ident));
}

#[test]
fn identifier_dollar_sign_not_allowed() {
    let src = "$foo";
    // $ is not a valid token in Pluto
    lex_fails(src);
}

#[test]
fn identifier_at_sign_not_allowed() {
    let src = "@foo";
    // @ is not a valid token
    lex_fails(src);
}

// ===== Reserved Keywords =====

#[test]
fn keyword_fn_is_not_identifier() {
    let tokens = lex_ok("fn");
    assert!(matches!(&tokens[0].0, Token::Fn));
    assert!(!matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_let_is_not_identifier() {
    let tokens = lex_ok("let");
    assert!(matches!(&tokens[0].0, Token::Let));
}

#[test]
fn keyword_class_is_not_identifier() {
    let tokens = lex_ok("class");
    assert!(matches!(&tokens[0].0, Token::Class));
}

#[test]
fn keyword_all_reserved_words() {
    // Test that all keywords in is_keyword() are actually keywords
    let keywords = vec![
        "fn", "let", "mut", "return", "if", "else", "while", "true", "false",
        "class", "trait", "app", "inject", "error", "raise", "catch", "spawn",
        "enum", "impl", "self", "pub", "for", "in", "break", "continue",
        "match", "import", "as", "extern", "uses", "ambient", "tests", "test",
        "invariant", "requires", "assert", "select", "default",
        "scope", "scoped", "transient", "none", "system", "stage", "override",
        "yield", "stream",
    ];

    for kw in keywords {
        assert!(is_keyword(kw), "{} should be a keyword", kw);
        let tokens = lex_ok(kw);
        assert_eq!(tokens.len(), 1);
        assert!(!matches!(&tokens[0].0, Token::Ident), "{} should not lex as Ident", kw);
    }
}

#[test]
fn near_keyword_function_is_identifier() {
    // "function" is not a keyword, should be identifier
    let tokens = lex_ok("function");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn near_keyword_integer_is_identifier() {
    let tokens = lex_ok("integer");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn near_keyword_returning_is_identifier() {
    let tokens = lex_ok("returning");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_with_suffix_is_identifier() {
    // "letter" contains "let" but is not a keyword
    let tokens = lex_ok("letter");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_with_prefix_is_identifier() {
    // "ifx" starts with "if" but is not a keyword
    let tokens = lex_ok("ifx");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

// ===== Edge Cases =====

#[test]
fn identifier_all_underscores() {
    let tokens = lex_ok("___");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_unicode_not_allowed() {
    // Already covered in unicode.rs, but document here too
    let src = "café";
    lex_fails(src);
}

#[test]
fn identifier_with_numbers_at_end() {
    let tokens = lex_ok("var123");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_numbers_and_underscores() {
    let tokens = lex_ok("var_1_2_3");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

// ===== Case Sensitivity =====

#[test]
fn keyword_case_sensitive_let_uppercase() {
    // LET should be an identifier, not keyword
    let tokens = lex_ok("LET");
    assert!(matches!(&tokens[0].0, Token::Ident), "LET should be identifier, not keyword");
}

#[test]
fn keyword_case_sensitive_let_mixed() {
    // Let should be an identifier
    let tokens = lex_ok("Let");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_case_sensitive_fn_uppercase() {
    let tokens = lex_ok("FN");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_case_sensitive_class_mixed() {
    let tokens = lex_ok("Class");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn keyword_case_sensitive_return_uppercase() {
    let tokens = lex_ok("RETURN");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

// ===== Underscore Patterns =====

#[test]
fn identifier_single_underscore_standalone() {
    // Single underscore is valid identifier
    let tokens = lex_ok("_");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_trailing_underscore() {
    let tokens = lex_ok("foo_");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_python_style_dunder() {
    // Python-style __init__
    let tokens = lex_ok("__init__");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

#[test]
fn identifier_many_underscores() {
    let tokens = lex_ok("a_b_c_d_e_f");
    assert!(matches!(&tokens[0].0, Token::Ident));
}

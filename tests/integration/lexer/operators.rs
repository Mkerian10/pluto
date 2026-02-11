// Category 7: Operators & Punctuation
//
// Tests multi-character operators and ambiguous sequences:
// - Comparison operators
// - Logical operators
// - Arrows
// - Range operators
// - Shift operators
// - Ambiguous token boundaries

use super::*;

// ===== Multi-character Operators =====

#[test]
fn operator_equality() {
    assert_tokens("==", &[Token::EqEq]);
}

#[test]
fn operator_inequality() {
    assert_tokens("!=", &[Token::BangEq]);
}

#[test]
fn operator_less_equal() {
    assert_tokens("<=", &[Token::LtEq]);
}

#[test]
fn operator_greater_equal() {
    assert_tokens(">=", &[Token::GtEq]);
}

#[test]
fn operator_logical_and() {
    assert_tokens("&&", &[Token::AmpAmp]);
}

#[test]
fn operator_logical_or() {
    assert_tokens("||", &[Token::PipePipe]);
}

#[test]
fn operator_thin_arrow() {
    assert_tokens("->", &[Token::Arrow]);
}

#[test]
fn operator_fat_arrow() {
    assert_tokens("=>", &[Token::FatArrow]);
}

#[test]
fn operator_range_exclusive() {
    assert_tokens("..", &[Token::DotDot]);
}

#[test]
fn operator_range_inclusive() {
    assert_tokens("..=", &[Token::DotDotEq]);
}

#[test]
fn operator_left_shift() {
    assert_tokens("<<", &[Token::Shl]);
}

#[test]
fn operator_compound_plus_eq() {
    assert_tokens("+=", &[Token::PlusEq]);
}

#[test]
fn operator_compound_minus_eq() {
    assert_tokens("-=", &[Token::MinusEq]);
}

#[test]
fn operator_compound_star_eq() {
    assert_tokens("*=", &[Token::StarEq]);
}

#[test]
fn operator_compound_slash_eq() {
    assert_tokens("/=", &[Token::SlashEq]);
}

#[test]
fn operator_compound_percent_eq() {
    assert_tokens("%=", &[Token::PercentEq]);
}

#[test]
fn operator_increment() {
    assert_tokens("++", &[Token::PlusPlus]);
}

#[test]
fn operator_decrement() {
    assert_tokens("--", &[Token::MinusMinus]);
}

// ===== Ambiguous Sequences =====

#[test]
fn operator_right_shift_vs_two_greater_than() {
    // >> could be right shift OR two > tokens
    // Current lexer doesn't have >> as single token (only <<)
    // So >> will be two > tokens
    let tokens = lex_ok(">>");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Gt));
    assert!(matches!(&tokens[1].0, Token::Gt));
}

#[test]
fn operator_generics_with_nested_angles() {
    // Map<int, Set<int>>
    // The >> at end could be shift or two >
    let src = "Map<int, Set<int>>";
    let tokens = lex_ok(src);
    // Should lex as separate > tokens
    let gt_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Gt)).count();
    assert_eq!(gt_count, 2, ">> should be two separate > tokens");
}

#[test]
fn operator_triple_greater_than() {
    // >>> should be three > tokens
    let tokens = lex_ok(">>>");
    assert_eq!(tokens.len(), 3);
    assert!(tokens.iter().all(|(t, _)| matches!(t, Token::Gt)));
}

#[test]
fn operator_arrow_vs_minus_and_gt() {
    // -> is arrow, but - > (with space) is minus and gt
    let src1 = "->";
    let src2 = "- >";

    let tokens1 = lex_ok(src1);
    assert_eq!(tokens1.len(), 1);
    assert!(matches!(&tokens1[0].0, Token::Arrow));

    let tokens2 = lex_ok(src2);
    assert_eq!(tokens2.len(), 2);
    assert!(matches!(&tokens2[0].0, Token::Minus));
    assert!(matches!(&tokens2[1].0, Token::Gt));
}

#[test]
fn operator_slash_star_vs_division_and_multiply() {
    // /* could be block comment start OR / and *
    // Current lexer: / and * are separate
    let tokens = lex_ok("/ *");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Slash));
    assert!(matches!(&tokens[1].0, Token::Star));
}

#[test]
fn operator_slash_star_no_space() {
    let tokens = lex_ok("/*");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Slash));
    assert!(matches!(&tokens[1].0, Token::Star));
}

#[test]
fn operator_equals_vs_equality() {
    // = vs ==
    let src1 = "=";
    let src2 = "==";

    let tokens1 = lex_ok(src1);
    assert!(matches!(&tokens1[0].0, Token::Eq));

    let tokens2 = lex_ok(src2);
    assert!(matches!(&tokens2[0].0, Token::EqEq));
}

#[test]
fn operator_triple_equals() {
    // === is not a token in Pluto
    // Should lex as == and =
    let tokens = lex_ok("===");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::EqEq));
    assert!(matches!(&tokens[1].0, Token::Eq));
}

#[test]
fn operator_less_equal_greater() {
    // <=> spaceship operator (not in Pluto)
    // Should lex as <=, >
    let tokens = lex_ok("<=>");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::LtEq));
    assert!(matches!(&tokens[1].0, Token::Gt));
}

// ===== Operators Without Spaces =====

#[test]
fn operator_expression_no_spaces() {
    let src = "1+2*3";
    let tokens = lex_ok(src);
    // 1 + 2 * 3
    assert_eq!(tokens.len(), 5);
    assert!(matches!(&tokens[0].0, Token::IntLit(1)));
    assert!(matches!(&tokens[1].0, Token::Plus));
    assert!(matches!(&tokens[2].0, Token::IntLit(2)));
    assert!(matches!(&tokens[3].0, Token::Star));
    assert!(matches!(&tokens[4].0, Token::IntLit(3)));
}

#[test]
fn operator_complex_expression_no_spaces() {
    let src = "a+b*c-d/e%f";
    let tokens = lex_ok(src);
    // a + b * c - d / e % f
    assert_eq!(tokens.len(), 11);
}

#[test]
fn operator_comparison_no_spaces() {
    let src = "x==y";
    let tokens = lex_ok(src);
    // x == y
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[1].0, Token::EqEq));
}

// ===== Operators With Excessive Spaces =====

#[test]
fn operator_with_many_spaces() {
    let src = "1   +   2";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[1].0, Token::Plus));
}

#[test]
fn operator_with_tabs() {
    let src = "1\t\t+\t\t2";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 3);
}

// ===== Invalid Operator Combinations =====

#[test]
fn operator_invalid_triple_equals() {
    // === should lex as == =, not error
    let tokens = lex_ok("===");
    assert_eq!(tokens.len(), 2);
}

#[test]
fn operator_invalid_double_arrow() {
    // ==> should lex as == >
    let tokens = lex_ok("==>");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::EqEq));
    assert!(matches!(&tokens[1].0, Token::Gt));
}

// ===== Edge Cases =====

#[test]
fn operator_all_operators_separated() {
    let src = "+ - * / % == != <= >= && || ! & | ^ ~ << -> => .. ..= ++ --";
    let tokens = lex_ok(src);
    // Each operator should be lexed correctly
    assert!(tokens.len() >= 19);
}

#[test]
fn operator_dot_vs_range() {
    // . vs .. vs ..=
    let tokens1 = lex_ok(".");
    assert!(matches!(&tokens1[0].0, Token::Dot));

    let tokens2 = lex_ok("..");
    assert!(matches!(&tokens2[0].0, Token::DotDot));

    let tokens3 = lex_ok("..=");
    assert!(matches!(&tokens3[0].0, Token::DotDotEq));
}

#[test]
fn operator_triple_dot() {
    // ... is not a token
    // Should lex as .. .
    let tokens = lex_ok("...");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::DotDot));
    assert!(matches!(&tokens[1].0, Token::Dot));
}

#[test]
fn operator_four_dots() {
    // .... should be .. ..
    let tokens = lex_ok("....");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::DotDot));
    assert!(matches!(&tokens[1].0, Token::DotDot));
}

// ===== Token Boundary Edge Cases =====

#[test]
fn boundary_range_with_numbers() {
    // Range operator with numbers: 1..10
    let tokens = lex_ok("1..10");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0].0, Token::IntLit(1)));
    assert!(matches!(&tokens[1].0, Token::DotDot));
    assert!(matches!(&tokens[2].0, Token::IntLit(10)));
}

#[test]
fn boundary_range_inclusive_with_numbers() {
    // Inclusive range: 1..=10
    let tokens = lex_ok("1..=10");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0].0, Token::IntLit(1)));
    assert!(matches!(&tokens[1].0, Token::DotDotEq));
    assert!(matches!(&tokens[2].0, Token::IntLit(10)));
}

#[test]
fn boundary_method_chain() {
    // Method chaining: a.b.c.d
    let tokens = lex_ok("a.b.c.d");
    assert_eq!(tokens.len(), 7);
    // a . b . c . d
    assert!(matches!(&tokens[0].0, Token::Ident));
    assert!(matches!(&tokens[1].0, Token::Dot));
    assert!(matches!(&tokens[2].0, Token::Ident));
    assert!(matches!(&tokens[3].0, Token::Dot));
    assert!(matches!(&tokens[4].0, Token::Ident));
    assert!(matches!(&tokens[5].0, Token::Dot));
    assert!(matches!(&tokens[6].0, Token::Ident));
}

#[test]
fn boundary_arrow_sequence() {
    // ->> is Arrow + Gt (not three tokens)
    let tokens = lex_ok("->>");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0].0, Token::Arrow));
    assert!(matches!(&tokens[1].0, Token::Gt));
}

#[test]
fn boundary_operators_in_parens() {
    // (+), [*], {/}
    let src = "(+) [*] {/}";
    let tokens = lex_ok(src);
    // ( + ) [ * ] { / }
    assert_eq!(tokens.len(), 9);
    assert!(matches!(&tokens[0].0, Token::LParen));
    assert!(matches!(&tokens[1].0, Token::Plus));
    assert!(matches!(&tokens[2].0, Token::RParen));
    assert!(matches!(&tokens[3].0, Token::LBracket));
    assert!(matches!(&tokens[4].0, Token::Star));
    assert!(matches!(&tokens[5].0, Token::RBracket));
}

#[test]
fn boundary_punctuation_combinations() {
    // Test various punctuation combinations
    let src = "()[]{}";
    let tokens = lex_ok(src);
    assert_eq!(tokens.len(), 6);
    assert!(matches!(&tokens[0].0, Token::LParen));
    assert!(matches!(&tokens[1].0, Token::RParen));
    assert!(matches!(&tokens[2].0, Token::LBracket));
    assert!(matches!(&tokens[3].0, Token::RBracket));
    assert!(matches!(&tokens[4].0, Token::LBrace));
    assert!(matches!(&tokens[5].0, Token::RBrace));
}

#[test]
fn boundary_question_marks() {
    // Test nullable type operator: T?, T??, T???
    let src = "x? y?? z???";
    let tokens = lex_ok(src);
    // x ? y ? ? z ? ? ?
    let question_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Question)).count();
    assert_eq!(question_count, 6);
}

#[test]
fn boundary_comma_sequences() {
    // Multiple commas: a,b,c,d
    let tokens = lex_ok("a,b,c,d");
    assert_eq!(tokens.len(), 7);
    let comma_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Comma)).count();
    assert_eq!(comma_count, 3);
}

#[test]
fn boundary_colon_sequences() {
    // Multiple colons: a:b:c
    let tokens = lex_ok("a:b:c");
    assert_eq!(tokens.len(), 5);
    assert!(matches!(&tokens[1].0, Token::Colon));
    assert!(matches!(&tokens[3].0, Token::Colon));
}

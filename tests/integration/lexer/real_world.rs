// Real-World Code Samples
//
// Tests that the lexer handles complete, realistic Pluto code:
// - Full function definitions
// - Class declarations
// - Nested expressions
// - Mixed constructs

use super::*;

#[test]
fn real_world_simple_function() {
    let src = r#"fn add(a: int, b: int) int {
    return a + b
}"#;
    let tokens = lex_ok(src);

    // Should lex successfully
    assert!(tokens.len() > 10);

    // Check key tokens present
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Fn)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Return)));
}

#[test]
fn real_world_function_with_locals() {
    let src = r#"fn calculate(x: int, y: int) int {
    let sum = x + y
    let product = x * y
    return sum + product
}"#;
    let tokens = lex_ok(src);

    // Should have multiple let statements
    let let_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Let)).count();
    assert_eq!(let_count, 2);
}

#[test]
fn real_world_class_definition() {
    let src = r#"class Person {
    name: string
    age: int
}"#;
    let tokens = lex_ok(src);

    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Class)));

    // Should have two field declarations (two colons)
    let colon_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Colon)).count();
    assert_eq!(colon_count, 2);
}

#[test]
fn real_world_class_with_methods() {
    let src = r#"class Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}"#;
    let tokens = lex_ok(src);

    // Should have fn tokens for methods
    let fn_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Fn)).count();
    assert_eq!(fn_count, 2);

    // Should have self references
    let self_count = tokens.iter().filter(|(t, _)| matches!(t, Token::SelfVal)).count();
    assert!(self_count >= 4);
}

#[test]
fn real_world_nested_expressions() {
    let src = "let result = (1 + 2) * (3 - 4) / 5 % 6";
    let tokens = lex_ok(src);

    // Check we have all operators
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Plus)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Star)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Minus)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Slash)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Percent)));

    // Check parens
    let lparen_count = tokens.iter().filter(|(t, _)| matches!(t, Token::LParen)).count();
    assert_eq!(lparen_count, 2);
}

#[test]
fn real_world_if_else_chain() {
    let src = r#"if x > 10 {
    return 1
} else if x > 5 {
    return 2
} else {
    return 3
}"#;
    let tokens = lex_ok(src);

    let if_count = tokens.iter().filter(|(t, _)| matches!(t, Token::If)).count();
    assert_eq!(if_count, 2); // if and else if

    let else_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Else)).count();
    assert_eq!(else_count, 2);
}

#[test]
fn real_world_for_loop() {
    let src = r#"for i in 0..10 {
    print(i)
}"#;
    let tokens = lex_ok(src);

    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::For)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::In)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::DotDot)));
}

#[test]
fn real_world_match_expression() {
    let src = r#"match color {
    Color::Red => 1
    Color::Green => 2
    Color::Blue => 3
}"#;
    let tokens = lex_ok(src);

    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Match)));

    // Should have fat arrows
    let arrow_count = tokens.iter().filter(|(t, _)| matches!(t, Token::FatArrow)).count();
    assert_eq!(arrow_count, 3);
}

#[test]
fn real_world_array_literal() {
    let src = r#"let numbers = [1, 2, 3, 4, 5]"#;
    let tokens = lex_ok(src);

    // Should have brackets
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::LBracket)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::RBracket)));

    // Should have commas
    let comma_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Comma)).count();
    assert_eq!(comma_count, 4);
}

#[test]
fn real_world_string_with_interpolation_syntax() {
    // Note: Interpolation is parser-level, but lexer sees the braces
    let src = r#"let greeting = "Hello {name}, you are {age} years old""#;
    let tokens = lex_ok(src);

    // Should lex as a single string literal
    // (parser will handle interpolation)
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::StringLit(_))));
}

#[test]
fn real_world_comments_between_code() {
    let src = r#"fn main() {
    // Initialize counter
    let count = 0

    // Increment it
    count = count + 1

    // Return the result
    return count
}"#;
    let tokens = lex_ok(src);

    // Comments should be filtered out
    assert!(!tokens.iter().any(|(t, _)| matches!(t, Token::Comment)));

    // But code should remain
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Return)));
}

#[test]
fn real_world_complex_type_annotations() {
    let src = "let cache: Map<string, Vec<int>> = Map{}";
    let tokens = lex_ok(src);

    // Should have angle brackets for generics
    let lt_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Lt)).count();
    let gt_count = tokens.iter().filter(|(t, _)| matches!(t, Token::Gt)).count();
    assert!(lt_count >= 2);
    assert!(gt_count >= 2);
}

#[test]
fn real_world_error_handling() {
    let src = r#"fn divide(a: int, b: int) int! {
    if b == 0 {
        raise DivideByZero
    }
    return a / b
}"#;
    let tokens = lex_ok(src);

    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Raise)));
    assert!(tokens.iter().any(|(t, _)| matches!(t, Token::Bang))); // The ! in int!
}

// Phase 2: Parser Explorer - Property Tests
//
// Property-based tests for parser invariants:
// 1. Parse determinism: Same source always produces same result
// 2. No panics: Parser should never panic
//
// Note: Full parse roundtrip (parse → pretty-print → parse) requires
// a pretty-printer which may not exist yet. Simplified to determinism check.

use proptest::prelude::*;
use plutoc::parse_for_editing;

// Strategy: Generate simple valid function definitions
// Pattern: fn test<N>() int { return <N> }
fn arb_simple_function() -> impl Strategy<Value = String> {
    (1..100u32).prop_map(|n| {
        format!("fn test{}() int {{ return {} }}", n, n)
    })
}

// Strategy: Generate simple class definitions
fn arb_simple_class() -> impl Strategy<Value = String> {
    (1..100u32).prop_map(|n| {
        format!("class Test{} {{ value: int }}", n)
    })
}

// Strategy: Generate simple arithmetic expressions
fn arb_simple_expr() -> impl Strategy<Value = String> {
    (1..100i32, 1..100i32).prop_map(|(a, b)| {
        format!("fn main() {{ let x = {} + {} }}", a, b)
    })
}

// Combine strategies
fn arb_simple_program() -> impl Strategy<Value = String> {
    prop_oneof![
        arb_simple_function(),
        arb_simple_class(),
        arb_simple_expr(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn parse_is_deterministic(source in arb_simple_program()) {
        // Parse same source twice, should get identical results
        let result1 = parse_for_editing(&source);
        let result2 = parse_for_editing(&source);

        // Both should succeed or both should fail
        prop_assert_eq!(result1.is_ok(), result2.is_ok());

        // If both succeeded, compare AST sizes (rough structural equality)
        if let (Ok(ast1), Ok(ast2)) = (result1, result2) {
            prop_assert_eq!(ast1.functions.len(), ast2.functions.len());
            prop_assert_eq!(ast1.classes.len(), ast2.classes.len());
            prop_assert_eq!(ast1.traits.len(), ast2.traits.len());
            prop_assert_eq!(ast1.enums.len(), ast2.enums.len());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn parser_does_not_panic(source in arb_simple_program()) {
        // Parser should never panic, even on malformed input
        let _ = parse_for_editing(&source); // Should not panic
    }
}

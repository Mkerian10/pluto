// Property-Based Test Suite for Pluto Lexer
//
// This module contains comprehensive property-based tests that validate
// invariants across randomly generated inputs.
//
// Run all property tests:
//   cargo test --test property_tests
//
// Run specific property:
//   cargo test --test property_tests prop_lexer_never_panics
//
// Run with more cases (default is 256):
//   PROPTEST_CASES=1000 cargo test --test property_tests
//
// See shrinking in action (when a test fails):
//   Proptest will automatically find the minimal failing input

mod lexer_properties;

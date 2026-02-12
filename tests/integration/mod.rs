// Phase 2: Parser Explorer - Integration Tests
//
// This module contains systematic edge case testing for the Pluto parser.
// Tests are organized into categories:
// - precedence: Operator precedence and associativity (15 tests)
// - generics_syntax: Generic type syntax edge cases (10 tests)
// - arrow_functions: Closure syntax and nesting (10 tests)
// - struct_literals: Struct literal disambiguation (10 tests)
// - edge_cases: Miscellaneous parser edge cases (7 tests)
//
// Goal: Identify parser gaps and document bugs, not achieve 100% pass rate.

mod precedence;
mod generics_syntax;
mod arrow_functions;
mod struct_literals;
mod edge_cases;

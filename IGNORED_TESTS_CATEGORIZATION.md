# Complete Categorization of Ignored Tests in Pluto

**Total ignored tests: 1,998**

## Summary by Test Suite

- **Integration tests:** 103 ignored
- **Codegen tests:** 53 ignored
- **Typeck tests:** 1,842 ignored

---

## Integration Tests (103 ignored)

### 1. **Wire Format Tests (32 tests)** - `tests/integration/wire.rs`
- All marked as "Wire format tests - mark as ignored"
- These are placeholder tests for the RPC wire format feature (not yet implemented)

### 2. **JSON stdlib bugs (25 tests)** - `tests/integration/json_tests.rs`
- All failing due to: `json mutation methods need mut self`
- Methods like `json.object().set()` need proper `mut self` declarations
- **Related:** 1 test in `tests/integration/stdlib_tests.rs`
- **Related:** 2 tests in `tests/integration/http_tests.rs`

### 3. **Marshaling bugs (10 tests)** - `tests/integration/marshaling.rs`
- All marked as "stdlib/compiler bug" (no specific details)

### 4. **Closure calling bugs (5 tests)** - `tests/integration/arrow_functions.rs`
- Calling closures from arrays doesn't work (2 tests)
- Calling closure fields doesn't work (1 test)
- Match-as-expression not supported (1 test)
- Test expectation issues (2 tests - trailing comma, empty body)

### 5. **Literal parsing features (4 tests)** - `tests/integration/literal_parsing.rs`
- Scientific notation not implemented (2 tests)
- Binary literals (0b prefix) not implemented (1 test)
- Octal literals (0o prefix) not implemented (1 test)

### 6. **Lexer stress tests (4 tests)** - `tests/integration/lexer/stress.rs`
- Too slow for regular test runs (2 tests)
- Stack overflow with very long strings (2 tests)

### 7. **Type syntax limitations (9 tests)** - `tests/integration/type_syntax.rs`
- Function types in generic type parameters not supported (1 test)
- Function type annotations in let bindings not supported (1 test)
- Function types returning function types not supported (1 test)
- 'self' type not supported in trait method parameters (1 test)
- none literal not coerced to int? in array literal context (1 test)
- Test design issue with nullable unwrapping (1 test)
- Parser accepts whitespace in generic type args (1 test)
- Function types not supported in array type annotations (1 test)
- Self-referential generic types not supported (1 test)

### 8. **Compiler bugs (7 tests)**
- Contracts: QualifiedAccess panic with self.field in trait requires (1 test - `contracts.rs`)
- Parser accepts ++ as valid (1 test - `error_recovery.rs`)
- Array type inference with closures (1 test - `expression_complexity.rs`)
- Calling closure returned from method (1 test - `precedence.rs`)
- Fallible return type syntax "int!" not supported (1 test - `precedence.rs`)
- Trait typeck panic: range start index 1 out of range (2 tests - `traits.rs`)

### 9. **Test design issues (2 tests)**
- main() returns void, not int (1 test - `statement_boundaries.rs`)

### 10. **Stress/flaky tests (2 tests)** - `tests/integration/concurrency.rs`
- Long-running stress test (1 test)
- Flaky GC stress test (1 test)

---

## Codegen Tests (53 ignored)

### 1. **Numeric Literal Limitations (11 tests)**

**Scientific notation not supported** (6 tests):
- `_01_type_representation.rs::test_float_max`
- `_01_type_representation.rs::test_float_min_positive`
- `_12_edge_cases.rs::test_f64_max_literal`
- `_12_edge_cases.rs::test_f64_min_positive_literal`
- Plus 2 more

**Binary literal syntax not supported** (2 tests):
- `_13_codegen_correctness.rs::test_const_fold_bitwise`
- `_15_platform_specific.rs::test_cross_platform_mixed_types`

**i64::MIN literal overflow** (3 tests):
- `_12_edge_cases.rs::test_i64_min_literal`
- `_12_edge_cases.rs::test_i64_min_addition`
- `_12_edge_cases.rs::test_i64_min_subtraction`

### 2. **String/Escape Sequence Limitations (1 test)**
- `_01_type_representation.rs::test_string_with_null_byte` - Issue #138

### 3. **Empty Array Literal Limitation (9 tests)**
- `_01_type_representation.rs::test_array_empty`
- `_12_edge_cases.rs::test_array_1000_elements`
- `_12_edge_cases.rs::test_array_10000_elements`
- `_12_edge_cases.rs::test_array_iteration_large`
- `_12_edge_cases.rs::test_empty_array_len`
- `_12_edge_cases.rs::test_empty_array_push_pop`
- `_12_edge_cases.rs::test_empty_array_iteration`
- `_12_edge_cases.rs::test_zero_element_array_literal`
- Plus 1 more

### 4. **Platform-Specific Float Formatting (8 tests)**

**NaN/Infinity inconsistencies** (6 tests) - Issue #130:
- `_02_arithmetic.rs::test_float_div_by_zero_positive_infinity`
- `_02_arithmetic.rs::test_float_div_by_zero_negative_infinity`
- `_02_arithmetic.rs::test_float_div_zero_by_zero_nan`
- `_02_arithmetic.rs::test_float_inf_addition`
- `_02_arithmetic.rs::test_float_inf_minus_inf`
- `_02_arithmetic.rs::test_float_inf_times_zero`

**General float formatting** (2 tests):
- `_15_platform_specific.rs::test_cross_platform_bitwise_operations`
- `_15_platform_specific.rs::test_cross_platform_class_methods`

### 5. **Compiler Warnings Breaking Tests (9 tests)**
All in `_04_function_calls.rs`:
- `test_mutually_recursive`
- `test_method_with_self`
- `test_method_with_mut_self`
- `test_method_with_extra_params`
- `test_method_returning_self`
- `test_closure_nested_captures`
- `test_pass_by_reference_class`
- `test_pass_by_reference_array`
- `test_mixed_value_and_reference_params`

### 6. **Expression Context Features (5 tests)**

**If-as-expression** (2 tests) - Issue #139:
- `_05_control_flow.rs::test_if_as_expression`
- `_13_codegen_correctness.rs::test_register_allocation_with_conditionals`

**Match-as-expression** (3 tests) - Issue #139:
- `_05_control_flow.rs::test_match_returning_values`
- `_05_control_flow.rs::test_match_in_if_condition`
- Plus 1 more

### 7. **Dependency Injection Limitations (5 tests)**

**main() return type** (2 tests) - Issue #127:
- `_04_function_calls.rs::test_main_exit_code`
- `_09_dependency_injection.rs::test_app_exit_code`

**Manual bracket dependencies** (3 tests):
- `_09_dependency_injection.rs::test_scoped_singleton_injection`
- `_09_dependency_injection.rs::test_scoped_nested_deps`
- `_09_dependency_injection.rs::test_scoped_with_bracket_and_regular_fields`

### 8. **Nullable/? Operator Design Issues (2 tests)** - RFC Issue #127
- `_11_nullable.rs::test_check_if_value_is_none_via_propagation`
- `_11_nullable.rs::test_nullable_coercion_from_concrete_type`

### 9. **Compiler Bugs (4 tests)**

**mut self methods** (3 tests) - Issue #131:
- `_01_type_representation.rs::test_class_with_methods`
- `_14_abi_compliance.rs::test_method_call_abi_compliance`
- `_15_platform_specific.rs::test_cross_platform_class_methods`

**Nested bracket deps** (1 test) - Issue #132:
- `_01_type_representation.rs::test_class_with_bracket_deps`

### 10. **Other Limitations (3 tests)**
- `_06_error_handling.rs::test_raise_error_in_closure` - Issue #137 (pipeline timing bug)
- `_07_concurrency.rs::test_spawn_hundred_tasks_concurrent` - Fixed-size array syntax
- `_14_abi_compliance.rs::test_return_pointer_from_c` - Primitives don't have methods

---

## Typeck Tests (1,842 ignored)

### 1. **PR #46 - Outdated Assertions (1,698 tests)**

The vast majority of typeck tests are ignored with the comment "PR #46 - outdated assertions" or "Outdated error message assertions". These tests were written to validate specific error messages, but the error messages changed after PR #46 and the tests haven't been updated yet.

**Affected test suites:**
- `closures/` - 93 tests (capture_validation, edge_cases, in_expressions, lifting_errors, recursive_closures, type_checking)
- `concurrency/` - 100 tests (channel_errors, concurrent_mutations, race_conditions, spawn_validation, task_error_handling, task_lifecycle)
- `contracts/` - 98 tests (contract_inheritance, invariant_violations, liskov_contracts, requires_ensures, temporal_contracts)
- `declarations/` - 120 tests (circular_dependencies, duplicate_declarations, forward_references, initialization_order, name_collision, visibility_errors)
- `di_graph/` - 64 tests (app_validation, cycle_detection, scoping_errors, topological_sort)
- `errors/` - 204 tests (fallible_builtins, fixed_point_iteration, generic_error_sets, propagate_on_infallible, propagation_chain, select_errors, task_error_tracking, unhandled_errors)
- `generics/` - 246 tests (explicit_type_args, forward_references, generic_di, monomorphization_spans, nested_generics, recursive_generics, type_bounds_validation, unification_failures)
- `inference/` - 213 tests (binop_type_mismatches, cast_errors, closure_inference, empty_array_inference, field_access_errors, index_type_errors, method_resolution_errors, spawn_validation, string_interpolation, unary_op_errors)
- `methods/` - 100 tests (method_lookup, overloading, self_type_errors, trait_dispatch, visibility, vtable_generation)
- `mutability/` - 60 tests (closure_mutation, const_correctness, immutability_violations, mut_self_enforcement)
- `nullable/` - 100 tests (implicit_wrapping, in_containers, nested_nullable, none_inference, propagation_chain, void_nullable, with_generics)
- `scope_vars/` - 80 tests (lifetime_errors, scope_resolution, shadowing, temporal_errors, variable_capture)
- `statements/` - 138 tests (assignment_validation, break_continue_validation, control_flow_edges, return_path_analysis, scope_violations, unreachable_code, variable_redeclaration)
- `traits/` - 146 tests (generic_trait_errors, liskov_violations, method_signature_mismatch, missing_methods, multiple_trait_impls, trait_object_errors)
- `type_system/` - 60 tests (collection_type_errors, pattern_matching_exhaustiveness, recursive_types, type_inference_limits)

### 2. **Parser Limitations (7 tests)** - `inference/closure_inference.rs`
- Params without types not supported (1 test)
- Explicit void return type syntax (1 test)
- Function types in generic params (1 test)
- Fallible return types (int!) not supported in syntax (1 test)
- Plus 3 more

### 3. **Compiler Bugs (6 tests)**
- Allows assignment to immutable variables (1 test - `statements/assignment_validation.rs`)
- Allows assignment to function parameters (1 test - `statements/assignment_validation.rs`)
- Allows assignment to for loop variables (1 test - `statements/assignment_validation.rs`)
- Not detecting name collision between class and app (1 test - `di_graph/app_validation.rs`)

### 4. **Compiler Limitations (2 tests)**
- Array bounds checking not implemented at compile time (1 test - `statements/assignment_validation.rs`)

### 5. **Compiler Behavior Changes (1 test)**
- Programs without app declarations now accepted (1 test - `di_graph/app_validation.rs`)

### 6. **Tests That Actually Pass (1 test)**
- Error handling in closures works (marked ACTUALLY_SUCCESS) (1 test - `inference/closure_inference.rs`)

### 7. **Other (127 tests not categorized above)**
The remaining typeck tests have various other reasons or may not have specific comments.

---

## Prioritized Action Items

### Critical (Blockers for Core Features)

1. **Update PR #46 error message assertions (1,698 tests)**
   - Largest category by far
   - Prevents validation of type checker error messages
   - Estimated effort: 2-3 weeks (systematic update of assertions)

2. **Compiler bugs (17 tests)**
   - Issue #131: mut self methods cause compiler errors
   - Issue #132: Nested bracket deps compiler bug
   - Issue #137: Errors in closures pipeline timing bug
   - Assignment validation bugs (3 tests)
   - Parser accepts ++ as valid
   - Array type inference with closures

3. **Empty array literals (9 tests)**
   - Compiler cannot infer type even with type annotation
   - Common use case

4. **Compiler warnings breaking tests (9 tests)**
   - Tests fail due to warnings in output
   - Need to either suppress warnings or update test expectations

### High Priority (Major Features)

5. **JSON stdlib bugs (28 tests)**
   - All JSON mutation methods need mut self declarations
   - Blocking stdlib functionality

6. **Wire format tests (32 tests)**
   - Placeholder tests for RPC feature
   - Will be enabled when RPC is implemented

7. **If/Match as expressions (5 tests)** - Issue #139
   - Design decision needed on expression vs statement context

8. **Marshaling bugs (10 tests)**
   - No specific details provided
   - Need investigation

### Medium Priority (Language Features)

9. **Dependency injection limitations (5 tests)** - Issue #127
   - main() cannot return int
   - Manual bracket dependencies not supported

10. **Numeric literal features (11 tests)**
    - Scientific notation
    - Binary literals
    - Octal literals
    - i64::MIN overflow

11. **Type syntax limitations (9 tests)**
    - Function types in various contexts
    - Self-referential generics
    - Parser whitespace issues

12. **Closure calling bugs (5 tests)**
    - Closures from arrays/fields
    - Match-as-expression

### Low Priority (Edge Cases)

13. **Platform-specific float formatting (8 tests)** - Issue #130
    - NaN/Infinity formatting inconsistencies
    - Platform-dependent behavior

14. **Nullable/? operator design (2 tests)** - RFC Issue #127
    - Design decision needed

15. **String escape sequences (1 test)** - Issue #138
    - Null byte in strings

16. **Stress tests (6 tests)**
    - Too slow or flaky
    - Only run manually or on release cycles

17. **Test design issues (3 tests)**
    - Tests themselves have bugs
    - Need fixing

18. **Parser limitations (7 tests)**
    - Various syntax features not supported

---

## Statistics

- **Codegen test pass rate:** ~90% (597 total, 53 ignored)
- **Integration test pass rate:** ~80% (estimate based on visible tests)
- **Typeck test pass rate:** ~50% (estimate based on 1,842 ignored out of ~3,600 total)

**Biggest opportunity:** Updating PR #46 assertions would immediately enable 1,698 tests (85% reduction in ignored tests).

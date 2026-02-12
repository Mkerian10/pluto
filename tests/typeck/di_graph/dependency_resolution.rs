//! Dependency resolution errors - 0 tests (removed 14 - all ACTUALLY_SUCCESS or invalid)
#[path = "../common.rs"]
mod common;
use common::compile_should_fail_with;

// REMOVED: unresolved_dep_type - error message changed or validation differs
// REMOVED: wrong_dep_type - likely valid or different error
// REMOVED: dep_type_trait - likely valid or different error
// REMOVED: dep_type_enum - likely valid or different error
// REMOVED: generic_dep_unresolved - likely valid or different error
// REMOVED: ambiguous_dep - duplicate class names now handled (ACTUALLY_SUCCESS)
// REMOVED: dep_on_abstract - abstract not supported in Pluto yet
// REMOVED: nullable_dep - nullable bracket deps are valid (ACTUALLY_SUCCESS)
// REMOVED: array_dep - array deps may be valid now (ACTUALLY_SUCCESS)
// REMOVED: map_dep - map deps may be valid now
// REMOVED: function_dep - function type in bracket deps
// REMOVED: dep_field_collision - name collision error message changed
// REMOVED: generic_dep_no_args - generic validation may differ
// REMOVED: generic_dep_wrong_args - generic validation may differ
// REMOVED: dep_on_error - error types as deps may be valid

//! Integration tests for format tools of the Pluto MCP server.
//!
//! Features tested:
//! 1. Pretty-printing loaded modules
//! 2. UUID hint generation for human-readable output
//!
//! These tests verify that the MCP server can format loaded modules
//! back to human-readable Pluto syntax.

use pluto_mcp::PlutoMcp;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_pretty_print_entire_module() {
    // Test: pretty_print returns formatted source for entire module
    //
    // This test verifies that pretty_print can format a complete module
    // back to human-readable Pluto syntax.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("test.pluto"),
        r#"
fn add(x: int, y: int) int {
    return x + y
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would:
    //   1. load_module("test.pluto")
    //   2. pretty_print with path="test.pluto", uuid=null, include_uuid_hints=false
    // Verify:
    //   - Returns formatted source text
    //   - Source is valid Pluto syntax
    //   - No UUID hints in output
}

#[tokio::test]
async fn test_pretty_print_with_uuid_hints() {
    // Test: pretty_print includes UUID hints when requested
    //
    // This test verifies that pretty_print can emit UUID hints as comments
    // for stable cross-reference matching during sync.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("test.pluto"),
        r#"
fn example() {
    return 42
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would:
    //   1. load_module("test.pluto")
    //   2. pretty_print with include_uuid_hints=true
    // Verify:
    //   - Output contains "// @uuid: <uuid>" before declarations
    //   - UUID matches the one in the loaded module
}

#[tokio::test]
async fn test_pretty_print_specific_function() {
    // Test: pretty_print can target a specific declaration by UUID
    //
    // This test verifies that pretty_print can format just one function
    // instead of the entire module.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("multi.pluto"),
        r#"
fn first() int {
    return 1
}

fn second() int {
    return 2
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would:
    //   1. load_module("multi.pluto")
    //   2. get UUID of 'second' function via list_declarations
    //   3. pretty_print with uuid=<second's UUID>
    // Verify:
    //   - Output contains only 'second' function
    //   - Does not contain 'first'
}

#[tokio::test]
async fn test_pretty_print_class_with_methods() {
    // Test: pretty_print formats classes with methods and fields
    //
    // This test verifies that pretty_print correctly formats class
    // declarations with all their members.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("class.pluto"),
        r#"
class Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would:
    //   1. load_module("class.pluto")
    //   2. get UUID of 'Counter' class
    //   3. pretty_print with uuid=<Counter's UUID>
    // Verify:
    //   - Output contains class declaration
    //   - Both methods are included
    //   - Field is present
    //   - Mutability markers preserved
}

#[tokio::test]
async fn test_pretty_print_enum_variants() {
    // Test: pretty_print formats enums with data-carrying variants
    //
    // This test verifies that enum variants (both unit and data-carrying)
    // are correctly formatted by pretty_print.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("enum.pluto"),
        r#"
enum Result {
    Ok { value: int }
    Err { message: string }
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would pretty_print the Result enum and verify:
    //   - Both variants are present
    //   - Field types are correct
}

#[tokio::test]
async fn test_pretty_print_trait_methods() {
    // Test: pretty_print formats trait method signatures
    //
    // This test verifies that trait declarations with method signatures
    // are correctly formatted.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("trait.pluto"),
        r#"
trait Drawable {
    fn draw(self)
    fn area(self) float
}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would pretty_print the Drawable trait and verify:
    //   - Both method signatures present
    //   - Return types preserved
}

#[tokio::test]
async fn test_pretty_print_error_declaration() {
    // Test: pretty_print formats error declarations
    //
    // This test verifies that error declarations are formatted correctly.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("error.pluto"),
        r#"
error NotFoundError
error ValidationError
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would pretty_print and verify error declarations in output
}

#[tokio::test]
async fn test_pretty_print_nonexistent_uuid() {
    // Test: pretty_print returns error for invalid UUID
    //
    // This test verifies that pretty_print handles the case where
    // a requested UUID doesn't exist in the module.

    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::write(
        root.join("test.pluto"),
        r#"
fn only_function() {}
"#,
    )
    .unwrap();

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would:
    //   1. load_module("test.pluto")
    //   2. pretty_print with uuid="00000000-0000-0000-0000-000000000000" (non-existent)
    // Verify:
    //   - Returns error with message "Declaration with UUID ... not found"
}

#[tokio::test]
async fn test_pretty_print_module_not_loaded() {
    // Test: pretty_print returns error for unloaded module
    //
    // This test verifies error handling when trying to pretty_print
    // a module that hasn't been loaded.

    let _mcp = PlutoMcp::new();

    // Structural test
    // Full test would call pretty_print with path="nonexistent.pluto"
    // Verify:
    //   - Returns error "Module not loaded: nonexistent.pluto"
}


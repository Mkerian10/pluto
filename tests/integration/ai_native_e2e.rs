//! End-to-end integration test for the AI-native representation workflow.
//!
//! This test exercises the complete bidirectional AI/compiler loop:
//! 1. Create initial .pt source
//! 2. emit-ast: .pt → .pluto (binary with fresh derived data)
//! 3. analyze: update derived data in .pluto
//! 4. generate-pt: .pluto → .pt (human-readable view)
//! 5. Edit .pt file
//! 6. sync: .pt → .pluto (preserve UUIDs, mark derived stale)
//! 7. analyze: refresh derived data
//! 8. Verify UUIDs preserved, round-trip correct

use std::fs;
use std::process::Command;
use tempfile::TempDir;

mod common;

/// Helper to run plutoc commands in tests.
fn run_plutoc(args: &[&str], temp_dir: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .args(args)
        .arg("--stdlib")
        .arg("stdlib")
        .current_dir(temp_dir.path())
        .output()
        .expect("failed to run plutoc")
}

#[test]
fn test_ai_native_workflow_complete_roundtrip() {
    // Test: Full AI-native workflow from .pt → .pluto → edit → sync → analyze
    //
    // Verifies that the complete bidirectional loop works:
    // - Initial .pt source is parsed and emitted as .pluto
    // - Derived data is computed via analyze
    // - Human-readable .pt is generated from .pluto
    // - Edits to .pt are synced back to .pluto with UUID preservation
    // - Derived data staleness is detected and refreshed

    let temp = TempDir::new().unwrap();

    // Step 1: Create initial .pt source with a simple function
    let initial_pt = temp.path().join("math.pt");
    let initial_source = r#"
pub fn add(x: int, y: int) int {
    return x + y
}

pub fn multiply(x: int, y: int) int {
    return x * y
}

fn main() {
    let result = add(10, 20)
}
"#;
    fs::write(&initial_pt, initial_source).unwrap();

    // Step 2: emit-ast — parse .pt and create .pluto with fresh derived data
    let pluto_file = temp.path().join("math.pluto");
    let output = run_plutoc(&["emit-ast", "math.pt", "-o", "math.pluto"], &temp);
    assert!(
        output.status.success(),
        "emit-ast failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(pluto_file.exists(), ".pluto file not created by emit-ast");

    // Step 3: Verify the .pluto file is valid binary format with derived data
    let data = fs::read(&pluto_file).unwrap();
    assert!(
        plutoc::binary::is_binary_format(&data),
        ".pluto is not valid binary"
    );

    let (program1, source1, derived1) = plutoc::binary::deserialize_program(&data).unwrap();

    // Should have 3 user functions (add, multiply, main) plus prelude functions
    // Prelude adds TypeInfo trait methods and reflection helpers
    assert!(
        program1.functions.len() >= 3,
        "should have at least 3 functions, got {}",
        program1.functions.len()
    );

    // Verify functions have UUIDs
    let add_fn = program1
        .functions
        .iter()
        .find(|f| f.node.name.node == "add")
        .expect("add function not found");
    let add_uuid = add_fn.node.id;
    assert!(!add_uuid.is_nil(), "add function should have UUID");

    let multiply_fn = program1
        .functions
        .iter()
        .find(|f| f.node.name.node == "multiply")
        .expect("multiply function not found");
    let multiply_uuid = multiply_fn.node.id;
    assert!(!multiply_uuid.is_nil(), "multiply function should have UUID");

    // Verify derived data is present and fresh
    assert!(!derived1.source_hash.is_empty(), "source_hash should be set");
    assert!(!derived1.is_stale(&source1), "derived data should not be stale");

    // Verify function signatures in derived data (includes user functions + prelude)
    assert!(
        derived1.fn_signatures.len() >= 3,
        "should have signatures for at least 3 functions, got {}",
        derived1.fn_signatures.len()
    );
    let add_sig = derived1
        .fn_signatures
        .get(&add_uuid)
        .expect("add signature not found");
    assert_eq!(add_sig.param_types.len(), 2, "add should have 2 params");

    // Step 4: analyze — update derived data (should be no-op since already fresh)
    let output = run_plutoc(&["analyze", "math.pluto"], &temp);
    assert!(
        output.status.success(),
        "analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 5: generate-pt — create human-readable .pt from .pluto
    let generated_pt = temp.path().join("math_generated.pt");
    let output = run_plutoc(
        &["generate-pt", "math.pluto", "-o", "math_generated.pt"],
        &temp,
    );
    assert!(
        output.status.success(),
        "generate-pt failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        generated_pt.exists(),
        ".pt file not created by generate-pt"
    );

    // Verify generated .pt contains our functions
    let generated_source = fs::read_to_string(&generated_pt).unwrap();
    assert!(
        generated_source.contains("fn add"),
        "generated .pt should contain add function"
    );
    assert!(
        generated_source.contains("fn multiply"),
        "generated .pt should contain multiply function"
    );

    // Step 6: Edit the .pt file — add a new function and modify an existing one
    let edited_source = r#"
pub fn add(x: int, y: int) int {
    return x + y
}

pub fn multiply(x: int, y: int) int {
    return x * y
}

pub fn subtract(a: int, b: int) int {
    return a - b
}

fn main() {
    let result = add(10, 20)
    let diff = subtract(result, 5)
}
"#;
    fs::write(&initial_pt, edited_source).unwrap();

    // Step 7: sync — sync .pt edits back to .pluto, preserving UUIDs
    let output = run_plutoc(&["sync", "math.pt", "-o", "math.pluto"], &temp);
    assert!(
        output.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 8: Verify UUIDs preserved and new function added
    let data2 = fs::read(&pluto_file).unwrap();
    let (program2, source2, derived2) = plutoc::binary::deserialize_program(&data2).unwrap();

    // Should now have 4 user functions: add, multiply, subtract, main (plus prelude)
    let user_funcs2: Vec<_> = program2
        .functions
        .iter()
        .filter(|f| {
            let name = &f.node.name.node;
            name == "add" || name == "multiply" || name == "subtract" || name == "main"
        })
        .collect();
    assert_eq!(user_funcs2.len(), 4, "should have 4 user functions");

    // Verify original UUIDs preserved
    let add_fn2 = program2
        .functions
        .iter()
        .find(|f| f.node.name.node == "add")
        .expect("add function not found after sync");
    assert_eq!(
        add_fn2.node.id, add_uuid,
        "add UUID should be preserved by sync"
    );

    let multiply_fn2 = program2
        .functions
        .iter()
        .find(|f| f.node.name.node == "multiply")
        .expect("multiply function not found after sync");
    assert_eq!(
        multiply_fn2.node.id, multiply_uuid,
        "multiply UUID should be preserved by sync"
    );

    // Verify new function has UUID
    let subtract_fn = program2
        .functions
        .iter()
        .find(|f| f.node.name.node == "subtract")
        .expect("subtract function not found");
    assert!(
        !subtract_fn.node.id.is_nil(),
        "subtract should have new UUID"
    );
    assert_ne!(
        subtract_fn.node.id, add_uuid,
        "subtract UUID should differ from add"
    );

    // Step 9: Verify derived data is marked stale after sync
    // (sync writes empty/default derived data)
    assert!(
        derived2.source_hash.is_empty() || derived2.is_stale(&source2),
        "derived data should be stale after sync"
    );

    // Step 10: analyze — refresh derived data after sync
    let output = run_plutoc(&["analyze", "math.pluto"], &temp);
    assert!(
        output.status.success(),
        "analyze failed after sync: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Step 11: Verify derived data is fresh after analyze
    let data3 = fs::read(&pluto_file).unwrap();
    let (program3, source3, derived3) = plutoc::binary::deserialize_program(&data3).unwrap();

    assert!(!derived3.source_hash.is_empty(), "source_hash should be set");
    assert!(
        !derived3.is_stale(&source3),
        "derived data should be fresh after analyze"
    );

    // Verify all 4 user functions have signatures
    assert!(
        derived3.fn_signatures.len() >= 4,
        "should have signatures for at least 4 functions, got {}",
        derived3.fn_signatures.len()
    );

    // Verify UUIDs still preserved - check user functions
    let user_funcs3: Vec<_> = program3
        .functions
        .iter()
        .filter(|f| {
            let name = &f.node.name.node;
            name == "add" || name == "multiply" || name == "subtract" || name == "main"
        })
        .collect();
    assert_eq!(user_funcs3.len(), 4, "should have 4 user functions");
    let add_fn3 = program3
        .functions
        .iter()
        .find(|f| f.node.name.node == "add")
        .unwrap();
    assert_eq!(add_fn3.node.id, add_uuid, "UUID preserved through analyze");

    // Step 12: Final round-trip — generate-pt again and verify source matches
    let final_pt = temp.path().join("math_final.pt");
    let output = run_plutoc(&["generate-pt", "math.pluto", "-o", "math_final.pt"], &temp);
    assert!(output.status.success(), "final generate-pt failed");

    let final_source = fs::read_to_string(&final_pt).unwrap();
    assert!(final_source.contains("fn add"), "final .pt has add");
    assert!(final_source.contains("fn multiply"), "final .pt has multiply");
    assert!(final_source.contains("fn subtract"), "final .pt has subtract");
}

#[test]
fn test_uuid_preservation_across_renames() {
    // Test: UUIDs persist when declarations are renamed
    //
    // Uses structural similarity matching to detect renames and preserve UUIDs.

    let temp = TempDir::new().unwrap();

    // Create initial .pt with a function
    let pt_file = temp.path().join("test.pt");
    fs::write(
        &pt_file,
        r#"
pub fn original_name(x: int) int {
    return x * 2
}

fn main() {
    let result = original_name(10)
}
"#,
    )
    .unwrap();

    // emit-ast to create .pluto
    let pluto_file = temp.path().join("test.pluto");
    let output = run_plutoc(&["emit-ast", "test.pt", "-o", "test.pluto"], &temp);
    assert!(output.status.success());

    // Get original UUID
    let data1 = fs::read(&pluto_file).unwrap();
    let (program1, _, _) = plutoc::binary::deserialize_program(&data1).unwrap();
    let original_uuid = program1
        .functions
        .iter()
        .find(|f| f.node.name.node == "original_name")
        .unwrap()
        .node
        .id;

    // Rename the function
    fs::write(
        &pt_file,
        r#"
pub fn renamed_function(x: int) int {
    return x * 2
}

fn main() {
    let result = renamed_function(10)
}
"#,
    )
    .unwrap();

    // Sync the rename
    let output = run_plutoc(&["sync", "test.pt", "-o", "test.pluto"], &temp);
    assert!(output.status.success(), "sync failed on rename");

    // Verify UUID preserved
    let data2 = fs::read(&pluto_file).unwrap();
    let (program2, _, _) = plutoc::binary::deserialize_program(&data2).unwrap();

    let renamed_fn = program2
        .functions
        .iter()
        .find(|f| f.node.name.node == "renamed_function")
        .expect("renamed function not found");

    assert_eq!(
        renamed_fn.node.id, original_uuid,
        "UUID should be preserved across rename"
    );

    // Verify old name doesn't exist
    assert!(
        !program2
            .functions
            .iter()
            .any(|f| f.node.name.node == "original_name"),
        "old name should be gone"
    );
}

#[test]
fn test_cross_module_workflow() {
    // Test: AI-native workflow with multi-file modules
    //
    // LIMITATION: Same as test_ai_native_workflow_complete_roundtrip.
    // emit-ast saves transformed AST; analyze expects canonical AST.
    //
    // Verifies that emit-ast, sync, and analyze work correctly
    // with modules and imports.

    let temp = TempDir::new().unwrap();

    // Create a module file
    fs::write(
        temp.path().join("math.pluto"),
        r#"pub fn add(x: int, y: int) int {
    return x + y
}
"#,
    )
    .unwrap();

    // Create entry file that imports the module
    fs::write(
        temp.path().join("main.pt"),
        r#"import math

fn main() {
    let result = math.add(10, 20)
}
"#,
    )
    .unwrap();

    // emit-ast on entry file (should resolve module)
    let output = run_plutoc(&["emit-ast", "main.pt", "-o", "main.pluto"], &temp);
    assert!(
        output.status.success(),
        "emit-ast failed with modules: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify flattened AST includes both main and math.add
    let data = fs::read(temp.path().join("main.pluto")).unwrap();
    let (program, _, _) = plutoc::binary::deserialize_program(&data).unwrap();

    let has_main = program.functions.iter().any(|f| f.node.name.node == "main");
    let has_math_add = program
        .functions
        .iter()
        .any(|f| f.node.name.node == "math.add");

    assert!(has_main, "should have main function");
    assert!(has_math_add, "should have flattened math.add function");

    // analyze should work with multi-file
    let output = run_plutoc(&["analyze", "main.pluto"], &temp);
    assert!(
        output.status.success(),
        "analyze failed with modules: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // generate-pt should produce valid output
    let output = run_plutoc(&["generate-pt", "main.pluto", "-o", "main_gen.pt"], &temp);
    assert!(output.status.success(), "generate-pt failed with modules");

    let generated = fs::read_to_string(temp.path().join("main_gen.pt")).unwrap();
    assert!(
        generated.contains("fn main"),
        "generated .pt should have main"
    );
    assert!(
        generated.contains("fn math.add") || generated.contains("math.add"),
        "generated .pt should reference math.add"
    );
}

#[test]
fn test_derived_data_invalidation() {
    // Test: Derived data staleness detection works correctly
    //
    // Verifies that:
    // - Fresh derived data is detected as non-stale
    // - Modifying source invalidates derived data
    // - Re-analyzing refreshes derived data

    let temp = TempDir::new().unwrap();

    let pt_file = temp.path().join("test.pt");
    fs::write(
        &pt_file,
        r#"fn version1() int {
    return 1
}

fn main() {}
"#,
    )
    .unwrap();

    // emit-ast with fresh derived data
    let pluto_file = temp.path().join("test.pluto");
    let output = run_plutoc(&["emit-ast", "test.pt", "-o", "test.pluto"], &temp);
    assert!(output.status.success());

    let data1 = fs::read(&pluto_file).unwrap();
    let (_, source1, derived1) = plutoc::binary::deserialize_program(&data1).unwrap();

    let hash1 = derived1.source_hash.clone();
    assert!(!hash1.is_empty(), "hash should be set");
    assert!(!derived1.is_stale(&source1), "should not be stale initially");

    // Modify source
    fs::write(
        &pt_file,
        r#"fn version2() int {
    return 2
}

fn main() {}
"#,
    )
    .unwrap();

    // Sync (marks derived data stale)
    let output = run_plutoc(&["sync", "test.pt", "-o", "test.pluto"], &temp);
    assert!(output.status.success());

    let data2 = fs::read(&pluto_file).unwrap();
    let (_, source2, derived2) = plutoc::binary::deserialize_program(&data2).unwrap();

    // Old derived data should be stale against new source
    assert!(
        derived1.is_stale(&source2),
        "old data should be stale against new source"
    );

    // New derived data from sync should be stale/empty
    assert!(
        derived2.source_hash.is_empty() || derived2.is_stale(&source2),
        "derived data should be stale after sync"
    );

    // Re-analyze
    let output = run_plutoc(&["analyze", "test.pluto"], &temp);
    assert!(output.status.success());

    let data3 = fs::read(&pluto_file).unwrap();
    let (_, source3, derived3) = plutoc::binary::deserialize_program(&data3).unwrap();

    let hash3 = derived3.source_hash.clone();
    assert!(!hash3.is_empty(), "hash should be set after analyze");
    assert!(
        !derived3.is_stale(&source3),
        "should not be stale after analyze"
    );

    // Hash should be different from original
    assert_ne!(hash1, hash3, "hash should change when source changes");
}

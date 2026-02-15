//! Integration tests for `plutoc analyze` command.

use tempfile::TempDir;

mod common;

#[test]
fn test_analyze_pt_file_creates_pluto() {
    // Test: plutoc analyze creates .pluto file from .pt source
    //
    // Verifies that analyzing a .pt text file produces a .pluto binary
    // with fresh derived data.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("test.pt");
    let pluto_file = temp.path().join("test.pluto");

    std::fs::write(
        &pt_file,
        r#"
fn add(x: int, y: int) int {
    return x + y
}

fn main() {
    let result = add(10, 20)
}
"#,
    )
    .unwrap();

    // Run analyze
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success(), "analyze command failed");
    assert!(pluto_file.exists(), ".pluto file not created");

    // Verify the .pluto file is valid binary format
    let data = std::fs::read(&pluto_file).unwrap();
    assert!(plutoc::binary::is_binary_format(&data), ".pluto is not valid binary");

    // Verify we can deserialize it and it has derived data
    let (_program, _source, derived) = plutoc::binary::deserialize_program(&data).unwrap();
    assert!(!derived.source_hash.is_empty(), "source_hash not computed");
}

#[test]
fn test_analyze_computes_function_metadata() {
    // Test: analyze computes function signatures and error sets
    //
    // Verifies that the derived data includes function metadata.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("funcs.pt");

    std::fs::write(
        &pt_file,
        r#"fn add(x: int, y: int) int {
    return x + y
}

fn main() {}
"#,
    )
    .unwrap();

    // Run analyze
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success());

    // Load and check derived data
    let pluto_file = temp.path().join("funcs.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = plutoc::binary::deserialize_program(&data).unwrap();

    // Find the add function's UUID
    let add_fn = program
        .functions
        .iter()
        .find(|f| f.node.name.node == "add")
        .expect("add function not found");

    // Check function signature
    let sig = derived
        .fn_signatures
        .get(&add_fn.node.id)
        .expect("signature not found for add");

    assert_eq!(sig.param_types.len(), 2, "should have 2 parameters");
    assert!(!sig.is_fallible, "add should not be fallible");

    // Check error set (should be empty for non-fallible function)
    let error_set = derived
        .fn_error_sets
        .get(&add_fn.node.id)
        .expect("error set not found for add");

    assert!(error_set.is_empty(), "add should have no errors");
}

#[test]
fn test_analyze_computes_function_signatures() {
    // Test: analyze computes resolved function signatures
    //
    // Verifies that derived data includes param and return types.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("sigs.pt");

    std::fs::write(
        &pt_file,
        r#"
fn multiply(x: int, y: int) int {
    return x * y
}

fn main() {}
"#,
    )
    .unwrap();

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success());

    let pluto_file = temp.path().join("sigs.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = plutoc::binary::deserialize_program(&data).unwrap();

    let multiply_fn = program
        .functions
        .iter()
        .find(|f| f.node.name.node == "multiply")
        .unwrap();

    let sig = derived
        .fn_signatures
        .get(&multiply_fn.node.id)
        .expect("signature not found");

    assert_eq!(sig.param_types.len(), 2);
    assert!(!sig.is_fallible);
}

#[test]
fn test_analyze_staleness_detection() {
    // Test: derived data includes source hash for staleness detection
    //
    // Verifies that the source_hash field is populated and changes
    // when the source changes.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("stale.pt");

    std::fs::write(
        &pt_file,
        r#"
fn version_one() int {
    return 1
}

fn main() {}
"#,
    )
    .unwrap();

    // First analyze
    std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let pluto_file = temp.path().join("stale.pluto");
    let data1 = std::fs::read(&pluto_file).unwrap();
    let (_prog1, source1, derived1) = plutoc::binary::deserialize_program(&data1).unwrap();

    let hash1 = derived1.source_hash.clone();
    assert!(!hash1.is_empty(), "hash should be set");

    // Verify not stale
    assert!(!derived1.is_stale(&source1), "should not be stale");

    // Modify source
    std::fs::write(
        &pt_file,
        r#"
fn version_two() int {
    return 2
}

fn main() {}
"#,
    )
    .unwrap();

    // Re-analyze
    std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let data2 = std::fs::read(&pluto_file).unwrap();
    let (_prog2, source2, derived2) = plutoc::binary::deserialize_program(&data2).unwrap();

    let hash2 = derived2.source_hash.clone();

    // Hash should be different
    assert_ne!(hash1, hash2, "hash should change when source changes");

    // Old derived data should be stale against new source
    assert!(derived1.is_stale(&source2), "old data should be stale");

    // New derived data should not be stale
    assert!(!derived2.is_stale(&source2), "new data should not be stale");
}

#[test]
fn test_analyze_preserves_ast_and_source() {
    // Test: analyze preserves the original AST and source text
    //
    // Only the derived layer should change, not the authored content.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("preserve.pt");

    let source_text = r#"
fn original_name(param: int) int {
    return param * 2
}

fn main() {}
"#;

    std::fs::write(&pt_file, source_text).unwrap();

    std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    let pluto_file = temp.path().join("preserve.pluto");
    let data = std::fs::read(&pluto_file).unwrap();
    let (program, source, _derived) = plutoc::binary::deserialize_program(&data).unwrap();

    // Source should be exactly what we wrote (modulo leading/trailing whitespace)
    assert!(source.contains("original_name"));
    assert!(source.contains("param * 2"));

    // AST function name should match
    let func = program.functions.iter().find(|f| f.node.name.node == "original_name");
    assert!(func.is_some(), "function name preserved in AST");
}

#[test]
fn test_analyze_invalid_syntax() {
    // Test: analyze reports parse errors gracefully
    //
    // Should exit with error, not crash or create invalid .pluto file.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("invalid.pt");

    std::fs::write(
        &pt_file,
        r#"
fn broken(x: int  // Missing closing paren and brace
"#,
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on invalid syntax");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error:"), "should report an error");

    // .pluto file should not be created
    let pluto_file = temp.path().join("invalid.pluto");
    assert!(!pluto_file.exists(), "should not create .pluto on parse error");
}

#[test]
fn test_analyze_output_message() {
    // Test: analyze command prints success message
    //
    // Should show input → output paths.

    let temp = TempDir::new().unwrap();
    let pt_file = temp.path().join("msg.pt");

    std::fs::write(
        &pt_file,
        r#"
fn main() {}
"#,
    )
    .unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&pt_file)
        .arg("--stdlib")
        .arg("stdlib")
        .output()
        .unwrap();

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analyzed"));
    assert!(stderr.contains("msg.pt"));
    assert!(stderr.contains("msg.pluto"));
}

#[test]
fn test_analyze_multi_file_modules() {
    // Test: analyze handles multi-file projects with imports
    //
    // Verifies that module resolution works correctly.

    let temp = TempDir::new().unwrap();

    // Create a math module (must be .pluto extension for module system)
    let math_file = temp.path().join("math.pluto");
    std::fs::write(
        &math_file,
        r#"pub fn add(x: int, y: int) int {
    return x + y
}

pub fn multiply(x: int, y: int) int {
    return x * y
}
"#,
    )
    .unwrap();

    // Create entry file that imports math
    let main_file = temp.path().join("main.pt");
    std::fs::write(
        &main_file,
        r#"
import math

fn main() {
    let sum = math.add(10, 20)
    let product = math.multiply(sum, 2)
}
"#,
    )
    .unwrap();

    // Run analyze on entry file
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_plutoc"))
        .arg("analyze")
        .arg(&main_file)
        .arg("--stdlib")
        .arg("stdlib")
        .status()
        .unwrap();

    assert!(status.success(), "analyze should succeed on multi-file project");

    // Load and check derived data
    let pluto_file = temp.path().join("main.pluto");
    assert!(pluto_file.exists(), ".pluto file should be created");

    let data = std::fs::read(&pluto_file).unwrap();
    let (program, _source, derived) = plutoc::binary::deserialize_program(&data).unwrap();

    // Verify both local and imported functions are in the flattened AST
    let has_main = program.functions.iter().any(|f| f.node.name.node == "main");
    let has_math_add = program.functions.iter().any(|f| f.node.name.node == "math.add");
    let has_math_multiply = program.functions.iter().any(|f| f.node.name.node == "math.multiply");

    assert!(has_main, "main function should be in AST");
    assert!(has_math_add, "math.add function should be in flattened AST");
    assert!(has_math_multiply, "math.multiply function should be in flattened AST");

    // Verify derived data includes all functions
    assert_eq!(derived.fn_signatures.len(), 3, "should have signatures for all 3 functions");
}

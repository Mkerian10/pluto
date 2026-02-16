mod common;
use common::{compile_test_and_run, compile_test_should_fail_with, compile_should_fail_with, compile_and_run_stdout};

// ── Basic test execution ──────────────────────────────────────────────────────

#[test]
fn test_basic_passing() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "one equals one" {
    expect(1).to_equal(1)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test one equals one ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

#[test]
fn test_multiple_tests() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "first" {
    expect(1).to_equal(1)
}

test "second" {
    expect(true).to_be_true()
}

test "third" {
    expect(false).to_be_false()
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test first ... ok"));
    assert!(stdout.contains("test second ... ok"));
    assert!(stdout.contains("test third ... ok"));
    assert!(stdout.contains("3 tests passed"));
}

#[test]
fn test_with_helper_functions() {
    let (stdout, _, code) = compile_test_and_run(r#"
fn add(a: int, b: int) int {
    return a + b
}

test "addition works" {
    expect(add(1, 2)).to_equal(3)
    expect(add(-1, 1)).to_equal(0)
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test addition works ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

// ── Assertion methods ─────────────────────────────────────────────────────────

#[test]
fn test_to_equal_int() {
    let (_, _, code) = compile_test_and_run(r#"
test "int equality" {
    expect(42).to_equal(42)
    expect(-1).to_equal(-1)
    expect(0).to_equal(0)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_float() {
    let (_, _, code) = compile_test_and_run(r#"
test "float equality" {
    expect(3.14).to_equal(3.14)
    expect(0.0).to_equal(0.0)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_bool() {
    let (_, _, code) = compile_test_and_run(r#"
test "bool equality" {
    expect(true).to_equal(true)
    expect(false).to_equal(false)
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_equal_string() {
    let (_, _, code) = compile_test_and_run(r#"
test "string equality" {
    expect("hello").to_equal("hello")
    expect("").to_equal("")
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_be_true() {
    let (_, _, code) = compile_test_and_run(r#"
test "true check" {
    expect(true).to_be_true()
    expect(1 > 0).to_be_true()
    expect(1 == 1).to_be_true()
}
"#);
    assert_eq!(code, 0);
}

#[test]
fn test_to_be_false() {
    let (_, _, code) = compile_test_and_run(r#"
test "false check" {
    expect(false).to_be_false()
    expect(1 > 2).to_be_false()
    expect(1 == 2).to_be_false()
}
"#);
    assert_eq!(code, 0);
}

// ── Failing assertions ────────────────────────────────────────────────────────

#[test]
fn test_failing_int_equality() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(1).to_equal(2)
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected 1 to equal 2"));
}

#[test]
fn test_failing_to_be_true() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(false).to_be_true()
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected true but got false"));
}

#[test]
fn test_failing_to_be_false() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect(true).to_be_false()
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("expected false but got true"));
}

#[test]
fn test_failing_string_equality() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "will fail" {
    expect("hello").to_equal("world")
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("FAIL"));
    assert!(stderr.contains("hello"));
    assert!(stderr.contains("world"));
}

// ── Compile errors ────────────────────────────────────────────────────────────

#[test]
fn test_type_mismatch_to_equal() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_equal("hello")
}
"#, "to_equal");
}

#[test]
fn test_to_be_true_non_bool() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_true()
}
"#, "requires bool");
}

#[test]
fn test_to_be_false_non_bool() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_false()
}
"#, "requires bool");
}

#[test]
fn test_unknown_assertion_method() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1).to_be_awesome()
}
"#, "unknown assertion method");
}

#[test]
fn test_duplicate_test_names() {
    compile_test_should_fail_with(r#"
test "same name" {
    expect(1).to_equal(1)
}

test "same name" {
    expect(2).to_equal(2)
}
"#, "duplicate test name");
}

#[test]
fn test_pub_test_rejected() {
    compile_test_should_fail_with(r#"
pub test "bad" {
    expect(1).to_equal(1)
}
"#, "tests cannot be pub");
}

#[test]
fn test_bare_expect_rejected() {
    compile_test_should_fail_with(r#"
test "bad" {
    expect(1)
}
"#, "expect() must be followed by an assertion method");
}

#[test]
fn test_expect_builtin_shadowing_rejected() {
    compile_should_fail_with(r#"
fn expect(x: int) int {
    return x
}

fn main() int {
    return expect(5)
}
"#, "expect");
}

// ── Non-test mode stripping ───────────────────────────────────────────────────

#[test]
fn test_strip_tests_in_non_test_mode() {
    // Tests should be stripped in normal compilation mode.
    // This program has a test block but also a valid main function.
    let stdout = compile_and_run_stdout(r#"
fn main() {
    print(42)
}

test "this should be stripped" {
    expect(1).to_equal(2)
}
"#);
    assert_eq!(stdout.trim(), "42");
}

// ── Empty test body ───────────────────────────────────────────────────────────

#[test]
fn test_empty_body_passes() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "empty" {
}
"#);
    assert_eq!(code, 0);
    assert!(stdout.contains("test empty ... ok"));
    assert!(stdout.contains("1 tests passed"));
}

// ── Line numbers in failure messages ──────────────────────────────────────────

#[test]
fn test_line_numbers_in_failure() {
    let (_, stderr, code) = compile_test_and_run(r#"
test "line check" {
    expect(1).to_equal(1)
    expect(2).to_equal(3)
}
"#);
    assert_ne!(code, 0);
    // The failing assertion is on line 4
    assert!(stderr.contains("line 4"), "Expected 'line 4' in stderr: {}", stderr);
}

// ── Declaration order ─────────────────────────────────────────────────────────

#[test]
fn test_declaration_order() {
    let (stdout, _, code) = compile_test_and_run(r#"
test "alpha" {
    expect(true).to_be_true()
}

test "beta" {
    expect(true).to_be_true()
}

test "gamma" {
    expect(true).to_be_true()
}
"#);
    assert_eq!(code, 0);
    // Tests run in declaration order
    let alpha_pos = stdout.find("test alpha").unwrap();
    let beta_pos = stdout.find("test beta").unwrap();
    let gamma_pos = stdout.find("test gamma").unwrap();
    assert!(alpha_pos < beta_pos);
    assert!(beta_pos < gamma_pos);
}

// ── Multiple files with tests ─────────────────────────────────────────────────

#[test]
fn test_multiple_files_unique_test_ids() {
    // Regression test for P1 #3: Test Runner Generates Duplicate IDs for Multiple Files
    // When multiple .pluto files in the same directory have test blocks, each test should
    // get a unique ID based on the file path hash to avoid duplicate symbol errors at link time.
    use std::process::Command;

    let dir = tempfile::tempdir().unwrap();

    // Create two sibling files, each with test blocks
    std::fs::write(
        dir.path().join("file_a.pluto"),
        r#"
test "test in file a" {
    expect(1).to_equal(1)
}

test "another test in file a" {
    expect(true).to_be_true()
}
"#,
    ).unwrap();

    std::fs::write(
        dir.path().join("file_b.pluto"),
        r#"
test "test in file b" {
    expect(2).to_equal(2)
}

test "another test in file b" {
    expect(false).to_be_false()
}
"#,
    ).unwrap();

    let entry_a = dir.path().join("file_a.pluto");
    let entry_b = dir.path().join("file_b.pluto");
    let bin_path_a = dir.path().join("test_bin_a");
    let bin_path_b = dir.path().join("test_bin_b");

    // Compile file_a in test mode - siblings should NOT be auto-merged to prevent test ID collisions
    pluto::compile_file_for_tests(&entry_a, &bin_path_a, None, false)
        .unwrap_or_else(|e| panic!("Test compilation of file_a failed: {e}"));

    // Compile file_b separately
    pluto::compile_file_for_tests(&entry_b, &bin_path_b, None, false)
        .unwrap_or_else(|e| panic!("Test compilation of file_b failed: {e}"));

    // Run file_a tests - should only see file_a's 2 tests
    let output_a = Command::new(&bin_path_a).output().unwrap();
    let stdout_a = String::from_utf8_lossy(&output_a.stdout);
    let stderr_a = String::from_utf8_lossy(&output_a.stderr);

    assert!(output_a.status.success(), "file_a tests should pass. stderr: {}", stderr_a);
    assert!(stdout_a.contains("test in file a ... ok"), "stdout: {}", stdout_a);
    assert!(stdout_a.contains("another test in file a ... ok"), "stdout: {}", stdout_a);
    assert!(stdout_a.contains("2 tests passed"), "stdout: {}", stdout_a);
    // Should NOT contain file_b tests
    assert!(!stdout_a.contains("test in file b"), "file_a should not include file_b tests");

    // Run file_b tests - should only see file_b's 2 tests
    let output_b = Command::new(&bin_path_b).output().unwrap();
    let stdout_b = String::from_utf8_lossy(&output_b.stdout);
    let stderr_b = String::from_utf8_lossy(&output_b.stderr);

    assert!(output_b.status.success(), "file_b tests should pass. stderr: {}", stderr_b);
    assert!(stdout_b.contains("test in file b ... ok"), "stdout: {}", stdout_b);
    assert!(stdout_b.contains("another test in file b ... ok"), "stdout: {}", stdout_b);
    assert!(stdout_b.contains("2 tests passed"), "stdout: {}", stdout_b);
    // Should NOT contain file_a tests
    assert!(!stdout_b.contains("test in file a"), "file_b should not include file_a tests");
}

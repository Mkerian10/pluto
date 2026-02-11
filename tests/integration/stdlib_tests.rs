//! Integration tests that run pure-Pluto test files against the stdlib.
//!
//! Each test compiles a `.pluto` file in test mode with stdlib available,
//! then runs the resulting binary and asserts all Pluto tests pass.

use std::path::{Path, PathBuf};
use std::process::Command;

fn stdlib_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib")
}

fn run_pluto_test_file(name: &str) -> (String, String, i32) {
    let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/stdlib")
        .join(name)
        .join("main.pluto");
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");

    plutoc::compile_file_for_tests(&test_file, &bin_path, Some(&stdlib_root()), false)
        .unwrap_or_else(|e| panic!("Failed to compile {name}: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// Compile and run a Pluto file in normal mode (not test mode).
/// The binary runs with current_dir set to the project root.
fn run_pluto_file(name: &str) -> (String, String, i32) {
    let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/stdlib")
        .join(name)
        .join("main.pluto");
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("run_bin");
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    plutoc::compile_file_with_stdlib(&test_file, &bin_path, Some(&stdlib_root()))
        .unwrap_or_else(|e| panic!("Failed to compile {name}: {e}"));

    let output = Command::new(&bin_path)
        .current_dir(project_root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn stdlib_math() {
    let (stdout, stderr, code) = run_pluto_test_file("math");
    if code != 0 {
        panic!("math tests failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("tests passed"), "Expected test summary in output:\n{stdout}");
}

#[test]
fn stdlib_strings() {
    let (stdout, stderr, code) = run_pluto_test_file("strings");
    if code != 0 {
        panic!("strings tests failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("tests passed"), "Expected test summary in output:\n{stdout}");
}

#[test]
fn stdlib_collections() {
    let (stdout, stderr, code) = run_pluto_test_file("collections");
    if code != 0 {
        panic!("collections tests failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("tests passed"), "Expected test summary in output:\n{stdout}");
}

#[test]
fn stdlib_time() {
    let (stdout, stderr, code) = run_pluto_test_file("time");
    if code != 0 {
        panic!("time tests failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("tests passed"), "Expected test summary in output:\n{stdout}");
}

#[test]
fn stdlib_random() {
    let (stdout, stderr, code) = run_pluto_test_file("random");
    if code != 0 {
        panic!("random tests failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("tests passed"), "Expected test summary in output:\n{stdout}");
}

#[test]
fn stdlib_json_conformance() {
    let (stdout, stderr, code) = run_pluto_file("json");
    if code != 0 {
        panic!("JSON conformance failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(!stdout.contains("FAIL"), "JSON conformance had failures:\n{stdout}");
    assert!(stdout.contains("JSON Conformance:"), "Expected summary in output:\n{stdout}");
}

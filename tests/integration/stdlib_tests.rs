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

    plutoc::compile_file_for_tests(&test_file, &bin_path, Some(&stdlib_root()))
        .unwrap_or_else(|e| panic!("Failed to compile {name}: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
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

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

/// Compile and run a Pluto file in normal mode (not test mode).
/// Runs with current_dir set to project root so relative paths work.
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

    // Get exit code; on Unix, also capture signal info
    let code = output.status.code().unwrap_or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(sig) = output.status.signal() {
                eprintln!("[run_pluto_file] {name}: killed by signal {sig}");
                return -(sig as i32);
            }
        }
        -1
    });
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
fn stdlib_normal_mode_hello() {
    let (stdout, stderr, code) = run_pluto_file("hello");
    if code != 0 {
        panic!("hello failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert_eq!(stdout.trim(), "hello from normal mode");
}

#[test]
fn stdlib_fs_stress() {
    let (stdout, stderr, code) = run_pluto_file("fs_stress");
    if code != 0 {
        panic!("fs_stress failed (exit {code}):\nstdout: {stdout}\nstderr: {stderr}");
    }
    assert!(stdout.contains("files:"), "Expected file count in output:\n{stdout}");
}

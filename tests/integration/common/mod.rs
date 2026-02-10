#![allow(dead_code)]
use std::collections::HashMap;
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Returns a Command for the plutoc binary. Use for CLI smoke tests only —
/// most tests should use the library-call helpers below instead.
pub fn plutoc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
}

/// Compile source via plutoc::compile() (library call, no subprocess) and run the binary.
/// Returns the process exit code.
pub fn compile_and_run(source: &str) -> i32 {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");

    plutoc::compile(source, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    output.status.code().unwrap_or(-1)
}

/// Compile source via plutoc::compile() (library call) and capture stdout.
pub fn compile_and_run_stdout(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");

    plutoc::compile(source, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    assert!(output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Assert compilation fails with a specific error message substring.
/// Uses compile_to_object() — no file I/O or linking needed for failure tests.
pub fn compile_should_fail_with(source: &str, expected_msg: &str) {
    match plutoc::compile_to_object(source) {
        Ok(_) => panic!("Compilation should have failed"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "Expected error containing '{}', got: {}",
                expected_msg,
                msg
            );
        }
    }
}

/// Assert compilation fails (any error).
/// Uses compile_to_object() — no file I/O or linking needed for failure tests.
pub fn compile_should_fail(source: &str) {
    assert!(
        plutoc::compile_to_object(source).is_err(),
        "Compilation should have failed"
    );
}

/// Compile source in test mode and run the resulting binary, capturing stdout + stderr.
/// Returns (stdout, exit_code).
pub fn compile_test_and_run(source: &str) -> (String, String, i32) {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");

    plutoc::compile_test(source, &bin_path).unwrap_or_else(|e| panic!("Test compilation failed: {e}"));

    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

/// Compile source and run, returning (stdout, stderr, exit_code).
/// Does NOT assert success — use for testing runtime aborts.
pub fn compile_and_run_output(source: &str) -> (String, String, i32) {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");
    plutoc::compile(source, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let output = Command::new(&bin_path).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.code().unwrap_or(-1))
}

/// Compile and run with a timeout (for tests that may deadlock).
/// Panics if the binary doesn't exit within `timeout_secs`.
pub fn compile_and_run_stdout_timeout(source: &str, timeout_secs: u64) -> String {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");
    plutoc::compile(source, &bin_path).unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let mut child = Command::new(&bin_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                let mut stdout = String::new();
                child.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
                assert!(status.success(), "Binary exited with non-zero status");
                return stdout;
            }
            None if Instant::now() >= deadline => {
                child.kill().ok();
                panic!("test timed out after {timeout_secs}s — possible deadlock");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Assert compilation fails in test mode with a specific error message substring.
pub fn compile_test_should_fail_with(source: &str, expected_msg: &str) {
    match plutoc::compile_to_object_test_mode(source) {
        Ok(_) => panic!("Compilation should have failed"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "Expected error containing '{}', got: {}",
                expected_msg,
                msg
            );
        }
    }
}

/// Batch-compile multiple test sources into a single binary using the module system.
///
/// Each `(test_name, source)` pair is written as a separate module file (`bt0.pluto`,
/// `bt1.pluto`, ...). A driver `main.pluto` imports all modules and calls each module's
/// `main()` function with a delimiter between outputs. Returns a map from test name to
/// its captured stdout.
///
/// On batch compile failure, falls back to individual `compile_and_run_stdout` calls so
/// only the broken test panics.
pub fn compile_batch_stdout(tests: &[(&str, &str)]) -> HashMap<String, String> {
    const DELIM: &str = "<<__BATCH_DELIM__>>\n";

    if tests.is_empty() {
        return HashMap::new();
    }

    let dir = tempfile::tempdir().unwrap();

    // Write each test source as a module file
    let mut imports = String::new();
    let mut calls = String::new();
    for (i, (_name, source)) in tests.iter().enumerate() {
        let module_name = format!("bt{i}");
        let file_path = dir.path().join(format!("{module_name}.pluto"));
        std::fs::write(&file_path, source).unwrap();

        imports.push_str(&format!("import {module_name}\n"));
        if i > 0 {
            calls.push_str(&format!("    print(\"<<__BATCH_DELIM__>>\")\n"));
        }
        calls.push_str(&format!("    {module_name}.main()\n"));
    }

    // Generate driver main.pluto
    let driver = format!("{imports}\nfn main() {{\n{calls}}}\n");
    let entry = dir.path().join("main.pluto");
    std::fs::write(&entry, &driver).unwrap();

    let bin_path = dir.path().join("test_bin");

    // Try batch compilation
    match plutoc::compile_file(&entry, &bin_path) {
        Ok(()) => {
            let output = Command::new(&bin_path).output().unwrap();
            assert!(
                output.status.success(),
                "Batch binary exited with non-zero status.\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let parts: Vec<&str> = stdout.split(DELIM).collect();
            assert_eq!(
                parts.len(),
                tests.len(),
                "Expected {} outputs, got {} (delimiter mismatch).\nFull output:\n{}",
                tests.len(),
                parts.len(),
                stdout
            );
            tests
                .iter()
                .zip(parts.iter())
                .map(|((name, _), output)| (name.to_string(), output.to_string()))
                .collect()
        }
        Err(_) => {
            // Fallback: compile individually so only the broken test fails
            eprintln!("Batch compilation failed — falling back to individual compilation");
            tests
                .iter()
                .map(|(name, source)| {
                    (name.to_string(), compile_and_run_stdout(source))
                })
                .collect()
        }
    }
}

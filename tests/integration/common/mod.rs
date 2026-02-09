use std::process::Command;

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

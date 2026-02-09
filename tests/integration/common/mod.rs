use std::process::Command;

pub fn plutoc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_plutoc"))
}

pub fn compile_and_run(source: &str) -> i32 {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let compile_output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    run_output.status.code().unwrap_or(-1)
}

pub fn compile_and_run_stdout(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let compile_output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(
        compile_output.status.success(),
        "Compilation failed: {}",
        String::from_utf8_lossy(&compile_output.stderr)
    );

    assert!(bin_path.exists(), "Binary was not created");

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

pub fn compile_should_fail_with(source: &str, expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_msg),
        "Expected error containing '{}', got: {}",
        expected_msg,
        stderr
    );
}

pub fn compile_should_fail(source: &str) {
    let dir = tempfile::tempdir().unwrap();
    let src_path = dir.path().join("test.pluto");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&src_path, source).unwrap();

    let output = plutoc()
        .arg("compile")
        .arg(&src_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .unwrap();

    assert!(!output.status.success(), "Compilation should have failed");
}

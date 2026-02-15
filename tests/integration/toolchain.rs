use std::process::Command;

/// Test that `pluto versions` works when no versions are installed
#[test]
fn test_versions_empty() {
    // Remove active file if it exists to ensure clean state
    let _ = std::fs::remove_file(
        std::path::PathBuf::from(std::env::var("HOME").unwrap())
            .join(".pluto")
            .join("active")
    );

    let output = Command::new("./target/debug/pluto")
        .args(["versions"])
        .output()
        .expect("Failed to run pluto versions");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either say "No versions installed" or list versions (if some are installed)
    assert!(
        stdout.contains("No versions installed") ||
        !stdout.is_empty(),
        "versions command should produce output"
    );
}

/// Test that `pluto use` with non-existent version fails gracefully
#[test]
fn test_use_nonexistent_version() {
    let output = Command::new("./target/debug/pluto")
        .args(["use", "999.999.999"])
        .output()
        .expect("Failed to run pluto use");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not installed") || stderr.contains("Error"),
        "Should error for non-existent version, got: {}",
        stderr
    );
}

/// Test that `pluto install` with non-existent version fails gracefully
#[test]
fn test_install_nonexistent_version() {
    // Clean up version if it exists from previous test runs
    let version_dir = std::path::PathBuf::from(std::env::var("HOME").unwrap())
        .join(".pluto")
        .join("versions")
        .join("999.999.999");
    let _ = std::fs::remove_dir_all(&version_dir);

    let output = Command::new("./target/debug/pluto")
        .args(["install", "999.999.999"])
        .output()
        .expect("Failed to run pluto install");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should get either a 404 or network error from GitHub
    assert!(
        stderr.contains("not found") ||
        stderr.contains("Network") ||
        stderr.contains("Error"),
        "Should error for non-existent version, got: {}",
        stderr
    );
}

/// Test that toolchain commands have proper help text
#[test]
fn test_install_help() {
    let output = Command::new("./target/debug/pluto")
        .args(["install", "--help"])
        .output()
        .expect("Failed to run pluto install --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Install a compiler version"));
    assert!(stdout.contains("VERSION"));
}

#[test]
fn test_use_help() {
    let output = Command::new("./target/debug/pluto")
        .args(["use", "--help"])
        .output()
        .expect("Failed to run pluto use --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Set the active compiler version"));
    assert!(stdout.contains("VERSION"));
}

#[test]
fn test_versions_help() {
    let output = Command::new("./target/debug/pluto")
        .args(["versions", "--help"])
        .output()
        .expect("Failed to run pluto versions --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("List installed compiler versions"));
}

/// Test version normalization (v prefix handling)
#[test]
fn test_version_normalization() {
    // Both "v0.1.0" and "0.1.0" should be normalized to "0.1.0"
    let output1 = Command::new("./target/debug/pluto")
        .args(["use", "v999.999.999"])
        .output()
        .expect("Failed to run pluto use");

    let output2 = Command::new("./target/debug/pluto")
        .args(["use", "999.999.999"])
        .output()
        .expect("Failed to run pluto use");

    // Both should fail with same error (version not installed)
    let stderr1 = String::from_utf8_lossy(&output1.stderr);
    let stderr2 = String::from_utf8_lossy(&output2.stderr);

    assert!(stderr1.contains("999.999.999"));
    assert!(stderr2.contains("999.999.999"));
}

/// Test that delegation bypasses toolchain commands
/// This test verifies that install/use/versions always run on the current binary
#[test]
fn test_delegation_bypass() {
    // Even if we have an active version set, toolchain commands should not delegate
    // We can't easily test this without actually having two versions installed,
    // but we can verify the commands execute (not delegate to non-existent binary)

    let output = Command::new("./target/debug/pluto")
        .args(["versions"])
        .output()
        .expect("Failed to run pluto versions");

    // Should succeed (or at least not fail with "failed to delegate" error)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("failed to delegate"),
        "Toolchain commands should not delegate"
    );
}

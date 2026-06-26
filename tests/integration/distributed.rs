// Phase 1 of whole-program distributed safety: a `remote` dependency lets one
// service hold a typed reference to another service's interface. The call is
// type-checked across the boundary against the real signature, and crossing the
// boundary implicitly adds NetworkError to the caller's inferred error set.
//
// There is no transport yet: a remote call always raises NetworkError at runtime
// (as if the network were unreachable). These tests pin the compile-time
// guarantees plus the runtime stub behavior.

mod common;

use std::process::Command;

/// Write multiple files to a temp dir, compile `main.pluto`, run it, return stdout.
fn run_project(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    pluto::compile_file(&entry, &bin_path)
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Compile `main.pluto`; assert it fails with an error containing `expected_msg`.
fn compile_project_should_fail_with(files: &[(&str, &str)], expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    match pluto::compile_file(&entry, &bin_path) {
        Ok(_) => panic!("Compilation should have failed (expected: {expected_msg})"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "error did not contain '{expected_msg}'.\nActual: {msg}"
            );
        }
    }
}

// Service B's interface, in its own module so service A can reference it.
const BILLING: &str = "\
pub class BillingService {
    fn charge(self, amount: int) int {
        return amount * 2
    }
}";

/// The whole-program type checker validates a remote call against the target
/// service's real signature — a wrong argument type is rejected at compile time.
#[test]
fn cross_service_signature_mismatch_fails() {
    compile_project_should_fail_with(
        &[
            ("billing.pluto", BILLING),
            ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(\"not an int\") catch 0
        print(f\"result: {x}\")
    }
}"),
        ],
        "expected int",
    );
}

/// Crossing a service boundary adds NetworkError to the caller's error set, so a
/// bare remote call (no `!`/`catch`) is rejected — the boundary failure mode must
/// be handled.
#[test]
fn bare_remote_call_requires_error_handling() {
    compile_project_should_fail_with(
        &[
            ("billing.pluto", BILLING),
            ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(10)
        print(f\"result: {x}\")
    }
}"),
        ],
        "must be handled with ! or catch",
    );
}

/// A handled remote call compiles and runs. With no transport configured, the
/// call raises NetworkError, the `catch` supplies a fallback, and the program
/// completes — end-to-end proof that the boundary call is wired through the
/// error system.
#[test]
fn handled_remote_call_runs_and_falls_back() {
    let out = run_project(&[
        ("billing.pluto", BILLING),
        ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(10) catch -1
        print(f\"result: {x}\")
    }
}"),
    ]);
    assert_eq!(out, "result: -1\n");
}

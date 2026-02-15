mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// Write multiple files to a temp directory, compile the system file,
/// and return a map of member_name -> binary_path.
fn compile_system_project(files: &[(&str, &str)]) -> HashMap<String, PathBuf> {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let output_dir = dir.path().join("build");

    let members = pluto::compile_system_file_with_stdlib(&entry, &output_dir, None)
        .unwrap_or_else(|e| panic!("System compilation failed: {e}"));

    // Keep the tempdir alive by leaking it (tests are short-lived)
    let _ = dir.keep();

    members.into_iter().collect()
}

/// Write multiple files to a temp directory, compile the system file,
/// and assert compilation fails.
fn compile_system_should_fail(files: &[(&str, &str)]) {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let output_dir = dir.path().join("build");

    assert!(
        pluto::compile_system_file_with_stdlib(&entry, &output_dir, None).is_err(),
        "System compilation should have failed"
    );
}

/// Write multiple files to a temp directory, compile the system file,
/// and assert compilation fails with a specific error message.
fn compile_system_should_fail_with(files: &[(&str, &str)], expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let entry = dir.path().join("main.pluto");
    let output_dir = dir.path().join("build");

    match pluto::compile_system_file_with_stdlib(&entry, &output_dir, None) {
        Ok(_) => panic!("System compilation should have failed"),
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

/// Run a compiled binary and return its stdout.
fn run_binary(path: &PathBuf) -> String {
    let output = Command::new(path).output().unwrap();
    assert!(
        output.status.success(),
        "Binary {} exited with non-zero status.\nstderr: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ============================================================
// Basic system: one member
// ============================================================

#[test]
fn system_basic() {
    let members = compile_system_project(&[
        ("main.pluto", r#"
import api

system MySystem {
    api_server: api
}
"#),
        ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("hello from api")
    }
}
"#),
    ]);

    assert_eq!(members.len(), 1);
    assert!(members.contains_key("api_server"));
    let out = run_binary(members.get("api_server").unwrap());
    assert_eq!(out, "hello from api\n");
}

// ============================================================
// System with two members
// ============================================================

#[test]
fn system_two_members() {
    let members = compile_system_project(&[
        ("main.pluto", r#"
import api
import worker

system OrderPlatform {
    api_server: api
    background: worker
}
"#),
        ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api running")
    }
}
"#),
        ("worker.pluto", r#"
app WorkerApp {
    fn main(self) {
        print("worker running")
    }
}
"#),
    ]);

    assert_eq!(members.len(), 2);
    assert!(members.contains_key("api_server"));
    assert!(members.contains_key("background"));

    let api_out = run_binary(members.get("api_server").unwrap());
    assert_eq!(api_out, "api running\n");

    let worker_out = run_binary(members.get("background").unwrap());
    assert_eq!(worker_out, "worker running\n");
}

// ============================================================
// System with shared library module
// ============================================================

#[test]
fn system_with_shared_module() {
    let members = compile_system_project(&[
        ("main.pluto", r#"
import api
import shared

system MySystem {
    api_server: api
}
"#),
        ("shared.pluto", r#"
pub fn greet() string {
    return "hello world"
}
"#),
        ("api.pluto", r#"
import shared

app ApiApp {
    fn main(self) {
        print(shared.greet())
    }
}
"#),
    ]);

    assert_eq!(members.len(), 1);
    let out = run_binary(members.get("api_server").unwrap());
    assert_eq!(out, "hello world\n");
}

// ============================================================
// System with directory module
// ============================================================

#[test]
fn system_directory_module() {
    let members = compile_system_project(&[
        ("main.pluto", r#"
import api

system MySystem {
    web: api
}
"#),
        ("api/main.pluto", r#"
app ApiApp {
    fn main(self) {
        print("directory api")
    }
}
"#),
    ]);

    assert_eq!(members.len(), 1);
    let out = run_binary(members.get("web").unwrap());
    assert_eq!(out, "directory api\n");
}

// ============================================================
// Rejection: member references nonexistent module
// ============================================================

#[test]
fn system_rejects_nonexistent_module() {
    compile_system_should_fail_with(
        &[
            ("main.pluto", r#"
import api

system MySystem {
    api_server: api
    worker_server: worker
}
"#),
            ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api")
    }
}
"#),
        ],
        "not imported",
    );
}

// ============================================================
// Rejection: member references library module (no app)
// ============================================================

#[test]
fn system_rejects_module_without_app() {
    compile_system_should_fail_with(
        &[
            ("main.pluto", r#"
import utils

system MySystem {
    util_server: utils
}
"#),
            ("utils.pluto", r#"
pub fn helper() int {
    return 42
}
"#),
        ],
        "does not contain an app declaration",
    );
}

// ============================================================
// Rejection: same file has both app and system
// ============================================================

#[test]
fn system_rejects_app_and_system() {
    compile_system_should_fail(&[
        ("main.pluto", r#"
import api

app MyApp {
    fn main(self) {
        print("hi")
    }
}

system MySystem {
    api_server: api
}
"#),
        ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api")
    }
}
"#),
    ]);
}

// ============================================================
// Rejection: duplicate member names
// ============================================================

#[test]
fn system_rejects_duplicate_members() {
    compile_system_should_fail(&[
        ("main.pluto", r#"
import api
import worker

system MySystem {
    server: api
    server: worker
}
"#),
        ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api")
    }
}
"#),
        ("worker.pluto", r#"
app WorkerApp {
    fn main(self) {
        print("worker")
    }
}
"#),
    ]);
}

// ============================================================
// Rejection: system file with fn main()
// ============================================================

#[test]
fn system_rejects_fn_main() {
    compile_system_should_fail_with(
        &[
            ("main.pluto", r#"
import api

fn main() {
    print("nope")
}

system MySystem {
    api_server: api
}
"#),
            ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api")
    }
}
"#),
        ],
        "must not contain a top-level fn main()",
    );
}

// ============================================================
// detect_system_file returns None for regular files
// ============================================================

#[test]
fn detect_system_file_returns_none_for_regular() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("regular.pluto");
    std::fs::write(&file, "fn main() {\n    print(\"hi\")\n}").unwrap();

    let result = pluto::detect_system_file(&file).unwrap();
    assert!(result.is_none());
}

// ============================================================
// detect_system_file returns Some for system files
// ============================================================

#[test]
fn detect_system_file_returns_some_for_system() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sys.pluto");
    std::fs::write(&file, "import api\n\nsystem MySystem {\n    s: api\n}").unwrap();
    // We also need api.pluto for detection (parser doesn't resolve imports)
    // Actually detect_system_file only parses, doesn't resolve. Just need valid parse.

    let result = pluto::detect_system_file(&file).unwrap();
    assert!(result.is_some());
}

// ============================================================
// Rejection: pub system not allowed
// ============================================================

#[test]
fn system_rejects_pub() {
    compile_system_should_fail(&[
        ("main.pluto", r#"
import api

pub system MySystem {
    api_server: api
}
"#),
        ("api.pluto", r#"
app ApiApp {
    fn main(self) {
        print("api")
    }
}
"#),
    ]);
}

// ============================================================
// System with DI in member app
// ============================================================

#[test]
fn system_member_with_di() {
    let members = compile_system_project(&[
        ("main.pluto", r#"
import api

system MySystem {
    web: api
}
"#),
        ("api.pluto", r#"
pub class Counter {
    value: int

    fn get(self) int {
        return self.value
    }
}

app ApiApp[c: Counter] {
    fn main(self) {
        print(self.c.get())
    }
}
"#),
    ]);

    assert_eq!(members.len(), 1);
    // DI will construct Counter with zeroed int (0)
    let out = run_binary(members.get("web").unwrap());
    assert_eq!(out, "0\n");
}

mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path);
        } else {
            std::fs::copy(entry.path(), &dest_path).unwrap();
        }
    }
}

fn run_project_with_stdin(files: &[(&str, &str)], stdin_data: &str) -> String {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_src = manifest_dir.join("stdlib");
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&stdlib_src, &stdlib_dst);

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    plutoc::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let mut child = Command::new(&bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(stdin_data.as_bytes()).unwrap();
    }
    // Drop stdin to signal EOF
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ============================================================
// read_line reads a single line from stdin
// ============================================================

#[test]
fn io_read_line_basic() {
    let out = run_project_with_stdin(
        &[(
            "main.pluto",
            r#"import std.io

fn main() {
    let line = io.read_line()
    print(line)
}
"#,
        )],
        "hello\n",
    );
    assert_eq!(out, "hello\n");
}

// ============================================================
// read_line called multiple times reads successive lines
// ============================================================

#[test]
fn io_read_line_multiple() {
    let out = run_project_with_stdin(
        &[(
            "main.pluto",
            r#"import std.io

fn main() {
    let a = io.read_line()
    let b = io.read_line()
    print(a)
    print(b)
}
"#,
        )],
        "line1\nline2\n",
    );
    assert_eq!(out, "line1\nline2\n");
}

// ============================================================
// read_line on empty stdin returns empty string
// ============================================================

#[test]
fn io_read_line_eof() {
    let out = run_project_with_stdin(
        &[(
            "main.pluto",
            r#"import std.io

fn main() {
    let line = io.read_line()
    let len = line.len()
    print(len)
}
"#,
        )],
        "",
    );
    assert_eq!(out, "0\n");
}

// ============================================================
// read_line with no trailing newline still returns content
// ============================================================

#[test]
fn io_read_line_no_trailing_newline() {
    let out = run_project_with_stdin(
        &[(
            "main.pluto",
            r#"import std.io

fn main() {
    let line = io.read_line()
    print(line)
}
"#,
        )],
        "hello",
    );
    assert_eq!(out, "hello\n");
}

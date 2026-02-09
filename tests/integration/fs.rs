mod common;

use std::path::Path;
use std::process::Command;

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

fn run_project_with_stdlib(files: &[(&str, &str)]) -> String {
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

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

// ============================================================
// write_all + read_all roundtrip
// ============================================================

#[test]
fn fs_write_all_read_all() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/test.txt"
    fs.write_all(path, "hello world")!
    let content = fs.read_all(path)!
    print(content)
    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "hello world\n");
}

// ============================================================
// exists returns true after write, false after remove
// ============================================================

#[test]
fn fs_exists_and_remove() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/exists_test.txt"
    print(fs.exists(path))
    fs.write_all(path, "data")!
    print(fs.exists(path))
    fs.remove(path)!
    print(fs.exists(path))
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "false\ntrue\nfalse\n");
}

// ============================================================
// File class: open_write, write, close, open_read, read
// ============================================================

#[test]
fn fs_file_class_write_read() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/file_class.txt"

    let f = fs.open_write(path)!
    f.write("hello file")!
    f.close()!

    let f2 = fs.open_read(path)!
    let content = f2.read(1024)
    print(content)
    f2.close()!

    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "hello file\n");
}

// ============================================================
// append_all appends to existing file
// ============================================================

#[test]
fn fs_append_all() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/append.txt"
    fs.write_all(path, "hello")!
    fs.append_all(path, " world")!
    let content = fs.read_all(path)!
    print(content)
    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "hello world\n");
}

// ============================================================
// mkdir + write files + list_dir + cleanup
// ============================================================

#[test]
fn fs_mkdir_list_dir_rmdir() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let sub = tmp + "/subdir"
    fs.mkdir(sub)!
    print(fs.is_dir(sub))

    fs.write_all(sub + "/a.txt", "a")!
    fs.write_all(sub + "/b.txt", "b")!

    let entries = fs.list_dir(sub)!
    print(entries.len())

    fs.remove(sub + "/a.txt")!
    fs.remove(sub + "/b.txt")!
    fs.rmdir(sub)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\n2\n");
}

// ============================================================
// file_size returns correct byte count
// ============================================================

#[test]
fn fs_file_size() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/size.txt"
    fs.write_all(path, "12345")!
    let sz = fs.file_size(path)!
    print(sz)
    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "5\n");
}

// ============================================================
// rename moves file
// ============================================================

#[test]
fn fs_rename() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let src = tmp + "/old.txt"
    let dst = tmp + "/new.txt"
    fs.write_all(src, "moved")!
    fs.rename(src, dst)!
    print(fs.exists(src))
    let content = fs.read_all(dst)!
    print(content)
    fs.remove(dst)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "false\nmoved\n");
}

// ============================================================
// copy duplicates file
// ============================================================

#[test]
fn fs_copy() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let src = tmp + "/original.txt"
    let dst = tmp + "/copy.txt"
    fs.write_all(src, "copied data")!
    fs.copy(src, dst)!
    let content = fs.read_all(dst)!
    print(content)
    print(fs.exists(src))
    fs.remove(src)!
    fs.remove(dst)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "copied data\ntrue\n");
}

// ============================================================
// is_dir / is_file return correct bools
// ============================================================

#[test]
fn fs_is_dir_is_file() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/check.txt"
    fs.write_all(path, "x")!
    print(fs.is_file(path))
    print(fs.is_dir(path))
    print(fs.is_dir(tmp))
    print(fs.is_file(tmp))
    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\nfalse\ntrue\nfalse\n");
}

// ============================================================
// File.seek to beginning after write, re-read
// ============================================================

#[test]
fn fs_seek() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/seek.txt"

    fs.write_all(path, "abcdef")!

    let f = fs.open_read(path)!
    let first = f.read(3)
    print(first)
    f.seek(0, fs.SEEK_SET())!
    let again = f.read(6)
    print(again)
    f.close()!

    fs.remove(path)!
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "abc\nabcdef\n");
}

// ============================================================
// read_all on missing file caught with catch
// ============================================================

#[test]
fn fs_read_all_nonexistent_catches() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let content = fs.read_all("/nonexistent_file_12345.txt") catch "caught"
    print(content)
}
"#,
    )]);
    assert_eq!(out, "caught\n");
}

// ============================================================
// ! propagates FileError through call chain
// ============================================================

#[test]
fn fs_error_propagation() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn read_file(path: string) string {
    let content = fs.read_all(path)!
    return content
}

fn main() {
    let result = read_file("/nonexistent_xyz.txt") catch "propagated"
    print(result)
}
"#,
    )]);
    assert_eq!(out, "propagated\n");
}

// ============================================================
// open_read on missing file raises
// ============================================================

#[test]
fn fs_open_read_nonexistent() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let f = fs.open_read("/nonexistent_abc.txt") catch fs.File { fd: -1 }
    print(f.fd)
}
"#,
    )]);
    assert_eq!(out, "-1\n");
}

// ============================================================
// temp_dir returns valid directory path
// ============================================================

#[test]
fn fs_temp_dir() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    print(fs.is_dir(tmp))
    fs.rmdir(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\n");
}

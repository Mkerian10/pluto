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
    run_project_with_stdlib_env(files, &[])
}

fn run_project_with_stdlib_env(files: &[(&str, &str)], envs: &[(&str, &str)]) -> String {
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

    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let mut cmd = Command::new(&bin_path);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let run_output = cmd.output().unwrap();
    assert!(
        run_output.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Compile a main.pluto against the real stdlib and assert it FAILS with
/// `expected_msg` — the typestate-rejection battery (fd leaks,
/// use-after-close, mode violations, poison swallowing) is a set of
/// compile errors, not runtime behaviors.
fn compile_with_stdlib_should_fail(source: &str, expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.pluto"), source).unwrap();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_src = manifest_dir.join("stdlib");
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&stdlib_src, &stdlib_dst);

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    match pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst)) {
        Ok(_) => panic!("Expected compilation to fail, but it succeeded"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "Expected error containing {expected_msg:?}, got: {msg}"
            );
        }
    }
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
    let content = f2.read(1024)!
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
    let first = f.read(3)!
    print(first)
    f.seek(fs.Seek.Start { offset: 0 })!
    let again = f.read(6)!
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
// open_read on missing file raises NotFound (the branch-worthy case);
// the old `catch fs.File { fd: -1 }` fallback-construction pattern is
// replaced by a terminating catch — a File can no longer be forged.
// ============================================================

#[test]
fn fs_open_read_nonexistent() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let f = fs.open_read("/nonexistent_abc.txt") catch e: fs.NotFound {
        print(f"not found: {e.path}")
        return
    } catch e: fs.FileError {
        print(f"file error: {e.code}")
        return
    }
    f.close()!
}
"#,
    )]);
    assert_eq!(out, "not found: /nonexistent_abc.txt\n");
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

// ============================================================
// Durability: sync / sync_data on a written handle
// ============================================================

#[test]
fn fs_sync_and_sync_data() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/durable.txt"
    let w = fs.open_write(path)!
    w.write("batch one ")!
    w.sync()!
    w.write("batch two")!
    w.sync_data()!
    w.close()!
    print(fs.read_all(path)!)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "batch one batch two\n");
}

// ============================================================
// Degradation: an injected sync failure raises Degraded carrying the
// file as File<Write, Poisoned>; the typed catch takes on the payload
// obligation and discard() is the legal exit. The success path of the
// same program (no injection) is unaffected.
// ============================================================

const DEGRADED_RECOVERY_SRC: &str = r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let w = fs.open_write(tmp + "/wal.log")!
    w.write("entry")!
    w.sync() catch e: fs.Degraded {
        print(f"degraded: {e.code}")
        let p = e.file
        p.discard()
        fs.remove_dir_all(tmp)!
        return
    }
    print("synced")
    w.close()!
    fs.remove_dir_all(tmp)!
}
"#;

#[test]
fn fs_sync_failure_degrades_and_discard_recovers() {
    let out = run_project_with_stdlib_env(
        &[("main.pluto", DEGRADED_RECOVERY_SRC)],
        &[("PLUTO_FS_SYNC_FAIL_AT", "1")],
    );
    // EIO == 5: the injected failure, reported through the Degraded payload.
    assert_eq!(out, "degraded: 5\n");
}

#[test]
fn fs_sync_success_path_unaffected_by_hook_absence() {
    let out = run_project_with_stdlib(&[("main.pluto", DEGRADED_RECOVERY_SRC)]);
    assert_eq!(out, "synced\n");
}

// ============================================================
// Degradation on write (owner decision D3): a failed write poisons too.
// Writing to a closed-out fd cannot be expressed, so inject via sync's
// sibling: write to a read-only fd obtained by reopening — instead we
// exercise the error-edge consumption shape: the RFC's flush_batch
// helper moves the obligation through a parameter, a catch, and a
// return, with the WAL dying on an injected failure of the SECOND sync.
// ============================================================

#[test]
fn fs_flush_batch_wal_shape() {
    let src = r#"import std.fs

error WalDead {
    code: int
}

fn flush_batch(w: fs.File<fs.Write, fs.Open>, batch: string) fs.File<fs.Write, fs.Open> {
    w.write(batch) catch e: fs.Degraded {
        let p = e.file
        p.discard()
        raise WalDead { code: e.code }
    }
    w.sync_data() catch e: fs.Degraded {
        let p = e.file
        p.discard()
        raise WalDead { code: e.code }
    }
    return w
}

fn main() {
    let tmp = fs.temp_dir()
    let mut w = fs.open_write(tmp + "/wal.log")!
    w = flush_batch(w, "entry 1\n") catch e: WalDead {
        print(f"wal dead: {e.code}")
        fs.remove_dir_all(tmp)!
        return
    }
    w = flush_batch(w, "entry 2\n") catch e: WalDead {
        print(f"wal dead: {e.code}")
        fs.remove_dir_all(tmp)!
        return
    }
    w.close()!
    print(fs.read_all(tmp + "/wal.log")!)
    fs.remove_dir_all(tmp)!
}
"#;
    let ok = run_project_with_stdlib(&[("main.pluto", src)]);
    assert_eq!(ok, "entry 1\nentry 2\n\n");
    let dead = run_project_with_stdlib_env(
        &[("main.pluto", src)],
        &[("PLUTO_FS_SYNC_FAIL_AT", "2")],
    );
    assert_eq!(dead, "wal dead: 5\n");
}

// ============================================================
// replace_all: atomic durable replace — content replaced, no temp file
// left behind, works for both existing and new targets
// ============================================================

#[test]
fn fs_replace_all() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/config.json"
    fs.write_all(path, "old contents")!
    fs.replace_all(path, "new contents")!
    print(fs.read_all(path)!)
    // No temp file left behind: the directory holds exactly one entry.
    print(fs.list_dir(tmp)!.len())
    // Works when the target does not exist yet.
    fs.replace_all(tmp + "/fresh.txt", "fresh")!
    print(fs.read_all(tmp + "/fresh.txt")!)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "new contents\n1\nfresh\n");
}

// ============================================================
// sync_dir on a real directory
// ============================================================

#[test]
fn fs_sync_dir() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    fs.write_all(tmp + "/f.txt", "x")!
    fs.sync_dir(tmp)!
    print("dir synced")
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "dir synced\n");
}

// ============================================================
// stat: Metadata fields (and the size >= 0 invariant proven in-module)
// ============================================================

#[test]
fn fs_stat_metadata() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/meta.txt"
    fs.write_all(path, "12345")!
    let md = fs.stat(path)!
    print(md.size)
    print(md.is_file)
    print(md.is_dir)
    print(md.modified > 0)
    let dmd = fs.stat(tmp)!
    print(dmd.is_dir)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "5\ntrue\nfalse\ntrue\ntrue\n");
}

// ============================================================
// stat / file_size on a missing path raises NotFound (branch-worthy)
// ============================================================

#[test]
fn fs_stat_not_found() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let md = fs.stat("/nonexistent_stat_xyz.txt") catch e: fs.NotFound {
        print(f"missing: {e.path}")
        return
    } catch e: fs.FileError {
        print("other")
        return
    }
    print(md.size)
}
"#,
    )]);
    assert_eq!(out, "missing: /nonexistent_stat_xyz.txt\n");
}

// ============================================================
// create_dir_all / remove_dir_all: nested trees
// ============================================================

#[test]
fn fs_create_dir_all_remove_dir_all() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let deep = tmp + "/a/b/c"
    fs.create_dir_all(deep)!
    print(fs.is_dir(deep))
    // Idempotent on an existing tree.
    fs.create_dir_all(deep)!
    fs.write_all(deep + "/leaf.txt", "x")!
    fs.remove_dir_all(tmp + "/a")!
    print(fs.exists(tmp + "/a"))
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\nfalse\n");
}

// ============================================================
// remove_dir_all refuses "/" and ""
// ============================================================

#[test]
fn fs_remove_dir_all_refusals() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    fs.remove_dir_all("/") catch e: fs.FileError {
        print("refused root")
    } catch e: fs.NotFound {
        print("wrong error")
    }
    fs.remove_dir_all("") catch e: fs.FileError {
        print("refused empty")
    } catch e: fs.NotFound {
        print("wrong error")
    }
}
"#,
    )]);
    assert_eq!(out, "refused root\nrefused empty\n");
}

// ============================================================
// read at EOF returns "" (disambiguated from errors in the runtime)
// ============================================================

#[test]
fn fs_read_eof_returns_empty() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/short.txt"
    fs.write_all(path, "ab")!
    let f = fs.open_read(path)!
    print(f.read(10)!)
    let at_eof = f.read(10)!
    print(at_eof == "")
    f.close()!
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "ab\ntrue\n");
}

// ============================================================
// Seek.Current and Seek.End
// ============================================================

#[test]
fn fs_seek_current_and_end() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/seek2.txt"
    fs.write_all(path, "abcdef")!
    let f = fs.open_read(path)!
    f.seek(fs.Seek.Current { delta: 2 })!
    print(f.read(2)!)
    let pos = f.seek(fs.Seek.End { delta: -1 })!
    print(pos)
    print(f.read(1)!)
    f.close()!
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "cd\n5\nf\n");
}

// ============================================================
// open_append through the handle API
// ============================================================

#[test]
fn fs_open_append_handle() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/log.txt"
    fs.write_all(path, "first")!
    let w = fs.open_append(path)!
    w.write(" second")!
    w.close()!
    print(fs.read_all(path)!)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "first second\n");
}

// ============================================================
// list_dir on a non-directory raises
// ============================================================

#[test]
fn fs_list_dir_not_a_directory() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/plain.txt"
    fs.write_all(path, "x")!
    let entries = fs.list_dir(path) catch e: fs.FileError {
        print(f"not a dir: code {e.code > 0}")
        fs.remove_dir_all(tmp)!
        return
    } catch e: fs.NotFound {
        print("wrong error")
        return
    }
    print(entries.len())
}
"#,
    )]);
    assert_eq!(out, "not a dir: code true\n");
}

// ============================================================
// Typestate rejections: the whole protocol-violation battery is
// compile errors (rfc-fs-api.md test strategy — every violation is a
// CI-testable diagnostic)
// ============================================================

#[test]
fn fs_reject_dropped_open_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    print("leak")
}
"#,
        "'f' still holds fs.File<fs.Read, fs.Open>, a must_release state",
    );
}

#[test]
fn fs_reject_use_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.close()!
    let s = f.read(10)!
    print(s)
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_double_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.close()!
    f.close()!
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_write_on_read_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.write("nope")!
    f.close()!
}
"#,
        "method 'write' does not exist on 'fs.File<fs.Read, fs.Open>'",
    );
}

#[test]
fn fs_reject_read_on_write_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    let s = f.read(10)!
    print(s)
    f.close()!
}
"#,
        "method 'read' does not exist on 'fs.File<fs.Write, fs.Open>'",
    );
}

#[test]
fn fs_reject_wildcard_catch_of_degraded() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.write("data") catch e {
        print("swallowed")
    }
    f.close()!
}
"#,
        "carries fs.File<fs.Write, fs.Poisoned> in field 'file' — a must_release state",
    );
}

#[test]
fn fs_reject_undischarged_poisoned_payload() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.sync() catch e: fs.Degraded {
        print("ignored the poisoned payload")
    }
    f.close()!
}
"#,
        "must_release state, when the catch block ends",
    );
}

#[test]
fn fs_reject_sync_on_poisoned_payload() {
    // The fsyncgate rule as a type error: the method to retry does not
    // exist on the state the failure hands you.
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.sync() catch e: fs.Degraded {
        let p = e.file
        p.sync()!
        p.discard()
        return
    }
    f.close()!
}
"#,
        "method 'sync' does not exist on 'fs.File<fs.Write, fs.Poisoned>'",
    );
}

#[test]
fn fs_reject_closure_capture_of_open_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    let g = () => f.read(10)!
    f.close()!
}
"#,
        "cannot be captured by a closure",
    );
}

// ============================================================
// Bytes I/O (issue #368): binary safety is the point — the string
// paths are byte-exact too (strings carry no UTF-8 invariant), but
// bytes is the honest type at the boundary. Every test round-trips
// all 256 byte values or pins the typestate contract.
// ============================================================

#[test]
fn fs_handle_bytes_round_trip_all_256_values() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/handle.bin"

    let mut payload = bytes_new()
    let mut i = 0
    while i < 256 {
        payload.push(i as byte)
        i = i + 1
    }

    let w = fs.open_write(path)!
    w.write_bytes(payload)!
    w.close()!

    let r = fs.open_read(path)!
    let mut got = bytes_new()
    while true {
        let chunk = r.read_bytes(64)!
        if chunk.len() == 0 {
            break
        }
        let mut c = 0
        while c < chunk.len() {
            got.push(chunk[c])
            c = c + 1
        }
    }
    r.close()!

    let mut ok = got.len() == 256
    let mut j = 0
    while j < got.len() {
        if (got[j] as int) != j {
            ok = false
        }
        j = j + 1
    }
    print(ok)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\n");
}

#[test]
fn fs_one_shot_bytes_round_trip_all_256_values() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/oneshot.bin"

    let mut payload = bytes_new()
    let mut i = 0
    while i < 256 {
        payload.push(i as byte)
        i = i + 1
    }

    fs.write_all_bytes(path, payload)!
    let back = fs.read_all_bytes(path)!
    let mut ok = back.len() == 256
    let mut j = 0
    while j < back.len() {
        if (back[j] as int) != j {
            ok = false
        }
        j = j + 1
    }
    print(ok)

    // append_all_bytes: the same 256 values again — the file now holds
    // two exact copies.
    fs.append_all_bytes(path, payload)!
    let both = fs.read_all_bytes(path)!
    let mut ok2 = both.len() == 512
    let mut k = 0
    while k < both.len() {
        if (both[k] as int) != k % 256 {
            ok2 = false
        }
        k = k + 1
    }
    print(ok2)

    // write_all_bytes truncates like write_all.
    let mut two = bytes_new()
    two.push(0 as byte)
    two.push(255 as byte)
    fs.write_all_bytes(path, two)!
    let small = fs.read_all_bytes(path)!
    print(small.len())
    print(small[0] as int)
    print(small[1] as int)

    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\n2\n0\n255\n");
}

// read_at/write_at are offset-stateless (pread/pwrite): interleaving
// them with sequential read/write proves the descriptor's seek cursor
// is untouched.
#[test]
fn fs_read_at_write_at_positional_independence() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/positional.bin"

    // Sequential write, then a positional patch, then more sequential:
    // if write_at left the cursor alone, the tail lands at offset 8.
    let w = fs.open_write(path)!
    w.write_bytes("abcdefgh".to_bytes())!
    w.write_at(2, "XY".to_bytes())!
    w.write_bytes("ij".to_bytes())!
    w.close()!
    print(fs.read_all(path)!)

    // Sequential read, a positional read, sequential again: the second
    // sequential read continues from offset 2 — read_at never moved it.
    let r = fs.open_read(path)!
    let first = r.read_bytes(2)!
    print(first.to_string())
    let at4 = r.read_at(4, 2)!
    print(at4.to_string())
    let second = r.read_bytes(2)!
    print(second.to_string())

    // read_at composes with seek, and leaves the seeked cursor alone too.
    r.seek(fs.Seek.Start { offset: 8 })!
    let at0 = r.read_at(0, 2)!
    print(at0.to_string())
    let tail = r.read_bytes(2)!
    print(tail.to_string())
    r.close()!

    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "abXYefghij\nab\nef\nXY\nab\nij\n");
}

// EOF is empty bytes with no error — for read_bytes at the end of the
// file and for read_at past it.
#[test]
fn fs_read_bytes_eof_yields_empty() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/eof.bin"
    fs.write_all_bytes(path, "abc".to_bytes())!

    let r = fs.open_read(path)!
    let all = r.read_bytes(16)!
    print(all.len())
    let eof = r.read_bytes(16)!
    print(eof.len())
    let past = r.read_at(100, 16)!
    print(past.len())
    r.close()!

    // One-shot on an empty file: empty bytes, not an error.
    fs.write_all_bytes(tmp + "/empty.bin", bytes_new())!
    print(fs.read_all_bytes(tmp + "/empty.bin")!.len())

    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "3\n0\n0\n0\n");
}

#[test]
fn fs_read_all_bytes_not_found() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let b = fs.read_all_bytes("/definitely_not_here.bin") catch e: fs.NotFound {
        print(f"not found: {e.path}")
        return
    } catch e: fs.FileError {
        print("other")
        return
    }
    print(b.len())
}
"#,
    )]);
    assert_eq!(out, "not found: /definitely_not_here.bin\n");
}

// ============================================================
// Bytes typestate pins: the mode/state/poisoning contract is identical
// to the string methods — compile errors, not runtime behaviors. (No
// write-failure injection hook exists, so Degraded-on-write_bytes is
// pinned at the type level: the poisoned payload cannot be swallowed
// and the write methods do not exist on Poisoned or Closed.)
// ============================================================

#[test]
fn fs_reject_read_bytes_on_write_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    let b = f.read_bytes(10)!
    print(b.len())
    f.close()!
}
"#,
        "method 'read_bytes' does not exist on 'fs.File<fs.Write, fs.Open>'",
    );
}

#[test]
fn fs_reject_read_at_on_write_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    let b = f.read_at(0, 10)!
    print(b.len())
    f.close()!
}
"#,
        "method 'read_at' does not exist on 'fs.File<fs.Write, fs.Open>'",
    );
}

#[test]
fn fs_reject_write_bytes_on_read_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.write_bytes("nope".to_bytes())!
    f.close()!
}
"#,
        "method 'write_bytes' does not exist on 'fs.File<fs.Read, fs.Open>'",
    );
}

#[test]
fn fs_reject_write_at_on_read_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.write_at(0, "nope".to_bytes())!
    f.close()!
}
"#,
        "method 'write_at' does not exist on 'fs.File<fs.Read, fs.Open>'",
    );
}

#[test]
fn fs_reject_read_bytes_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.close()!
    let b = f.read_bytes(10)!
    print(b.len())
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_write_bytes_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.close()!
    f.write_bytes("late".to_bytes())!
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_wildcard_catch_of_write_bytes_degraded() {
    // write_bytes degrades like write (owner decision D3): the Poisoned
    // payload is must_release, so a wildcard catch cannot swallow it.
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.write_bytes("data".to_bytes()) catch e {
        print("swallowed")
    }
    f.close()!
}
"#,
        "carries fs.File<fs.Write, fs.Poisoned> in field 'file' — a must_release state",
    );
}

// ============================================================
// File↔socket relay (issue #373, half 1): send_to moves file bytes into a
// socket kernel-side (sendfile(2), falling back to a runtime-C pread→write
// loop), receive_from is the C-loop counterpart — neither round-trips the
// GC heap. Integrity tests push ≥ 1 MiB with a non-power-of-two tail (all
// 256 byte values repeated) through real TCP loopback and verify exact
// reassembly on the far side.
// ============================================================

// send_to integrity + partial/looping: a 100000-byte max_bytes loop over a
// 1 MiB + 37 file completes the transfer; a spawned drain re-assembles and
// byte-compares; the file's seek cursor is untouched afterwards.
const SEND_TO_INTEGRITY_SRC: &str = r#"import std.fs
import std.socket

fn drain_check(fd: int) int {
    let mut total = 0
    let mut idx = 0
    let mut ok = true
    while true {
        let chunk = socket.read_bytes(fd, 65536)
        if chunk.len() == 0 {
            break
        }
        let mut c = 0
        while c < chunk.len() {
            if (chunk[c] as int) != idx % 256 {
                ok = false
            }
            idx = idx + 1
            c = c + 1
        }
        total = total + chunk.len()
    }
    if ok {
        return total
    }
    return 0 - 1
}

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/payload.bin"
    let mut payload = bytes_new()
    let mut i = 0
    while i < 1048613 {
        payload.push((i % 256) as byte)
        i = i + 1
    }
    fs.write_all_bytes(path, payload)!

    let lfd = socket.create(2, 1, 0)
    socket.set_reuseaddr(lfd)
    socket.bind(lfd, "127.0.0.1", 0)
    socket.listen(lfd, 8)
    let port = socket.get_port(lfd)
    let cfd = socket.create(2, 1, 0)
    socket.connect(cfd, "127.0.0.1", port)
    let sfd = socket.accept(lfd)

    let t = spawn drain_check(sfd)

    let f = fs.open_read(path)!
    let mut sent = 0
    while true {
        let n = f.send_to(cfd, sent, 100000)!
        if n == 0 {
            break
        }
        sent = sent + n
    }
    socket.close(cfd)
    print(sent)
    print(t.get())

    // Cursor untouched by the whole relay: a sequential read starts at 0.
    let head = f.read_bytes(2)!
    print(head[0] as int)
    print(head[1] as int)
    f.close()!
    socket.close(sfd)
    socket.close(lfd)
    fs.remove_dir_all(tmp)!
}
"#;

#[test]
fn fs_send_to_relays_1mib_exactly() {
    let out = run_project_with_stdlib(&[("main.pluto", SEND_TO_INTEGRITY_SRC)]);
    assert_eq!(out, "1048613\n1048613\n0\n1\n");
}

// The same integrity battery through the forced C fallback loop
// (PLUTO_FS_RELAY_NO_SENDFILE=1 skips the kernel path): observable
// semantics must be identical.
#[test]
fn fs_send_to_fallback_loop_relays_1mib_exactly() {
    let out = run_project_with_stdlib_env(
        &[("main.pluto", SEND_TO_INTEGRITY_SRC)],
        &[("PLUTO_FS_RELAY_NO_SENDFILE", "1")],
    );
    assert_eq!(out, "1048613\n1048613\n0\n1\n");
}

// Offset semantics, single-threaded (small windows fit loopback buffers):
// send_to from a nonzero offset sends exactly the right window, EOF before
// max_bytes returns the short count, and the seek cursor never moves —
// proven by interleaved sequential reads.
#[test]
fn fs_send_to_offset_window_and_cursor() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs
import std.socket

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/window.bin"
    fs.write_all(path, "abcdefghij")!

    let lfd = socket.create(2, 1, 0)
    socket.set_reuseaddr(lfd)
    socket.bind(lfd, "127.0.0.1", 0)
    socket.listen(lfd, 8)
    let port = socket.get_port(lfd)
    let cfd = socket.create(2, 1, 0)
    socket.connect(cfd, "127.0.0.1", port)
    let sfd = socket.accept(lfd)

    let f = fs.open_read(path)!
    // Advance the cursor first, so "untouched" is distinguishable from "reset".
    print(f.read(2)!)

    // Window from a nonzero offset: exactly bytes 3..7.
    let n = f.send_to(cfd, 3, 4)!
    print(n)
    print(socket.read_bytes(sfd, 16).to_string())

    // EOF before max_bytes: the short count comes back, not an error.
    let short = f.send_to(cfd, 8, 100)!
    print(short)
    print(socket.read_bytes(sfd, 16).to_string())

    // The cursor is still where the sequential read left it.
    print(f.read(2)!)

    f.close()!
    socket.close(cfd)
    socket.close(sfd)
    socket.close(lfd)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "ab\n4\ndefg\n2\nij\ncd\n");
}

// receive_from integrity: a spawned task relays the payload file into the
// socket with send_to; the main thread receive_from-loops it into a fresh
// file; the file is re-read and byte-compared. Also pins EOF-before-data
// returning 0 with no error.
#[test]
fn fs_receive_from_relays_1mib_exactly() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs
import std.socket

fn feed_file(path: string, total: int, sock_fd: int) int {
    let f = fs.open_read(path) catch e: fs.NotFound {
        return 0 - 1
    } catch e: fs.FileError {
        return 0 - 2
    }
    let mut sent = 0
    while sent < total {
        let n = f.send_to(sock_fd, sent, 65536) catch 0
        if n == 0 {
            break
        }
        sent = sent + n
    }
    f.close() catch e: fs.CloseError {
        socket.close(sock_fd)
        return 0 - 3
    }
    socket.close(sock_fd)
    return sent
}

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/payload.bin"
    let mut payload = bytes_new()
    let mut i = 0
    while i < 1048613 {
        payload.push((i % 256) as byte)
        i = i + 1
    }
    fs.write_all_bytes(path, payload)!

    let lfd = socket.create(2, 1, 0)
    socket.set_reuseaddr(lfd)
    socket.bind(lfd, "127.0.0.1", 0)
    socket.listen(lfd, 8)
    let port = socket.get_port(lfd)
    let cfd = socket.create(2, 1, 0)
    socket.connect(cfd, "127.0.0.1", port)
    let sfd = socket.accept(lfd)

    let t = spawn feed_file(path, 1048613, cfd)

    let dst = tmp + "/dst.bin"
    let w = fs.open_write(dst)!
    let mut recd = 0
    while true {
        let n = w.receive_from(sfd, 70000)!
        if n == 0 {
            break
        }
        recd = recd + n
    }
    w.close()!
    print(t.get())
    print(recd)

    let back = fs.read_all_bytes(dst)!
    let mut ok = back.len() == 1048613
    let mut j = 0
    while j < back.len() {
        if (back[j] as int) != j % 256 {
            ok = false
        }
        j = j + 1
    }
    print(ok)

    // EOF before any data: a pair whose writer closes immediately yields 0.
    let c2 = socket.create(2, 1, 0)
    socket.connect(c2, "127.0.0.1", port)
    let s2 = socket.accept(lfd)
    socket.close(c2)
    let w2 = fs.open_write(tmp + "/empty.bin")!
    print(w2.receive_from(s2, 1000)!)
    w2.close()!
    socket.close(s2)

    socket.close(sfd)
    socket.close(lfd)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "1048613\n1048613\ntrue\n0\n");
}

// net.TcpConnection.fd(): the bridge between the typed connection object
// and the all-int relay primitives — a round-trip through it.
#[test]
fn fs_send_to_through_tcp_connection_fd() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs
import std.net

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/doc.txt"
    fs.write_all(path, "hello relay")!

    let server = net.listen("127.0.0.1", 0)
    let client = net.connect("127.0.0.1", server.port())
    let conn = server.accept()

    let f = fs.open_read(path)!
    let n = f.send_to(client.fd(), 0, 1024)!
    print(n)
    f.close()!
    print(conn.read(1024)!)

    client.close()
    conn.close()
    server.close()
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "11\nhello relay\n");
}

// ── Relay typestate pins: wrong mode, wrong state, and poison-swallowing
// are compile errors, exactly like the #386 read/write pins. ──

#[test]
fn fs_reject_send_to_on_write_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    let n = f.send_to(5, 0, 10)!
    print(n)
    f.close()!
}
"#,
        "method 'send_to' does not exist on 'fs.File<fs.Write, fs.Open>'",
    );
}

#[test]
fn fs_reject_receive_from_on_read_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    let n = f.receive_from(5, 10)!
    print(n)
    f.close()!
}
"#,
        "method 'receive_from' does not exist on 'fs.File<fs.Read, fs.Open>'",
    );
}

#[test]
fn fs_reject_send_to_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.close()!
    let n = f.send_to(5, 0, 10)!
    print(n)
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_receive_from_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.close()!
    let n = f.receive_from(5, 10)!
    print(n)
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

#[test]
fn fs_reject_wildcard_catch_of_receive_from_degraded() {
    // A file-side write failure during receive_from degrades the handle
    // exactly like write: the Poisoned payload is must_release, so a
    // wildcard catch cannot swallow it.
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    let n = f.receive_from(5, 10) catch e {
        print("swallowed")
        0
    }
    print(n)
    f.close()!
}
"#,
        "carries fs.File<fs.Write, fs.Poisoned> in field 'file' — a must_release state",
    );
}

#[test]
fn fs_reject_receive_from_on_poisoned_payload() {
    // No relay off a destroyed warrant: receive_from does not exist on
    // the state a Degraded failure hands you.
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.sync() catch e: fs.Degraded {
        let p = e.file
        let n = p.receive_from(5, 10)!
        print(n)
        p.discard()
        return
    }
    f.close()!
}
"#,
        "method 'receive_from' does not exist on 'fs.File<fs.Write, fs.Poisoned>'",
    );
}

#[test]
fn fs_reject_write_bytes_on_poisoned_payload() {
    // The degradation rule as a type error: the method to retry does not
    // exist on the state the failure hands you.
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.sync() catch e: fs.Degraded {
        let p = e.file
        p.write_bytes("retry".to_bytes())!
        p.discard()
        return
    }
    f.close()!
}
"#,
        "method 'write_bytes' does not exist on 'fs.File<fs.Write, fs.Poisoned>'",
    );
}

// ============================================================
// Truncation (issue #397): File.truncate (ftruncate) and the
// path-level one-shot fs.truncate (truncate(2)). Shrinking discards
// the tail, extending zero-fills — syscall-faithful POSIX semantics,
// documented rather than restricted. Negative len is a condition on
// caller input: it raises FileError with the handle still sound,
// before any syscall. A failed ftruncate degrades like a failed
// write. Wrong mode/state are compile errors, as everywhere else.
// ============================================================

#[test]
fn fs_truncate_shrinks() {
    // The WAL-recovery shape the issue asks for: cut the torn tail to
    // the valid prefix in one syscall, sync so the shrink is durable,
    // and read back exactly the prefix.
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/wal.log"
    fs.write_all(path, "good entry|torn ent")!
    let w = fs.open_append(path)!
    w.truncate(10)!
    w.sync()!
    w.close()!
    print(fs.read_all(path)!)
    print(fs.file_size(path)!)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "good entry\n10\n");
}

#[test]
fn fs_truncate_to_zero() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/empty_me.txt"
    fs.write_all(path, "contents")!
    let w = fs.open_append(path)!
    w.truncate(0)!
    w.close()!
    print(fs.file_size(path)!)
    let back = fs.read_all(path)!
    print(back.len())
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "0\n0\n");
}

#[test]
fn fs_truncate_extends_zero_fills() {
    // Growth is allowed and means what ftruncate means: the new tail
    // reads back as zero bytes.
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/grow.bin"
    fs.write_all(path, "ab")!
    let w = fs.open_append(path)!
    w.truncate(5)!
    w.close()!
    let b = fs.read_all_bytes(path)!
    print(b.len())
    print(b[0] as int)
    print(b[1] as int)
    print(b[2] as int)
    print(b[4] as int)
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    // 'a' = 97, 'b' = 98, zero-filled tail.
    assert_eq!(out, "5\n97\n98\n0\n0\n");
}

#[test]
fn fs_truncate_negative_len_raises_file_error() {
    // Negative length is caller input, not corruption: FileError (code
    // 0, checked before any syscall), NOT Degraded — the same
    // sound-file/destroyed-warrant split receive_from draws. The
    // linearity checker is per-call, so both arms terminate (any error
    // path of a Degraded-raising call consumes the binding).
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/neg.txt"
    let w = fs.open_write(path)!
    w.write("intact")!
    w.truncate(-1) catch e: fs.FileError {
        print(f"rejected: {e.message}")
        print(f"code: {e.code}")
        print(fs.read_all(path)!)
        fs.remove_dir_all(tmp)!
        return
    } catch e: fs.Degraded {
        print("wrong error")
        let p = e.file
        p.discard()
        fs.remove_dir_all(tmp)!
        return
    }
    print("not rejected")
    w.close()!
}
"#,
    )]);
    assert_eq!(out, "rejected: truncate: negative length\ncode: 0\nintact\n");
}

#[test]
fn fs_truncate_path_level() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.fs

fn main() {
    let tmp = fs.temp_dir()
    let path = tmp + "/oneshot.txt"
    fs.write_all(path, "keep|drop")!
    fs.truncate(path, 4)!
    print(fs.read_all(path)!)
    fs.truncate(path, -5) catch e: fs.FileError {
        print(f"rejected: {e.message}")
    } catch e: fs.NotFound {
        print("wrong error")
    }
    fs.truncate(tmp + "/missing.txt", 3) catch e: fs.NotFound {
        print("not found")
    } catch e: fs.FileError {
        print("wrong error")
    }
    fs.remove_dir_all(tmp)!
}
"#,
    )]);
    assert_eq!(out, "keep\nrejected: truncate: negative length\nnot found\n");
}

#[test]
fn fs_reject_truncate_on_read_handle() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_read("/etc/hosts")!
    f.truncate(0)!
    f.close()!
}
"#,
        "method 'truncate' does not exist on 'fs.File<fs.Read, fs.Open>'",
    );
}

#[test]
fn fs_reject_truncate_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.fs

fn main() {
    let f = fs.open_write("/tmp/reject_probe.txt")!
    f.close()!
    f.truncate(0)!
}
"#,
        "'f' was consumed by the transition '.close()'",
    );
}

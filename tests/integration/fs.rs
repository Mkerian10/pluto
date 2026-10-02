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

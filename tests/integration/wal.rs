//! std.wal — the write-ahead log as evidence.
//!
//! Runtime tests drive the full protocol (recover → replay → append →
//! acknowledge → checkpoint → close) across simulated crashes: a "crash"
//! is closing without checkpointing (or not closing at all and letting
//! the file speak for itself), then recovering in a fresh Wal. Torn
//! tails and CRC corruption are manufactured with raw std.fs writes.
//!
//! The typestate battery mirrors fs.rs: every protocol violation —
//! appending before recovery, leaking a live log, using a consumed
//! binding, swallowing a Degraded payload — is a compile error.

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

fn run_with_stdlib(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.pluto"), source).unwrap();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&manifest_dir.join("stdlib"), &stdlib_dst);

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "Binary exited with non-zero status. stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

fn compile_with_stdlib_should_fail(source: &str, expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.pluto"), source).unwrap();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&manifest_dir.join("stdlib"), &stdlib_dst);

    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");

    match pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst)) {
        Ok(_) => panic!("Expected compilation to fail, but it succeeded"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "Expected error containing '{expected_msg}', got:\n{msg}"
            );
        }
    }
}

// ============================================================
// The protocol, across a simulated crash
// ============================================================

#[test]
fn wal_first_boot_replay_and_checkpoint_cycle() {
    let out = run_with_stdlib(
        r#"import std.wal
import std.fs

fn main() {
    let path = fs.temp_dir() + "/pluto_wal_cycle.log"
    fs.remove(path) catch e {}

    // First boot: nothing to recover.
    let intent = wal.open(path)
    let mut w = intent.recover()!
    let r0 = w.recovery()
    print(f"boot: {r0.recovered} entries, {r0.truncated_bytes} cut")

    // Durable intent: two keys, one written twice (offsets are exact:
    // each record is 8 header bytes + 4 key_len bytes + key + data).
    let o1 = w.append("a", "v1".to_bytes())!
    let o2 = w.append("b", "v2".to_bytes())!
    let o3 = w.append("a", "v3".to_bytes())!
    print(f"offsets: {o1} {o2} {o3}")

    // Crash before checkpoint: close keeps the intent on disk.
    w.close()!

    // Resurrection: everything unacknowledged comes back, in order.
    let intent2 = wal.open(path)
    let mut w2 = intent2.recover()!
    let r1 = w2.recovery()
    print(f"recovered: {r1.recovered} entries, {r1.truncated_bytes} cut")
    let entries = w2.replay()
    let mut i = 0
    while i < entries.len() {
        print(f"  {entries[i].offset}: {entries[i].key} = {entries[i].data.to_string()}")
        i = i + 1
    }

    // The dedup view: last write per key.
    let m = w2.latest()
    let la = m["a"].to_string()
    let lb = m["b"].to_string()
    print(f"latest a = {la}, b = {lb}")

    // Acknowledge and checkpoint: history drops, offsets restart.
    w2.mark_applied()
    let ck = w2.checkpoint() catch e: wal.Degraded {
        let p = e.wal
        p.discard()
        print("degraded")
        return
    }
    match ck {
        wal.Checkpoint.Done {
            print("checkpointed")
        }
        wal.Checkpoint.Refused { appended: a, applied: b } {
            print(f"refused {b}/{a}")
        }
    }
    let o4 = w2.append("c", "v4".to_bytes())!
    print(f"after checkpoint: {o4}")
    w2.close()!

    // Only the post-checkpoint record survives.
    let intent3 = wal.open(path)
    let mut w3 = intent3.recover()!
    print(f"final: {w3.recovery().recovered} entries")
    w3.close()!

    // An unexecuted intent can be walked away from — explicitly.
    let spare = wal.open(path)
    spare.abandon()
    fs.remove(path)!
}
"#,
    );
    assert_eq!(
        out,
        "boot: 0 entries, 0 cut\noffsets: 0 15 30\nrecovered: 3 entries, 0 cut\n  0: a = v1\n  15: b = v2\n  30: a = v3\nlatest a = v3, b = v2\ncheckpointed\nafter checkpoint: 0\nfinal: 1 entries\n"
    );
}

#[test]
fn wal_torn_tail_is_cut_durably_and_reported() {
    let out = run_with_stdlib(
        r#"import std.wal
import std.fs

fn main() {
    let path = fs.temp_dir() + "/pluto_wal_torn.log"
    fs.remove(path) catch e {}

    let intent = wal.open(path)
    let mut w = intent.recover()!
    w.append("k1", "alpha".to_bytes())!
    w.append("k2", "beta".to_bytes())!
    w.close()!
    let clean = fs.file_size(path)!

    // Crash mid-append: a frame header claiming more payload than the
    // file holds (the torn-write signature).
    let mut garbage = bytes_new()
    garbage.push((0xFF).to_byte())
    garbage.push((0xFF).to_byte())
    garbage.push((0x00).to_byte())
    garbage.push((0x10).to_byte())
    garbage.push((0xDE).to_byte())
    garbage.push((0xAD).to_byte())
    garbage.push((0xBE).to_byte())
    garbage.push((0xEF).to_byte())
    garbage.push((0x01).to_byte())
    fs.append_all_bytes(path, garbage)!

    // Recovery cuts the tail, keeps the intact prefix, and says so.
    let intent2 = wal.open(path)
    let mut w2 = intent2.recover()!
    let r = w2.recovery()
    print(f"recovered {r.recovered}, cut {r.truncated_bytes}")
    print(f"file back to clean length: {fs.file_size(path)! == clean}")

    // The log is appendable again, right where the good data ends.
    let o = w2.append("k3", "gamma".to_bytes())!
    print(f"appended at {o == clean}")
    w2.close()!

    let intent3 = wal.open(path)
    let mut w3 = intent3.recover()!
    print(f"all three: {w3.recovery().recovered}")
    w3.close()!
    fs.remove(path)!
}
"#,
    );
    assert_eq!(
        out,
        "recovered 2, cut 9\nfile back to clean length: true\nappended at true\nall three: 3\n"
    );
}

#[test]
fn wal_crc_mismatch_ends_the_readable_log() {
    let out = run_with_stdlib(
        r#"import std.wal
import std.fs

fn main() {
    let path = fs.temp_dir() + "/pluto_wal_crc.log"
    fs.remove(path) catch e {}

    let intent = wal.open(path)
    let mut w = intent.recover()!
    let o1 = w.append("k1", "alpha".to_bytes())!
    let o2 = w.append("k2", "beta".to_bytes())!
    w.close()!

    // Flip the last byte of the second record's payload: its checksum
    // no longer verifies, so recovery must stop at the first record.
    let data = fs.read_all_bytes(path)!
    let n = data.len()
    let mut patched = bytes_new()
    let mut i = 0
    while i < n - 1 {
        patched.push(data[i])
        i = i + 1
    }
    patched.push((data[n - 1].to_int() ^ 0xFF).low_byte())
    fs.write_all_bytes(path, patched)!

    let intent2 = wal.open(path)
    let mut w2 = intent2.recover()!
    let r = w2.recovery()
    print(f"recovered {r.recovered}, cut-to-first-record {r.truncated_bytes == n - o2}")
    print(f"survivor: {w2.replay()[0].key}")
    w2.close()!
    fs.remove(path)!
}
"#,
    );
    assert_eq!(out, "recovered 1, cut-to-first-record true\nsurvivor: k1\n");
}

#[test]
fn wal_checkpoint_refuses_unacknowledged_intent() {
    let out = run_with_stdlib(
        r#"import std.wal
import std.fs

fn main() {
    let path = fs.temp_dir() + "/pluto_wal_ack.log"
    fs.remove(path) catch e {}

    let intent = wal.open(path)
    let mut w = intent.recover()!
    w.append("job", "payload".to_bytes())!
    w.close()!

    // Recovered intent counts as appended-but-unapplied: a fresh
    // recovery cannot drop history it has not acknowledged.
    let intent2 = wal.open(path)
    let mut w2 = intent2.recover()!
    print(f"pending: {w2.pending()}")
    let first = w2.checkpoint() catch e: wal.Degraded {
        let p = e.wal
        p.discard()
        print("degraded")
        return
    }
    match first {
        wal.Checkpoint.Done {
            print("dropped unacknowledged history!")
        }
        wal.Checkpoint.Refused { appended: a, applied: b } {
            print(f"refused: {b} of {a} acknowledged")
        }
    }

    // Acknowledge, then the same call succeeds.
    w2.mark_applied()
    let second = w2.checkpoint() catch e: wal.Degraded {
        let p = e.wal
        p.discard()
        print("degraded")
        return
    }
    match second {
        wal.Checkpoint.Done {
            print(f"empty now: {fs.file_size(path)! == 0}")
        }
        wal.Checkpoint.Refused { appended: a, applied: b } {
            print(f"still refused {b}/{a}")
        }
    }
    w2.close()!
    fs.remove(path)!
}
"#,
    );
    assert_eq!(out, "pending: 1\nrefused: 0 of 1 acknowledged\nempty now: true\n");
}

// ============================================================
// Typestate rejections: the protocol-violation battery is compile
// errors, not runtime behaviors (same strategy as fs.rs)
// ============================================================

#[test]
fn wal_reject_append_before_recovery() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let mut w = wal.open("/tmp/wal_reject.log")
    w.append("k", "v".to_bytes())!
}
"#,
        "method 'append' does not exist on 'wal.Wal<wal.Unrecovered>'",
    );
}

#[test]
fn wal_reject_dropped_unrecovered_log() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let w = wal.open("/tmp/wal_reject.log")
    print("leak")
}
"#,
        "'w' still holds wal.Wal<wal.Unrecovered>, a must_release state",
    );
}

#[test]
fn wal_reject_dropped_ready_log() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let intent = wal.open("/tmp/wal_reject.log")
    let w = intent.recover()!
    print("leak")
}
"#,
        "'w' still holds wal.Wal<wal.Ready>, a must_release state",
    );
}

#[test]
fn wal_reject_use_after_close() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let intent = wal.open("/tmp/wal_reject.log")
    let mut w = intent.recover()!
    w.close()!
    w.append("k", "v".to_bytes())!
}
"#,
        "'w' was consumed by the transition '.close()'",
    );
}

#[test]
fn wal_reject_wildcard_catch_of_degraded() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let intent = wal.open("/tmp/wal_reject.log")
    let mut w = intent.recover()!
    let o = w.append("k", "v".to_bytes()) catch e {
        0
    }
    print(o)
    w.close()!
}
"#,
        "carries wal.Wal<wal.Poisoned> in field 'wal' — a must_release state",
    );
}

#[test]
fn wal_reject_undischarged_poisoned_payload() {
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let intent = wal.open("/tmp/wal_reject.log")
    let mut w = intent.recover()!
    let o = w.append("k", "v".to_bytes()) catch e: wal.Degraded {
        0
    } catch e: wal.WalError {
        0
    }
    print(o)
    w.close()!
}
"#,
        "must_release state, when the catch block ends",
    );
}

#[test]
fn wal_reject_append_on_poisoned_log() {
    // There is no way to keep appending to a log whose durability
    // warrant died: the method does not exist on the poisoned state.
    compile_with_stdlib_should_fail(
        r#"import std.wal

fn main() {
    let intent = wal.open("/tmp/wal_reject.log")
    let mut w = intent.recover()!
    w.append("k", "v".to_bytes()) catch e: wal.Degraded {
        let p = e.wal
        p.append("k", "v".to_bytes())!
        p.discard()
        return
    }
    w.close()!
}
"#,
        "method 'append' does not exist on 'wal.Wal<wal.Poisoned>'",
    );
}

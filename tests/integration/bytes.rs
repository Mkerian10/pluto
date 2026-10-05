mod common;
use common::*;

// ── Byte basics ──────────────────────────────────────────────────────────────

#[test]
fn byte_basic_cast_and_print() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = 42 as byte
    print(b as int)
    return 0
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn byte_function_param_and_return() {
    let out = compile_and_run_stdout(r#"
fn double(b: byte) byte {
    return ((b as int) * 2) as byte
}
fn main() int {
    let result = double(21 as byte)
    print(result as int)
    return 0
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn byte_let_with_type_annotation() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b: byte = 65 as byte
    print(b as int)
    return 0
}
"#);
    assert_eq!(out, "65\n");
}

// ── Hex literals ─────────────────────────────────────────────────────────────

#[test]
fn hex_literal_ff() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0xFF
    print(x)
    return 0
}
"#);
    assert_eq!(out, "255\n");
}

#[test]
fn hex_literal_lowercase() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0xff
    print(x)
    return 0
}
"#);
    assert_eq!(out, "255\n");
}

#[test]
fn hex_literal_0a() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0x0A
    print(x)
    return 0
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn hex_literal_single_digit() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0xA
    print(x)
    return 0
}
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn hex_literal_zero() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0x0
    print(x)
    return 0
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn hex_literal_with_underscores() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let x = 0xFF_AB
    print(x)
    return 0
}
"#);
    assert_eq!(out, "65451\n");
}

#[test]
fn hex_literal_as_byte() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = 0xFF as byte
    print(b as int)
    return 0
}
"#);
    assert_eq!(out, "255\n");
}

// ── Casting ──────────────────────────────────────────────────────────────────

#[test]
fn byte_cast_truncation() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = 256 as byte
    print(b as int)
    return 0
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
fn byte_cast_negative() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = -1 as byte
    print(b as int)
    return 0
}
"#);
    assert_eq!(out, "255\n");
}

#[test]
fn byte_cast_roundtrip() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = 42 as byte
    let i = b as int
    print(i)
    return 0
}
"#);
    assert_eq!(out, "42\n");
}

// ── Byte equality ────────────────────────────────────────────────────────────

#[test]
fn byte_equality() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let a = 42 as byte
    let b = 42 as byte
    let c = 43 as byte
    if a == b {
        print("eq")
    }
    if a != c {
        print("neq")
    }
    return 0
}
"#);
    assert_eq!(out, "eq\nneq\n");
}

// ── Byte ordering ────────────────────────────────────────────────────────────

#[test]
fn byte_ordering() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let a = 10 as byte
    let b = 20 as byte
    if a < b {
        print("lt")
    }
    if b > a {
        print("gt")
    }
    if a <= 10 as byte {
        print("lte")
    }
    if b >= 20 as byte {
        print("gte")
    }
    return 0
}
"#);
    assert_eq!(out, "lt\ngt\nlte\ngte\n");
}

#[test]
fn byte_ordering_unsigned() {
    // Regression: bytes are unsigned, 0xFF > 0x7F
    let out = compile_and_run_stdout(r#"
fn main() int {
    let high = 0xFF as byte
    let mid = 0x7F as byte
    if high > mid {
        print("unsigned_correct")
    }
    return 0
}
"#);
    assert_eq!(out, "unsigned_correct\n");
}

// ── Bytes new, push, len ─────────────────────────────────────────────────────

#[test]
fn bytes_new_push_len() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(65 as byte)
    buf.push(66 as byte)
    buf.push(67 as byte)
    print(buf.len())
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

// ── Bytes indexing ────────────────────────────────────────────────────────────

#[test]
fn bytes_index_read() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(65 as byte)
    buf.push(66 as byte)
    let b = buf[0]
    print(b as int)
    print(buf[1] as int)
    return 0
}
"#);
    assert_eq!(out, "65\n66\n");
}

#[test]
fn bytes_index_write() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let mut buf = bytes_new()
    buf.push(65 as byte)
    buf[0] = 90 as byte
    print(buf[0] as int)
    return 0
}
"#);
    assert_eq!(out, "90\n");
}

// ── Bytes iteration ──────────────────────────────────────────────────────────

#[test]
fn bytes_for_loop() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(1 as byte)
    buf.push(2 as byte)
    buf.push(3 as byte)
    let mut sum = 0
    for b in buf {
        sum = sum + (b as int)
    }
    print(sum)
    return 0
}
"#);
    assert_eq!(out, "6\n");
}

// ── String conversion ────────────────────────────────────────────────────────

#[test]
fn bytes_to_string() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(72 as byte)
    buf.push(105 as byte)
    let s = buf.to_string()
    print(s)
    return 0
}
"#);
    assert_eq!(out, "Hi\n");
}

#[test]
fn string_to_bytes() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = "ABC"
    let buf = s.to_bytes()
    print(buf.len())
    print(buf[0] as int)
    print(buf[1] as int)
    print(buf[2] as int)
    return 0
}
"#);
    assert_eq!(out, "3\n65\n66\n67\n");
}

#[test]
fn string_to_bytes_to_string_roundtrip() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let original = "Hello"
    let buf = original.to_bytes()
    let restored = buf.to_string()
    print(restored)
    return 0
}
"#);
    assert_eq!(out, "Hello\n");
}

#[test]
fn bytes_interior_nul_truncates_on_print() {
    // Known limitation: NUL-terminated strings truncate on print
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(65 as byte)
    buf.push(0 as byte)
    buf.push(66 as byte)
    let s = buf.to_string()
    print(s)
    return 0
}
"#);
    assert_eq!(out, "A\n");
}

// ── Byte as Map key ──────────────────────────────────────────────────────────

#[test]
fn byte_as_map_key() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let mut m = Map<byte, string> {}
    m[65 as byte] = "A"
    m[66 as byte] = "B"
    print(m[65 as byte])
    print(m[66 as byte])
    return 0
}
"#);
    assert_eq!(out, "A\nB\n");
}

// ── Byte as Set element ──────────────────────────────────────────────────────

#[test]
fn byte_as_set_element() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let s = Set<byte> {}
    s.insert(10 as byte)
    s.insert(20 as byte)
    s.insert(10 as byte)
    print(s.len())
    if s.contains(10 as byte) {
        print("has_10")
    }
    return 0
}
"#);
    assert_eq!(out, "2\nhas_10\n");
}

// ── String interpolation with byte ───────────────────────────────────────────

#[test]
fn byte_string_interpolation() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = 42 as byte
    print(f"value: {b}")
    return 0
}
"#);
    assert_eq!(out, "value: 42\n");
}

// ── Bytes in functions ───────────────────────────────────────────────────────

#[test]
fn bytes_as_function_param() {
    let out = compile_and_run_stdout(r#"
fn sum_bytes(buf: bytes) int {
    let mut total = 0
    for b in buf {
        total = total + (b as int)
    }
    return total
}

fn main() int {
    let buf = bytes_new()
    buf.push(10 as byte)
    buf.push(20 as byte)
    buf.push(30 as byte)
    print(sum_bytes(buf))
    return 0
}
"#);
    assert_eq!(out, "60\n");
}

#[test]
fn bytes_as_function_return() {
    let out = compile_and_run_stdout(r#"
fn make_bytes() bytes {
    let buf = bytes_new()
    buf.push(1 as byte)
    buf.push(2 as byte)
    return buf
}

fn main() int {
    let buf = make_bytes()
    print(buf.len())
    print(buf[0] as int)
    return 0
}
"#);
    assert_eq!(out, "2\n1\n");
}

// ── Test framework ───────────────────────────────────────────────────────────

#[test]
fn byte_test_to_equal() {
    let (stdout, _stderr, code) = compile_test_and_run(r#"
test "byte equality" {
    let b = 42 as byte
    expect(b).to_equal(42 as byte)
}
"#);
    assert_eq!(code, 0, "stdout: {stdout}");
}

// ── Compile errors ───────────────────────────────────────────────────────────

#[test]
fn byte_no_implicit_coercion() {
    // `let b: byte = 42` should fail — 42 is int, not byte
    compile_should_fail_with(r#"
fn main() int {
    let b: byte = 42
    return 0
}
"#, "expected byte, found int");
}

#[test]
fn bytes_equality_disallowed() {
    compile_should_fail_with(r#"
fn main() int {
    let a = bytes_new()
    let b = bytes_new()
    if a == b {
        print("same")
    }
    return 0
}
"#, "cannot compare bytes");
}

#[test]
fn bytes_to_equal_disallowed() {
    compile_test_should_fail_with(r#"
test "bytes eq" {
    let a = bytes_new()
    expect(a).to_equal(bytes_new())
}
"#, "cannot use to_equal() with bytes");
}

#[test]
fn bytes_push_wrong_type() {
    compile_should_fail_with(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(42)
    return 0
}
"#, "expected byte, found int");
}

#[test]
fn bytes_unknown_method() {
    compile_should_fail_with(r#"
fn main() int {
    let buf = bytes_new()
    buf.foo()
    return 0
}
"#, "bytes has no method");
}

// ── Runtime abort: OOB index ─────────────────────────────────────────────────

#[test]
fn bytes_oob_index_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let buf = bytes_new()
    buf.push(1 as byte)
    let x = buf[5]
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes index out of bounds"), "stderr: {stderr}");
}

// ── Many bytes (grow buffer) ─────────────────────────────────────────────────

#[test]
fn bytes_grow_beyond_initial_capacity() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    let mut i = 0
    while i < 100 {
        buf.push((i as byte))
        i = i + 1
    }
    print(buf.len())
    print(buf[0] as int)
    print(buf[99] as int)
    return 0
}
"#);
    assert_eq!(out, "100\n0\n99\n");
}

// ── Bytes at the extern boundary (#368 slice 1) ──────────────────────────────

/// Extern fns may take and return `bytes` — previously rejected by the
/// extern whitelist, which was the hard blocker for stdlib bytes I/O.
#[test]
fn extern_fn_accepts_bytes_params_and_return() {
    let out = compile_and_run_stdout(r#"
extern fn __pluto_string_to_bytes(s: string) bytes
extern fn __pluto_bytes_len(b: bytes) int

fn main() int {
    let b = __pluto_string_to_bytes("hey")
    print(__pluto_bytes_len(b))
    return 0
}
"#);
    assert_eq!(out, "3\n");
}

// ── Type honesty: bytes do not interpolate ────────────────────────────────────

/// A binary frame should not silently become display text: f-string
/// interpolation of bytes stays rejected (pinned intentionally — use
/// .to_string() for an explicit conversion).
#[test]
fn fstring_interpolation_of_bytes_rejected() {
    compile_should_fail_with(r#"
fn main() {
    let b = bytes_new()
    b.push(65 as byte)
    print(f"{b}")
}
"#, "cannot interpolate bytes into string");
}

/// string <-> bytes conversions are total, unchecked, O(n) copies — exact for
/// all 256 byte values (strings carry no UTF-8 invariant).
#[test]
fn string_bytes_round_trip_all_256_values() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let buf = bytes_new()
    let mut i = 0
    while i < 256 {
        buf.push(i as byte)
        i = i + 1
    }
    let s = buf.to_string()
    let back = s.to_bytes()
    let mut ok = back.len() == 256
    let mut j = 0
    while j < back.len() {
        if (back[j] as int) != j {
            ok = false
        }
        j = j + 1
    }
    print(ok)
    return 0
}
"#);
    assert_eq!(out, "true\n");
}

// ── Bytes socket/net I/O (#368 slices 1) ─────────────────────────────────────

/// Compile with the repo stdlib and run, returning stdout.
fn run_with_stdlib(source: &str) -> String {
    use std::path::Path;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.pluto");
    std::fs::write(&path, source).unwrap();
    let stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib");
    let bin_path = dir.path().join("test_bin");
    pluto::compile_file_with_stdlib(&path, &bin_path, Some(&stdlib))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let out = std::process::Command::new(&bin_path).output().unwrap();
    assert!(
        out.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// All 256 byte values round-trip through a localhost TCP connection using the
/// bytes-typed read/write on std.net — no string laundering.
#[test]
fn net_bytes_round_trip_all_256_values() {
    let out = run_with_stdlib(r#"
import std.net

fn main() {
    let server = net.listen("127.0.0.1", 0)
    let port = server.port()

    let payload = bytes_new()
    let mut i = 0
    while i < 256 {
        payload.push(i as byte)
        i = i + 1
    }

    let client = net.connect("127.0.0.1", port)
    let wrote = client.write_bytes(payload)
    print(wrote)

    let conn = server.accept()
    let got = bytes_new()
    while got.len() < 256 {
        let chunk = conn.read_bytes(256 - got.len())
        if chunk.len() == 0 {
            break
        }
        let mut c = 0
        while c < chunk.len() {
            got.push(chunk[c])
            c = c + 1
        }
    }

    let mut ok = got.len() == 256
    let mut j = 0
    while j < got.len() {
        if (got[j] as int) != j {
            ok = false
        }
        j = j + 1
    }
    print(ok)

    conn.close()
    client.close()
    server.close()
}
"#);
    assert_eq!(out, "256\ntrue\n");
}

/// The low-level std.socket bytes variants: empty read on EOF, write returns
/// the byte count.
#[test]
fn socket_bytes_eof_yields_empty() {
    let out = run_with_stdlib(r#"
import std.net

fn main() {
    let server = net.listen("127.0.0.1", 0)
    let port = server.port()

    let client = net.connect("127.0.0.1", port)
    let data = bytes_new()
    data.push(0 as byte)
    data.push(255 as byte)
    print(client.write_bytes(data))
    client.close()

    let conn = server.accept()
    let first = conn.read_bytes(16)
    print(first.len())
    print(first[0] as int)
    print(first[1] as int)
    // Peer closed: next read yields empty bytes (EOF)
    let rest = conn.read_bytes(16)
    print(rest.len())
    conn.close()
    server.close()
}
"#);
    assert_eq!(out, "2\n2\n0\n255\n0\n");
}

// ── Bulk operations (#393): slice ────────────────────────────────────────────

#[test]
fn bytes_slice_basic() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "Hello, Pluto!".to_bytes()
    let s = b.slice(7, 12)
    print(s.to_string())
    print(s.len())
    return 0
}
"#);
    assert_eq!(out, "Pluto\n5\n");
}

#[test]
fn bytes_slice_is_a_fresh_copy() {
    // Mutating the slice must not touch the original (no Go-style views).
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "abcdef".to_bytes()
    let mut s = b.slice(0, 3)
    s[0] = 122 as byte
    print(b.to_string())
    print(s.to_string())
    return 0
}
"#);
    assert_eq!(out, "abcdef\nzbc\n");
}

#[test]
fn bytes_slice_empty_and_full() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "abc".to_bytes()
    let empty = b.slice(1, 1)
    print(empty.len())
    let full = b.slice(0, b.len())
    print(full.to_string())
    let empty_of_empty = bytes_new().slice(0, 0)
    print(empty_of_empty.len())
    return 0
}
"#);
    assert_eq!(out, "0\nabc\n0\n");
}

#[test]
fn bytes_slice_oob_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = "abc".to_bytes()
    let s = b.slice(1, 4)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes slice out of bounds: start 1, end 4, length 3"), "stderr: {stderr}");
}

#[test]
fn bytes_slice_reversed_bounds_abort() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = "abc".to_bytes()
    let s = b.slice(2, 1)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes slice out of bounds"), "stderr: {stderr}");
}

// ── Bulk operations (#393): extend ───────────────────────────────────────────

#[test]
fn bytes_extend_equals_push_loop() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let src = "0123456789".to_bytes()

    // Push loop (the workaround extend replaces)
    let via_push = "abc".to_bytes()
    let mut i = 0
    while i < src.len() {
        via_push.push(src[i])
        i = i + 1
    }

    // extend
    let via_extend = "abc".to_bytes()
    via_extend.extend(src)

    print(via_push.len())
    print(via_extend.len())
    print(via_push.to_string())
    print(via_extend.to_string())
    print(via_push.compare(via_extend))
    return 0
}
"#);
    assert_eq!(out, "13\n13\nabc0123456789\nabc0123456789\n0\n");
}

#[test]
fn bytes_extend_empty_and_self() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "ab".to_bytes()
    b.extend(bytes_new())
    print(b.to_string())
    b.extend(b)
    print(b.to_string())
    return 0
}
"#);
    assert_eq!(out, "ab\nabab\n");
}

#[test]
fn bytes_extend_grows_past_capacity() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_new()
    let chunk = "0123456789abcdef".to_bytes()
    let mut i = 0
    while i < 100 {
        b.extend(chunk)
        i = i + 1
    }
    print(b.len())
    print(b[1599] as int)
    return 0
}
"#);
    assert_eq!(out, "1600\n102\n"); // 'f' == 102
}

// ── Bulk operations (#393): fill + bytes_filled ──────────────────────────────

#[test]
fn bytes_filled_and_fill() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(4, 65 as byte)
    print(b.len())
    print(b.to_string())
    b.fill(66 as byte)
    print(b.to_string())
    let z = bytes_filled(0, 1 as byte)
    print(z.len())
    bytes_new().fill(0 as byte)
    return 0
}
"#);
    assert_eq!(out, "4\nAAAA\nBBBB\n0\n");
}

#[test]
fn bytes_filled_negative_length_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let n = 0 - 1
    let b = bytes_filled(n, 0 as byte)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes_filled length out of range: -1"), "stderr: {stderr}");
}

// ── Bulk operations (#393): copy_from ────────────────────────────────────────

#[test]
fn bytes_copy_from_distinct_buffers() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let dst = "XXXXXXXX".to_bytes()
    let src = "abcd".to_bytes()
    dst.copy_from(src, 1, 2, 3)
    print(dst.to_string())
    dst.copy_from(src, 0, 0, 0)
    print(dst.to_string())
    return 0
}
"#);
    assert_eq!(out, "XXbcdXXX\nXXbcdXXX\n");
}

#[test]
fn bytes_copy_from_overlap_forward_and_backward() {
    // memmove semantics: both overlap directions on the same buffer.
    let out = compile_and_run_stdout(r#"
fn main() int {
    // Forward overlap: copy [0, 6) to offset 2 (dst > src)
    let a = "abcdefgh".to_bytes()
    a.copy_from(a, 0, 2, 6)
    print(a.to_string())

    // Backward overlap: copy [2, 8) to offset 0 (dst < src)
    let b = "abcdefgh".to_bytes()
    b.copy_from(b, 2, 0, 6)
    print(b.to_string())
    return 0
}
"#);
    assert_eq!(out, "ababcdef\ncdefghgh\n");
}

#[test]
fn bytes_copy_from_oob_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let dst = "XXXX".to_bytes()
    let src = "ab".to_bytes()
    dst.copy_from(src, 0, 3, 2)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes copy_from out of bounds: src_off 0, dst_off 3, n 2, src length 2, dst length 4"), "stderr: {stderr}");
}

#[test]
fn bytes_copy_from_negative_n_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let dst = "XXXX".to_bytes()
    let n = 0 - 1
    dst.copy_from(dst, 0, 0, n)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes copy_from out of bounds"), "stderr: {stderr}");
}

// ── Bulk operations (#393): find ─────────────────────────────────────────────

#[test]
fn bytes_find_hit_miss_and_from_offset() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "Hello, Pluto!".to_bytes()
    print(b.find(111 as byte, 0))
    print(b.find(111 as byte, 5))
    print(b.find(111 as byte, 12))
    print(b.find(122 as byte, 0))
    print(b.find(72 as byte, 0))
    // from == len is the natural end of a scanning loop: -1, not a trap
    print(b.find(111 as byte, b.len()))
    print(bytes_new().find(0 as byte, 0))
    return 0
}
"#);
    assert_eq!(out, "4\n11\n-1\n-1\n0\n-1\n-1\n");
}

#[test]
fn bytes_find_scanning_loop() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = "a,b,,c,".to_bytes()
    let mut count = 0
    let mut at = b.find(44 as byte, 0)
    while at != -1 {
        count = count + 1
        at = b.find(44 as byte, at + 1)
    }
    print(count)
    return 0
}
"#);
    assert_eq!(out, "4\n");
}

#[test]
fn bytes_find_negative_from_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = "abc".to_bytes()
    let from = 0 - 1
    let i = b.find(97 as byte, from)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes find out of bounds: from -1"), "stderr: {stderr}");
}

// ── Bulk operations (#393): compare ──────────────────────────────────────────

#[test]
fn bytes_compare_orders_lexicographically() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    print("abc".to_bytes().compare("abd".to_bytes()))
    print("abd".to_bytes().compare("abc".to_bytes()))
    print("abc".to_bytes().compare("abc".to_bytes()))
    print("ab".to_bytes().compare("abc".to_bytes()))
    print("abc".to_bytes().compare("ab".to_bytes()))
    print(bytes_new().compare(bytes_new()))
    print(bytes_new().compare("a".to_bytes()))
    return 0
}
"#);
    assert_eq!(out, "-1\n1\n0\n-1\n1\n0\n-1\n");
}

#[test]
fn bytes_compare_unsigned_byte_order() {
    // 0xFF must order above 0x01 (unsigned, not signed char order).
    let out = compile_and_run_stdout(r#"
fn main() int {
    let hi = bytes_new()
    hi.push(255 as byte)
    let lo = bytes_new()
    lo.push(1 as byte)
    print(hi.compare(lo))
    print(lo.compare(hi))
    return 0
}
"#);
    assert_eq!(out, "1\n-1\n");
}

// ── Fixed-width codecs (#393): roundtrips ────────────────────────────────────

#[test]
fn bytes_codec_u8_roundtrip_boundaries() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(1, 0 as byte)
    b.write_u8(0, 0)
    print(b.read_u8(0))
    b.write_u8(0, 255)
    print(b.read_u8(0))
    b.write_u8(0, 128)
    print(b.read_u8(0))
    return 0
}
"#);
    assert_eq!(out, "0\n255\n128\n");
}

#[test]
fn bytes_codec_u16_roundtrip_both_endiannesses() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(2, 0 as byte)
    b.write_u16_le(0, 0)
    print(b.read_u16_le(0))
    b.write_u16_le(0, 65535)
    print(b.read_u16_le(0))
    b.write_u16_le(0, 4660)
    print(b.read_u16_le(0))
    print(b[0] as int)
    print(b[1] as int)
    b.write_u16_be(0, 4660)
    print(b.read_u16_be(0))
    print(b[0] as int)
    print(b[1] as int)
    b.write_u16_be(0, 65535)
    print(b.read_u16_be(0))
    return 0
}
"#);
    // 4660 == 0x1234: LE lays down 0x34 0x12, BE lays down 0x12 0x34
    assert_eq!(out, "0\n65535\n4660\n52\n18\n4660\n18\n52\n65535\n");
}

#[test]
fn bytes_codec_u32_roundtrip_both_endiannesses() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(4, 0 as byte)
    b.write_u32_le(0, 0)
    print(b.read_u32_le(0))
    b.write_u32_le(0, 4294967295)
    print(b.read_u32_le(0))
    b.write_u32_be(0, 4294967295)
    print(b.read_u32_be(0))
    b.write_u32_le(0, 305419896)
    print(b.read_u32_le(0))
    print(b[0] as int)
    b.write_u32_be(0, 305419896)
    print(b.read_u32_be(0))
    print(b[0] as int)
    b.write_u32_be(0, 65535)
    print(b.read_u32_be(0))
    b.write_u32_le(0, 255)
    print(b.read_u32_le(0))
    return 0
}
"#);
    // 305419896 == 0x12345678: LE first byte 0x78 (120), BE first byte 0x12 (18)
    assert_eq!(out, "0\n4294967295\n4294967295\n305419896\n120\n305419896\n18\n65535\n255\n");
}

#[test]
fn bytes_codec_i64_roundtrip_both_endiannesses() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(8, 0 as byte)
    let lo = -9223372036854775807 - 1
    let hi = 9223372036854775807

    b.write_i64_le(0, lo)
    print(b.read_i64_le(0))
    b.write_i64_be(0, lo)
    print(b.read_i64_be(0))

    b.write_i64_le(0, hi)
    print(b.read_i64_le(0))
    b.write_i64_be(0, hi)
    print(b.read_i64_be(0))

    b.write_i64_le(0, -1)
    print(b.read_i64_le(0))
    b.write_i64_be(0, -1)
    print(b.read_i64_be(0))

    b.write_i64_le(0, 0)
    print(b.read_i64_le(0))
    b.write_i64_be(0, -42)
    print(b.read_i64_be(0))
    return 0
}
"#);
    assert_eq!(
        out,
        "-9223372036854775808\n-9223372036854775808\n9223372036854775807\n9223372036854775807\n-1\n-1\n0\n-42\n"
    );
}

#[test]
fn bytes_codec_endianness_cross_check() {
    // Writing LE and reading BE must byte-swap, proving the lane order.
    let out = compile_and_run_stdout(r#"
fn main() int {
    let b = bytes_filled(8, 0 as byte)
    b.write_u16_le(0, 1)
    print(b.read_u16_be(0))
    b.write_u32_le(0, 1)
    print(b.read_u32_be(0))
    b.write_i64_le(0, 1)
    print(b.read_i64_be(0))
    return 0
}
"#);
    assert_eq!(out, "256\n16777216\n72057594037927936\n");
}

#[test]
fn bytes_codec_at_nonzero_offsets() {
    let out = compile_and_run_stdout(r#"
fn main() int {
    // A little frame: u8 tag, u16_be length, u32_be value, i64_le payload
    let b = bytes_filled(15, 0 as byte)
    b.write_u8(0, 7)
    b.write_u16_be(1, 513)
    b.write_u32_be(3, 70000)
    b.write_i64_le(7, -5)
    print(b.read_u8(0))
    print(b.read_u16_be(1))
    print(b.read_u32_be(3))
    print(b.read_i64_le(7))
    return 0
}
"#);
    assert_eq!(out, "7\n513\n70000\n-5\n");
}

// ── Fixed-width codecs (#393): OOB offsets abort ─────────────────────────────

#[test]
fn bytes_codec_read_oob_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(4, 0 as byte)
    let v = b.read_u32_le(1)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes read_u32_le out of bounds: offset 1, length 4"), "stderr: {stderr}");
}

#[test]
fn bytes_codec_read_i64_from_short_buffer_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(7, 0 as byte)
    let v = b.read_i64_be(0)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes read_i64_be out of bounds: offset 0, length 7"), "stderr: {stderr}");
}

#[test]
fn bytes_codec_write_oob_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(2, 0 as byte)
    b.write_u16_be(1, 1)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes write_u16_be out of bounds: offset 1, length 2"), "stderr: {stderr}");
}

#[test]
fn bytes_codec_write_negative_offset_aborts() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(4, 0 as byte)
    let off = 0 - 1
    b.write_u8(off, 1)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("bytes write_u8 out of bounds: offset -1, length 4"), "stderr: {stderr}");
}

// ── Fixed-width codecs (#393): out-of-range write values are defects ─────────

#[test]
fn bytes_write_u8_out_of_range_traps() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(1, 0 as byte)
    b.write_u8(0, 256)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("pluto: defect: bytes write_u8 value 256 out of range 0..255"), "stderr: {stderr}");
}

#[test]
fn bytes_write_u16_negative_value_traps() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(2, 0 as byte)
    let v = 0 - 1
    b.write_u16_le(0, v)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("pluto: defect: bytes write_u16_le value -1 out of range 0..65535"), "stderr: {stderr}");
}

#[test]
fn bytes_write_u32_out_of_range_traps() {
    let (_stdout, stderr, code) = compile_and_run_output(r#"
fn main() int {
    let b = bytes_filled(4, 0 as byte)
    b.write_u32_be(0, 4294967296)
    return 0
}
"#);
    assert_ne!(code, 0);
    assert!(stderr.contains("pluto: defect: bytes write_u32_be value 4294967296 out of range 0..4294967295"), "stderr: {stderr}");
}

// ── No read_u64 by design ─────────────────────────────────────────────────────

#[test]
fn bytes_read_u64_rejected_with_guidance() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_filled(8, 0 as byte)
    let v = b.read_u64_le(0)
    return 0
}
"#, "use read_i64_le instead");
}

// ── Compile errors for the new surface ───────────────────────────────────────

#[test]
fn bytes_slice_wrong_arg_type() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_new()
    let s = b.slice(0 as byte, 1)
    return 0
}
"#, "expected int, found byte");
}

#[test]
fn bytes_extend_wrong_arg_type() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_new()
    b.extend("abc")
    return 0
}
"#, "expected bytes, found string");
}

#[test]
fn bytes_fill_wrong_arg_type() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_new()
    b.fill(65)
    return 0
}
"#, "expected byte, found int");
}

#[test]
fn bytes_find_wrong_needle_type() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_new()
    let i = b.find(65, 0)
    return 0
}
"#, "expected byte, found int");
}

#[test]
fn bytes_filled_wrong_value_type() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_filled(4, 65)
    return 0
}
"#, "expected byte, found int");
}

#[test]
fn bytes_write_wrong_arity() {
    compile_should_fail_with(r#"
fn main() int {
    let b = bytes_filled(4, 0 as byte)
    b.write_u32_le(0)
    return 0
}
"#, "write_u32_le() expects 2 arguments");
}

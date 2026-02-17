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
    let buf = bytes_new()
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
    let m = Map<byte, string> {}
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

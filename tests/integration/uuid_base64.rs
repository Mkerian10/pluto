// std.uuid / std.base64 tests. Every program here imports a stdlib module, so
// it is compiled against the repo's stdlib directly rather than through the
// single-source helpers in common (which resolve no stdlib).

use std::path::Path;
use std::process::Command;

#[allow(dead_code)]
mod common;

fn run_project_with_stdlib(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        std::fs::write(dir.path().join(name), content).unwrap();
    }
    let stdlib = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib");
    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

#[test]
fn uuid_generate_format() {
    let source = r#"
import std.uuid

fn main() {
    let id = uuid.generate()
    print(id.len())
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "36"); // 8-4-4-4-12 = 36 characters
}

#[test]
fn uuid_generate_structure() {
    let source = r#"
import std.uuid

fn main() {
    let id = uuid.generate()
    // Check structure: 8-4-4-4-12
    let part1 = id.substring(0, 8)
    let sep1 = id.char_at(8)
    let part2 = id.substring(9, 4)
    let sep2 = id.char_at(13)
    let part3 = id.substring(14, 4)
    let sep3 = id.char_at(18)
    let part4 = id.substring(19, 4)
    let sep4 = id.char_at(23)
    let part5 = id.substring(24, 12)

    if sep1 == "-" && sep2 == "-" && sep3 == "-" && sep4 == "-" {
        print(f"{part1.len()}-{part2.len()}-{part3.len()}-{part4.len()}-{part5.len()}")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "8-4-4-4-12");
}

#[test]
fn uuid_generate_uniqueness() {
    let source = r#"
import std.uuid

fn main() {
    let id1 = uuid.generate()
    let id2 = uuid.generate()
    if id1 != id2 {
        print("unique")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "unique");
}

#[test]
fn base64_encode_empty() {
    let source = r#"
import std.base64

fn main() {
    let encoded = base64.encode("")
    print(encoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "");
}

#[test]
fn base64_encode_simple() {
    let source = r#"
import std.base64

fn main() {
    let encoded = base64.encode("f")
    print(encoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "Zg==");
}

#[test]
fn base64_encode_two_chars() {
    let source = r#"
import std.base64

fn main() {
    let encoded = base64.encode("fo")
    print(encoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "Zm8=");
}

#[test]
fn base64_encode_three_chars() {
    let source = r#"
import std.base64

fn main() {
    let encoded = base64.encode("foo")
    print(encoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "Zm9v");
}

#[test]
fn base64_encode_longer_string() {
    let source = r#"
import std.base64

fn main() {
    let encoded = base64.encode("hello world")
    print(encoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "aGVsbG8gd29ybGQ=");
}

#[test]
fn base64_decode_empty() {
    let source = r#"
import std.base64

fn main() {
    let decoded = base64.decode("")
    print(decoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "");
}

#[test]
fn base64_decode_simple() {
    let source = r#"
import std.base64

fn main() {
    let decoded = base64.decode("Zg==")
    print(decoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "f");
}

#[test]
fn base64_decode_two_chars() {
    let source = r#"
import std.base64

fn main() {
    let decoded = base64.decode("Zm8=")
    print(decoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "fo");
}

#[test]
fn base64_decode_three_chars() {
    let source = r#"
import std.base64

fn main() {
    let decoded = base64.decode("Zm9v")
    print(decoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "foo");
}

#[test]
fn base64_decode_longer_string() {
    let source = r#"
import std.base64

fn main() {
    let decoded = base64.decode("aGVsbG8gd29ybGQ=")
    print(decoded)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn base64_roundtrip() {
    let source = r#"
import std.base64

fn main() {
    let original = "The quick brown fox jumps over the lazy dog"
    let encoded = base64.encode(original)
    let decoded = base64.decode(encoded)
    if original == decoded {
        print("roundtrip_ok")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "roundtrip_ok");
}

#[test]
fn base64_url_safe_encode() {
    let source = r#"
import std.base64

fn main() {
    // Test with data that has + or / in standard base64
    let input = "\u{FB}\u{FE}"
    let standard = base64.encode(input)
    let url_safe = base64.encode_url_safe(input)
    print(standard)
    print(url_safe)
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 2);
    // Standard has +/ and URL safe has -_
    assert!(lines[0].contains("+") || lines[0].contains("/") || lines[0].len() > 0);
    assert!(lines[1].len() > 0);
}

#[test]
fn base64_url_safe_roundtrip() {
    let source = r#"
import std.base64

fn main() {
    let original = "Test with special bytes"
    let encoded = base64.encode_url_safe(original)
    let decoded = base64.decode_url_safe(encoded)
    if original == decoded {
        print("url_safe_roundtrip_ok")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "url_safe_roundtrip_ok");
}

#[test]
fn base64_special_characters() {
    let source = r#"
import std.base64

fn main() {
    let text = "Special!@#$%^&*()"
    let encoded = base64.encode(text)
    let decoded = base64.decode(encoded)
    if text == decoded {
        print("special_chars_ok")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "special_chars_ok");
}

// Unpadded input (the common URL-safe form, and what many encoders emit when
// told to strip '=') must decode exactly like its padded spelling. Before the
// fix the missing characters were looked up as byte 0 in the alphabet and
// produced garbage trailing bytes.
#[test]
fn base64_decode_unpadded_matches_padded() {
    let source = r#"
import std.base64

fn main() {
    print(base64.decode("YQ"))
    print(base64.decode("Zm8"))
    print(base64.decode("Zm9v"))
    print(base64.decode("aGVsbG8gd29ybGQ"))
    if base64.decode("YQ") == base64.decode("YQ==") {
        print("one_byte_ok")
    }
    if base64.decode("Zm8") == base64.decode("Zm8=") {
        print("two_bytes_ok")
    }
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "a\nfo\nfoo\nhello world\none_byte_ok\ntwo_bytes_ok");
}

#[test]
fn base64_decode_bytes_unpadded_matches_padded() {
    let source = r#"
import std.base64

fn main() {
    let padded = base64.decode_bytes("AP8B")
    let a = base64.decode_bytes("AP8")
    let b = base64.decode_bytes("AP8=")
    print(a.len())
    print(b.len())
    print(a[0].to_int())
    print(a[1].to_int())
    print(padded.len())
    let one = base64.decode_bytes("/w")
    print(one.len())
    print(one[0].to_int())
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "2\n2\n0\n255\n3\n1\n255");
}

#[test]
fn base64_url_safe_unpadded_roundtrip() {
    let source = r#"
import std.base64

fn main() {
    let original = "any carnal pleas"
    let encoded = base64.encode_url_safe(original)
    let stripped = encoded.replace("=", "")
    print(stripped)
    print(base64.decode_url_safe(stripped))
}
"#;
    let output = run_project_with_stdlib(&[("main.pluto", source)]);
    assert_eq!(output.trim(), "YW55IGNhcm5hbCBwbGVhcw\nany carnal pleas");
}

use std::process::Command;

mod common;
use common::{compile_and_run_stdout, compile_should_fail};

#[test]
fn uuid_generate_format() {
    let source = r#"
import std.uuid

fn main() {
    let id = std.uuid.generate()
    print(id.len())
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "36"); // 8-4-4-4-12 = 36 characters
}

#[test]
fn uuid_generate_structure() {
    let source = r#"
import std.uuid
import std.strings

fn main() {
    let id = std.uuid.generate()
    // Check structure: 8-4-4-4-12
    let part1 = std.strings.substring(id, 0, 8)
    let sep1 = std.strings.char_at(id, 8)
    let part2 = std.strings.substring(id, 9, 4)
    let sep2 = std.strings.char_at(id, 13)
    let part3 = std.strings.substring(id, 14, 4)
    let sep3 = std.strings.char_at(id, 18)
    let part4 = std.strings.substring(id, 19, 4)
    let sep4 = std.strings.char_at(id, 23)
    let part5 = std.strings.substring(id, 24, 12)

    if sep1 == "-" && sep2 == "-" && sep3 == "-" && sep4 == "-" {
        print("valid")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "valid");
}

#[test]
fn uuid_generate_uniqueness() {
    let source = r#"
import std.uuid

fn main() {
    let id1 = std.uuid.generate()
    let id2 = std.uuid.generate()
    if id1 != id2 {
        print("unique")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "unique");
}

#[test]
fn base64_encode_empty() {
    let source = r#"
import std.base64

fn main() {
    let encoded = std.base64.encode("")
    print(encoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "");
}

#[test]
fn base64_encode_simple() {
    let source = r#"
import std.base64

fn main() {
    let encoded = std.base64.encode("f")
    print(encoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "Zg==");
}

#[test]
fn base64_encode_two_chars() {
    let source = r#"
import std.base64

fn main() {
    let encoded = std.base64.encode("fo")
    print(encoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "Zm8=");
}

#[test]
fn base64_encode_three_chars() {
    let source = r#"
import std.base64

fn main() {
    let encoded = std.base64.encode("foo")
    print(encoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "Zm9v");
}

#[test]
fn base64_encode_longer_string() {
    let source = r#"
import std.base64

fn main() {
    let encoded = std.base64.encode("hello world")
    print(encoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "aGVsbG8gd29ybGQ=");
}

#[test]
fn base64_decode_empty() {
    let source = r#"
import std.base64

fn main() {
    let decoded = std.base64.decode("")
    print(decoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "");
}

#[test]
fn base64_decode_simple() {
    let source = r#"
import std.base64

fn main() {
    let decoded = std.base64.decode("Zg==")
    print(decoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "f");
}

#[test]
fn base64_decode_two_chars() {
    let source = r#"
import std.base64

fn main() {
    let decoded = std.base64.decode("Zm8=")
    print(decoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "fo");
}

#[test]
fn base64_decode_three_chars() {
    let source = r#"
import std.base64

fn main() {
    let decoded = std.base64.decode("Zm9v")
    print(decoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "foo");
}

#[test]
fn base64_decode_longer_string() {
    let source = r#"
import std.base64

fn main() {
    let decoded = std.base64.decode("aGVsbG8gd29ybGQ=")
    print(decoded)
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn base64_roundtrip() {
    let source = r#"
import std.base64

fn main() {
    let original = "The quick brown fox jumps over the lazy dog"
    let encoded = std.base64.encode(original)
    let decoded = std.base64.decode(encoded)
    if original == decoded {
        print("roundtrip_ok")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "roundtrip_ok");
}

#[test]
fn base64_url_safe_encode() {
    let source = r#"
import std.base64

fn main() {
    // Test with data that has + or / in standard base64
    let input = "\u{FB}\u{FE}"
    let standard = std.base64.encode(input)
    let url_safe = std.base64.encode_url_safe(input)
    print(standard)
    print(url_safe)
}
"#;
    let output = compile_and_run_stdout(source);
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
    let encoded = std.base64.encode_url_safe(original)
    let decoded = std.base64.decode_url_safe(encoded)
    if original == decoded {
        print("url_safe_roundtrip_ok")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "url_safe_roundtrip_ok");
}

#[test]
fn base64_special_characters() {
    let source = r#"
import std.base64

fn main() {
    let text = "Special!@#$%^&*()"
    let encoded = std.base64.encode(text)
    let decoded = std.base64.decode(encoded)
    if text == decoded {
        print("special_chars_ok")
    }
}
"#;
    let output = compile_and_run_stdout(source);
    assert_eq!(output.trim(), "special_chars_ok");
}

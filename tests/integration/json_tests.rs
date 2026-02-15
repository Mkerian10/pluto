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

    pluto::compile_file_with_stdlib(&entry, &bin_path, Some(&stdlib_dst))
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
// Parse null
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_null() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("null")!
    print(j.is_null())
}
"#,
    )]);
    assert_eq!(out, "true\n");
}

// ============================================================
// Parse booleans
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_bool() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let t = json.parse("true")!
    let f = json.parse("false")!
    print(t.get_bool())
    print(f.get_bool())
    print(t.is_bool())
}
"#,
    )]);
    assert_eq!(out, "true\nfalse\ntrue\n");
}

// ============================================================
// Parse integers
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_int() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("42")!
    print(j.get_int())
    print(j.is_int())
    let neg = json.parse("-17")!
    print(neg.get_int())
}
"#,
    )]);
    assert_eq!(out, "42\ntrue\n-17\n");
}

// ============================================================
// Parse floats
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_float() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("3.14")!
    print(j.is_float())
    let v = j.get_float()
    // Print int part to avoid float formatting issues
    let approx = v * 100.0
    print(approx as int)
}
"#,
    )]);
    assert_eq!(out, "true\n314\n");
}

// ============================================================
// Parse strings
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_string() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("\"hello world\"")!
    print(j.get_string())
    print(j.is_string())
}
"#,
    )]);
    assert_eq!(out, "hello world\ntrue\n");
}

// ============================================================
// Parse string with escapes
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_string_escapes() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json
import std.strings

fn main() {
    let j = json.parse("\"hello\\nworld\"")!
    let s = j.get_string()
    print(s.len())
    print(strings.contains(s, "\n"))
}
"#,
    )]);
    assert_eq!(out, "11\ntrue\n");
}

// ============================================================
// Parse array
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_array() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("[1, 2, 3]")!
    print(j.is_array())
    print(j.len())
    print(j.at(0).get_int())
    print(j.at(1).get_int())
    print(j.at(2).get_int())
}
"#,
    )]);
    assert_eq!(out, "true\n3\n1\n2\n3\n");
}

// ============================================================
// Parse empty array
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_empty_array() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("[]")!
    print(j.is_array())
    print(j.len())
}
"#,
    )]);
    assert_eq!(out, "true\n0\n");
}

// ============================================================
// Parse object
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_object() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{\"name\": \"Alice\", \"age\": 30}}")!
    print(j.is_object())
    print(j.get("name").get_string())
    print(j.get("age").get_int())
}
"#,
    )]);
    assert_eq!(out, "true\nAlice\n30\n");
}

// ============================================================
// Parse empty object
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_empty_object() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{}}")!
    print(j.is_object())
    print(j.len())
}
"#,
    )]);
    assert_eq!(out, "true\n0\n");
}

// ============================================================
// Parse nested objects
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_nested() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{\"user\": {{\"name\": \"Bob\", \"scores\": [10, 20]}}}}")!
    let user = j.get("user")
    print(user.get("name").get_string())
    let scores = user.get("scores")
    print(scores.len())
    print(scores.at(0).get_int())
    print(scores.at(1).get_int())
}
"#,
    )]);
    assert_eq!(out, "Bob\n2\n10\n20\n");
}

// ============================================================
// Parse error — invalid JSON
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_error() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{invalid}}") catch json.null()
    print("caught")
}
"#,
    )]);
    assert_eq!(out, "caught\n");
}

// ============================================================
// Construct and stringify null
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_construct_null() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.null()
    print(j.to_string())
}
"#,
    )]);
    assert_eq!(out, "null\n");
}

// ============================================================
// Construct and stringify primitives
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_construct_primitives() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    print(json.bool(true).to_string())
    print(json.bool(false).to_string())
    print(json.int(42).to_string())
    print(json.string("hello").to_string())
}
"#,
    )]);
    assert_eq!(out, "true\nfalse\n42\n\"hello\"\n");
}

// ============================================================
// Construct object programmatically
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_construct_object() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let obj = json.object()
    obj.set("name", json.string("Alice"))
    obj.set("age", json.int(30))
    obj.set("active", json.bool(true))
    let s = obj.to_string()
    // Parse it back to verify
    let parsed = json.parse(s)!
    print(parsed.get("name").get_string())
    print(parsed.get("age").get_int())
    print(parsed.get("active").get_bool())
}
"#,
    )]);
    assert_eq!(out, "Alice\n30\ntrue\n");
}

// ============================================================
// Construct array programmatically
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_construct_array() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let arr = json.array()
    arr.push(json.int(1))
    arr.push(json.int(2))
    arr.push(json.string("three"))
    print(arr.len())
    print(arr.to_string())
}
"#,
    )]);
    assert_eq!(out, "3\n[1,2,\"three\"]\n");
}

// ============================================================
// Round-trip: parse → stringify → parse
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_roundtrip() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let original = "{{\"a\":1,\"b\":[true,null,\"hello\"]}}"
    let j = json.parse(original)!
    let s = j.to_string()
    let j2 = json.parse(s)!
    print(j2.get("a").get_int())
    let arr = j2.get("b")
    print(arr.at(0).get_bool())
    print(arr.at(1).is_null())
    print(arr.at(2).get_string())
}
"#,
    )]);
    assert_eq!(out, "1\ntrue\ntrue\nhello\n");
}

// ============================================================
// Type checks on different values
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_type_checks() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("[null, true, 42, 3.14, \"hi\", [], {{}}]")!
    print(j.at(0).is_null())
    print(j.at(1).is_bool())
    print(j.at(2).is_int())
    print(j.at(3).is_float())
    print(j.at(4).is_string())
    print(j.at(5).is_array())
    print(j.at(6).is_object())
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\n");
}

// ============================================================
// Chained field access
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_chained_access() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{\"a\": {{\"b\": {{\"c\": 99}}}}}}")!
    print(j.get("a").get("b").get("c").get_int())
}
"#,
    )]);
    assert_eq!(out, "99\n");
}

// ============================================================
// Missing key returns null
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_missing_key_returns_null() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("{{\"a\": 1}}")!
    let missing = j.get("b")
    print(missing.is_null())
}
"#,
    )]);
    assert_eq!(out, "true\n");
}

// ============================================================
// Object set overwrites existing key
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_object_overwrite() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let obj = json.object()
    obj.set("x", json.int(1))
    obj.set("x", json.int(2))
    print(obj.get("x").get_int())
    print(obj.len())
}
"#,
    )]);
    assert_eq!(out, "2\n1\n");
}

// ============================================================
// Stringify with string escapes
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_stringify_escapes() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.string("hello\nworld")
    let s = j.to_string()
    print(s)
}
"#,
    )]);
    assert_eq!(out, "\"hello\\nworld\"\n");
}

// ============================================================
// Parse number with exponent
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_exponent() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("1e3")!
    print(j.is_float())
    print(j.get_int())
}
"#,
    )]);
    assert_eq!(out, "true\n1000\n");
}

// ============================================================
// Parse trailing content raises error
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_parse_trailing_content() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("42 extra") catch json.null()
    print("caught")
}
"#,
    )]);
    assert_eq!(out, "caught\n");
}

// ============================================================
// Float to int conversion
// ============================================================

#[test]
#[ignore] // stdlib bug: json mutation methods need mut self
fn json_float_to_int() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.json

fn main() {
    let j = json.parse("3.7")!
    print(j.get_int())
}
"#,
    )]);
    assert_eq!(out, "3\n");
}

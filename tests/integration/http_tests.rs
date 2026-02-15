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
// Response serialization — ok
// ============================================================

#[test]
fn http_response_ok() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.strings

fn main() {
    let resp = http.ok("hello")
    let s = resp.to_string()
    print(strings.contains(s, "200 OK"))
    print(strings.contains(s, "Content-Length: 5"))
    print(strings.contains(s, "hello"))
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\ntrue\n");
}

// ============================================================
// Response serialization — not_found
// ============================================================

#[test]
fn http_response_not_found() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.strings

fn main() {
    let resp = http.not_found()
    let s = resp.to_string()
    print(strings.contains(s, "404 Not Found"))
    print(strings.contains(s, "Not Found"))
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\n");
}

// ============================================================
// Response serialization — bad_request
// ============================================================

#[test]
fn http_response_bad_request() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.strings

fn main() {
    let resp = http.bad_request()
    let s = resp.to_string()
    print(strings.contains(s, "400 Bad Request"))
}
"#,
    )]);
    assert_eq!(out, "true\n");
}

// ============================================================
// Response serialization — ok_json
// ============================================================

#[test]
#[ignore] // stdlib bug: json.object().set() needs mut self declaration
fn http_response_ok_json() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.json
import std.strings

fn main() {
    let obj = json.object()
    obj.set("status", json.string("ok"))
    let resp = http.ok_json(obj.to_string())
    let s = resp.to_string()
    print(strings.contains(s, "application/json"))
    print(strings.contains(s, "status"))
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\n");
}

// ============================================================
// Response serialization — custom status
// ============================================================

#[test]
fn http_response_custom() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.strings

fn main() {
    let resp = http.response(201, "Created", "resource created")
    let s = resp.to_string()
    print(strings.contains(s, "201 Created"))
    print(strings.contains(s, "resource created"))
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\n");
}

// ============================================================
// Request parsing
// ============================================================

#[test]
fn http_request_parse() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.net
import std.strings

fn main() {
    let server = http.listen("127.0.0.1", 0)!
    let port = server.port()

    let client = net.connect("127.0.0.1", port)
    client.write("GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n")

    let conn = server.accept()!
    let req = conn.read_request()!
    print(req.method)
    print(req.path)
    conn.send_response(http.ok("done"))
    conn.close()

    let resp = client.read(4096)
    print(strings.contains(resp, "200 OK"))
    client.close()
    server.close()
}
"#,
    )]);
    assert_eq!(out, "GET\n/hello\ntrue\n");
}

// ============================================================
// Request header lookup
// ============================================================

#[test]
fn http_request_header() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.net

fn main() {
    let server = http.listen("127.0.0.1", 0)!
    let port = server.port()

    let client = net.connect("127.0.0.1", port)
    client.write("GET / HTTP/1.1\r\nHost: example.com\r\nX-Custom: test-value\r\n\r\n")

    let conn = server.accept()!
    let req = conn.read_request()!
    print(req.header("Host"))
    print(req.header("X-Custom"))
    print(req.header("Missing"))
    conn.send_response(http.ok("ok"))
    conn.close()
    client.close()
    server.close()
}
"#,
    )]);
    assert_eq!(out, "example.com\ntest-value\n\n");
}

// ============================================================
// Request with body (POST)
// ============================================================

#[test]
fn http_request_with_body() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.net

fn main() {
    let server = http.listen("127.0.0.1", 0)!
    let port = server.port()

    let body = "{{\"key\":\"value\"}}"
    let blen = body.len()
    let req_str = "POST /api HTTP/1.1\r\nContent-Length: {blen}\r\n\r\n" + body

    let client = net.connect("127.0.0.1", port)
    client.write(req_str)

    let conn = server.accept()!
    let req = conn.read_request()!
    print(req.method)
    print(req.path)
    print(req.body)
    conn.send_response(http.ok("received"))
    conn.close()
    client.close()
    server.close()
}
"#,
    )]);
    assert_eq!(out, "POST\n/api\n{\"key\":\"value\"}\n");
}

// ============================================================
// JSON API round-trip
// ============================================================

#[test]
#[ignore] // stdlib bug: json.object().set() needs mut self declaration
fn http_json_api_roundtrip() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.json
import std.net
import std.strings

fn main() {
    let server = http.listen("127.0.0.1", 0)!
    let port = server.port()

    let body = "{{\"name\":\"Alice\"}}"
    let blen = body.len()
    let req_str = "POST /greet HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {blen}\r\n\r\n" + body

    let client = net.connect("127.0.0.1", port)
    client.write(req_str)

    let conn = server.accept()!
    let req = conn.read_request()!

    let input = json.parse(req.body)!
    let name = input.get("name").get_string()

    let resp_json = json.object()
    resp_json.set("greeting", json.string("Hello, " + name + "!"))
    conn.send_response(http.ok_json(resp_json.to_string()))
    conn.close()

    let resp = client.read(4096)
    print(strings.contains(resp, "Hello, Alice!"))
    print(strings.contains(resp, "application/json"))
    client.close()
    server.close()
}
"#,
    )]);
    assert_eq!(out, "true\ntrue\n");
}

// ============================================================
// URL decode
// ============================================================

#[test]
fn http_url_decode() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http

extern fn __pluto_http_url_decode(s: string) string

fn main() {
    let decoded = __pluto_http_url_decode("hello%20world")
    print(decoded)
    let decoded2 = __pluto_http_url_decode("a+b%3Dc")
    print(decoded2)
}
"#,
    )]);
    assert_eq!(out, "hello world\na b=c\n");
}

// ============================================================
// Multiple requests on same server
// ============================================================

#[test]
fn http_multiple_requests() {
    let out = run_project_with_stdlib(&[(
        "main.pluto",
        r#"import std.http
import std.net

fn main() {
    let server = http.listen("127.0.0.1", 0)!
    let port = server.port()

    // Request 1
    let c1 = net.connect("127.0.0.1", port)
    c1.write("GET /first HTTP/1.1\r\n\r\n")
    let conn1 = server.accept()!
    let req1 = conn1.read_request()!
    print(req1.path)
    conn1.send_response(http.ok("one"))
    conn1.close()
    c1.close()

    // Request 2
    let c2 = net.connect("127.0.0.1", port)
    c2.write("GET /second HTTP/1.1\r\n\r\n")
    let conn2 = server.accept()!
    let req2 = conn2.read_request()!
    print(req2.path)
    conn2.send_response(http.ok("two"))
    conn2.close()
    c2.close()

    server.close()
}
"#,
    )]);
    assert_eq!(out, "/first\n/second\n");
}

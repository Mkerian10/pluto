// Whole-program distributed safety. A `remote` dependency lets one service hold
// a typed reference to another service's interface. The call is type-checked
// across the boundary against the real signature, and crossing the boundary
// implicitly adds NetworkError to the caller's inferred error set.
//
// Phase 1 tests pin the compile-time guarantees. Phase 2 tests (further down)
// exercise real transport: a remote call marshals its args, connects to the
// address in env PLUTO_REMOTE_<SERVICE>, and parses the response — raising
// NetworkError on any transport failure.

mod common;

use std::process::Command;

/// Write multiple files to a temp dir, compile `main.pluto`, run it, return stdout.
fn run_project(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    pluto::compile_file(&entry, &bin_path)
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(run_output.status.success(), "Binary exited with non-zero status");
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

/// Compile `main.pluto`; assert it fails with an error containing `expected_msg`.
fn compile_project_should_fail_with(files: &[(&str, &str)], expected_msg: &str) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let entry = dir.path().join("main.pluto");
    let bin_path = dir.path().join("test_bin");
    match pluto::compile_file(&entry, &bin_path) {
        Ok(_) => panic!("Compilation should have failed (expected: {expected_msg})"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains(expected_msg),
                "error did not contain '{expected_msg}'.\nActual: {msg}"
            );
        }
    }
}

// Service B's interface, in its own module so service A can reference it.
const BILLING: &str = "\
pub class BillingService {
    fn charge(self, amount: int) int {
        return amount * 2
    }
}";

/// The whole-program type checker validates a remote call against the target
/// service's real signature — a wrong argument type is rejected at compile time.
#[test]
fn cross_service_signature_mismatch_fails() {
    compile_project_should_fail_with(
        &[
            ("billing.pluto", BILLING),
            ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(\"not an int\") catch 0
        print(f\"result: {x}\")
    }
}"),
        ],
        "expected int",
    );
}

/// Crossing a service boundary adds NetworkError to the caller's error set, so a
/// bare remote call (no `!`/`catch`) is rejected — the boundary failure mode must
/// be handled.
#[test]
fn bare_remote_call_requires_error_handling() {
    compile_project_should_fail_with(
        &[
            ("billing.pluto", BILLING),
            ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(10)
        print(f\"result: {x}\")
    }
}"),
        ],
        "must be handled with ! or catch",
    );
}

/// A handled remote call compiles and runs. With no transport configured, the
/// call raises NetworkError, the `catch` supplies a fallback, and the program
/// completes — end-to-end proof that the boundary call is wired through the
/// error system.
#[test]
fn handled_remote_call_runs_and_falls_back() {
    let out = run_project(&[
        ("billing.pluto", BILLING),
        ("main.pluto", "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(10) catch -1
        print(f\"result: {x}\")
    }
}"),
    ]);
    assert_eq!(out, "result: -1\n");
}

// ── Phase 2: real transport over a socket ───────────────────────────────────────

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn manifest_stdlib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib")
}

/// Compile a project (entry `main.pluto`) to a binary, resolving the repo stdlib.
/// Returns the TempDir (kept alive by the caller) and the binary path.
fn build_binary(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }
    let bin = dir.path().join("bin");
    pluto::compile_file_with_stdlib(&dir.path().join("main.pluto"), &bin, Some(&manifest_stdlib()))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));
    (dir, bin)
}

// Service B's interface, as imported by the client (a plain `pub class` — stages
// can't yet be `pub`/cross-module, see Phase 1 notes).
const BILLING_IFACE: &str = "\
pub class BillingService {
    fn charge(self, amount: int) int {
        return amount
    }
}";

// The client app: the remote call looks exactly like a local method call.
const CLIENT_SRC: &str = "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(21) catch -1
        print(f\"result:{x}\")
    }
}";

// (The round-trip over a real socket is covered by `serve_generated_server_round_trips`
// below, which uses the generated server — the supported, framing-compatible path.)

/// When the service is unreachable, the boundary call raises NetworkError, which
/// the caller handles via `catch` — yielding the fallback -1.
#[test]
fn remote_call_raises_networkerror_when_unreachable() {
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLIENT_SRC)]);

    // Port 1 has nothing listening: connect fails -> NetworkError -> catch -1.
    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", "127.0.0.1:1")
        .output()
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:-1\n");
}

// ── Phase 3: generated server (the `serve` statement) ───────────────────────────

// A server with NO hand-written protocol code: `serve` generates the accept
// loop, request parsing, method dispatch, and reply. It prints its bound port.
const SERVE_SERVER_SRC: &str = "\
class BillingService {
    rate: int
    fn charge(self, amount: int) int {
        return amount * self.rate
    }
}

fn main() {
    let svc = BillingService { rate: 2 }
    serve svc on 0
}";

/// Both ends of the RPC are now compiler-generated: the client uses a `remote`
/// dep, the server uses `serve`. A real two-process round-trip with no
/// hand-written transport on either side.
#[test]
fn serve_generated_server_round_trips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", SERVE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLIENT_SRC)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();
    assert!(!port.is_empty(), "serve did not report a port");

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:42\n");
}

// ── Phase 4: complex types over the wire ────────────────────────────────────────

// Server exposing a method that takes AND returns a struct. Both sides import
// std.wire; the generated marshalers carry the struct as JSON across the socket.
const STRUCT_SERVER_SRC: &str = "\
import std.wire

class User {
    id: int
    name: string
}

class Echo {
    seed: int
    fn relabel(self, u: User) User {
        return User { id: u.id, name: u.name + \"!\" }
    }
}

fn main() {
    let e = Echo { seed: 1 }
    serve e on 0
}";

const STRUCT_IFACE: &str = "\
pub class User {
    id: int
    name: string
}
pub class Echo {
    fn relabel(self, u: User) User {
        return u
    }
}";

const STRUCT_CLIENT_SRC: &str = "\
import std.wire
import echo

app App[e: remote echo.Echo] {
    fn main(self) {
        let input = echo.User { id: 3, name: \"bob\" }
        let out = self.e.relabel(input) catch echo.User { id: -1, name: \"ERR\" }
        print(f\"id={out.id} name={out.name}\")
    }
}";

/// A struct crosses the wire in both directions: the client sends a `User`, the
/// server relabels it and returns a `User` — marshaled as JSON by the generated
/// wire wrappers on each side.
#[test]
fn complex_type_round_trips_over_rpc() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", STRUCT_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("echo.pluto", STRUCT_IFACE), ("main.pluto", STRUCT_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_ECHO", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "id=3 name=bob!\n");
}

// ── Phase 5: length-framed transport (large payloads) ───────────────────────────

// Returns a string of `n` 'x' chars — a payload that exceeds any single socket
// read, exercising the length-framed transport.
const BLOB_SERVER_SRC: &str = "\
import std.wire
import std.strings

class Blob {
    text: string
}

class Store {
    seed: int
    fn fetch(self, n: int) Blob {
        return Blob { text: strings.repeat(\"x\", n) }
    }
}

fn main() {
    let s = Store { seed: 1 }
    serve s on 0
}";

const BLOB_IFACE: &str = "\
pub class Blob {
    text: string
}
pub class Store {
    fn fetch(self, n: int) Blob {
        return Blob { text: \"\" }
    }
}";

const BLOB_CLIENT_SRC: &str = "\
import std.wire
import store

app App[store: remote store.Store] {
    fn main(self) {
        let b = self.store.fetch(200000) catch store.Blob { text: \"ERR\" }
        print(f\"len={b.text.len()}\")
    }
}";

/// A 200 KB payload round-trips intact. Without length-framing the response
/// would be truncated at the first socket read.
#[test]
fn large_payload_round_trips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", BLOB_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("store.pluto", BLOB_IFACE), ("main.pluto", BLOB_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_STORE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "len=200000\n");
}

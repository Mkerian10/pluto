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

// ── Phase 6: typed-error propagation ────────────────────────────────────────────

// Server method raises ValidationError on bad input; the error crosses the wire
// in an ERR frame and re-raises on the client.
const ERR_SERVER_SRC: &str = "\
import std.wire

error ValidationError {
    reason: string
}

class User {
    id: int
    name: string
}

class Directory {
    seed: int
    fn lookup(self, id: int) User {
        if id < 0 {
            raise ValidationError { reason: \"negative id\" }
        }
        return User { id: id, name: \"alice\" }
    }
}

fn main() {
    let d = Directory { seed: 1 }
    serve d on 0
}";

// The interface declares the error contract (via a raising body, never executed
// on the client) so the remote call's error set includes ValidationError.
const ERR_IFACE: &str = "\
pub error ValidationError {
    reason: string
}
pub class User {
    id: int
    name: string
}
pub class Directory {
    fn lookup(self, id: int) User {
        raise ValidationError { reason: \"contract\" }
    }
}";

const ERR_CLIENT_SRC: &str = "\
import std.wire
import directory

app App[dir: remote directory.Directory] {
    fn main(self) {
        let u = self.dir.lookup(-5) catch err {
            print(\"caught\")
            return
        }
        print(f\"ok id={u.id}\")
    }
}";

/// A typed error raised by the server crosses the wire and fires the client's
/// `catch` — instead of the client receiving a garbage value as if it succeeded.
#[test]
fn typed_error_propagates_over_rpc() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", ERR_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("directory.pluto", ERR_IFACE), ("main.pluto", ERR_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_DIRECTORY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "caught\n");
}

// ── Phase 7: multi-catch on a remote call ───────────────────────────────────────

// Server raises a typed ValidationError on bad input; otherwise returns a User.
const MC_SERVER_SRC: &str = "\
import std.wire

error ValidationError {
    reason: string
}

class User {
    id: int
    name: string
}

class Directory {
    seed: int
    fn lookup(self, id: int) User {
        if id < 0 {
            raise ValidationError { reason: \"id must be positive\" }
        }
        return User { id: id, name: \"alice\" }
    }
}

fn main() {
    let d = Directory { seed: 1 }
    serve d on 0
}";

const MC_IFACE: &str = "\
pub error ValidationError {
    reason: string
}
pub class User {
    id: int
    name: string
}
pub class Directory {
    fn lookup(self, id: int) User {
        raise ValidationError { reason: \"contract\" }
    }
}";

// Multi-catch: read the typed ValidationError's field, and handle the transport
// NetworkError with a wildcard — both error paths of one remote call.
const MC_CLIENT_SRC: &str = "\
import std.wire
import directory

app App[dir: remote directory.Directory] {
    fn main(self) {
        let u = self.dir.lookup(-5) catch err: directory.ValidationError {
            print(f\"rejected: {err.reason}\")
            return
        }
        catch err {
            print(\"network error\")
            return
        }
        print(f\"ok id={u.id}\")
    }
}";

/// Multi-catch makes both ends of the error story reachable on a single remote
/// call: the typed server error (with its field data) AND the transport error.
#[test]
fn multi_catch_handles_typed_and_network_errors() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", MC_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("directory.pluto", MC_IFACE), ("main.pluto", MC_CLIENT_SRC)]);

    // Validation path: server is up and rejects id=-5.
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();
    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_DIRECTORY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "rejected: id must be positive\n");

    // Network path: nothing listening -> NetworkError -> wildcard handler.
    let down = Command::new(&client_bin)
        .env("PLUTO_REMOTE_DIRECTORY", "127.0.0.1:1")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&down.stdout), "network error\n");
}

// ── Robustness: a stuck client must not be able to hang the server ──────────────

/// A client that opens a connection, sends a partial request, then stalls must
/// not hang the (single-threaded) server forever. The server's receive timeout
/// drops the stuck connection and goes on to serve real clients — bounding what
/// would otherwise be a head-of-line denial of service.
#[test]
fn serve_recovers_from_a_stuck_client() {
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let (_sd, server_bin) = build_binary(&[("main.pluto", SERVE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim().to_string();

    // Stuck client: a partial length-framed request, then hold the socket open.
    let mut stuck = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    stuck.write_all(&[0, 0, 0, 0x10]).unwrap(); // header claims 16 bytes...
    stuck.write_all(b"charge").unwrap();        // ...but only 6 are sent, then we stall
    stuck.flush().unwrap();

    // A real client must still be served, after the server's recv timeout.
    let mut client = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(20);
    let served = loop {
        if client.try_wait().unwrap().is_some() { break true; }
        if Instant::now() > deadline { break false; }
        std::thread::sleep(Duration::from_millis(200));
    };
    if !served { let _ = client.kill(); }
    let out = client.wait_with_output().unwrap();
    let _ = server.kill();
    drop(stuck);

    assert!(served, "real client hung behind a stuck client (head-of-line DoS)");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:42\n");
}

// ── Interface-hash handshake: reject version-skewed clients ─────────────────────

// A client interface that disagrees with the server: charge takes a string, not
// an int. Its interface hash differs, so the server won't dispatch to it.
const SKEW_IFACE: &str = "\
pub class BillingService {
    fn charge(self, amount: string) int {
        return 0
    }
}";

const SKEW_CLIENT_SRC: &str = "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(\"hi\") catch -1
        print(f\"result:{x}\")
    }
}";

/// A client compiled against a different interface signature is rejected by the
/// server's interface-hash check: the call fails cleanly (NetworkError -> -1)
/// instead of silently running the method with a misparsed argument. This is the
/// runtime complement to the compile-time conformance check — it catches version
/// skew between independently-compiled binaries.
#[test]
fn remote_call_rejected_on_interface_skew() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", SERVE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", SKEW_IFACE), ("main.pluto", SKEW_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:-1\n");
}

// ── Contract-aware interface hashing (rfc-properties.md, phase 5.5) ─────────────
//
// Type-level contract clauses of boundary-crossing types fold into the
// interface hash: a contract change is a wire-compatibility event, exactly
// like a signature change (downstream proofs assume the clauses). The
// consumer must mirror the clauses in its interface declaration to pair.

// The server's service now carries an invariant.
const CONTRACT_SERVER_SRC: &str = "\
class BillingService {
    rate: int
    invariant self.rate >= 0
    fn charge(self, amount: int) int {
        return amount * self.rate
    }
}

fn main() {
    let svc = BillingService { rate: 2 }
    serve svc on 0
}";

// A consumer interface that mirrors the contract: hashes align.
const CONTRACT_IFACE: &str = "\
pub class BillingService {
    rate: int
    invariant self.rate >= 0
    fn charge(self, amount: int) int {
        return amount
    }
}";

/// A client compiled against an interface WITHOUT the server's invariant is
/// refused by the interface-hash check: the contract is part of the wire
/// surface, and a consumer that does not assume it must not pair.
#[test]
fn remote_call_rejected_on_contract_skew() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", CONTRACT_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:-1\n");
}

/// The same pairing with the contract mirrored in the consumer's interface
/// declaration round-trips: same contracts ⇒ same hash.
#[test]
fn remote_call_round_trips_with_mirrored_contract() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", CONTRACT_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", CONTRACT_IFACE), ("main.pluto", CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:42\n");
}

// ── Layout-aware interface hashing (issue #424) ────────────────────────────────
//
// The field shape of a wire-crossing value type is interface surface. Two
// peers whose only difference is the ORDER of same-typed fields are
// positionally compatible on the wire — before #424 they paired hash-equal
// and silently swapped field values. The layout now folds into the interface
// hash, so the pairing is refused pre-dispatch.

const REORDER_SERVER_SRC: &str = "\
import std.wire

class Person {
    first: string
    last: string
}

class Registry {
    pad: int
    fn whois(self, key: int) Person {
        return Person { first: \"Ada\", last: \"Lovelace\" }
    }
}

fn main() {
    let r = Registry { pad: 0 }
    serve r on 0
}";

// Identical to the server's declarations except `last` before `first`.
const REORDER_IFACE: &str = "\
pub class Person {
    last: string
    first: string
}
pub class Registry {
    fn whois(self, key: int) Person {
        return Person { last: \"x\", first: \"y\" }
    }
}";

// The same-layout stub: pairs cleanly.
const ALIGNED_IFACE: &str = "\
pub class Person {
    first: string
    last: string
}
pub class Registry {
    fn whois(self, key: int) Person {
        return Person { first: \"x\", last: \"y\" }
    }
}";

const REORDER_CLIENT_SRC: &str = "\
import std.wire
import registry

app Client[r: remote registry.Registry] {
    fn main(self) {
        let p = self.r.whois(1) catch err {
            print(\"refused\")
            return
        }
        print(f\"first={p.first} last={p.last}\")
    }
}";

/// The #424 repro: a client compiled with Person's fields reordered must be
/// REFUSED by the interface-hash check — not served a value whose fields it
/// will silently swap (the original failure printed `first=Lovelace`).
#[test]
fn remote_call_rejected_on_field_reorder_skew() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", REORDER_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("registry.pluto", REORDER_IFACE), ("main.pluto", REORDER_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "refused\n");
}

/// The control: the same pairing with the layouts aligned round-trips, and the
/// values land in the right fields.
#[test]
fn remote_call_round_trips_with_same_layout() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", REORDER_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("registry.pluto", ALIGNED_IFACE), ("main.pluto", REORDER_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "first=Ada last=Lovelace\n");
}

// A nullable flip on a wire-crossing field used to be LATENT: it only failed
// at decode time, and only when a none actually flowed. It is a layout
// change, so the pairing is now refused pre-dispatch.

const NULLABLE_SERVER_SRC: &str = "\
import std.wire

class Rec {
    nick: string?
    age: int
}

class Svc {
    pad: int
    fn get(self, k: int) Rec {
        return Rec { nick: none, age: 30 }
    }
}

fn main() {
    let s = Svc { pad: 0 }
    serve s on 0
}";

// The stub drops the `?`: same field names, same order, skewed nullability.
const NULLABLE_FLIP_IFACE: &str = "\
pub class Rec {
    nick: string
    age: int
}
pub class Svc {
    fn get(self, k: int) Rec {
        return Rec { nick: \"x\", age: 0 }
    }
}";

const NULLABLE_CLIENT_SRC: &str = "\
import std.wire
import svc

app Client[s: remote svc.Svc] {
    fn main(self) {
        let r = self.s.get(1) catch err {
            print(\"refused\")
            return
        }
        print(f\"nick={r.nick} age={r.age}\")
    }
}";

/// A client whose stub flips a field's nullability is refused by the
/// interface-hash check before dispatch — not left to fail (or not) depending
/// on whether a none happens to flow.
#[test]
fn remote_call_rejected_on_nullable_flip_skew() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", NULLABLE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("svc.pluto", NULLABLE_FLIP_IFACE), ("main.pluto", NULLABLE_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_SVC", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(String::from_utf8_lossy(&out.stdout), "refused\n");
}

// ── Failure classification: Definite vs Ambiguous (epistemics.md) ───────────────
//
// Every failed boundary call carries an epistemic classification in
// NetworkError's `definite` field — true means the request is KNOWN not to have
// been dispatched (the effect did not apply; safe to react freely), false means
// the request left the process and no response returned (the effect may or may
// not have applied). These tests verify the classification EMPIRICALLY against
// real transport conditions, not by reading the runtime.

// A client that catches the transport error with a typed handler and reports
// the classification. Also pins the typed-catch path itself: transport
// NetworkErrors must match `catch err: NetworkError` (they once carried no
// type tag and silently escaped typed handlers).
const CLASSIFY_CLIENT_SRC: &str = "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(21) catch err: NetworkError {
            print(f\"definite:{err.definite}\")
            return
        }
        print(f\"result:{x}\")
    }
}";

/// Connection refused (nothing listening): the connection never opened, so the
/// request was never sent — a DEFINITE failure. Likewise when no service
/// address is bound at all.
#[test]
fn boundary_failure_definite_when_never_sent() {
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLASSIFY_CLIENT_SRC)]);

    // Port 1: connect() is refused before anything is sent.
    let refused = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", "127.0.0.1:1")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&refused.stdout), "definite:true\n");

    // No address bound: nothing was ever dialed.
    let unbound = Command::new(&client_bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&unbound.stdout), "definite:true\n");
}

/// The server consumes the full request frame and closes without replying —
/// indistinguishable (to the client) from a crash after dispatch. The request
/// left the process and no response returned: the effect may or may not have
/// applied, so the classification must be AMBIGUOUS.
#[test]
fn boundary_failure_ambiguous_when_request_consumed_without_reply() {
    use std::io::{Read, Write as _};
    use std::net::TcpListener;

    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLASSIFY_CLIENT_SRC)]);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let srv = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        // Read the complete length-framed request so the send provably
        // succeeded, then drop the connection without any response.
        let mut hdr = [0u8; 4];
        conn.read_exact(&mut hdr).unwrap();
        let len = u32::from_be_bytes(hdr) as usize;
        let mut body = vec![0u8; len];
        conn.read_exact(&mut body).unwrap();
        let _ = conn.flush();
        // conn dropped here: EOF at the client, no response ever written.
    });

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    srv.join().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "definite:false\n");
}

// The classify client compiled against a skewed interface (string instead of
// int), so the server's interface-hash check refuses to dispatch it.
const CLASSIFY_SKEW_CLIENT_SRC: &str = "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let x = self.billing.charge(\"hi\") catch err: NetworkError {
            print(f\"definite:{err.definite}\")
            print(f\"msg:{err.message}\")
            return
        }
        print(f\"result:{x}\")
    }
}";

/// A version-skewed client is refused by the server's interface-hash check
/// BEFORE dispatch, and the server says so in its reply — the authority itself
/// reports the request never ran, converting what would otherwise be an
/// ambiguous no-reply close into a DEFINITE failure.
#[test]
fn boundary_failure_definite_on_interface_skew_rejection() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", SERVE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", SKEW_IFACE), ("main.pluto", CLASSIFY_SKEW_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("definite:true"));
    let msg = lines.next().unwrap_or_default();
    assert!(
        msg.contains("rejected before dispatch"),
        "rejection message should say the request was not dispatched: {msg}"
    );
}

/// The forked server handles connections concurrently: a real client is served
/// promptly even while another client holds a stuck, half-open request. Under
/// the old single-threaded server the real client would wait behind the stuck
/// one until its 5s recv timeout; concurrently it returns almost immediately.
#[test]
fn serve_handles_clients_concurrently() {
    use std::io::Write;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let (_sd, server_bin) = build_binary(&[("main.pluto", SERVE_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", CLIENT_SRC)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim().to_string();

    // A stuck client holds a half-open request; its handler blocks ~5s.
    let mut stuck = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    stuck.write_all(&[0, 0, 0, 0x10]).unwrap();
    stuck.write_all(b"charge").unwrap();
    stuck.flush().unwrap();

    // The real client should be served in well under the stuck handler's timeout.
    let start = Instant::now();
    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    let _ = server.kill();
    drop(stuck);

    assert_eq!(String::from_utf8_lossy(&out.stdout), "result:42\n");
    assert!(
        elapsed < Duration::from_secs(3),
        "real client took {elapsed:?} — not served concurrently with the stuck client"
    );
}

// ── Bug hunt: newline in a string argument corrupts the line-delimited wire ─────

const NL_SERVER: &str = "\
class Echo {
    tag: int
    fn slen(self, s: string) int {
        return s.len()
    }
}
fn main() {
    let e = Echo { tag: 1 }
    serve e on 0
}";

const NL_IFACE: &str = "\
pub class Echo {
    fn slen(self, s: string) int {
        return 0
    }
}";

const NL_CLIENT: &str = "\
import echo
app App[e: remote echo.Echo] {
    fn main(self) {
        let n = self.e.slen(\"ab\\ncd\") catch -1
        print(f\"len:{n}\")
    }
}";

/// A string argument that contains a newline must survive the round-trip. The
/// request is newline-delimited, so an unescaped newline in a string arg splits
/// into extra fields and the server sees a truncated argument.
#[test]
fn remote_string_arg_with_newline_roundtrips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", NL_SERVER)]);
    let (_cd, client_bin) =
        build_binary(&[("echo.pluto", NL_IFACE), ("main.pluto", NL_CLIENT)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_ECHO", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    // "ab\ncd" is 5 bytes; a corrupted split would yield 2 ("ab").
    assert_eq!(String::from_utf8_lossy(&out.stdout), "len:5\n");
}

// Does a newline in a STRUCT's string field survive? (structs go through JSON.)
const SNL_SERVER: &str = "\
import std.wire
class User {
    id: int
    name: string
}
class Store {
    tag: int
    fn namelen(self, u: User) int {
        return u.name.len()
    }
}
fn main() {
    let s = Store { tag: 1 }
    serve s on 0
}";

const SNL_IFACE: &str = "\
import std.wire
pub class User {
    id: int
    name: string
}
pub class Store {
    fn namelen(self, u: User) int {
        return 0
    }
}";

const SNL_CLIENT: &str = "\
import std.wire
import store
app App[s: remote store.Store] {
    fn main(self) {
        let u = store.User { id: 1, name: \"a\\nb\" }
        let n = self.s.namelen(u) catch -1
        print(f\"nlen:{n}\")
    }
}";

#[test]
fn remote_struct_string_field_with_newline_roundtrips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", SNL_SERVER)]);
    let (_cd, client_bin) =
        build_binary(&[("store.pluto", SNL_IFACE), ("main.pluto", SNL_CLIENT)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_STORE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    // "a\nb" is 3 bytes.
    assert_eq!(String::from_utf8_lossy(&out.stdout), "nlen:3\n");
}

// The return-value direction of the same bug: a returned string with a newline.
const RET_SERVER: &str = "\
class Banner {
    tag: int
    fn make(self) string {
        return \"x\\ny\"
    }
}
fn main() {
    let b = Banner { tag: 1 }
    serve b on 0
}";

const RET_IFACE: &str = "\
pub class Banner {
    fn make(self) string {
        return \"\"
    }
}";

const RET_CLIENT: &str = "\
import banner
app App[b: remote banner.Banner] {
    fn main(self) {
        let s = self.b.make() catch \"?\"
        print(f\"blen:{s.len()}\")
    }
}";

#[test]
fn remote_string_return_with_newline_roundtrips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", RET_SERVER)]);
    let (_cd, client_bin) =
        build_binary(&[("banner.pluto", RET_IFACE), ("main.pluto", RET_CLIENT)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BANNER", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    // "x\ny" is 3 bytes; truncation at the newline would yield 1.
    assert_eq!(String::from_utf8_lossy(&out.stdout), "blen:3\n");
}

// Stress the escape scheme: a string with a literal backslash AND a newline.
const BS_CLIENT: &str = "\
import echo
app App[e: remote echo.Echo] {
    fn main(self) {
        let n = self.e.slen(\"a\\\\b\\nc\") catch -1
        print(f\"len:{n}\")
    }
}";

/// A string with a backslash and a newline must round-trip: the escaping of the
/// delimiter must not confuse a literal backslash with an escape.
#[test]
fn remote_string_arg_with_backslash_and_newline_roundtrips() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", NL_SERVER)]);
    let (_cd, client_bin) =
        build_binary(&[("echo.pluto", NL_IFACE), ("main.pluto", BS_CLIENT)]);

    let mut server = Command::new(&server_bin).stdout(Stdio::piped()).spawn().unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_ECHO", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    // "a\b\nc" is 5 bytes: 'a', '\', 'b', newline, 'c'.
    assert_eq!(String::from_utf8_lossy(&out.stdout), "len:5\n");
}

// ── Phase 7: float / bool / nullable over the wire ──────────────────────────────

const SCALAR_SERVER_SRC: &str = "\
import std.wire

class Calc {
    seed: int
    fn half(self, x: float) float {
        return x / 2.0
    }
    fn flip(self, b: bool) bool {
        return !b
    }
    fn tag(self, s: string) string? {
        if s == \"none\" {
            return none
        }
        return s
    }
}

fn main() {
    let c = Calc { seed: 1 }
    serve c on 0
}";

const SCALAR_IFACE: &str = "\
pub class Calc {
    fn half(self, x: float) float {
        return x / 2.0
    }
    fn flip(self, b: bool) bool {
        return !b
    }
    fn tag(self, s: string) string? {
        if s == \"none\" {
            return none
        }
        return s
    }
}";

const SCALAR_CLIENT_SRC: &str = "\
import std.wire
import calc

app App[c: remote calc.Calc] {
    fn main(self) {
        let h = self.c.half(10.0) catch -1.0
        print(f\"half={h}\")
        let f = self.c.flip(true) catch true
        print(f\"flip={f}\")
        let t = self.c.tag(\"hello\") catch \"ERR\"
        let tv = t ?? \"was-none\"
        print(f\"tag={tv}\")
        let n = self.c.tag(\"none\") catch \"ERR\"
        let nv = n ?? \"was-none\"
        print(f\"none-case={nv}\")
    }
}";

/// Floats, bools, and nullables cross the wire in both directions: float and
/// bool as arguments and returns, and a nullable return in both its some and
/// none states.
#[test]
fn scalar_and_nullable_round_trip_over_rpc() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", SCALAR_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("calc.pluto", SCALAR_IFACE), ("main.pluto", SCALAR_CLIENT_SRC)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&client_bin)
        .env("PLUTO_REMOTE_CALC", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "half=5\nflip=false\ntag=hello\nnone-case=was-none\n"
    );
}

const CONTAINER_SERVER_SRC: &str = r#"
import std.wire

class User {
    id: int
    name: string
}

class Store {
    seed: int
    fn double_all(self, xs: [int]) [int] {
        let mut out: [int] = []
        let mut i = 0
        while i < xs.len() {
            out.push(xs[i] * 2)
            i = i + 1
        }
        return out
    }
    fn counts(self) Map<string, int> {
        let m = Map<string, int> {}
        m.insert("a", 1)
        m.insert("b", 2)
        return m
    }
    fn users(self, n: int) [User] {
        let mut out: [User] = []
        let mut i = 0
        while i < n {
            out.push(User { id: i, name: f"u{i}" })
            i = i + 1
        }
        return out
    }
    fn uniq(self, xs: [int]) Set<int> {
        let s = Set<int> {}
        let mut i = 0
        while i < xs.len() {
            s.insert(xs[i])
            i = i + 1
        }
        return s
    }
    fn maybe(self, want: bool) [int]? {
        if want {
            return [7, 8]
        }
        return none
    }
    fn rows(self) [[int]] {
        let mut out: [[int]] = []
        out.push([1, 2])
        out.push([3])
        return out
    }
}

fn main() {
    let s = Store { seed: 1 }
    serve s on 0
}"#;

const CONTAINER_IFACE: &str = r#"
pub class User {
    id: int
    name: string
}

pub class Store {
    seed: int
    fn double_all(self, xs: [int]) [int] {
        return xs
    }
    fn counts(self) Map<string, int> {
        let m = Map<string, int> {}
        return m
    }
    fn users(self, n: int) [User] {
        let out: [User] = []
        return out
    }
    fn uniq(self, xs: [int]) Set<int> {
        let s = Set<int> {}
        return s
    }
    fn maybe(self, want: bool) [int]? {
        return none
    }
    fn rows(self) [[int]] {
        let out: [[int]] = []
        return out
    }
}"#;

const CONTAINER_CLIENT_SRC: &str = r#"
import std.wire
import store

app App[s: remote store.Store] {
    fn main(self) {
        let empty: [int] = []
        let d = self.s.double_all([1, 2, 3]) catch empty
        let d0 = d[0]
        let d1 = d[1]
        let d2 = d[2]
        print(f"doubled={d0},{d1},{d2}")

        let fallback = Map<string, int> {}
        let m = self.s.counts() catch fallback
        let ma = m["a"]
        let mb = m["b"]
        print(f"counts a={ma} b={mb}")

        let nousers: [store.User] = []
        let us = self.s.users(2) catch nousers
        let u0 = us[0].name
        let u1 = us[1].name
        print(f"users={u0},{u1}")

        let noset = Set<int> {}
        let sq = self.s.uniq([3, 3, 4]) catch noset
        let sqlen = sq.len()
        let has3 = sq.contains(3)
        let has5 = sq.contains(5)
        print(f"set len={sqlen} has3={has3} has5={has5}")

        let some = self.s.maybe(true) catch empty
        let somev = some ?? empty
        let s0 = somev[0]
        let s1 = somev[1]
        print(f"maybe={s0},{s1}")
        let nothing = self.s.maybe(false) catch empty
        if nothing == none {
            print("maybe-none=yes")
        } else {
            print("maybe-none=no")
        }

        let norows: [[int]] = []
        let r = self.s.rows() catch norows
        let r00 = r[0][0]
        let r01 = r[0][1]
        let r10 = r[1][0]
        print(f"rows={r00},{r01},{r10}")
    }
}"#;

/// Containers cross the wire as top-level argument and return types: arrays
/// (of ints and of classes), maps, sets, nullable arrays (both states), and
/// nested arrays — each round-tripped over a real socket.
#[test]
fn containers_round_trip_over_rpc() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", CONTAINER_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("store.pluto", CONTAINER_IFACE), ("main.pluto", CONTAINER_CLIENT_SRC)]);

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

    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "doubled=2,4,6\n\
         counts a=1 b=2\n\
         users=u0,u1\n\
         set len=2 has3=true has5=false\n\
         maybe=7,8\n\
         maybe-none=yes\n\
         rows=1,2,3\n"
    );
}

// ── Phase 8: `at` placement expressions (docs/design/rfc-at-placement.md) ──

const AT_APP_SRC: &str = r#"
import std.wire

error PaymentDeclined {
    code: int
}

class PaymentService {
    fn charge(self, amount: int) int {
        if amount > 50 {
            raise PaymentDeclined { code: 402 }
        }
        return amount + 1
    }
    fn batch(self, amounts: [int]) [int] {
        let mut out: [int] = []
        let mut i = 0
        while i < amounts.len() {
            out.push(amounts[i] + 1)
            i = i + 1
        }
        return out
    }
}

app Shop[pay: domain PaymentService] {
    fn main(self) {
        let ok = at self.pay { charge(10) } catch -1
        print(f"ok: {ok}")
        let declined = at self.pay {
            charge(100)
        } catch err: PaymentDeclined {
            0 - err.code
        } catch err {
            -1
        }
        print(f"declined: {declined}")
        let empty: [int] = []
        let b = at self.pay { batch([5, 6]) } catch empty
        let b0 = b[0]
        let b1 = b[1]
        print(f"batch: {b0},{b1}")
    }
}"#;

const AT_SERVER_SRC: &str = r#"
import std.wire

error PaymentDeclined {
    code: int
}

class PaymentService {
    fn charge(self, amount: int) int {
        if amount > 50 {
            raise PaymentDeclined { code: 402 }
        }
        return amount + 1
    }
    fn batch(self, amounts: [int]) [int] {
        let mut out: [int] = []
        let mut i = 0
        while i < amounts.len() {
            out.push(amounts[i] + 1)
            i = i + 1
        }
        return out
    }
}

fn main() {
    let svc = PaymentService {}
    serve svc on 0
}"#;

/// The placement model's central claim, as running code: ONE app binary whose
/// `at` boundary lowers to a direct local call when the domain is unbound and
/// to the socket transport when PLUTO_DOMAIN_<SERVICE> is set — with identical
/// observable behavior, including typed-error reconstruction and container
/// returns, in both plans.
#[test]
fn at_placement_same_binary_two_plans() {
    let (_ad, app_bin) = build_binary(&[("main.pluto", AT_APP_SRC)]);
    let expected = "ok: 11\ndeclined: -402\nbatch: 6,7\n";

    // Plan A: colocated — no binding, direct in-process call.
    let colocated = Command::new(&app_bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), expected);

    // Plan B: distributed — same binary, domain bound to a real server.
    let (_sd, server_bin) = build_binary(&[("main.pluto", AT_SERVER_SRC)]);
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let distributed = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_PAYMENTSERVICE", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&distributed.stdout), expected);
}

/// The boundary contract is plan-independent: an unhandled `at` is rejected
/// at compile time even though the program may only ever run colocated.
#[test]
fn at_unhandled_rejected_in_any_plan() {
    compile_project_should_fail_with(
        &[(
            "main.pluto",
            "class Svc {\n    fn ping(self) int {\n        return 1\n    }\n}\n\napp A[s: domain Svc] {\n    fn main(self) {\n        let x = at self.s { ping() }\n        print(x)\n    }\n}",
        )],
        "placement expression `at` (calling 'ping') must be handled",
    );
}

/// The boundary must be syntactically visible: direct method calls on a
/// domain dep are rejected with a pointer to `at`.
#[test]
fn direct_call_on_domain_dep_rejected() {
    compile_project_should_fail_with(
        &[(
            "main.pluto",
            "class Svc {\n    fn ping(self) int {\n        return 1\n    }\n}\n\napp A[s: domain Svc] {\n    fn main(self) {\n        let x = self.s.ping() catch -1\n        print(x)\n    }\n}",
        )],
        "computation in domain 'Svc' must be placed with 'at'",
    );
}

/// `at` only places into declared domains — an ordinary dep is rejected.
#[test]
fn at_on_non_domain_dep_rejected() {
    compile_project_should_fail_with(
        &[(
            "main.pluto",
            "class Svc {\n    fn ping(self) int {\n        return 1\n    }\n}\n\napp A[s: Svc] {\n    fn main(self) {\n        let x = at self.s { ping() } catch -1\n        print(x)\n    }\n}",
        )],
        "'at' requires a domain or an entity: 'Svc' is neither",
    );
}

// ── Phase 9: entity identity handles (rfc-objects.md phase 2, slice 1) ──

const HANDLE_APP_SRC: &str = r#"
import std.wire

object Vault {
    secret: int

    fn bump(mut self) {
        self.secret = self.secret + 1
    }

    fn get(self) int {
        return self.secret
    }
}

class Escrow {
    fn hold(self, v: Vault) Vault {
        return v
    }
}

app A[esc: domain Escrow] {
    fn main(self) {
        let mut v = Vault { secret: 41 }
        let held = at self.esc { hold(v) } catch v
        print(v == held)
        let mut h2 = held
        h2.bump()
        print(v.get())
    }
}"#;

const HANDLE_SERVER_SRC: &str = r#"
import std.wire

object Vault {
    secret: int

    fn bump(mut self) {
        self.secret = self.secret + 1
    }

    fn get(self) int {
        return self.secret
    }
}

class Escrow {
    fn hold(self, v: Vault) Vault {
        return v
    }
}

fn main() {
    let e = Escrow {}
    serve e on 0
}"#;

/// Identity survives the wire: an entity crosses to another process as a
/// handle and, returned to its home, resolves to the SAME entity — `==` is
/// true and mutation through the returned reference hits the original.
/// Verified in both physical plans from one binary.
#[test]
fn entity_identity_survives_round_trip() {
    let (_ad, app_bin) = build_binary(&[("main.pluto", HANDLE_APP_SRC)]);
    let expected = "true\n42\n";

    // Plan A: colocated — the "handle" never exists; direct call.
    let colocated = Command::new(&app_bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), expected);

    // Plan B: distributed — the entity round-trips a real socket.
    let (_sd, server_bin) = build_binary(&[("main.pluto", HANDLE_SERVER_SRC)]);
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let distributed = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_ESCROW", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&distributed.stdout), expected);
}

/// Entities nested inside values are untransferable — a copy would fork
/// their identity. Only the entity itself (as a handle) may cross.
#[test]
fn nested_entity_rejected_at_boundary() {
    compile_project_should_fail_with(
        &[(
            "main.pluto",
            "object Vault {\n    secret: int\n}\n\nclass Wrap {\n    v: Vault\n}\n\nclass Escrow {\n    fn hold(self, w: Wrap) int {\n        return 1\n    }\n}\n\napp A[esc: domain Escrow] {\n    fn main(self) {\n        let v = Vault { secret: 1 }\n        let w = Wrap { v: v }\n        let r = at self.esc { hold(w) } catch -1\n        print(r)\n    }\n}",
        )],
        "a value containing an object cannot enter domain 'Escrow'",
    );
}

// ── Phase 10: handle-call routing (rfc-objects.md phase 2, slice 2) ──

const ROUTE_SHARED: &str = r#"
import std.wire

object Vault {
    secret: int

    fn reveal(self) int {
        return self.secret
    }

    fn rotate(mut self) {
        self.secret = self.secret + 100
    }
}

class Registry {
    v: Vault

    fn vault(self) Vault {
        return self.v
    }
}
"#;

/// The vision's opening image, executing: a client fetches a HANDLE to a
/// server-owned entity, and `at v { reveal() }` routes the call to the
/// entity's home process. `rotate()` mutates the entity AT HOME (the
/// thread-per-connection serve model makes the mutation stick), so a second
/// reveal reads the changed state.
///
/// Both physical plans from the one client binary. Colocated (no domain
/// binding), the DI-synthesized Registry zero-constructs its data fields:
/// `v` is a REAL zero-state entity (secret 0), fetched directly, and
/// `rotate()` mutates it in place — 0 then 100. This used to hand back a
/// null entity (the field was a calloc'd null pointer) and segfault.
#[test]
fn entity_handle_call_routes_home() {
    let server_src = format!(
        "{ROUTE_SHARED}\nfn main() {{\n    let v = Vault {{ secret: 7 }}\n    let r = Registry {{ v: v }}\n    serve r on 0\n}}"
    );
    let app_src = format!(
        "{ROUTE_SHARED}\napp A[reg: domain Registry] {{\n    fn main(self) {{\n        let fallback = Vault {{ secret: -1 }}\n        let v = at self.reg {{ vault() }} catch fallback\n        let s1 = at v {{ reveal() }} catch -2\n        print(s1)\n        at v {{ rotate() }} catch err {{}}\n        let s2 = at v {{ reveal() }} catch -2\n        print(s2)\n    }}\n}}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    // Plan A: colocated — the registry is DI-constructed in-process with a
    // live zero-state entity in its field.
    let colocated = Command::new(&app_bin).output().unwrap();
    assert!(
        colocated.status.success(),
        "colocated plan crashed (zero-filled registry field?): {:?}",
        colocated.status
    );
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), "0\n100\n");

    // Plan B: distributed — the call routes to the entity's home process.
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7\n107\n");
}

// Shared source for the two-plan data-field test: a domain class carrying
// plain data (string + int + array), no entities.
const DATA_ROUTE_SHARED: &str = r#"
import std.wire

class Config {
    label: string
    count: int
    tags: [string]

    fn describe(self) string {
        return self.label
    }

    fn total(self) int {
        return self.count + self.tags.len()
    }
}
"#;

/// A domain dependency's PLAIN data fields in both physical plans. Served,
/// the fields hold whatever the serving process constructed. Colocated, the
/// DI-synthesized instance is zero-STATE constructed — empty string, 0,
/// empty array — so the same calls run against real values instead of
/// dereferencing the null pointers the old synthesis left behind.
#[test]
fn domain_data_fields_in_both_plans() {
    let server_src = format!(
        "{DATA_ROUTE_SHARED}\nfn main() {{\n    let c = Config {{ label: \"prod\", count: 5, tags: [\"a\", \"b\"] }}\n    serve c on 0\n}}"
    );
    let app_src = format!(
        "{DATA_ROUTE_SHARED}\napp A[cfg: domain Config] {{\n    fn main(self) {{\n        let l = at self.cfg {{ describe() }} catch \"?\"\n        let t = at self.cfg {{ total() }} catch -1\n        print(f\"[{{l}}]\")\n        print(t)\n    }}\n}}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    // Plan A: colocated — zero-state data fields ("" / 0 / []).
    let colocated = Command::new(&app_bin).output().unwrap();
    assert!(
        colocated.status.success(),
        "colocated plan crashed (zero-filled data fields?): {:?}",
        colocated.status
    );
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), "[]\n0\n");

    // Plan B: distributed — the server's constructed state.
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_CONFIG", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "[prod]\n7\n");
}

// Shared source for the dispatch-reentrancy test: the served entity's
// rotate_twice self-calls rotate.
const REENTRANT_SHARED: &str = r#"
import std.wire

object Vault {
    secret: int

    fn reveal(self) int {
        return self.secret
    }

    fn rotate(mut self) {
        self.secret = self.secret + 100
    }

    fn rotate_twice(mut self) {
        self.rotate()
        self.rotate()
    }
}

class Registry {
    v: Vault

    fn vault(self) Vault {
        return self.v
    }
}
"#;

/// Handle-dispatch reentrancy (rfc-objects.md open question 6): a routed
/// call lands on a serve connection thread, which takes the entity's write
/// lock in the generated dispatch and invokes `rotate_twice` — whose body
/// self-calls `rotate` on the same instance. The self-call is part of the
/// same message and must proceed (codegen fast path; owner-aware lock
/// backstop). A deadlock regression hangs the server, so the client is
/// reaped on a deadline instead of waiting forever.
#[test]
fn served_entity_self_call_in_dispatch() {
    let server_src = format!(
        "{REENTRANT_SHARED}\nfn main() {{\n    let v = Vault {{ secret: 7 }}\n    let r = Registry {{ v: v }}\n    serve r on 0\n}}"
    );
    let app_src = format!(
        "{REENTRANT_SHARED}\napp A[reg: domain Registry] {{\n    fn main(self) {{\n        let fallback = Vault {{ secret: -1 }}\n        let v = at self.reg {{ vault() }} catch fallback\n        at v {{ rotate_twice() }} catch err {{}}\n        let s = at v {{ reveal() }} catch -2\n        print(s)\n    }}\n}}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let mut client = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_REGISTRY", format!("127.0.0.1:{port}"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        match client.try_wait().unwrap() {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                client.kill().ok();
                server.kill().ok();
                panic!("dispatch self-call timed out — possible entity lock deadlock");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let mut stdout = String::new();
    use std::io::Read as _;
    client.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
    let _ = server.kill();
    assert!(status.success(), "client exited with non-zero status");
    assert_eq!(stdout, "207\n");
}

/// Entity placement can carry CLASS values across the boundary surface:
/// marshalers are generated for the param/return types of object methods
/// even when the program has no serve/remote/domain boundary. Pins the
/// examples/blob protocol (rfc-verification.md's acceptance-test shape):
/// a WriteGrant evidence value crosses `at blob { ... }` in both
/// directions, and the authority's fence rejects a stale grant with a
/// typed error.
#[test]
fn entity_placement_carries_evidence_values() {
    let src = r#"
import std.wire

error StaleGrant {
    token: int
    epoch: int
}

class WriteGrant {
    token: int
}

object BlobAuthority {
    data: string
    epoch: int
    applied: int

    invariant self.epoch >= 0
    invariant self.applied <= self.epoch

    fn grant_write(mut self) WriteGrant {
        self.epoch = self.epoch + 1
        return WriteGrant { token: self.epoch }
    }

    fn apply(mut self, grant: WriteGrant, d: string) {
        let tok = grant.token
        if tok != self.epoch {
            raise StaleGrant { token: tok, epoch: self.epoch }
        }
        self.applied = tok
        self.data = d
    }

    fn read(self) string {
        return self.data
    }
}

fn run(mut blob: BlobAuthority) {
    let a = at blob { grant_write() }!
    at blob { apply(a, "A") }!
    let b = at blob { grant_write() }!
    at blob { apply(a, "A-stale") } catch err: StaleGrant {
        print(f"fenced {err.token} < {err.epoch}")
    } catch err {
        print("boundary failure")
    }
    at blob { apply(b, "B") }!
    let v = at blob { read() }!
    print(v)
}

fn main() {
    let mut blob = BlobAuthority { data: "genesis", epoch: 0, applied: 0 }
    run(blob) catch err {
        print("unexpected failure")
    }
}
"#;
    let (_d, bin) = build_binary(&[("main.pluto", src)]);
    let out = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "fenced 1 < 2\nB\n");
}

/// Marshalers for entity-placement surface types are generated even when
/// std.wire only arrives through a library module's own import. Module
/// flattening names that copy `m.wire.*`, not `wire.*` — marshal generation
/// must find it there and emit its wire calls against that prefix, or the
/// build dies with an internal "missing generated function" error. The
/// entry program here never names std.wire, yet the evidence value still
/// crosses `at` in both directions.
#[test]
fn entity_placement_marshals_with_library_wire_import() {
    let (_d, bin) = build_binary(&[
        (
            "m/m.pluto",
            "import std.wire\n\npub class Token {\n    id: int\n}\n\npub object Vault {\n    secret: int\n\n    fn mint(mut self) Token {\n        self.secret = self.secret + 1\n        return Token { id: self.secret }\n    }\n}\n",
        ),
        (
            "main.pluto",
            "import m\n\nfn main() {\n    let mut v = m.Vault { secret: 41 }\n    let miss = m.Token { id: -1 }\n    let t = at v { mint() } catch miss\n    print(t.id)\n}\n",
        ),
    ]);
    let out = Command::new(&bin).output().unwrap();
    assert!(out.status.success(), "binary exited with non-zero status");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

// ── std.blob: the verified single-writer store, served and consumed ──

// The acceptance test of rfc-verification.md ("Blob is a stdlib module"),
// distributed: a server OWNS a blob.BlobAuthority inside a served registry;
// clients fetch the authority as an entity handle and drive the whole
// protocol — mint, fenced write, stale rejection with the typed payload —
// through `at`, against the entity's home process. Neither side imports
// std.wire: the library's own import carries the boundary machinery.
const BLOB_SHARED: &str = r#"
import std.blob

pub class BlobRegistry {
    store: blob.BlobAuthority

    fn authority(self) blob.BlobAuthority {
        return self.store
    }
}
"#;

const BLOB_CLIENT_BODY: &str = r#"
app Writer[reg: domain BlobRegistry] {
    fn main(self) {
        let local = blob.create("local-fallback")
        let auth = at self.reg { authority() } catch local
        let miss = blob.WriteGrant { token: -1 }

        let grant_a = at auth { grant_write() } catch miss
        at auth { apply(grant_a, "A: first draft") } catch err {
            print("boundary failure")
        }

        let grant_b = at auth { grant_write() } catch miss
        at auth { apply(grant_a, "A: sneaky overwrite") } catch err: blob.StaleGrant {
            print(f"fenced {err.token} < {err.epoch}")
        } catch err {
            print("boundary failure")
        }

        at auth { apply(grant_b, "B: the good copy") } catch err {
            print("boundary failure")
        }
        let v = at auth { read() } catch "?"
        let e = at auth { epoch_now() } catch -1
        print(f"{v} (epoch {e})")
    }
}
"#;

/// Two processes, one authority: the server owns the BlobAuthority, the
/// client holds only a handle, and the fence still judges every write at
/// the point of effect — the stale grant is rejected across the wire with
/// its typed payload intact. The client is reaped on a deadline so a
/// routing/serialization regression cannot hang the suite.
///
/// Both plans from the one client binary. Colocated, the DI-synthesized
/// registry zero-constructs a live BlobAuthority (data "", epoch 0 — its
/// invariants hold at zero state, discharged by the DI closure check), and
/// the whole fencing protocol runs in-process with the SAME output: the
/// protocol depends only on epoch arithmetic, not the genesis payload.
/// (Before the zero-state fix the colocated fetch yielded a null entity and
/// segfaulted — the gap this suite's route tests share a fix with.)
#[test]
fn blob_authority_serves_fenced_writes_across_processes() {
    let server_src = format!(
        "{BLOB_SHARED}\nfn main() {{\n    let store = blob.create(\"genesis\")\n    let r = BlobRegistry {{ store: store }}\n    serve r on 0\n}}"
    );
    let client_src = format!("{BLOB_SHARED}\n{BLOB_CLIENT_BODY}");
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_cd, client_bin) = build_binary(&[("main.pluto", &client_src)]);
    let expected = "fenced 1 < 2\nB: the good copy (epoch 2)\n";

    // Plan A: colocated — no domain binding; the authority is the
    // zero-constructed entity inside the DI-wired registry.
    let colocated = Command::new(&client_bin).output().unwrap();
    assert!(
        colocated.status.success(),
        "colocated blob client crashed (null authority?): {:?}",
        colocated.status
    );
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), expected);

    // Plan B: distributed.
    // The authority lives in the server process and every call routes home
    // over a real socket (dynamic port).
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();
    assert!(!port.is_empty(), "serve did not report a port");

    let mut client = Command::new(&client_bin)
        .env("PLUTO_DOMAIN_BLOBREGISTRY", format!("127.0.0.1:{port}"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        match client.try_wait().unwrap() {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                client.kill().ok();
                server.kill().ok();
                panic!("blob client timed out — possible routing or entity-lock hang");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let mut stdout = String::new();
    use std::io::Read as _;
    client.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
    let _ = server.kill();
    assert!(status.success(), "client exited with non-zero status");
    assert_eq!(stdout, expected);
}

// ── Generic objects at the boundary (rfc-objects.md phase 3, slice 1) ──

const GENERIC_HANDLE_SHARED: &str = r#"
import std.wire

object Vault<T> {
    tag: T
    secret: int

    fn bump(mut self) {
        self.secret = self.secret + 1
    }

    fn get(self) int {
        return self.secret
    }
}

class Escrow {
    fn hold(self, v: Vault<int>) Vault<int> {
        return v
    }
}
"#;

/// A generic-object INSTANTIATION crosses the wire as an identity handle:
/// the handle carries the monomorphized type name, and returned to its home
/// it resolves to the SAME entity — `==` is true and mutation through the
/// returned reference hits the original. Both physical plans, one binary.
#[test]
fn generic_entity_identity_survives_round_trip() {
    let app_src = format!(
        "{GENERIC_HANDLE_SHARED}\napp A[esc: domain Escrow] {{\n    fn main(self) {{\n        let mut v = Vault<int> {{ tag: 9, secret: 41 }}\n        let held = at self.esc {{ hold(v) }} catch v\n        print(v == held)\n        let mut h2 = held\n        h2.bump()\n        print(v.get())\n    }}\n}}"
    );
    let server_src = format!(
        "{GENERIC_HANDLE_SHARED}\nfn main() {{\n    let e = Escrow {{}}\n    serve e on 0\n}}"
    );
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);
    let expected = "true\n42\n";

    // Plan A: colocated — the "handle" never exists; direct call.
    let colocated = Command::new(&app_bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&colocated.stdout), expected);

    // Plan B: distributed — the instantiation round-trips a real socket.
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let distributed = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_ESCROW", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&distributed.stdout), expected);
}

/// Handle-call routing works per instantiation: the client holds a handle to
/// a server-owned `Vault<int>` and `at v { ... }` routes to its home, keyed
/// by the instantiation's own interface hash.
#[test]
fn generic_entity_handle_call_routes_home() {
    let shared = r#"
import std.wire

object Vault<T> {
    tag: T
    secret: int

    fn reveal(self) int {
        return self.secret
    }

    fn rotate(mut self) {
        self.secret = self.secret + 100
    }
}

class Registry {
    v: Vault<int>

    fn vault(self) Vault<int> {
        return self.v
    }
}
"#;
    let server_src = format!(
        "{shared}\nfn main() {{\n    let v = Vault<int> {{ tag: 1, secret: 7 }}\n    let r = Registry {{ v: v }}\n    serve r on 0\n}}"
    );
    let app_src = format!(
        "{shared}\napp A[reg: domain Registry] {{\n    fn main(self) {{\n        let fallback = Vault<int> {{ tag: 0, secret: -1 }}\n        let v = at self.reg {{ vault() }} catch fallback\n        let s1 = at v {{ reveal() }} catch -2\n        print(s1)\n        at v {{ rotate() }} catch err {{}}\n        let s2 = at v {{ reveal() }} catch -2\n        print(s2)\n    }}\n}}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7\n107\n");
}

// ── Marshaling generic signature types through entity placement ──
//
// PR #343 seeded wire codecs from NON-generic object method signatures.
// These tests pin the generic analogues: signature types of GENERIC objects,
// monomorphized generic classes in signatures, and instantiations minted by
// monomorphization itself (a generic object's `Box<T>` param).

/// A GENERIC object's method can carry a plain class value through `at`:
/// phase A seeds codecs from generic object signatures too (for types that
/// don't mention the object's own type params).
#[test]
fn generic_entity_placement_carries_class_values() {
    let src = r#"
import std.wire

class Deposit {
    amount: int
}

object Vault<T> {
    tag: T
    total: int

    fn put(mut self, d: Deposit) Deposit {
        self.total = self.total + d.amount
        return Deposit { amount: self.total }
    }
}

fn main() {
    let mut v = Vault<int> { tag: 1, total: 10 }
    let d = Deposit { amount: 5 }
    let fallback = Deposit { amount: -1 }
    let r = at v { put(d) } catch fallback
    print(r.amount)
}
"#;
    let (_d, bin) = build_binary(&[("main.pluto", src)]);
    let out = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "15\n");
}

/// A non-generic object's method can carry a monomorphized generic CLASS
/// value (`Box<int>`) through `at`: the instantiation gets marshalers AND
/// `__wire_encode_/__wire_decode_` wrappers, and codegen derives the same
/// sanitized symbol name for `Box$$int`.
#[test]
fn entity_placement_carries_generic_class_values() {
    let src = r#"
import std.wire

class Box<T> {
    item: T
}

object Store {
    count: int

    fn stash(mut self, b: Box<int>) Box<int> {
        self.count = self.count + b.item
        return Box<int> { item: self.count }
    }
}

fn main() {
    let mut s = Store { count: 1 }
    let b = Box<int> { item: 41 }
    let r = at s { stash(b) } catch Box<int> { item: -1 }
    print(r.item)
}
"#;
    let (_d, bin) = build_binary(&[("main.pluto", src)]);
    let out = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

/// The hardest shape: a GENERIC object whose method signature uses the
/// object's own type param inside a generic class (`Topic<T>::publish(b:
/// Box<T>)`). The concrete `Box$$int` only exists after monomorphization, so
/// its codecs can only be generated by phase B.
#[test]
fn generic_entity_placement_carries_instantiated_signature_values() {
    let src = r#"
import std.wire

class Box<T> {
    item: T
}

object Topic<T> {
    last: T

    fn publish(mut self, b: Box<T>) Box<T> {
        self.last = b.item
        return Box<T> { item: self.last }
    }
}

fn main() {
    let mut t = Topic<int> { last: 0 }
    let b = Box<int> { item: 7 }
    let r = at t { publish(b) } catch Box<int> { item: -1 }
    print(r.item)
}
"#;
    let (_d, bin) = build_binary(&[("main.pluto", src)]);
    let out = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7\n");
}

/// Typestate-style evidence crosses placement: `Lease<Held>` monomorphizes
/// to a distinct concrete type (phantom state param) and gets its own codec.
#[test]
fn entity_placement_carries_typestate_evidence() {
    let src = r#"
import std.wire

class Held {}
class Released {}

class Lease<S> {
    token: int
}

object Locker {
    holder: int

    fn redeem(mut self, l: Lease<Held>) int {
        self.holder = l.token
        return self.holder
    }
}

fn main() {
    let mut lk = Locker { holder: 0 }
    let l = Lease<Held> { token: 42 }
    let r = at lk { redeem(l) } catch -1
    print(r)
}
"#;
    let (_d, bin) = build_binary(&[("main.pluto", src)]);
    let out = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "42\n");
}

/// A signature type whose SHAPE cannot marshal (closure field) is rejected
/// by typeck's transferability diagnostic at the placement site — not a
/// compiler panic, not a missing-generated-function codegen error.
#[test]
fn entity_placement_rejects_unmarshalable_signature_value() {
    let src = r#"
import std.wire

class Callback {
    f: fn(int) int
}

object Holder {
    n: int

    fn take(mut self, c: Callback) {
        let g = c.f
        self.n = g(self.n)
    }
}

fn main() {
    let mut h = Holder { n: 1 }
    let c = Callback { f: (x: int) => x + 1 }
    at h { take(c) }!
    print(h.n)
}
"#;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.pluto"), src).unwrap();
    let bin = dir.path().join("bin");
    match pluto::compile_file_with_stdlib(
        &dir.path().join("main.pluto"),
        &bin,
        Some(&manifest_stdlib()),
    ) {
        Ok(_) => panic!("Compilation should have failed with a transferability error"),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("cannot enter domain 'Holder'"),
                "error did not contain the transferability diagnostic.\nActual: {msg}"
            );
        }
    }
}

/// Two-process: a handle to a server-owned GENERIC entity routes `at` calls
/// home, carrying a monomorphized generic class value (`Box$$int`) across
/// the socket in both directions — keyed by the instantiation's own
/// interface hash.
#[test]
fn generic_entity_handle_call_carries_instantiated_values() {
    let shared = r#"
import std.wire

class Box<T> {
    item: T
}

object Topic<T> {
    last: T
    count: int

    fn publish(mut self, b: Box<T>) Box<T> {
        self.last = b.item
        self.count = self.count + 1
        return Box<T> { item: self.last }
    }

    fn total(self) int {
        return self.count
    }
}

class Registry {
    t: Topic<int>

    fn topic(self) Topic<int> {
        return self.t
    }
}
"#;
    let server_src = format!(
        "{shared}\nfn main() {{\n    let t = Topic<int> {{ last: 0, count: 100 }}\n    let r = Registry {{ t: t }}\n    serve r on 0\n}}"
    );
    let app_src = format!(
        "{shared}\napp A[reg: domain Registry] {{\n    fn main(self) {{\n        let fallback = Topic<int> {{ last: -1, count: -1 }}\n        let t = at self.reg {{ topic() }} catch fallback\n        let boxed = Box<int> {{ item: 7 }}\n        let fb = Box<int> {{ item: -2 }}\n        let r = at t {{ publish(boxed) }} catch fb\n        print(r.item)\n        let c = at t {{ total() }} catch -3\n        print(c)\n    }}\n}}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "7\n101\n");
}

// ── Per-instance entity locks: handle calls vs local calls ──

const LOCK_SHARED: &str = r#"
import std.wire

object Counter {
    value: int

    fn increment(mut self) {
        self.value = self.value + 1
    }

    fn get(self) int {
        return self.value
    }
}

class Registry {
    c: Counter
    go: Sender<int>?

    fn counter(self) Counter {
        return self.c
    }

    fn kick(self) {
        let g = self.go
        if g != none {
            g.send(1)!
        }
    }
}
"#;
// (`go` is nullable: a channel has no zero state, so a non-nullable
// Sender field would make the client's `domain Registry` dep
// un-zero-constructible — a compile error under colocated DI
// construction. Only the serving process provides the channel.)

/// The handle-call dispatch path takes the SAME per-instance lock as local
/// calls: remote increments through a handle race a spawned local worker
/// hammering the same entity, and no update is lost. The worker starts only
/// on the client's `kick()` (so the two sides overlap), does its 1000
/// increments, and the client polls until both sides' 1050 total is visible.
#[test]
fn entity_handle_calls_serialize_with_local_calls() {
    let server_src = format!(
        "{LOCK_SHARED}\n\
        fn work(mut c: Counter, rx: Receiver<int>) {{\n\
            let x = rx.recv() catch 0\n\
            let mut i = 0\n\
            while i < 1000 {{\n\
                c.increment()\n\
                i = i + 1\n\
            }}\n\
        }}\n\
        \n\
        fn main() {{\n\
            let (tx, rx) = chan<int>(1)\n\
            let c = Counter {{ value: 0 }}\n\
            let r = Registry {{ c: c, go: tx }}\n\
            let t = spawn work(c, rx)\n\
            serve r on 0\n\
        }}"
    );
    let app_src = format!(
        "{LOCK_SHARED}\n\
        app A[reg: domain Registry] {{\n\
            fn main(self) {{\n\
                let fallback = Counter {{ value: -1 }}\n\
                let mut c = at self.reg {{ counter() }} catch fallback\n\
                at self.reg {{ kick() }} catch err {{}}\n\
                let mut i = 0\n\
                while i < 50 {{\n\
                    at c {{ increment() }} catch err {{}}\n\
                    i = i + 1\n\
                }}\n\
                let mut v = 0\n\
                let mut tries = 0\n\
                while tries < 2000 {{\n\
                    v = at c {{ get() }} catch -2\n\
                    if v == 1050 {{\n\
                        break\n\
                    }}\n\
                    tries = tries + 1\n\
                }}\n\
                print(v)\n\
            }}\n\
        }}"
    );
    let (_sd, server_bin) = build_binary(&[("main.pluto", &server_src)]);
    let (_ad, app_bin) = build_binary(&[("main.pluto", &app_src)]);

    let mut server = Command::new(&server_bin)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(server.stdout.take().unwrap());
    let mut port_line = String::new();
    reader.read_line(&mut port_line).unwrap();
    let port = port_line.trim();

    let out = Command::new(&app_bin)
        .env("PLUTO_DOMAIN_REGISTRY", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1050\n");
}

// ── RPC response deadline (issue #370) ──────────────────────────────────────────

const DEADLINE_CLIENT_SRC: &str = "\
import billing

app Payments[billing: remote billing.BillingService] {
    fn main(self) {
        let r = self.billing.charge(21) catch err: NetworkError {
            if err.definite {
                print(\"definite\")
            } else {
                print(f\"ambiguous: {err.message}\")
            }
            -1
        }
        print(f\"result:{r}\")
    }
}";

/// A server that accepts the connection and reads the request but never
/// replies. The client's response deadline (PLUTO_RPC_TIMEOUT_MS) must fire,
/// and the failure must be classified AMBIGUOUS (definite=false): the request
/// frame went out, so the effect may or may not have applied
/// (rfc-distributed-safety.md "Failure classification").
#[test]
fn rpc_response_deadline_fires_and_is_ambiguous() {
    use std::io::Read as _;
    use std::time::{Duration, Instant};

    let (_cd, client_bin) =
        build_binary(&[("billing.pluto", BILLING_IFACE), ("main.pluto", DEADLINE_CLIENT_SRC)]);

    // Stalling server: accept, read the request frame, never respond.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let stall = std::thread::spawn(move || {
        if let Ok((mut conn, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = conn.read(&mut buf); // consume the request frame
            // Hold the connection open, silent, until the client gives up.
            std::thread::sleep(Duration::from_secs(20));
            drop(conn);
        }
    });

    let start = Instant::now();
    let mut child = Command::new(&client_bin)
        .env("PLUTO_REMOTE_BILLINGSERVICE", format!("127.0.0.1:{port}"))
        .env("PLUTO_RPC_TIMEOUT_MS", "500")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    // Watchdog: the deadline is 500ms — if the client is still running after
    // 15s the deadline did not fire.
    let out = loop {
        match child.try_wait().unwrap() {
            Some(_) => break child.wait_with_output().unwrap(),
            None if start.elapsed() > Duration::from_secs(15) => {
                let _ = child.kill();
                panic!("RPC client hung: response deadline never fired");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    drop(stall); // detached; the sleeping thread dies with the test process

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ambiguous:"),
        "a post-send timeout must classify as ambiguous (definite=false); stdout: {stdout}"
    );
    assert!(
        stdout.contains("deadline"),
        "the classification message should name the deadline; stdout: {stdout}"
    );
    assert!(stdout.contains("result:-1"), "stdout: {stdout}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "client should give up shortly after the 500ms deadline (took {:?})",
        start.elapsed()
    );
}

// ── Bytes across the served boundary (#375) ─────────────────────────────────────

// A binary payload (all 256 byte values) crosses a served boundary intact, in
// both positions: as a direct bytes parameter/return (one wire-escaped blob),
// and nested as a bytes field inside a marshaled class (one base64 blob).
const BYTES_SERVER_SRC: &str = "\
import std.wire

class Frame {
    data: bytes
    tag: int
}

class BlobEcho {
    seed: int

    fn roundtrip(self, data: bytes) bytes {
        return data
    }

    fn wrap(self, f: Frame) Frame {
        return Frame { data: f.data, tag: f.tag + 1 }
    }
}

fn main() {
    let e = BlobEcho { seed: 1 }
    serve e on 0
}";

const BYTES_IFACE: &str = "\
import std.wire

pub class Frame {
    data: bytes
    tag: int
}

pub class BlobEcho {
    fn roundtrip(self, data: bytes) bytes {
        return data
    }

    fn wrap(self, f: Frame) Frame {
        return f
    }
}";

const BYTES_CLIENT_SRC: &str = "\
import std.wire
import blobecho

app App[e: remote blobecho.BlobEcho] {
    fn main(self) {
        let buf = bytes_new()
        let mut i = 0
        while i < 256 {
            buf.push(i as byte)
            i = i + 1
        }

        let direct = self.e.roundtrip(buf) catch err {
            print(\"direct err\")
            return
        }
        let mut ok = direct.len() == 256
        let mut j = 0
        while j < direct.len() {
            if (direct[j] as int) != j {
                ok = false
            }
            j = j + 1
        }
        print(f\"direct:{ok}\")

        let fallback = blobecho.Frame { data: bytes_new(), tag: -1 }
        let wrapped = self.e.wrap(blobecho.Frame { data: buf, tag: 41 }) catch fallback
        let mut ok2 = wrapped.data.len() == 256
        let mut k = 0
        while k < wrapped.data.len() {
            if (wrapped.data[k] as int) != k {
                ok2 = false
            }
            k = k + 1
        }
        print(f\"nested:{ok2} tag:{wrapped.tag}\")
    }
}";

/// Binary payloads cross a real two-process serve/remote boundary intact —
/// direct bytes param/return and bytes field nested in a marshaled class.
#[test]
fn bytes_payload_round_trips_over_rpc() {
    let (_sd, server_bin) = build_binary(&[("main.pluto", BYTES_SERVER_SRC)]);
    let (_cd, client_bin) =
        build_binary(&[("blobecho.pluto", BYTES_IFACE), ("main.pluto", BYTES_CLIENT_SRC)]);

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
        .env("PLUTO_REMOTE_BLOBECHO", format!("127.0.0.1:{port}"))
        .output()
        .unwrap();
    let _ = server.kill();

    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "direct:true\nnested:true tag:42\n"
    );
}

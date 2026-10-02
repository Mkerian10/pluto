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

fn run_marshal_test(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.pluto");
    std::fs::write(&path, source).unwrap();

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_src = manifest_dir.join("stdlib");
    let stdlib_dst = dir.path().join("stdlib");
    copy_dir_recursive(&stdlib_src, &stdlib_dst);

    let bin_path = dir.path().join("test_bin");
    pluto::compile_file_with_stdlib(&path, &bin_path, Some(&stdlib_dst))
        .unwrap_or_else(|e| panic!("Compilation failed: {e}"));

    let run_output = Command::new(&bin_path).output().unwrap();
    assert!(
        run_output.status.success(),
        "Binary exited with non-zero status. stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

// ── Class marshaling tests ──────────────────────────────────────────────────────

#[test]
fn marshal_simple_class() {
    let out = run_marshal_test(r#"
import std.wire

class Order {
    id: int
    total: float
}

stage Api {
    pub fn get_order(self) Order {
        return Order { id: 123, total: 45.67 }
    }

    fn main(self) {
        let order = Order { id: 123, total: 45.67 }
        let enc = wire.wire_value_encoder()
        __marshal_Order(order, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Order(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.id)
        print(decoded.total)
    }
}
"#);
    assert!(out.contains("123"));
    assert!(out.contains("45.67"));
}

#[test]
fn marshal_class_with_string() {
    let out = run_marshal_test(r#"
import std.wire

class User {
    id: int
    name: string
}

stage Api {
    pub fn get_user(self) User {
        return User { id: 1, name: "alice" }
    }

    fn main(self) {
        let user = User { id: 42, name: "bob" }
        let enc = wire.wire_value_encoder()
        __marshal_User(user, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_User(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.id)
        print(decoded.name)
    }
}
"#);
    assert!(out.contains("42"));
    assert!(out.contains("bob"));
}

#[test]
fn marshal_class_with_array() {
    let out = run_marshal_test(r#"
import std.wire

class Item {
    id: int
    tags: [string]
}

stage Api {
    pub fn get_item(self) Item {
        return Item { id: 1, tags: ["a"] }
    }

    fn main(self) {
        let item = Item { id: 100, tags: ["hello", "world"] }
        let enc = wire.wire_value_encoder()
        __marshal_Item(item, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Item(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.id)
        print("ok")
    }
}
"#);
    assert!(out.contains("100"));
    assert!(out.contains("ok"));
}

// ── Enum marshaling tests ────────────────────────────────────────────────────────

#[test]
fn marshal_enum_unit_variant() {
    let out = run_marshal_test(r#"
import std.wire

enum Status {
    Active
    Suspended
}

stage Api {
    pub fn get_status(self) Status {
        return Status.Active
    }

    fn main(self) {
        let status = Status.Active
        let enc = wire.wire_value_encoder()
        __marshal_Status(status, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Status(dec) catch err {
            print("decode failed")
            return
        }

        print("ok")
    }
}
"#);
    assert!(out.contains("ok"));
}

#[test]
fn marshal_enum_data_variant() {
    let out = run_marshal_test(r#"
import std.wire

enum Result {
    Ok { value: int }
    Err { message: string }
}

stage Api {
    pub fn get_result(self) Result {
        return Result.Ok { value: 42 }
    }

    fn main(self) {
        let res = Result.Err { message: "failed" }
        let enc = wire.wire_value_encoder()
        __marshal_Result(res, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Result(dec) catch err {
            print("decode failed")
            return
        }

        print("ok")
    }
}
"#);
    assert!(out.contains("ok"));
}

// ── Nullable type tests ──────────────────────────────────────────────────────────

#[test]
fn marshal_nullable_some() {
    let out = run_marshal_test(r#"
import std.wire

class Data {
    value: int?
}

stage Api {
    pub fn get_data(self) Data {
        return Data { value: 42 }
    }

    fn main(self) {
        let data = Data { value: 100 }
        let enc = wire.wire_value_encoder()
        __marshal_Data(data, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Data(dec) catch err {
            print("decode failed")
            return
        }

        print("ok")
    }
}
"#);
    assert!(out.contains("ok"));
}

#[test]
fn marshal_nullable_none() {
    let out = run_marshal_test(r#"
import std.wire

class Data {
    value: int?
}

stage Api {
    pub fn get_data(self) Data {
        return Data { value: none }
    }

    fn main(self) {
        let data = Data { value: none }
        let enc = wire.wire_value_encoder()
        __marshal_Data(data, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Data(dec) catch err {
            print("decode failed")
            return
        }

        print("ok")
    }
}
"#);
    assert!(out.contains("ok"));
}

// ── Generic type tests ────────────────────────────────────────────────────────────

#[test]
fn marshal_generic_class() {
    let out = run_marshal_test(r#"
import std.wire

class Box<T> {
    value: T
}

stage Api {
    pub fn get_box(self) Box<int> {
        return Box<int> { value: 42 }
    }

    fn main(self) {
        let b = Box<int> { value: 99 }
        let enc = wire.wire_value_encoder()
        __marshal_Box__int(b, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Box__int(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.value)
    }
}
"#);
    assert!(out.contains("99"));
}

/// Wire wrappers (`__wire_encode_/__wire_decode_`) are generated for generic
/// instantiations alongside their marshalers — entity placement and RPC
/// codegen reference them by sanitized name (`Box$$int` -> `Box__int`).
#[test]
fn wire_wrappers_round_trip_generic_instantiation() {
    let out = run_marshal_test(r#"
import std.wire

class Box<T> {
    value: T
}

stage Api {
    pub fn get_box(self) Box<int> {
        return Box<int> { value: 42 }
    }

    fn main(self) {
        let b = Box<int> { value: 7 }
        let s = __wire_encode_Box__int(b)
        let decoded = __wire_decode_Box__int(s) catch err {
            print("decode failed")
            return
        }
        print(decoded.value)
    }
}
"#);
    assert!(out.contains("7"), "round trip failed, got: {out}");
}

// ── Nested type tests ─────────────────────────────────────────────────────────────

#[test]
fn marshal_nested_class() {
    let out = run_marshal_test(r#"
import std.wire

class Address {
    city: string
}

class Person {
    name: string
    address: Address
}

stage Api {
    pub fn get_person(self) Person {
        return Person { name: "alice", address: Address { city: "NYC" } }
    }

    fn main(self) {
        let addr = Address { city: "SF" }
        let person = Person { name: "bob", address: addr }

        let enc = wire.wire_value_encoder()
        __marshal_Person(person, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Person(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.name)
        print("ok")
    }
}
"#);
    assert!(out.contains("bob"));
    assert!(out.contains("ok"));
}

#[test]
fn test_hand_written_unmarshal() {
    let out = run_marshal_test(r#"
import std.wire

class Point {
    x: int
    y: int
}

fn unmarshal_point(mut dec: wire.WireValueDecoder) Point {
    dec.decode_record_start("Point", 2)!
    dec.decode_field("x", 0)!
    let x = dec.decode_int()!
    dec.decode_field("y", 1)!
    let y = dec.decode_int()!
    dec.decode_record_end()
    return Point { x: x, y: y }
}

fn main() {
    print("ok")
}
"#);
    assert!(out.contains("ok"));
}

// ── Invariant validation at the decode boundary ─────────────────────────────
// Class invariants are statically discharged inside the compilation unit,
// but decoded wire data is external testimony: __unmarshal re-checks the
// invariants and raises wire.WireError on violation.

#[test]
fn unmarshal_validates_invariants_at_boundary() {
    let out = run_marshal_test(r#"
import std.wire

class Account {
    balance: int
    invariant self.balance >= 0
}

stage Api {
    pub fn get_account(self) Account {
        return Account { balance: 1 }
    }

    fn main(self) {
        // A valid round trip decodes fine.
        let acct = Account { balance: 100 }
        let enc = wire.wire_value_encoder()
        __marshal_Account(acct, enc)
        let good = enc.result()
        let mut dec = wire.wire_value_decoder(good)
        let decoded = __unmarshal_Account(dec) catch err {
            print("unexpected decode failure")
            return
        }
        print(decoded.balance)

        // Hand-crafted wire data violating the invariant raises WireError.
        let bad = wire.wire_record(["balance"], [wire.wire_int(0 - 5)])
        let mut dec2 = wire.wire_value_decoder(bad)
        let decoded2 = __unmarshal_Account(dec2) catch err {
            print("rejected at boundary")
            return
        }
        print(decoded2.balance)
    }
}
"#);
    assert!(out.contains("100"), "valid decode should succeed, got: {out}");
    assert!(
        out.contains("rejected at boundary"),
        "violating decode should raise, got: {out}"
    );
}

#[test]
fn unmarshal_boundary_error_names_invariant() {
    let out = run_marshal_test(r#"
import std.wire

class Account {
    balance: int
    invariant self.balance >= 0
}

stage Api {
    pub fn get_account(self) Account {
        return Account { balance: 1 }
    }

    fn main(self) {
        let bad = wire.wire_record(["balance"], [wire.wire_int(0 - 5)])
        let mut dec = wire.wire_value_decoder(bad)
        let decoded = __unmarshal_Account(dec) catch err: wire.WireError {
            print(err.message)
            return
        }
        print(decoded.balance)
    }
}
"#);
    assert!(
        out.contains("invariant violation on Account: self.balance >= 0"),
        "error should name class and invariant, got: {out}"
    );
}

// ── Two-state invariants at the decode boundary ─────────────────────────────

#[test]
fn marshal_two_state_invariant_skipped_at_decode() {
    // Two-state invariants (`old(...)`, rfc-properties.md atom 2) relate a
    // transition to its pre-state; a decoded value has no pre-state, so only
    // the single-state invariant becomes a decode guard. This program would
    // not even codegen if the old()-clause were emitted as a boundary check.
    let out = run_marshal_test(r#"
import std.wire

class Epoch {
    e: int
    invariant self.e >= 0
    invariant self.e >= old(self.e)
}

stage Api {
    pub fn get_epoch(self) Epoch {
        return Epoch { e: 7 }
    }

    fn main(self) {
        let epoch = Epoch { e: 7 }
        let enc = wire.wire_value_encoder()
        __marshal_Epoch(epoch, enc)
        let value = enc.result()

        let dec = wire.wire_value_decoder(value)
        let decoded = __unmarshal_Epoch(dec) catch err {
            print("decode failed")
            return
        }

        print(decoded.e)
    }
}
"#);
    assert_eq!(out.trim(), "7");
}

#[test]
fn unmarshal_validates_generic_instantiation_invariants() {
    // A generic class's invariant is template-declared; the decode guard is
    // generated per monomorphized instantiation and re-checks it at the
    // trust boundary (single-state clauses only, as for concrete classes).
    let out = run_marshal_test(r#"
import std.wire

class Gauge<T> {
    tag: T
    v: int
    invariant self.v >= 0
}

stage Api {
    pub fn get_gauge(self) Gauge<int> {
        return Gauge<int> { tag: 1, v: 1 }
    }

    fn main(self) {
        // A valid round trip decodes fine.
        let g = Gauge<int> { tag: 1, v: 100 }
        let enc = wire.wire_value_encoder()
        __marshal_Gauge__int(g, enc)
        let good = enc.result()
        let mut dec = wire.wire_value_decoder(good)
        let decoded = __unmarshal_Gauge__int(dec) catch err {
            print("unexpected decode failure")
            return
        }
        print(decoded.v)

        // Hand-crafted wire data violating the invariant raises WireError.
        let bad = wire.wire_record(["tag", "v"], [wire.wire_int(1), wire.wire_int(0 - 5)])
        let mut dec2 = wire.wire_value_decoder(bad)
        let decoded2 = __unmarshal_Gauge__int(dec2) catch err: wire.WireError {
            print(err.message)
            return
        }
        print(decoded2.v)
    }
}
"#);
    assert!(out.contains("100"), "valid decode should succeed, got: {out}");
    assert!(
        out.contains("invariant violation on Gauge$$int: self.v >= 0"),
        "violating decode should raise naming the instantiation, got: {out}"
    );
}

// ── Bytes fields (#374): serializable and marshal layers agree ─────────────────

/// The #374 repro: a class with a `bytes` field used at a marshal boundary used
/// to fail with "undefined function '__marshal_bytes'". It now round-trips, with
/// the payload carried as ONE blob (base64 in JSON), never element-wise.
#[test]
fn marshal_bytes_field_round_trips() {
    let out = run_marshal_test(r#"
import std.wire

class Frame {
    payload: bytes
    count: int
}

stage Api {
    pub fn echo(self, f: Frame) Frame {
        return f
    }

    fn main(self) {
        let buf = bytes_new()
        let mut i = 0
        while i < 256 {
            buf.push(i as byte)
            i = i + 1
        }
        let f = Frame { payload: buf, count: 256 }
        let out = self.echo(f)
        let mut ok = out.payload.len() == 256
        let mut j = 0
        while j < out.payload.len() {
            if (out.payload[j] as int) != j {
                ok = false
            }
            j = j + 1
        }
        print(ok)
        print(out.count)
    }
}
"#);
    assert_eq!(out, "true\n256\n");
}

/// Nullable bytes fields marshal as null / blob.
#[test]
fn marshal_nullable_bytes_field() {
    let out = run_marshal_test(r#"
import std.wire

class Chunk {
    data: bytes?
}

stage Api {
    pub fn echo(self, c: Chunk) Chunk {
        return c
    }

    fn main(self) {
        let buf = bytes_new()
        buf.push(255 as byte)
        buf.push(0 as byte)
        let some = self.echo(Chunk { data: buf })
        let d = some.data
        if d != none {
            print(d.len())
            print(d[0] as int)
        } else {
            print("missing")
        }

        let empty: bytes? = none
        let nothing = self.echo(Chunk { data: empty })
        if nothing.data == none {
            print("none")
        } else {
            print("unexpected")
        }
    }
}
"#);
    assert_eq!(out, "2\n255\nnone\n");
}

/// Bytes inside an array field: each element is still one blob.
#[test]
fn marshal_array_of_bytes_field() {
    let out = run_marshal_test(r#"
import std.wire

class Batch {
    chunks: [bytes]
}

stage Api {
    pub fn echo(self, b: Batch) Batch {
        return b
    }

    fn main(self) {
        let a = bytes_new()
        a.push(1 as byte)
        let b = bytes_new()
        b.push(2 as byte)
        b.push(3 as byte)
        let chunks = [a, b]
        let out = self.echo(Batch { chunks: chunks })
        print(out.chunks.len())
        print(out.chunks[0].len())
        print(out.chunks[1][1] as int)
    }
}
"#);
    assert_eq!(out, "2\n1\n3\n");
}

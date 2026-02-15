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

fn run_wire_test(source: &str) -> String {
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

fn compile_serializable_test(source: &str) {
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
}

// ── Primitive round-trips ──────────────────────────────────────────────────────

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_int_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_int(42)
    let s = fmt.serialize(v)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Int { value } { print(value) }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "42\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_negative_int_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_int(-99)
    let s = fmt.serialize(v)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Int { value } { print(value) }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "-99\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_float_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_float(3.14)
    let s = fmt.serialize(v)
    print(s)
}
"#);
    assert!(out.starts_with("3.14"), "Expected 3.14, got: {}", out);
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_bool_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v1 = wire.wire_bool(true)
    let s1 = fmt.serialize(v1)
    let r1 = fmt.deserialize(s1) catch wire.wire_null()
    match r1 {
        wire.WireValue.Bool { value } {
            if value { print("true") } else { print("false") }
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
    let v2 = wire.wire_bool(false)
    let s2 = fmt.serialize(v2)
    let r2 = fmt.deserialize(s2) catch wire.wire_null()
    match r2 {
        wire.WireValue.Bool { value } {
            if value { print("true") } else { print("false") }
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "true\nfalse\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_string_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_string("hello world")
    let s = fmt.serialize(v)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Str { value } { print(value) }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "hello world\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_null_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_null()
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_int(-1)
    match r {
        wire.WireValue.Null { print("null") }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
    }
}
"#);
    assert_eq!(out, "null\nnull\n");
}

// ── Compound type round-trips ──────────────────────────────────────────────────

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_array_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let elems: [wire.WireValue] = []
    elems.push(wire.wire_int(1))
    elems.push(wire.wire_int(2))
    elems.push(wire.wire_int(3))
    let v = wire.wire_array(elems)
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Array { elements } {
            print(elements.len())
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "[1,2,3]\n3\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_empty_array_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let elems: [wire.WireValue] = []
    let v = wire.wire_array(elems)
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Array { elements } {
            print(elements.len())
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "[]\n0\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_record_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let keys: [string] = []
    keys.push("name")
    keys.push("age")
    let vals: [wire.WireValue] = []
    vals.push(wire.wire_string("Alice"))
    vals.push(wire.wire_int(30))
    let v = wire.wire_record(keys, vals)
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Record { keys, values } {
            print(keys.len())
            print(keys[0])
            print(keys[1])
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "{\"name\":\"Alice\",\"age\":30}\n2\nname\nage\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_empty_record_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let keys: [string] = []
    let vals: [wire.WireValue] = []
    let v = wire.wire_record(keys, vals)
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Record { keys, values } {
            print(keys.len())
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "{}\n0\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_variant_roundtrip() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let v = wire.wire_variant("Ok", wire.wire_int(200))
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Variant { name, data } {
            print(name)
            match data {
                wire.WireValue.Int { value } { print(value) }
                wire.WireValue.Float { value } { print("wrong") }
                wire.WireValue.Bool { value } { print("wrong") }
                wire.WireValue.Str { value } { print("wrong") }
                wire.WireValue.Array { elements } { print("wrong") }
                wire.WireValue.Record { keys, values } { print("wrong") }
                wire.WireValue.Variant { name, data } { print("wrong") }
                wire.WireValue.Null { print("wrong") }
            }
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Record { keys, values } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert!(out.contains("\"__variant\":\"Ok\""), "Expected variant encoding, got: {}", out);
    assert!(out.contains("Ok\n200\n"), "Expected Ok\\n200\\n, got: {}", out);
}

// ── Nested structure round-trips ───────────────────────────────────────────────

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_nested_array_in_record() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let items: [wire.WireValue] = []
    items.push(wire.wire_int(10))
    items.push(wire.wire_int(20))
    let keys: [string] = []
    keys.push("items")
    keys.push("count")
    let vals: [wire.WireValue] = []
    vals.push(wire.wire_array(items))
    vals.push(wire.wire_int(2))
    let v = wire.wire_record(keys, vals)
    let s = fmt.serialize(v)
    print(s)
    let r = fmt.deserialize(s) catch wire.wire_null()
    match r {
        wire.WireValue.Record { keys, values } {
            print(keys.len())
            match values[0] {
                wire.WireValue.Array { elements } {
                    print(elements.len())
                }
                wire.WireValue.Int { value } { print("wrong") }
                wire.WireValue.Float { value } { print("wrong") }
                wire.WireValue.Bool { value } { print("wrong") }
                wire.WireValue.Str { value } { print("wrong") }
                wire.WireValue.Record { keys, values } { print("wrong") }
                wire.WireValue.Variant { name, data } { print("wrong") }
                wire.WireValue.Null { print("wrong") }
            }
        }
        wire.WireValue.Int { value } { print("wrong") }
        wire.WireValue.Float { value } { print("wrong") }
        wire.WireValue.Bool { value } { print("wrong") }
        wire.WireValue.Str { value } { print("wrong") }
        wire.WireValue.Array { elements } { print("wrong") }
        wire.WireValue.Variant { name, data } { print("wrong") }
        wire.WireValue.Null { print("wrong") }
    }
}
"#);
    assert_eq!(out, "{\"items\":[10,20],\"count\":2}\n2\n2\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_mixed_array_types() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    let elems: [wire.WireValue] = []
    elems.push(wire.wire_int(1))
    elems.push(wire.wire_string("two"))
    elems.push(wire.wire_bool(true))
    elems.push(wire.wire_null())
    let v = wire.wire_array(elems)
    let s = fmt.serialize(v)
    print(s)
}
"#);
    assert_eq!(out, "[1,\"two\",true,null]\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn wire_serialize_format() {
    // Verify specific JSON output format for each type
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let fmt = wire.json_wire_format()
    print(fmt.serialize(wire.wire_int(0)))
    print(fmt.serialize(wire.wire_bool(false)))
    print(fmt.serialize(wire.wire_string("")))
    print(fmt.serialize(wire.wire_null()))
}
"#);
    assert_eq!(out, "0\nfalse\n\"\"\nnull\n");
}

// ── Encoder/Decoder interface tests ────────────────────────────────────────────

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_primitives() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_int(42)
    let wv = enc.result()

    let mut dec = wire.wire_value_decoder(wv)
    let val = dec.decode_int()!
    print(val)

    let mut enc2 = wire.wire_value_encoder()
    enc2.encode_string("hello")
    let wv2 = enc2.result()

    let mut dec2 = wire.wire_value_decoder(wv2)
    let val2 = dec2.decode_string()!
    print(val2)

    let mut enc3 = wire.wire_value_encoder()
    enc3.encode_bool(true)
    let wv3 = enc3.result()

    let mut dec3 = wire.wire_value_decoder(wv3)
    let val3 = dec3.decode_bool()!
    if val3 {
        print("true")
    } else {
        print("false")
    }
}
"#);
    assert_eq!(out, "42\nhello\ntrue\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_array() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_array_start(3)
    enc.encode_int(1)
    enc.encode_int(2)
    enc.encode_int(3)
    enc.encode_array_end()
    let wv = enc.result()

    let mut dec = wire.wire_value_decoder(wv)
    let len = dec.decode_array_start()!
    print(len)
    dec.decode_array_end()
}
"#);
    assert_eq!(out, "3\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_record() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_record_start("Person", 2)
    enc.encode_field("name", 0)
    enc.encode_string("Alice")
    enc.encode_field("age", 1)
    enc.encode_int(30)
    enc.encode_record_end()
    let wv = enc.result()

    let mut dec = wire.wire_value_decoder(wv)
    dec.decode_record_start("Person", 2)!
    dec.decode_field("name", 0)!
    let name = dec.decode_string()!
    dec.decode_field("age", 1)!
    let age = dec.decode_int()!
    dec.decode_record_end()
    print(name)
    print(age)
}
"#);
    assert_eq!(out, "Alice\n30\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_variant() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_variant_start("Status", "Active", 0, 0)
    enc.encode_variant_end()
    let wv = enc.result()

    let names: [string] = []
    names.push("Active")
    names.push("Suspended")

    let mut dec = wire.wire_value_decoder(wv)
    let idx = dec.decode_variant("Status", names)!
    print(idx)
    dec.decode_variant_end()
}
"#);
    assert_eq!(out, "0\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_variant_with_fields() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_variant_start("Status", "Suspended", 1, 1)
    enc.encode_field("reason", 0)
    enc.encode_string("maintenance")
    enc.encode_variant_end()
    let wv = enc.result()

    let names: [string] = []
    names.push("Active")
    names.push("Suspended")

    let mut dec = wire.wire_value_decoder(wv)
    let idx = dec.decode_variant("Status", names)!
    print(idx)
    dec.decode_field("reason", 0)!
    let reason = dec.decode_string()!
    dec.decode_variant_end()
    print(reason)
}
"#);
    assert_eq!(out, "1\nmaintenance\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_nullable() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc1 = wire.wire_value_encoder()
    enc1.encode_null()
    let wv1 = enc1.result()

    let mut dec1 = wire.wire_value_decoder(wv1)
    let has_value1 = dec1.decode_nullable()
    if has_value1 {
        print("has value")
    } else {
        print("null")
    }

    let mut enc2 = wire.wire_value_encoder()
    enc2.encode_int(42)
    let wv2 = enc2.result()

    let mut dec2 = wire.wire_value_decoder(wv2)
    let has_value2 = dec2.decode_nullable()
    if has_value2 {
        print("has value")
        let val = dec2.decode_int()!
        print(val)
    } else {
        print("null")
    }
}
"#);
    assert_eq!(out, "null\nhas value\n42\n");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn encoder_decoder_nested_structures() {
    let out = run_wire_test(r#"
import std.wire

fn main() {
    let mut enc = wire.wire_value_encoder()
    enc.encode_record_start("Order", 2)
    enc.encode_field("id", 0)
    enc.encode_int(123)
    enc.encode_field("items", 1)
    enc.encode_array_start(2)
    enc.encode_string("apple")
    enc.encode_string("banana")
    enc.encode_array_end()
    enc.encode_record_end()
    let wv = enc.result()

    let mut dec = wire.wire_value_decoder(wv)
    dec.decode_record_start("Order", 2)!
    dec.decode_field("id", 0)!
    let id = dec.decode_int()!
    dec.decode_field("items", 1)!
    let len = dec.decode_array_start()!
    print(id)
    print(len)
    dec.decode_array_end()
    dec.decode_record_end()
}
"#);
    assert_eq!(out, "123\n2\n");
}

// ── Serializable type validation ────────────────────────────────────────────

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_closure_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
class Handler {
    callback: fn(int) int
    x: int
}

stage Api {
    pub fn get_handler(self) Handler {
        let h = Handler { callback: (x: int) => x + 1, x: 42 }
        return h
    }

    fn main(self) {
        print("ok")
    }
}
"#, "closures cannot be serialized");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_task_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
fn worker() int {
    return 42
}

stage Api {
    pub fn get_task(self) Task<int> {
        let t = spawn worker()
        return t
    }

    fn main(self) {
        print("ok")
    }
}
"#, "Task<T> is a runtime handle and cannot be serialized");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_sender_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
class Config {
    sender: Sender<int>
}

stage Api {
    pub fn get_config(self) Config {
        let (tx, rx) = chan<int>(1)
        return Config { sender: tx }
    }

    fn main(self) {
        print("ok")
    }
}
"#, "Sender<T> is a runtime handle and cannot be serialized");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_receiver_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
class Config {
    receiver: Receiver<int>
}

stage Api {
    pub fn get_config(self) Config {
        let (tx, rx) = chan<int>(1)
        return Config { receiver: rx }
    }

    fn main(self) {
        print("ok")
    }
}
"#, "Receiver<T> is a runtime handle and cannot be serialized");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_trait_param_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
trait Printable {
    fn show(self) string
}

stage Api {
    pub fn process(self, obj: Printable) int {
        return 42
    }

    fn main(self) {
        print("ok")
    }
}
"#, "trait types cannot be serialized");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_nested_closure_in_class_fails() {
    use common::compile_should_fail_with;

    compile_should_fail_with(r#"
class Config {
    name: string
    handler: fn(int) int
}

class Settings {
    config: Config
    value: int
}

stage Api {
    pub fn get_settings(self) Settings {
        let c = Config { name: "test", handler: (x: int) => x + 1 }
        let s = Settings { config: c, value: 42 }
        return s
    }

    fn main(self) {
        print("ok")
    }
}
"#, "field 'handler' has type that is not serializable");
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_primitives_pass() {
    // This should compile successfully (no assertion needed, compile failure would fail the test)
    compile_serializable_test(r#"
stage Api {
    pub fn get_int(self) int {
        return 42
    }

    pub fn get_string(self) string {
        return "hello"
    }

    pub fn get_bool(self) bool {
        return true
    }

    fn main(self) {
        print("ok")
    }
}
"#);
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_class_with_serializable_fields_pass() {
    compile_serializable_test(r#"
class Order {
    id: int
    name: string
    quantity: int
}

class OrderList {
    items: [Order]
    total: int
}

stage Api {
    pub fn get_order(self, id: int) Order {
        return Order { id: id, name: "test", quantity: 10 }
    }

    pub fn get_order_list(self) OrderList {
        let items: [Order] = []
        return OrderList { items: items, total: 0 }
    }

    fn main(self) {
        print("ok")
    }
}
"#);
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_nullable_and_collections_pass() {
    compile_serializable_test(r#"
class Data {
    optional_name: string?
    tags: [string]
    counts: Map<string, int>
    unique_ids: Set<int>
}

stage Api {
    pub fn get_data(self) Data {
        let tags: [string] = []
        let counts = Map<string, int> {}
        let unique_ids = Set<int> {}
        let opt_name: string? = none
        return Data {
            optional_name: opt_name,
            tags: tags,
            counts: counts,
            unique_ids: unique_ids
        }
    }

    fn main(self) {
        print("ok")
    }
}
"#);
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_enum_pass() {
    compile_serializable_test(r#"
enum Status {
    Pending
    Active { id: int }
    Completed { id: int, timestamp: string }
}

stage Api {
    pub fn get_status(self) Status {
        return Status.Pending
    }

    fn main(self) {
        print("ok")
    }
}
"#);
}

#[test]
#[ignore] // Wire format tests - mark as ignored
fn serializable_validation_injected_fields_ignored() {
    // Classes with injected fields (bracket deps) should pass validation
    // because injected fields are excluded from serialization
    compile_serializable_test(r#"
class Database {
    connection_string: string
}

class Repository[db: Database] {
    table_name: string
}

class Data {
    name: string
    value: int
}

stage Api[db: Database, repo: Repository] {
    pub fn get_data(self) Data {
        return Data { name: "test", value: 42 }
    }

    fn main(self) {
        print("ok")
    }
}
"#);
}

mod common;

use std::process::Command;

fn run_marshal_test(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let bin_path = dir.path().join("test_bin");
    plutoc::compile(source, &bin_path)
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
#[ignore]
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
}

fn main() {
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
"#);
    assert!(out.contains("123"));
    assert!(out.contains("45.67"));
}

#[test]
#[ignore]
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
#[ignore]
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
#[ignore]
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

}

fn main() {
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
"#);
    assert!(out.contains("ok"));
}

#[test]
#[ignore]
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
#[ignore]
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
#[ignore]
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
#[ignore]
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

// ── Nested type tests ─────────────────────────────────────────────────────────────

#[test]
#[ignore]
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

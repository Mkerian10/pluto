# RFC: Pluto Wire Format (Phase 1 RPC)

> **Status:** Design — Ready for implementation
> **Priority:** CRITICAL — Foundation for RPC
> **Effort:** 1 week (Phase 1 of RFC-RPC-Implementation)
> **Date:** February 2026

## Executive Summary

Pluto needs a binary serialization format for all types to support RPC and distributed systems. This RFC specifies a simple, self-describing binary format that is:

- **Type-safe:** All types encode their descriptor
- **Version-aware:** Can evolve without breaking compatibility
- **Space-efficient:** No unnecessary overhead
- **Human-debuggable:** Magic bytes and clear structure

---

## Overview

The wire format consists of **messages** (serialized values). Each message has:

1. **Header** (5 bytes) — protocol version, type code
2. **Value** (variable) — type-specific encoding

All integers are big-endian. Strings are UTF-8.

---

## Type Codes

```
0x01  null
0x02  bool
0x03  int
0x04  float
0x05  string
0x06  bytes
0x07  array
0x08  map
0x09  class
0x0A  enum
0x0B  (reserved)
0x0C  (reserved)
```

---

## Message Structure

### Header
```
Byte 0:    0xF0 (magic byte — identifies as Pluto wire)
Byte 1:    version (0x01 for now)
Byte 2-4:  type code (3 bytes, big-endian)
```

### Example
```
F0 01 00 00 03     // magic, version=1, type=int
```

Then the value follows based on type code.

---

## Type Encoding

### Null (0x01)
**Wire:** Just the header. No payload.
```
F0 01 00 00 01
```

### Bool (0x02)
**Wire:** Header + 1 byte (0x00 = false, 0x01 = true)
```
F0 01 00 00 02 00     // false
F0 01 00 00 02 01     // true
```

### Int (0x03)
**Wire:** Header + 8 bytes (I64, big-endian)
```
F0 01 00 00 03 7F FF FF FF FF FF FF FF     // 9223372036854775807 (max int64)
F0 01 00 00 03 FF FF FF FF FF FF FF FF     // -1
```

### Float (0x04)
**Wire:** Header + 8 bytes (F64, IEEE 754 big-endian)
```
F0 01 00 00 04 3F F0 00 00 00 00 00 00     // 1.0
F0 01 00 00 04 C0 24 00 00 00 00 00 00     // -10.0
```

### String (0x05)
**Wire:** Header + length (8 bytes, I64) + UTF-8 bytes
```
F0 01 00 00 05           // header
00 00 00 00 00 00 00 05  // length = 5
48 65 6C 6C 6F           // "hello" (ASCII bytes)
```

### Bytes (0x06)
**Wire:** Header + length (8 bytes, I64) + raw bytes
```
F0 01 00 00 06           // header
00 00 00 00 00 00 00 03  // length = 3
FF 42 7A                 // raw bytes
```

### Array (0x07)
**Wire:** Header + element type (1 byte code) + count (8 bytes, I64) + elements (recursively encoded)
```
F0 01 00 00 07 03        // header + element type = int
00 00 00 00 00 00 00 03  // count = 3
(int value 1)            // int 1
(int value 2)            // int 2
(int value 3)            // int 3
```

**Note:** Each element includes its own header. This is slightly redundant but ensures type safety at decode time.

### Map (0x08)
**Wire:** Header + key type (1 byte) + value type (1 byte) + count (8 bytes, I64) + (key, value) pairs
```
F0 01 00 00 08 05 03     // header + key type=string + value type=int
00 00 00 00 00 00 00 02  // count = 2
(string "a")             // key 1: "a"
(int value 10)           // value 1: 10
(string "b")             // key 2: "b"
(int value 20)           // value 2: 20
```

### Class (0x09)
**Wire:** Header + class name length (4 bytes) + class name (UTF-8) + field count (4 bytes) + fields (recursively encoded)
```
F0 01 00 00 09           // header
00 00 00 04              // class name length = 4
55 73 65 72              // "User"
00 00 00 02              // field count = 2
(int 42)                 // field 1: id
(string "Alice")         // field 2: name
```

**Field order:** Matches compile-time ordering. Compiler enforces consistency.

### Enum (0x0A)
**Wire:** Header + enum name length (4 bytes) + enum name (UTF-8) + variant name length (4 bytes) + variant name (UTF-8) + payload (if variant has data)
```
// Color::Red (no data)
F0 01 00 00 0A           // header
00 00 00 05              // enum name length = 5
43 6F 6C 6F 72           // "Color"
00 00 00 03              // variant name length = 3
52 65 64                 // "Red"
// (no payload)

// Circle { radius: 5.0 }
F0 01 00 00 0A           // header
00 00 00 05              // enum name length = 5
53 68 61 70 65           // "Shape"
00 00 00 06              // variant name length = 6
43 69 72 63 6C 65        // "Circle"
(float 5.0)              // payload: radius
```

---

## Nullable Types

Nullable types are encoded with a type code wrapper:

- **Null:** Encode as `0x01` (null)
- **Value:** Encode the value normally

At decode time, receiver knows the expected type is nullable, so `0x01` is coerced to `none`.

```
// int? with value 42
(int 42)    // decoded as int?, still represents 42

// int? with none
F0 01 00 00 01           // null header
```

---

## Error Types

Error types are serialized as classes with special naming convention:

```
class NetworkError { message: string }
// Encodes as class "error_NetworkError"
```

At decode time, receiver reconstructs the error.

---

## Versioning

If the protocol needs to evolve:

1. **Add new type code:** Use reserved bytes 0x0B, 0x0C, etc.
2. **Incompatible change:** Increment version byte (header byte 1)
3. **Receivers** check version and reject incompatible messages

Current version: `0x01`

---

## Impl Strategy (Phase 1)

### New `std.wire` Module

```pluto
// stdlib/wire/wire.pluto

pub enum WireValue {
    Null
    Bool { value: bool }
    Int { value: int }
    Float { value: float }
    String { value: string }
    Bytes { value: bytes }
    Array { element_type: int, values: [WireValue] }
    Map { key_type: int, value_type: int, pairs: [(WireValue, WireValue)] }
    Class { name: string, fields: [WireValue] }
    Enum { name: string, variant: string, payload: WireValue? }
}

trait Encoder {
    fn encode(self) WireValue
}

trait Decoder {
    fn decode(value: WireValue) self?
}

// Builtin implementations
impl int impl Encoder { /* ... */ }
impl int impl Decoder { /* ... */ }
// ... for all types
```

### C Runtime Functions

Add to `runtime/builtins.c`:

```c
// wire_encode_int: Takes int, returns serialized bytes
void *wire_encode_int(long val);

// wire_decode_int: Takes bytes, returns int or error
long wire_decode_int(void *data);

// Similar for other types...
```

### Compiler Integration

In codegen, when serializing a class instance:
1. Call `field1.encode()` for each field
2. Construct class `WireValue`
3. Serialize to bytes via `wire_encode_class()`

At deserialize:
1. Call `wire_decode_class()` to get `WireValue`
2. Pattern match on fields
3. Call `field1.decode()` on each field

---

## Testing Strategy (Phase 1)

**Unit tests** in `tests/integration/wire.rs`:

```pluto
test "encode_int" {
    let val = 42
    let encoded = val.encode()
    expect(encoded).to_equal(WireValue::Int { value: 42 })
}

test "roundtrip_int" {
    let original = 42
    let encoded = original.encode()
    let decoded = decode(encoded) as int?
    expect(decoded?).to_equal(original)
}

test "roundtrip_string" {
    let original = "hello"
    // ...
}

test "roundtrip_array" {
    let original = [1, 2, 3]
    // ...
}

test "roundtrip_class" {
    class Person { id: int, name: string }
    let original = Person { id: 1, name: "Alice" }
    // ...
}
```

**Expected:** 30+ tests covering all type combinations.

---

## Implementation Checklist

- [ ] Define wire format spec (done)
- [ ] Add `WireValue` enum to stdlib
- [ ] Implement `Encoder` trait with builtins
- [ ] Implement `Decoder` trait with builtins
- [ ] Add C runtime encoding/decoding functions
- [ ] Codegen: auto-implement Encoder for classes
- [ ] Codegen: auto-implement Decoder for classes
- [ ] Write 30+ roundtrip tests
- [ ] Document in book

---

## Future Enhancements

1. **Compression:** gzip wrapper for large payloads
2. **Compact mode:** Optional space-optimized encoding (no type codes per element)
3. **Schema versioning:** Field additions/deletions with forward/backward compat
4. **Custom serialization:** User `impl Encoder` for specific types

---

## References

- RFC-RPC-Implementation (Phase 1 context)
- communication.md (overall RPC design)
- Similar formats: Protocol Buffers, Thrift, MessagePack (reference, not copying)

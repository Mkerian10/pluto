# std.wire

Wire format for serializing Pluto values. Used internally by the RPC system to marshal data across service boundaries.

```
import std.wire
```

## Types

### WireValue

An enum representing any serializable value:

```
enum WireValue {
    Int { value: int }
    Float { value: float }
    Bool { value: bool }
    Str { value: string }
    Array { elements: [WireValue] }
    Record { keys: [string], values: [WireValue] }
    Variant { name: string, data: WireValue }
    Null
}
```

### WireError

```
error WireError {
    message: string
}
```

Raised on type mismatches during decoding.

## Factory Functions

```
wire.wire_int(v: int) WireValue
wire.wire_float(v: float) WireValue
wire.wire_bool(v: bool) WireValue
wire.wire_string(v: string) WireValue
wire.wire_array(elems: [WireValue]) WireValue
wire.wire_record(keys: [string], values: [WireValue]) WireValue
wire.wire_variant(name: string, data: WireValue) WireValue
wire.wire_null() WireValue
```

## Traits

### Encoder / Decoder

Traits for implementing custom wire formats. Methods for encoding/decoding each primitive type, arrays, maps, records, and variants.

### WireFormat

```
trait WireFormat {
    fn serialize(self, value: WireValue) string
    fn deserialize(self, data: string) WireValue
}
```

## Built-in Implementations

### JsonWireFormat

JSON-based wire format. Create with `wire.json_wire_format()`.

```
let fmt = wire.json_wire_format()
let encoded = fmt.serialize(wire.wire_int(42))
let decoded = fmt.deserialize(encoded)
```

### WireValueEncoder / WireValueDecoder

Streaming encoder/decoder for building `WireValue` trees. Create with `wire.wire_value_encoder()` and `wire.wire_value_decoder(value)`.

# std.base64

Base64 encoding and decoding.

```
import std.base64
```

## Functions

### encode

```
base64.encode(s: string) string
```

Encodes a string to standard base64.

```
let encoded = base64.encode("hello world")
// "aGVsbG8gd29ybGQ="
```

### decode

```
base64.decode(s: string) string
```

Decodes a standard base64 string.

```
let decoded = base64.decode("aGVsbG8gd29ybGQ=")
// "hello world"
```

### encode_url_safe

```
base64.encode_url_safe(s: string) string
```

Encodes a string to URL-safe base64 (uses `-` and `_` instead of `+` and `/`).

### decode_url_safe

```
base64.decode_url_safe(s: string) string
```

Decodes a URL-safe base64 string.

## Example

```
import std.base64

fn main() {
    let original = "hello world"
    let encoded = base64.encode(original)
    let decoded = base64.decode(encoded)
    print("Encoded: {encoded}")
    print("Decoded: {decoded}")
}
```

# std.uuid

UUID v4 generation.

```
import std.uuid
```

## Functions

### generate

```
uuid.generate() string
```

Returns a random UUID v4 string in standard format: `xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`.

```
let id = uuid.generate()
// e.g. "a3b4c5d6-1234-4abc-9def-0123456789ab"
```

## Example

```
import std.uuid

fn main() {
    let id1 = uuid.generate()
    let id2 = uuid.generate()
    print("ID 1: {id1}")
    print("ID 2: {id2}")
}
```

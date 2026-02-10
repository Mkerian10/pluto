# std.json

Parse, build, and stringify JSON. The module provides a single `Json` value type that represents all JSON forms.

```
import std.json
```

## Errors

```
pub error JsonError {
    message: string
}
```

`json.parse()` raises `JsonError` on invalid input.

## Constructors

Build JSON values programmatically:

```
json.null() Json
json.bool(v: bool) Json
json.int(v: int) Json
json.float(v: float) Json
json.string(v: string) Json
json.array() Json
json.object() Json
```

```
let obj = json.object()
obj.set("name", json.string("Alice"))
obj.set("age", json.int(30))
obj.set("active", json.bool(true))

let items = json.array()
items.push(json.int(1))
items.push(json.int(2))
obj.set("scores", items)
```

## Parsing

```
json.parse(s: string) Json     // raises JsonError
```

```
let data = json.parse("{\"name\": \"Alice\", \"age\": 30}")!
let name = data.get("name").get_string()
let age = data.get("age").get_int()
```

## Json Methods

### Type Checks

```
j.is_null() bool
j.is_bool() bool
j.is_int() bool
j.is_float() bool
j.is_string() bool
j.is_array() bool
j.is_object() bool
```

### Value Extraction

```
j.get_bool() bool
j.get_int() int
j.get_float() float
j.get_string() string
```

`get_int()` on a float value truncates. `get_float()` on an int value converts.

### Traversal

```
j.get(key: string) Json     // object field lookup; returns json.null() if missing
j.at(index: int) Json       // array index access
j.len() int                 // array or object length
```

### Mutation

```
j.set(key: string, value: Json)    // set or overwrite an object field
j.push(value: Json)                // append to an array
```

### Serialization

```
j.to_string() string    // JSON-encoded string
```

## Example: Round-Trip

```
import std.json

fn main() {
    let response = json.object()
    response.set("status", json.string("ok"))
    response.set("count", json.int(3))

    let items = json.array()
    items.push(json.string("apple"))
    items.push(json.string("banana"))
    response.set("items", items)

    let s = response.to_string()
    print(s)
    // {"status":"ok","count":3,"items":["apple","banana"]}

    let parsed = json.parse(s)!
    print(parsed.get("status").get_string())    // ok
    print("{parsed.get("count").get_int()}")     // 3
}
```

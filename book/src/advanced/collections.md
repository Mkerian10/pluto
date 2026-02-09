# Maps and Sets

Pluto has built-in `Map<K, V>` and `Set<T>` collection types backed by hash tables.

## Maps

### Creating Maps

```
let ages = Map<string, int> { "alice": 30, "bob": 25 }
let empty = Map<string, int> {}
```

### Reading and Writing

Use index syntax to get and set values:

```
fn main() {
    let m = Map<string, int> { "x": 1, "y": 2 }

    print(m["x"])       // 1

    m["z"] = 3
    print(m["z"])       // 3

    m["x"] = 99
    print(m["x"])       // 99
}
```

### Map Methods

```
let m = Map<string, int> {}

m.insert("a", 1)
m.insert("b", 2)

print(m.contains("a"))     // true
print(m.contains("c"))     // false

print(m.len())              // 2

m.remove("a")
print(m.len())              // 1
```

### Iterating Over Maps

Use `.keys()` and `.values()` to get arrays you can iterate over:

```
fn main() {
    let m = Map<string, int> { "x": 10, "y": 20 }

    for k in m.keys() {
        print(k)
    }

    for v in m.values() {
        print(v)
    }
}
```

## Sets

### Creating Sets

```
let nums = Set<int> { 1, 2, 3 }
let empty = Set<string> {}
```

### Set Methods

```
let s = Set<int> { 1, 2, 3 }

print(s.contains(2))       // true
print(s.contains(99))      // false

s.insert(4)
print(s.len())              // 4

s.remove(1)
print(s.len())              // 3
```

### Converting to Array

Use `.to_array()` to iterate:

```
fn main() {
    let s = Set<int> { 10, 20, 30 }
    for item in s.to_array() {
        print(item)
    }
}
```

## Key Types

Map keys and set elements must be hashable primitive types:

| Type | As Map Key | As Set Element |
|------|-----------|----------------|
| `int` | Yes | Yes |
| `float` | Yes | Yes |
| `bool` | Yes | Yes |
| `string` | Yes | Yes |
| `enum` (unit variants) | Yes | Yes |
| Classes | No | No |
| Arrays | No | No |

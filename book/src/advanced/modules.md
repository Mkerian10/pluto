# Modules and Imports

Modules let you organize code across multiple files.

## Importing a Module

Use `import` to bring a module into scope:

```
import math

fn main() {
    print(math.add(1, 2))
}
```

All access to imported items uses qualified names: `module.item`.

## Creating a Single-File Module

A module can be a single `.pluto` file next to your main file:

```
// math.pluto
pub fn add(a: int, b: int) int {
    return a + b
}

pub fn mul(a: int, b: int) int {
    return a * b
}

// main.pluto
import math

fn main() {
    print(math.add(2, 3))   // 5
    print(math.mul(4, 5))   // 20
}
```

## Creating a Directory Module

For larger modules, use a directory. All `.pluto` files in the directory are automatically merged:

```
math/
  add.pluto
  mul.pluto
main.pluto
```

```
// math/add.pluto
pub fn add(a: int, b: int) int {
    return a + b
}

// math/mul.pluto
pub fn mul(a: int, b: int) int {
    return a * b
}

// main.pluto
import math

fn main() {
    print(math.add(2, 3))
    print(math.mul(4, 5))
}
```

Files within the same directory module see each other automatically -- no import needed between them.

## Visibility

By default, all items are private. Use `pub` to make them visible to importers:

```
// helpers.pluto
fn internal_helper() int {    // private -- not accessible from outside
    return 42
}

pub fn public_api() int {     // public -- accessible via helpers.public_api()
    return internal_helper()
}
```

The `pub` keyword works on functions, classes, traits, enums, and errors.

## Importing Classes

```
// geo.pluto
pub class Point {
    x: int
    y: int

    fn sum(self) int {
        return self.x + self.y
    }
}

// main.pluto
import geo

fn main() {
    let p = geo.Point { x: 10, y: 20 }
    print(p.sum())      // 30
}
```

## Qualified Types in Signatures

Use the qualified name in type positions:

```
import geo

fn distance(a: geo.Point, b: geo.Point) int {
    let dx = a.x - b.x
    let dy = a.y - b.y
    return dx * dx + dy * dy
}
```

## Standard Library Modules

Pluto ships with a standard library. Import its modules with the `std.` prefix:

```
import std.strings
import std.math
import std.fs
import std.net
```

To use stdlib modules, pass `--stdlib stdlib` when compiling:

```bash
plutoc run --stdlib stdlib main.pluto
```

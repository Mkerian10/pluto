# Modules and Packages

Pluto has two levels of code organization: **modules** for splitting code across files, and **packages** for managing external dependencies.

## Modules

### Importing a Module

The `import` keyword loads a module by name. All access is qualified:

```
import math

fn main() {
    print(math.add(1, 2))
}
```

There are no unqualified imports, no `from X import Y`, no glob imports.

### Single-File and Directory Modules

`import math` looks for `math.pluto` or a `math/` directory next to your entry file.

A single-file module is just a `.pluto` file. A directory module merges all `.pluto` files within it into a single namespace -- files in the directory see each other without explicit imports.

### Visibility

Declarations are private by default. Use `pub` to export:

```
fn helper(x: int) int { return x * 2 }        // private

pub fn double(x: int) int { return helper(x) } // public
```

`pub` applies to `fn`, `class`, `trait`, and `enum`.

### Qualified Access

Importers use qualified names for everything -- functions, classes, enums, types in signatures:

```
import geo

fn show(p: geo.Point) {
    print(p.sum())
}

fn main() {
    let p = geo.Point { x: 10, y: 20 }
    show(p)

    let s = status.State.Active
    match s {
        status.State.Active { print("active") }
        status.State.Inactive { print("inactive") }
    }
}
```

### Hierarchical Imports and Aliases

Modules nest in subdirectories via dot syntax. The binding name is the last segment:

```
import utils.math          // resolves utils/math.pluto or utils/math/
                           // bound as: math.add(...)

import a.b.c               // resolves a/b/c.pluto or a/b/c/
                           // bound as: c.whatever()

import math as m           // alias: m.add(...)
```

### Same-Directory Auto-Merge

Files in the same directory as your entry file are automatically merged without imports:

```
// main.pluto                    // helper.pluto (same directory)
fn main() {                      fn helper() int {
    print(helper())                  return 99
}                                }
```

### Standard Library

Standard library modules use the `std` namespace:

```
import std.strings
import std.math
import std.collections
```

Use `--stdlib stdlib` to point the compiler at the stdlib directory, or place a `stdlib/` directory next to your entry file.

### Transitive and Circular Imports

Modules can import other modules transitively. If `shapes` imports `geo`, and you import `shapes`, everything resolves. Circular imports are detected and rejected at compile time.

---

## Packages

Modules organize code within a project. **Packages** handle external dependencies via `pluto.toml`.

### pluto.toml

```toml
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
mathlib = { path = "deps/mathlib" }
```

The compiler walks up from your entry file looking for `pluto.toml` (stopping at `.git` boundaries). Projects without a manifest work exactly as before.

### Path Dependencies

```toml
[dependencies]
mathlib = { path = "deps/mathlib" }
utils = { path = "../shared/utils" }
```

Paths are relative to `pluto.toml`. Import by the dependency name, not the directory name:

```
import mathlib

fn main() {
    print(mathlib.add(1, 2))
}
```

### Git Dependencies

```toml
[dependencies]
strutils = { git = "https://github.com/user/strutils.git" }
mylib = { git = "https://github.com/user/mylib.git", tag = "v1.0" }
pinned = { git = "https://github.com/user/lib.git", rev = "abc123f" }
latest = { git = "https://github.com/user/lib.git", branch = "dev" }
```

Specify at most one of `rev`, `tag`, or `branch`. Repos are cached in `~/.pluto/cache/git/`. Run `plutoc update` to re-fetch.

### A Complete Example

```
myapp/
  pluto.toml
  main.pluto
  deps/
    mathlib/
      add.pluto
      mul.pluto
```

`pluto.toml`:

```toml
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
mathlib = { path = "deps/mathlib" }
```

`deps/mathlib/add.pluto`:

```
pub fn add(a: int, b: int) int {
    return a + b
}
```

`main.pluto`:

```
import mathlib

fn main() {
    print(mathlib.add(1, 2))
}
```

### Transitive Dependencies

Packages can declare their own `pluto.toml` with their own dependencies:

```toml
# deps/liba/pluto.toml
[package]
name = "liba"

[dependencies]
libb = { path = "../../deps/libb" }
```

```
// deps/liba/compute.pluto
import libb

pub fn compute(x: int) int {
    return libb.double(x)
}
```

The compiler resolves the full graph, deduplicates diamonds, and detects circular chains.

### Scope Isolation

Transitive dependencies are **not** visible unless explicitly declared. If you depend on `liba` and `liba` depends on `libb`, you cannot `import libb` without adding it to your own `pluto.toml`. This prevents coupling to implementation details of your dependencies.

### Rules and Constraints

**Naming:** Dependency names must be valid identifiers. `std` and language keywords (`class`, `fn`, `if`, etc.) are reserved.

**Collisions:** If a dependency name matches a local module directory, the compiler reports an ambiguity error.

**Mixing:** Path and git dependencies work together in the same manifest.

---

## Summary

| Feature | Syntax |
|---|---|
| Import a module | `import math` |
| Hierarchical import | `import utils.math` |
| Import with alias | `import math as m` |
| Qualified access | `math.add(1, 2)` |
| Struct literal | `geo.Point { x: 1, y: 2 }` |
| Enum variant | `status.State.Active` |
| Qualified type | `fn show(p: geo.Point)` |
| Public declaration | `pub fn`, `pub class`, `pub enum`, `pub trait` |
| Stdlib import | `import std.strings` |
| Path dependency | `mathlib = { path = "deps/mathlib" }` |
| Git dependency | `mylib = { git = "url", tag = "v1.0" }` |
| Update git deps | `plutoc update` |
